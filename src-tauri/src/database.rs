use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Translation {
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
