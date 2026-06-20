/// Translation resolution pipeline: local dictionary → Free Dictionary → Youdao API.
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::{error, info, warn};

use crate::{
    database::Translation,
    local_dictionary::{self, OfflineDictionaryEntry},
    translator,
};

pub use translator::TranslationConfig;

// ─── Translation Builders ────────────────────────────────────────────────────

pub fn some_if_not_empty(items: Vec<String>) -> Option<Vec<String>> {
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

pub fn to_translation_content(entry: OfflineDictionaryEntry) -> translator::TranslationContent {
    translator::TranslationContent {
        translated_text: entry.translated_text,
        phonetic: entry.phonetic,
        us_phonetic: entry.us_phonetic,
        uk_phonetic: entry.uk_phonetic,
        audio_url: entry.audio_url,
        explains: entry.explains,
        examples: entry.examples,
        synonyms: entry.synonyms,
        word_type: entry.word_type,
    }
}

pub fn build_translation_with_timestamp(
    text: String,
    content: translator::TranslationContent,
    created_at: i64,
) -> Translation {
    Translation {
        id: None,
        source_text: text,
        translated_text: content.translated_text,
        phonetic: content.phonetic,
        us_phonetic: content.us_phonetic,
        uk_phonetic: content.uk_phonetic,
        audio_url: content.audio_url,
        explains: some_if_not_empty(content.explains),
        examples: some_if_not_empty(content.examples),
        synonyms: some_if_not_empty(content.synonyms),
        source_lang: "en".to_string(),
        target_lang: "zh".to_string(),
        word_type: content.word_type,
        created_at,
        access_count: 1,
        is_favorite: 0,
    }
}

pub fn build_translation(text: String, content: translator::TranslationContent) -> Translation {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    build_translation_with_timestamp(text, content, now)
}

pub fn build_translation_from_existing(
    base: &Translation,
    content: translator::TranslationContent,
) -> Translation {
    Translation {
        id: base.id,
        source_text: base.source_text.clone(),
        translated_text: content.translated_text,
        phonetic: content.phonetic,
        us_phonetic: content.us_phonetic,
        uk_phonetic: content.uk_phonetic,
        audio_url: content.audio_url,
        explains: some_if_not_empty(content.explains),
        examples: some_if_not_empty(content.examples),
        synonyms: some_if_not_empty(content.synonyms),
        source_lang: base.source_lang.clone(),
        target_lang: base.target_lang.clone(),
        word_type: content.word_type,
        created_at: base.created_at,
        access_count: base.access_count,
        is_favorite: base.is_favorite,
    }
}

// ─── Word Detection ──────────────────────────────────────────────────────────

/// Whether text looks like a single word suitable for local dictionary lookup.
pub fn is_local_dictionary_candidate(text: &str) -> bool {
    !text.contains(' ')
        && !text.contains(',')
        && !text.contains('.')
        && text.chars().all(|ch| ch.is_ascii_alphabetic())
}

/// Broader word detection: allows apostrophes (don't) and hyphens (well-known),
/// rejects camelCase identifiers.
pub fn is_single_word(text: &str) -> bool {
    let trimmed = text.trim();

    if trimmed.contains(' ')
        || trimmed.contains(',')
        || trimmed.contains('.')
        || trimmed.contains('!')
        || trimmed.contains('?')
    {
        return false;
    }

    // camelCase → not a regular word
    let has_internal_uppercase = trimmed.chars().skip(1).any(|c| c.is_uppercase());
    if has_internal_uppercase {
        return false;
    }

    trimmed
        .chars()
        .all(|c| c.is_alphabetic() || c == '\'' || c == '-')
}

// ─── Local Dictionary Lookup ─────────────────────────────────────────────────

pub fn lookup_local_translation(
    app: &tauri::AppHandle,
    text: &str,
) -> Result<Option<(OfflineDictionaryEntry, Translation)>, String> {
    if !is_local_dictionary_candidate(text) {
        return Ok(None);
    }

    let Some(entry) = local_dictionary::lookup_word(app, text)? else {
        info!("本地词典未命中: {}", text);
        return Ok(None);
    };

    info!("本地词典命中: {}", text);
    let translation = build_translation(text.to_string(), to_translation_content(entry.clone()));
    Ok(Some((entry, translation)))
}

// ─── Clipboard ───────────────────────────────────────────────────────────────

pub fn read_current_clipboard_text(app: &tauri::AppHandle) -> Result<String, String> {
    use crate::clipboard;
    let text = clipboard::read_clipboard(app)?;
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return Err("剪贴板为空".to_string());
    }

    Ok(trimmed.to_string())
}

// ─── Translation Resolution ──────────────────────────────────────────────────

/// Full resolution: word → local + Free Dict; sentence → Youdao.
pub async fn resolve_translation(
    app: &tauri::AppHandle,
    text: &str,
    config: &TranslationConfig,
) -> Result<Translation, String> {
    let is_word = is_single_word(text);
    info!(
        "翻译文本: {}, 类型: {}",
        text,
        if is_word { "单词" } else { "句子" }
    );

    if is_word {
        // 1. 本地词典
        if let Some((entry, base_translation)) = lookup_local_translation(app, text)? {
            let supplement = match translator::fetch_free_dictionary_supplement(text).await {
                Ok(supplement) => supplement,
                Err(error) => {
                    warn!("Free Dictionary 补全失败: {}", error);
                    None
                }
            };
            let merged = local_dictionary::merge_free_dictionary_supplement(entry, supplement);
            return Ok(build_translation_from_existing(
                &base_translation,
                to_translation_content(merged),
            ));
        }

        // 2. Free Dictionary 回退
        info!("本地词典未命中，尝试 Free Dictionary");
        match translator::fetch_free_dictionary_supplement(text).await {
            Ok(Some(supplement)) => {
                info!("Free Dictionary 查询成功");
                let content = translator::TranslationContent {
                    translated_text: supplement
                        .explains
                        .first()
                        .and_then(|s| s.split(". ").nth(1))
                        .unwrap_or(text)
                        .to_string(),
                    phonetic: supplement.phonetic.clone(),
                    us_phonetic: supplement.phonetic.clone(),
                    uk_phonetic: None,
                    audio_url: supplement.audio_url,
                    explains: supplement.explains,
                    examples: supplement.examples,
                    synonyms: supplement.synonyms,
                    word_type: None,
                };
                return Ok(build_translation(text.to_string(), content));
            }
            Ok(None) => {
                warn!("Free Dictionary 未找到单词: {}", text);
                return Err(format!("未找到单词 \"{}\" 的释义", text));
            }
            Err(e) => {
                error!("Free Dictionary 查询失败: {}", e);
                return Err(format!("查询单词失败: {}", e));
            }
        }
    }

    // 句子使用有道翻译
    resolve_remote_provider_translation(text, config).await
}

/// Youdao-only translation (for sentence fallback).
pub async fn resolve_youdao_translation(
    text: &str,
    app_key: &str,
    app_secret: &str,
) -> Result<Translation, String> {
    if app_key.is_empty() || app_secret.is_empty() {
        return Err("翻译句子需要配置有道翻译 API，请在设置中配置".to_string());
    }

    info!("使用有道翻译 API");
    let content = translator::translate_text(text, app_key, app_secret).await?;
    Ok(build_translation(text.to_string(), content))
}

pub async fn resolve_microsoft_translation(
    text: &str,
    key: &str,
    region: &str,
) -> Result<Translation, String> {
    info!("使用微软翻译 API");
    let content = translator::translate_with_microsoft(text, key, region).await?;
    Ok(build_translation(text.to_string(), content))
}

pub async fn resolve_remote_provider_translation(
    text: &str,
    config: &TranslationConfig,
) -> Result<Translation, String> {
    match config.provider.trim().to_lowercase().as_str() {
        "microsoft" => {
            resolve_microsoft_translation(text, &config.microsoft_key, &config.microsoft_region)
                .await
        }
        _ => {
            resolve_youdao_translation(text, &config.youdao_app_key, &config.youdao_app_secret)
                .await
        }
    }
}

/// Remote-only resolution (used by shortcut handler when local dict misses).
pub async fn resolve_remote_translation(
    text: &str,
    config: &TranslationConfig,
) -> Result<Translation, String> {
    let is_word = is_single_word(text);
    info!(
        "翻译文本: {}, 类型: {}",
        text,
        if is_word { "单词" } else { "句子" }
    );

    if is_word {
        info!("尝试 Free Dictionary");
        match translator::fetch_free_dictionary_supplement(text).await {
            Ok(Some(supplement)) => {
                info!("Free Dictionary 查询成功");
                let content = translator::TranslationContent {
                    translated_text: supplement
                        .explains
                        .first()
                        .and_then(|s| s.split(". ").nth(1))
                        .unwrap_or(text)
                        .to_string(),
                    phonetic: supplement.phonetic.clone(),
                    us_phonetic: supplement.phonetic.clone(),
                    uk_phonetic: None,
                    audio_url: supplement.audio_url,
                    explains: supplement.explains,
                    examples: supplement.examples,
                    synonyms: supplement.synonyms,
                    word_type: None,
                };
                return Ok(build_translation(text.to_string(), content));
            }
            Ok(None) => {
                warn!("Free Dictionary 未找到单词: {}", text);
                return Err(format!("未找到单词 \"{}\" 的释义", text));
            }
            Err(e) => {
                error!("Free Dictionary 查询失败: {}", e);
                return Err(format!("查询单词失败: {}", e));
            }
        }
    }

    resolve_remote_provider_translation(text, config).await
}
