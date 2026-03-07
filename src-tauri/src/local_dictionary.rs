use rusqlite::{Connection, OptionalExtension};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
use tauri::{path::BaseDirectory, AppHandle, Manager};

const DICTIONARY_FILE_NAME: &str = "dictionary.db";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineDictionaryEntry {
    pub word: String,
    pub translated_text: String,
    pub phonetic: Option<String>,
    pub us_phonetic: Option<String>,
    pub uk_phonetic: Option<String>,
    pub audio_url: Option<String>,
    pub explains: Vec<String>,
    pub examples: Vec<String>,
    pub synonyms: Vec<String>,
    pub word_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreeDictionarySupplement {
    pub phonetic: Option<String>,
    pub audio_url: Option<String>,
    pub explains: Vec<String>,
    pub examples: Vec<String>,
    pub synonyms: Vec<String>,
}

pub fn ensure_runtime_dictionary(app: &AppHandle) -> Result<Option<PathBuf>, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&app_data_dir).map_err(|e| e.to_string())?;

    let runtime_path = app_data_dir.join(DICTIONARY_FILE_NAME);
    if runtime_path.exists() {
        return Ok(Some(runtime_path));
    }

    let resource_path = candidate_resource_paths(app)
        .into_iter()
        .find(|path| path.exists());

    if let Some(resource_path) = resource_path {
        fs::copy(&resource_path, &runtime_path).map_err(|e| {
            format!(
                "复制本地词典失败: {} -> {} ({})",
                resource_path.display(),
                runtime_path.display(),
                e
            )
        })?;
        return Ok(Some(runtime_path));
    }

    Ok(None)
}

pub fn lookup_word(app: &AppHandle, word: &str) -> Result<Option<OfflineDictionaryEntry>, String> {
    let Some(runtime_path) = ensure_runtime_dictionary(app)? else {
        return Ok(None);
    };

    let connection = Connection::open(runtime_path).map_err(|e| format!("打开本地词典失败: {}", e))?;
    lookup_word_in_connection(&connection, word)
}

pub fn lookup_word_in_connection(
    connection: &Connection,
    word: &str,
) -> Result<Option<OfflineDictionaryEntry>, String> {
    let normalized_word = normalize_word(word);
    if normalized_word.is_empty() {
        return Ok(None);
    }

    let row = connection
        .query_row(
            "SELECT word, phonetic, definition, translation, pos FROM ecdict_entries WHERE word = ?1",
            [&normalized_word],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("查询本地词典失败: {}", e))?;

    let Some((word, phonetic, definition, translation, pos)) = row else {
        return Ok(None);
    };

    let (word_type, translated_text) = normalize_translation(translation.as_deref(), pos.as_deref());

    let mut explains = split_lines(definition.as_deref());
    for gloss in query_string_list(
        connection,
        "SELECT gloss FROM wordnet_glosses WHERE word = ?1 LIMIT 6",
        &normalized_word,
    )? {
        push_unique(&mut explains, gloss);
    }

    let mut examples = query_string_list(
        connection,
        "SELECT example FROM wordnet_examples WHERE word = ?1 LIMIT 6",
        &normalized_word,
    )?;
    examples.retain(|item| !item.is_empty());

    let mut synonyms = query_string_list(
        connection,
        "SELECT synonym FROM wordnet_synonyms WHERE word = ?1 LIMIT 12",
        &normalized_word,
    )?;
    synonyms.retain(|item| item != &normalized_word);

    Ok(Some(OfflineDictionaryEntry {
        word,
        translated_text,
        phonetic,
        us_phonetic: None,
        uk_phonetic: None,
        audio_url: None,
        explains,
        examples,
        synonyms,
        word_type,
    }))
}

pub fn merge_free_dictionary_supplement(
    mut entry: OfflineDictionaryEntry,
    supplement: Option<FreeDictionarySupplement>,
) -> OfflineDictionaryEntry {
    let Some(supplement) = supplement else {
        return entry;
    };

    if entry.us_phonetic.is_none() && supplement.phonetic.is_some() {
        entry.us_phonetic = supplement.phonetic.clone();
    }

    if entry.audio_url.is_none() {
        entry.audio_url = supplement.audio_url;
    }

    for explain in supplement.explains {
        push_unique(&mut entry.explains, explain);
    }
    for example in supplement.examples {
        push_unique(&mut entry.examples, example);
    }
    for synonym in supplement.synonyms {
        push_unique(&mut entry.synonyms, synonym);
    }

    entry
}

fn candidate_resource_paths(app: &AppHandle) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(path) = app.path().resolve(DICTIONARY_FILE_NAME, BaseDirectory::Resource) {
        paths.push(path);
    }

    if let Ok(path) = app
        .path()
        .resolve(&format!("resources/{}", DICTIONARY_FILE_NAME), BaseDirectory::Resource)
    {
        paths.push(path);
    }

    paths.push(Path::new("resources").join(DICTIONARY_FILE_NAME));
    paths
}

fn normalize_word(word: &str) -> String {
    word.trim().to_lowercase()
}

fn normalize_translation(translation: Option<&str>, pos: Option<&str>) -> (Option<String>, String) {
    let mut word_type = pos
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_word_type);

    let mut translated_parts = Vec::new();

    if let Some(translation) = translation {
        for raw_line in translation.lines() {
            let line = raw_line.trim().trim_matches('/');
            if line.is_empty() {
                continue;
            }

            if let Some((detected_type, text)) = split_tagged_translation(line) {
                if word_type.is_none() {
                    word_type = Some(detected_type);
                }
                translated_parts.push(text);
            } else {
                translated_parts.push(line.to_string());
            }
        }
    }

    let translated_text = if translated_parts.is_empty() {
        translation.unwrap_or_default().trim().to_string()
    } else {
        translated_parts.join("；")
    };

    (word_type, translated_text)
}

fn split_tagged_translation(line: &str) -> Option<(String, String)> {
    let (prefix, rest) = line.split_once(". ")?;
    if prefix
        .chars()
        .all(|ch| ch.is_ascii_alphabetic() || ch == '/')
        && !rest.trim().is_empty()
    {
        return Some((format!("{}.", prefix), rest.trim().to_string()));
    }

    None
}

fn normalize_word_type(word_type: &str) -> String {
    let trimmed = word_type.trim();
    if trimmed.ends_with('.') {
        trimmed.to_string()
    } else {
        format!("{}.", trimmed)
    }
}

fn split_lines(value: Option<&str>) -> Vec<String> {
    let mut items = Vec::new();
    for line in value.unwrap_or_default().lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            push_unique(&mut items, trimmed.to_string());
        }
    }
    items
}

fn query_string_list(
    connection: &Connection,
    sql: &str,
    word: &str,
) -> Result<Vec<String>, String> {
    let mut statement = connection.prepare(sql).map_err(|e| format!("准备查询失败: {}", e))?;
    let mut rows = statement.query([word]).map_err(|e| format!("执行查询失败: {}", e))?;
    let mut items = Vec::new();

    while let Some(row) = rows.next().map_err(|e| format!("读取查询结果失败: {}", e))? {
        let value = row
            .get::<_, String>(0)
            .map_err(|e| format!("解析查询结果失败: {}", e))?;
        push_unique(&mut items, value);
    }

    Ok(items)
}

fn push_unique(items: &mut Vec<String>, value: String) {
    let normalized = value.trim();
    if normalized.is_empty() {
        return;
    }

    let mut existing = HashSet::new();
    for item in items.iter() {
        existing.insert(item.trim().to_lowercase());
    }

    let key = normalized.to_lowercase();
    if existing.contains(&key) {
        return;
    }

    items.push(normalized.to_string());
}
