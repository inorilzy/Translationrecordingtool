use crate::translation_domain::TranslationResult;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::PathBuf};
use tauri::{AppHandle, Manager};

const TRANSLATIONS_DB_FILE_NAME: &str = "translations.db";

const REQUIRED_COLUMNS: [(&str, &str); 6] = [
    ("us_phonetic", "TEXT"),
    ("uk_phonetic", "TEXT"),
    ("audio_url", "TEXT"),
    ("explains", "TEXT"),
    ("examples", "TEXT"),
    ("synonyms", "TEXT"),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TranslationRecord {
    pub id: Option<i64>,
    pub source_text: String,
    pub translated_text: String,
    pub phonetic: Option<String>,
    pub us_phonetic: Option<String>,
    pub uk_phonetic: Option<String>,
    pub audio_url: Option<String>,
    pub explains: Option<Vec<String>>,
    pub examples: Option<Vec<String>>,
    pub synonyms: Option<Vec<String>>,
    pub source_lang: String,
    pub target_lang: String,
    pub word_type: Option<String>,
    pub created_at: i64,
    pub access_count: i32,
    pub is_favorite: i32,
}

impl TranslationRecord {
    pub fn from_result(result: TranslationResult, created_at: i64) -> Self {
        Self {
            id: None,
            source_text: result.source_text,
            translated_text: result.translated_text,
            phonetic: result.phonetic,
            us_phonetic: result.us_phonetic,
            uk_phonetic: result.uk_phonetic,
            audio_url: result.audio_url,
            explains: result.explains,
            examples: result.examples,
            synonyms: result.synonyms,
            source_lang: result.source_lang,
            target_lang: result.target_lang,
            word_type: result.word_type,
            created_at,
            access_count: 1,
            is_favorite: 0,
        }
    }

    pub fn with_result(&self, result: TranslationResult) -> Self {
        Self {
            id: self.id,
            source_text: result.source_text,
            translated_text: result.translated_text,
            phonetic: result.phonetic,
            us_phonetic: result.us_phonetic,
            uk_phonetic: result.uk_phonetic,
            audio_url: result.audio_url,
            explains: result.explains,
            examples: result.examples,
            synonyms: result.synonyms,
            source_lang: result.source_lang,
            target_lang: result.target_lang,
            word_type: result.word_type,
            created_at: self.created_at,
            access_count: self.access_count,
            is_favorite: self.is_favorite,
        }
    }

    pub fn has_same_content_as(&self, result: &TranslationResult) -> bool {
        self.source_text == result.source_text
            && self.translated_text == result.translated_text
            && self.phonetic.as_deref() == result.phonetic.as_deref()
            && self.us_phonetic.as_deref() == result.us_phonetic.as_deref()
            && self.uk_phonetic.as_deref() == result.uk_phonetic.as_deref()
            && self.audio_url.as_deref() == result.audio_url.as_deref()
            && self.explains.as_deref() == result.explains.as_deref()
            && self.examples.as_deref() == result.examples.as_deref()
            && self.synonyms.as_deref() == result.synonyms.as_deref()
            && self.source_lang == result.source_lang
            && self.target_lang == result.target_lang
            && self.word_type.as_deref() == result.word_type.as_deref()
    }

    pub fn to_result(&self) -> TranslationResult {
        TranslationResult {
            source_text: self.source_text.clone(),
            translated_text: self.translated_text.clone(),
            phonetic: self.phonetic.clone(),
            us_phonetic: self.us_phonetic.clone(),
            uk_phonetic: self.uk_phonetic.clone(),
            audio_url: self.audio_url.clone(),
            explains: self.explains.clone(),
            examples: self.examples.clone(),
            synonyms: self.synonyms.clone(),
            source_lang: self.source_lang.clone(),
            target_lang: self.target_lang.clone(),
            word_type: self.word_type.clone(),
        }
    }
}

pub const INIT_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS translations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_text TEXT NOT NULL,
    translated_text TEXT NOT NULL,
    phonetic TEXT,
    us_phonetic TEXT,
    uk_phonetic TEXT,
    audio_url TEXT,
    explains TEXT,
    examples TEXT,
    synonyms TEXT,
    source_lang TEXT DEFAULT 'en',
    target_lang TEXT DEFAULT 'zh',
    word_type TEXT,
    created_at INTEGER NOT NULL,
    access_count INTEGER DEFAULT 1,
    is_favorite INTEGER DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_source_text
ON translations(source_text, source_lang, target_lang);

CREATE INDEX IF NOT EXISTS idx_created_at ON translations(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_favorite ON translations(is_favorite, created_at DESC);
"#;

#[derive(Debug)]
struct TranslationRecordRow {
    id: Option<i64>,
    source_text: String,
    translated_text: String,
    phonetic: Option<String>,
    us_phonetic: Option<String>,
    uk_phonetic: Option<String>,
    audio_url: Option<String>,
    explains: Option<String>,
    examples: Option<String>,
    synonyms: Option<String>,
    source_lang: String,
    target_lang: String,
    word_type: Option<String>,
    created_at: i64,
    access_count: i32,
    is_favorite: i32,
}

fn parse_string_list(value: Option<String>) -> Result<Option<Vec<String>>, String> {
    let Some(value) = value else {
        return Ok(None);
    };

    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    match serde_json::from_str::<Vec<String>>(trimmed) {
        Ok(items) => {
            let filtered = items
                .into_iter()
                .filter(|item| !item.trim().is_empty())
                .collect::<Vec<_>>();
            if filtered.is_empty() {
                Ok(None)
            } else {
                Ok(Some(filtered))
            }
        }
        Err(_) => Ok(Some(vec![trimmed.to_string()])),
    }
}

fn serialize_string_list(value: &Option<Vec<String>>) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };

    if value.is_empty() {
        return Ok(None);
    }

    serde_json::to_string(value)
        .map(Some)
        .map_err(|e| format!("序列化翻译列表失败: {}", e))
}

fn translation_record_row_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<TranslationRecordRow> {
    Ok(TranslationRecordRow {
        id: row.get(0)?,
        source_text: row.get(1)?,
        translated_text: row.get(2)?,
        phonetic: row.get(3)?,
        us_phonetic: row.get(4)?,
        uk_phonetic: row.get(5)?,
        audio_url: row.get(6)?,
        explains: row.get(7)?,
        examples: row.get(8)?,
        synonyms: row.get(9)?,
        source_lang: row.get(10)?,
        target_lang: row.get(11)?,
        word_type: row.get(12)?,
        created_at: row.get(13)?,
        access_count: row.get(14)?,
        is_favorite: row.get(15)?,
    })
}

fn translation_from_record_row(row: TranslationRecordRow) -> Result<TranslationRecord, String> {
    Ok(TranslationRecord {
        id: row.id,
        source_text: row.source_text,
        translated_text: row.translated_text,
        phonetic: row.phonetic,
        us_phonetic: row.us_phonetic,
        uk_phonetic: row.uk_phonetic,
        audio_url: row.audio_url,
        explains: parse_string_list(row.explains)?,
        examples: parse_string_list(row.examples)?,
        synonyms: parse_string_list(row.synonyms)?,
        source_lang: row.source_lang,
        target_lang: row.target_lang,
        word_type: row.word_type,
        created_at: row.created_at,
        access_count: row.access_count,
        is_favorite: row.is_favorite,
    })
}

pub fn translations_db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&app_data_dir).map_err(|e| format!("创建应用数据目录失败: {}", e))?;
    Ok(app_data_dir.join(TRANSLATIONS_DB_FILE_NAME))
}

pub fn ensure_translations_schema(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(INIT_SQL)
        .map_err(|e| format!("初始化翻译表失败: {}", e))?;

    let mut statement = connection
        .prepare("PRAGMA table_info(translations)")
        .map_err(|e| format!("读取翻译表字段失败: {}", e))?;

    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| format!("查询翻译表字段失败: {}", e))?;

    let mut existing_columns = HashSet::new();
    for column in columns {
        existing_columns.insert(column.map_err(|e| format!("解析翻译表字段失败: {}", e))?);
    }

    for (column_name, column_type) in REQUIRED_COLUMNS {
        if existing_columns.contains(column_name) {
            continue;
        }

        connection
            .execute(
                &format!(
                    "ALTER TABLE translations ADD COLUMN {} {}",
                    column_name, column_type
                ),
                [],
            )
            .map_err(|e| format!("补充翻译表字段失败 ({}): {}", column_name, e))?;
    }

    Ok(())
}

pub fn open_translations_connection(app: &AppHandle) -> Result<Connection, String> {
    let database_path = translations_db_path(app)?;
    let connection = Connection::open(&database_path)
        .map_err(|e| format!("打开翻译数据库失败: {} ({})", database_path.display(), e))?;

    ensure_translations_schema(&connection)?;
    Ok(connection)
}

pub fn get_translation_by_lookup_key_in_connection(
    connection: &Connection,
    source_text: &str,
    source_lang: &str,
    target_lang: &str,
) -> Result<Option<TranslationRecord>, String> {
    let row = connection
        .query_row(
            "SELECT id, source_text, translated_text, phonetic, us_phonetic, uk_phonetic, audio_url, explains, examples, synonyms, source_lang, target_lang, word_type, created_at, access_count, is_favorite FROM translations WHERE source_text = ?1 AND source_lang = ?2 AND target_lang = ?3",
            params![source_text, source_lang, target_lang],
            translation_record_row_from_row,
        )
        .optional()
        .map_err(|e| format!("查询翻译记录失败: {}", e))?;

    row.map(translation_from_record_row).transpose()
}

pub fn save_translation_in_connection(
    connection: &Connection,
    translation: &TranslationRecord,
    increment_access_count: bool,
) -> Result<TranslationRecord, String> {
    let explains = serialize_string_list(&translation.explains)?;
    let examples = serialize_string_list(&translation.examples)?;
    let synonyms = serialize_string_list(&translation.synonyms)?;
    let access_count_clause = if increment_access_count {
        "access_count = access_count + 1,"
    } else {
        ""
    };

    connection
        .execute(
            &format!(
                "INSERT INTO translations (source_text, translated_text, phonetic, us_phonetic, uk_phonetic, audio_url, explains, examples, synonyms, source_lang, target_lang, word_type, created_at, access_count, is_favorite)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 0)
                 ON CONFLICT(source_text, source_lang, target_lang)
                 DO UPDATE SET
                    translated_text = excluded.translated_text,
                    phonetic = excluded.phonetic,
                    us_phonetic = excluded.us_phonetic,
                    uk_phonetic = excluded.uk_phonetic,
                    audio_url = excluded.audio_url,
                    explains = excluded.explains,
                    examples = excluded.examples,
                    synonyms = excluded.synonyms,
                    word_type = excluded.word_type,
                    {} created_at = excluded.created_at",
                access_count_clause
            ),
            params![
                translation.source_text,
                translation.translated_text,
                translation.phonetic,
                translation.us_phonetic,
                translation.uk_phonetic,
                translation.audio_url,
                explains,
                examples,
                synonyms,
                translation.source_lang,
                translation.target_lang,
                translation.word_type,
                translation.created_at,
                translation.access_count,
            ],
        )
        .map_err(|e| format!("保存翻译记录失败: {}", e))?;

    get_translation_by_lookup_key_in_connection(
        connection,
        &translation.source_text,
        &translation.source_lang,
        &translation.target_lang,
    )?
    .ok_or_else(|| "保存翻译记录后未找到该记录".to_string())
}

pub fn toggle_favorite_in_connection(
    connection: &Connection,
    id: i64,
    is_favorite: bool,
) -> Result<(), String> {
    let updated = connection
        .execute(
            "UPDATE translations SET is_favorite = ?1 WHERE id = ?2",
            params![if is_favorite { 1 } else { 0 }, id],
        )
        .map_err(|e| format!("更新收藏状态失败: {}", e))?;

    if updated == 0 {
        return Err(format!("未找到 ID 为 {} 的翻译记录", id));
    }

    Ok(())
}

pub fn load_favorites_in_connection(
    connection: &Connection,
) -> Result<Vec<TranslationRecord>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, source_text, translated_text, phonetic, us_phonetic, uk_phonetic, audio_url, explains, examples, synonyms, source_lang, target_lang, word_type, created_at, access_count, is_favorite FROM translations WHERE is_favorite = 1 ORDER BY created_at DESC",
        )
        .map_err(|e| format!("准备收藏查询失败: {}", e))?;

    let rows = statement
        .query_map([], translation_record_row_from_row)
        .map_err(|e| format!("查询收藏列表失败: {}", e))?;

    let mut translations = Vec::new();
    for row in rows {
        translations.push(translation_from_record_row(
            row.map_err(|e| format!("解析收藏记录失败: {}", e))?,
        )?);
    }

    Ok(translations)
}

pub fn load_history_in_connection(
    connection: &Connection,
) -> Result<Vec<TranslationRecord>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, source_text, translated_text, phonetic, us_phonetic, uk_phonetic, audio_url, explains, examples, synonyms, source_lang, target_lang, word_type, created_at, access_count, is_favorite FROM translations ORDER BY created_at DESC LIMIT 100",
        )
        .map_err(|e| format!("准备历史查询失败: {}", e))?;

    let rows = statement
        .query_map([], translation_record_row_from_row)
        .map_err(|e| format!("查询历史记录失败: {}", e))?;

    let mut translations = Vec::new();
    for row in rows {
        translations.push(translation_from_record_row(
            row.map_err(|e| format!("解析历史记录失败: {}", e))?,
        )?);
    }

    Ok(translations)
}

pub fn get_translation_by_id_in_connection(
    connection: &Connection,
    id: i64,
) -> Result<TranslationRecord, String> {
    let row = connection
        .query_row(
            "SELECT id, source_text, translated_text, phonetic, us_phonetic, uk_phonetic, audio_url, explains, examples, synonyms, source_lang, target_lang, word_type, created_at, access_count, is_favorite FROM translations WHERE id = ?1",
            params![id],
            translation_record_row_from_row,
        )
        .optional()
        .map_err(|e| format!("查询翻译详情失败: {}", e))?;

    let row = row.ok_or_else(|| format!("未找到 ID 为 {} 的翻译记录", id))?;
    translation_from_record_row(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_translation(source_text: &str, created_at: i64) -> TranslationRecord {
        TranslationRecord {
            id: None,
            source_text: source_text.to_string(),
            translated_text: format!("{}-translated", source_text),
            phonetic: Some("test-phonetic".to_string()),
            us_phonetic: Some("us-phonetic".to_string()),
            uk_phonetic: Some("uk-phonetic".to_string()),
            audio_url: Some("https://example.com/audio.mp3".to_string()),
            explains: Some(vec!["explain-1".to_string(), "explain-2".to_string()]),
            examples: Some(vec!["example-1".to_string()]),
            synonyms: Some(vec!["synonym-1".to_string()]),
            source_lang: "en".to_string(),
            target_lang: "zh".to_string(),
            word_type: Some("n.".to_string()),
            created_at,
            access_count: 1,
            is_favorite: 0,
        }
    }

    fn setup_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        ensure_translations_schema(&connection).unwrap();
        connection
    }

    #[test]
    fn result_record_mapping_round_trips_content_without_persistence_metadata() {
        let result = sample_translation("mapping", 100).to_result();

        let record = TranslationRecord::from_result(result.clone(), 321);

        assert_eq!(record.to_result(), result);
        assert_eq!(record.id, None);
        assert_eq!(record.created_at, 321);
        assert_eq!(record.access_count, 1);
        assert_eq!(record.is_favorite, 0);
    }

    #[test]
    fn content_comparison_accepts_equal_translation_content() {
        let record = sample_translation("same", 100);
        let result = record.to_result();

        assert!(record.has_same_content_as(&result));
    }

    #[test]
    fn content_comparison_detects_changed_translation_content() {
        let record = sample_translation("changed", 100);
        let mut result = record.to_result();
        result.examples = Some(vec!["different example".to_string()]);

        assert!(!record.has_same_content_as(&result));
    }

    #[test]
    fn content_comparison_ignores_persistence_metadata() {
        let mut record = sample_translation("metadata", 100);
        let result = record.to_result();
        record.id = Some(42);
        record.created_at = 999;
        record.access_count = 17;
        record.is_favorite = 1;

        assert!(record.has_same_content_as(&result));
    }

    #[test]
    fn enrichment_mapping_preserves_existing_record_identity_and_counters() {
        let mut existing = sample_translation("mapping", 100);
        existing.id = Some(42);
        existing.access_count = 7;
        existing.is_favorite = 1;
        let mut enriched = existing.to_result();
        enriched.examples = Some(vec!["new example".to_string()]);

        let updated = existing.with_result(enriched);

        assert_eq!(updated.id, Some(42));
        assert_eq!(updated.created_at, 100);
        assert_eq!(updated.access_count, 7);
        assert_eq!(updated.is_favorite, 1);
        assert_eq!(updated.examples, Some(vec!["new example".to_string()]));
    }

    #[test]
    fn save_translation_round_trips_lists_and_assigns_id() {
        let connection = setup_connection();
        let translation = sample_translation("hello", 100);

        let saved = save_translation_in_connection(&connection, &translation, true).unwrap();

        assert!(saved.id.is_some());
        assert_eq!(saved.explains, translation.explains);
        assert_eq!(saved.examples, translation.examples);
        assert_eq!(saved.synonyms, translation.synonyms);
        assert_eq!(saved.access_count, 1);
    }

    #[test]
    fn save_translation_updates_existing_row_and_increments_access_count() {
        let connection = setup_connection();
        let first = sample_translation("repeat", 100);
        let saved = save_translation_in_connection(&connection, &first, true).unwrap();

        toggle_favorite_in_connection(&connection, saved.id.unwrap(), true).unwrap();

        let mut updated = sample_translation("repeat", 200);
        updated.translated_text = "repeat-updated".to_string();

        let persisted = save_translation_in_connection(&connection, &updated, true).unwrap();

        assert_eq!(persisted.id, saved.id);
        assert_eq!(persisted.translated_text, "repeat-updated");
        assert_eq!(persisted.access_count, 2);
        assert_eq!(persisted.is_favorite, 1);
        assert_eq!(persisted.created_at, 200);
    }

    #[test]
    fn load_history_and_favorites_return_expected_rows() {
        let connection = setup_connection();

        for index in 0..101 {
            let translation = sample_translation(&format!("word-{}", index), index);
            let saved = save_translation_in_connection(&connection, &translation, true).unwrap();
            if index % 10 == 0 {
                toggle_favorite_in_connection(&connection, saved.id.unwrap(), true).unwrap();
            }
        }

        let history = load_history_in_connection(&connection).unwrap();
        let favorites = load_favorites_in_connection(&connection).unwrap();

        assert_eq!(history.len(), 100);
        assert_eq!(history.first().unwrap().source_text, "word-100");
        assert_eq!(history.last().unwrap().source_text, "word-1");
        assert!(favorites
            .iter()
            .all(|translation| translation.is_favorite == 1));
        assert_eq!(favorites.first().unwrap().source_text, "word-100");
    }

    #[test]
    fn get_translation_by_id_returns_saved_translation() {
        let connection = setup_connection();
        let translation = sample_translation("detail", 100);
        let saved = save_translation_in_connection(&connection, &translation, true).unwrap();

        let fetched = get_translation_by_id_in_connection(&connection, saved.id.unwrap()).unwrap();

        assert_eq!(fetched.id, saved.id);
        assert_eq!(fetched.source_text, "detail");
    }
}
