use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranslationContent {
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranslationConfig {
    pub provider: String,
    pub youdao_app_key: String,
    pub youdao_app_secret: String,
    pub microsoft_key: String,
    pub microsoft_region: String,
}

/// Translation content produced by the application workflow.
///
/// Persistence identity, access counters, favorite state, and timestamps belong
/// to the database record boundary rather than the translation domain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TranslationResult {
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
}

impl TranslationResult {
    pub fn from_content(source_text: String, content: TranslationContent) -> Self {
        Self {
            source_text,
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
        }
    }

    pub fn with_content(&self, content: TranslationContent) -> Self {
        Self {
            source_text: self.source_text.clone(),
            source_lang: self.source_lang.clone(),
            target_lang: self.target_lang.clone(),
            ..Self::from_content(self.source_text.clone(), content)
        }
    }
}

fn some_if_not_empty(items: Vec<String>) -> Option<Vec<String>> {
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}
