use std::future::Future;

use tracing::{info, warn};

use crate::{
    local_dictionary::{
        merge_free_dictionary_supplement, FreeDictionarySupplement, OfflineDictionaryEntry,
    },
    translation_domain::{TranslationConfig, TranslationContent, TranslationResult},
};

pub trait DictionaryGateway: Send + Sync {
    fn lookup_local(&self, text: &str) -> Result<Option<OfflineDictionaryEntry>, String>;

    fn fetch_supplement<'a>(
        &'a self,
        text: &'a str,
    ) -> impl Future<Output = Result<Option<FreeDictionarySupplement>, String>> + Send + 'a;
}

pub trait ProviderGateway: Send + Sync {
    fn translate_youdao<'a>(
        &'a self,
        text: &'a str,
        app_key: &'a str,
        app_secret: &'a str,
    ) -> impl Future<Output = Result<TranslationContent, String>> + Send + 'a;

    fn translate_microsoft<'a>(
        &'a self,
        text: &'a str,
        key: &'a str,
        region: &'a str,
    ) -> impl Future<Output = Result<TranslationContent, String>> + Send + 'a;

    fn translate_google<'a>(
        &'a self,
        text: &'a str,
        api_key: &'a str,
    ) -> impl Future<Output = Result<TranslationContent, String>> + Send + 'a;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionStage {
    InputAccepted { text: String },
    LocalResultAvailable(TranslationResult),
    EnrichmentAvailable(TranslationResult),
    RemoteTranslationInProgress,
    Completed(TranslationResult),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionError {
    Cancelled,
    Failed(String),
}

impl ResolutionError {
    pub fn into_message(self) -> String {
        match self {
            Self::Cancelled => "翻译请求已取消".to_string(),
            Self::Failed(message) => message,
        }
    }
}

pub async fn resolve_translation<D, P, F, C>(
    dictionary: &D,
    providers: &P,
    config: &TranslationConfig,
    text: &str,
    mut report: F,
    is_cancelled: C,
) -> Result<TranslationResult, ResolutionError>
where
    D: DictionaryGateway,
    P: ProviderGateway,
    F: FnMut(ResolutionStage) -> Result<(), String>,
    C: Fn() -> bool,
{
    let text = text.trim();
    if text.is_empty() {
        return Err(ResolutionError::Failed("输入文本为空".to_string()));
    }

    report_stage(
        &mut report,
        &is_cancelled,
        ResolutionStage::InputAccepted {
            text: text.to_string(),
        },
    )?;

    let is_word = is_single_word(text);
    info!(
        "翻译文本: {}, 类型: {}",
        text,
        if is_word { "单词" } else { "句子" }
    );

    if is_word {
        let local_entry = lookup_local(dictionary, text).map_err(ResolutionError::Failed)?;

        if let Some(entry) = local_entry {
            let local_result = TranslationResult::from_content(
                text.to_string(),
                content_from_dictionary_entry(entry.clone()),
            );
            report_stage(
                &mut report,
                &is_cancelled,
                ResolutionStage::LocalResultAvailable(local_result.clone()),
            )?;

            let supplement = dictionary.fetch_supplement(text).await;
            ensure_active(&is_cancelled)?;

            let final_result = match supplement {
                Ok(supplement) => {
                    let merged = merge_free_dictionary_supplement(entry, supplement);
                    let enriched = local_result.with_content(content_from_dictionary_entry(merged));
                    if enriched != local_result {
                        report_stage(
                            &mut report,
                            &is_cancelled,
                            ResolutionStage::EnrichmentAvailable(enriched.clone()),
                        )?;
                        enriched
                    } else {
                        local_result
                    }
                }
                Err(error) => {
                    warn!("Free Dictionary 补全失败: {}", error);
                    local_result
                }
            };

            report_stage(
                &mut report,
                &is_cancelled,
                ResolutionStage::Completed(final_result.clone()),
            )?;
            return Ok(final_result);
        }

        info!("本地词典未命中，尝试 Free Dictionary");
        let dictionary_error = match dictionary.fetch_supplement(text).await {
            Ok(Some(supplement)) => {
                ensure_active(&is_cancelled)?;
                let result = TranslationResult::from_content(
                    text.to_string(),
                    content_from_supplement(text, supplement),
                );
                report_stage(
                    &mut report,
                    &is_cancelled,
                    ResolutionStage::Completed(result.clone()),
                )?;
                return Ok(result);
            }
            Ok(None) => format!("未找到单词 \"{}\" 的释义", text),
            Err(error) => format!("查询单词失败: {}", error),
        };

        return resolve_remote(
            providers,
            config,
            text,
            Some(dictionary_error),
            report,
            is_cancelled,
        )
        .await;
    }

    resolve_remote(providers, config, text, None, report, is_cancelled).await
}

async fn resolve_remote<P, F, C>(
    providers: &P,
    config: &TranslationConfig,
    text: &str,
    dictionary_error: Option<String>,
    mut report: F,
    is_cancelled: C,
) -> Result<TranslationResult, ResolutionError>
where
    P: ProviderGateway,
    F: FnMut(ResolutionStage) -> Result<(), String>,
    C: Fn() -> bool,
{
    report_stage(
        &mut report,
        &is_cancelled,
        ResolutionStage::RemoteTranslationInProgress,
    )?;

    let content = match config.provider.trim().to_ascii_lowercase().as_str() {
        "microsoft" => {
            providers
                .translate_microsoft(text, &config.microsoft_key, &config.microsoft_region)
                .await
        }
        "google" => {
            if config.google_api_key.trim().is_empty() {
                Err("使用 Google 翻译需要配置 API Key，请在设置中配置".to_string())
            } else {
                providers
                    .translate_google(text, &config.google_api_key)
                    .await
            }
        }
        _ => {
            if config.youdao_app_key.trim().is_empty() || config.youdao_app_secret.trim().is_empty()
            {
                Err("翻译句子需要配置有道翻译 API，请在设置中配置".to_string())
            } else {
                providers
                    .translate_youdao(text, &config.youdao_app_key, &config.youdao_app_secret)
                    .await
            }
        }
    };
    ensure_active(&is_cancelled)?;

    let content = content.map_err(|remote_error| {
        ResolutionError::Failed(match dictionary_error {
            Some(dictionary_error) => {
                format!("{}；在线翻译回退失败: {}", dictionary_error, remote_error)
            }
            None => remote_error,
        })
    })?;

    let result = TranslationResult::from_content(text.to_string(), content);
    report_stage(
        &mut report,
        &is_cancelled,
        ResolutionStage::Completed(result.clone()),
    )?;
    Ok(result)
}

fn report_stage<F, C>(
    report: &mut F,
    is_cancelled: &C,
    stage: ResolutionStage,
) -> Result<(), ResolutionError>
where
    F: FnMut(ResolutionStage) -> Result<(), String>,
    C: Fn() -> bool,
{
    ensure_active(is_cancelled)?;
    report(stage).map_err(ResolutionError::Failed)
}

fn ensure_active<C>(is_cancelled: &C) -> Result<(), ResolutionError>
where
    C: Fn() -> bool,
{
    if is_cancelled() {
        Err(ResolutionError::Cancelled)
    } else {
        Ok(())
    }
}

fn lookup_local<D>(dictionary: &D, text: &str) -> Result<Option<OfflineDictionaryEntry>, String>
where
    D: DictionaryGateway,
{
    if !is_local_dictionary_candidate(text) {
        return Ok(None);
    }

    dictionary.lookup_local(text)
}

pub fn is_local_dictionary_candidate(text: &str) -> bool {
    !text.contains(' ')
        && !text.contains(',')
        && !text.contains('.')
        && text.chars().all(|ch| ch.is_ascii_alphabetic())
}

pub fn is_single_word(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty()
        || trimmed.contains(' ')
        || trimmed.contains(',')
        || trimmed.contains('.')
        || trimmed.contains('!')
        || trimmed.contains('?')
    {
        return false;
    }

    let has_internal_uppercase = trimmed.chars().skip(1).any(char::is_uppercase);
    if has_internal_uppercase {
        return false;
    }

    trimmed
        .chars()
        .all(|character| character.is_alphabetic() || character == '\'' || character == '-')
}

fn content_from_dictionary_entry(entry: OfflineDictionaryEntry) -> TranslationContent {
    TranslationContent {
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

fn content_from_supplement(
    source_text: &str,
    supplement: FreeDictionarySupplement,
) -> TranslationContent {
    let translated_text = supplement
        .explains
        .first()
        .and_then(|explanation| explanation.split(". ").nth(1))
        .unwrap_or(source_text)
        .to_string();

    TranslationContent {
        translated_text,
        phonetic: supplement.phonetic.clone(),
        us_phonetic: supplement.phonetic,
        uk_phonetic: None,
        audio_url: supplement.audio_url,
        explains: supplement.explains,
        examples: supplement.examples,
        synonyms: supplement.synonyms,
        word_type: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::collections::VecDeque;

    #[derive(Default)]
    struct FakeDictionary {
        local: Option<OfflineDictionaryEntry>,
        local_error: Option<String>,
        supplements: Mutex<VecDeque<Result<Option<FreeDictionarySupplement>, String>>>,
    }

    impl DictionaryGateway for FakeDictionary {
        fn lookup_local(&self, _text: &str) -> Result<Option<OfflineDictionaryEntry>, String> {
            match self.local_error.as_ref() {
                Some(error) => Err(error.clone()),
                None => Ok(self.local.clone()),
            }
        }

        fn fetch_supplement<'a>(
            &'a self,
            _text: &'a str,
        ) -> impl Future<Output = Result<Option<FreeDictionarySupplement>, String>> + Send + 'a
        {
            let result = self.supplements.lock().pop_front().unwrap_or(Ok(None));
            async move { result }
        }
    }

    #[derive(Default)]
    struct FakeProviders {
        calls: Mutex<Vec<String>>,
        youdao: Mutex<VecDeque<Result<TranslationContent, String>>>,
        microsoft: Mutex<VecDeque<Result<TranslationContent, String>>>,
        google: Mutex<VecDeque<Result<TranslationContent, String>>>,
    }

    impl ProviderGateway for FakeProviders {
        fn translate_youdao<'a>(
            &'a self,
            _text: &'a str,
            _app_key: &'a str,
            _app_secret: &'a str,
        ) -> impl Future<Output = Result<TranslationContent, String>> + Send + 'a {
            self.calls.lock().push("youdao".to_string());
            let result = self
                .youdao
                .lock()
                .pop_front()
                .unwrap_or_else(|| Err("unexpected youdao call".to_string()));
            async move { result }
        }

        fn translate_microsoft<'a>(
            &'a self,
            _text: &'a str,
            _key: &'a str,
            _region: &'a str,
        ) -> impl Future<Output = Result<TranslationContent, String>> + Send + 'a {
            self.calls.lock().push("microsoft".to_string());
            let result = self
                .microsoft
                .lock()
                .pop_front()
                .unwrap_or_else(|| Err("unexpected microsoft call".to_string()));
            async move { result }
        }

        fn translate_google<'a>(
            &'a self,
            _text: &'a str,
            _api_key: &'a str,
        ) -> impl Future<Output = Result<TranslationContent, String>> + Send + 'a {
            self.calls.lock().push("google".to_string());
            let result = self
                .google
                .lock()
                .pop_front()
                .unwrap_or_else(|| Err("unexpected google call".to_string()));
            async move { result }
        }
    }

    fn local_entry() -> OfflineDictionaryEntry {
        OfflineDictionaryEntry {
            word: "hello".to_string(),
            translated_text: "你好".to_string(),
            phonetic: None,
            us_phonetic: None,
            uk_phonetic: None,
            audio_url: None,
            explains: vec!["int. 你好".to_string()],
            examples: Vec::new(),
            synonyms: Vec::new(),
            word_type: Some("int.".to_string()),
        }
    }

    fn supplement() -> FreeDictionarySupplement {
        FreeDictionarySupplement {
            phonetic: Some("/həˈloʊ/".to_string()),
            audio_url: Some("https://example.com/hello.mp3".to_string()),
            explains: vec!["interjection. greeting".to_string()],
            examples: vec!["Hello, world!".to_string()],
            synonyms: vec!["hi".to_string()],
        }
    }

    fn remote_content(text: &str) -> TranslationContent {
        TranslationContent {
            translated_text: text.to_string(),
            ..TranslationContent::default()
        }
    }

    #[test]
    fn single_word_detection_accepts_regular_words_only() {
        assert!(is_single_word("hello"));
        assert!(is_single_word("well-known"));
        assert!(is_single_word("don't"));
        assert!(!is_single_word("hello world"));
        assert!(!is_single_word("camelCase"));
        assert!(!is_single_word(""));
    }

    #[test]
    fn local_result_is_reported_before_enrichment() {
        tauri::async_runtime::block_on(async {
            let dictionary = FakeDictionary {
                local: Some(local_entry()),
                supplements: Mutex::new(VecDeque::from([Ok(Some(supplement()))])),
                local_error: None,
            };
            let providers = FakeProviders::default();
            let mut stages = Vec::new();

            let result = resolve_translation(
                &dictionary,
                &providers,
                &TranslationConfig::default(),
                "hello",
                |stage| {
                    stages.push(stage);
                    Ok(())
                },
                || false,
            )
            .await
            .unwrap();

            assert_eq!(result.examples, Some(vec!["Hello, world!".to_string()]));
            assert!(matches!(stages[0], ResolutionStage::InputAccepted { .. }));
            assert!(matches!(
                stages[1],
                ResolutionStage::LocalResultAvailable(_)
            ));
            assert!(matches!(stages[2], ResolutionStage::EnrichmentAvailable(_)));
            assert!(matches!(stages[3], ResolutionStage::Completed(_)));
            assert!(providers.calls.lock().is_empty());
        });
    }

    #[test]
    fn local_miss_uses_current_configured_provider() {
        tauri::async_runtime::block_on(async {
            let dictionary = FakeDictionary {
                local: None,
                supplements: Mutex::new(VecDeque::from([Ok(None)])),
                local_error: None,
            };
            let providers = FakeProviders {
                microsoft: Mutex::new(VecDeque::from([Ok(remote_content("你好"))])),
                ..FakeProviders::default()
            };
            let config = TranslationConfig {
                provider: "microsoft".to_string(),
                microsoft_key: "key".to_string(),
                ..TranslationConfig::default()
            };

            let result = resolve_translation(
                &dictionary,
                &providers,
                &config,
                "hello",
                |_| Ok(()),
                || false,
            )
            .await
            .unwrap();

            assert_eq!(result.translated_text, "你好");
            assert_eq!(providers.calls.lock().as_slice(), ["microsoft"]);
        });
    }

    #[test]
    fn local_lookup_error_stops_before_online_collaborators() {
        tauri::async_runtime::block_on(async {
            let dictionary = FakeDictionary {
                local: None,
                local_error: Some("local database unavailable".to_string()),
                supplements: Mutex::new(VecDeque::from([Ok(None)])),
            };
            let providers = FakeProviders::default();

            let error = resolve_translation(
                &dictionary,
                &providers,
                &TranslationConfig::default(),
                "hello",
                |_| Ok(()),
                || false,
            )
            .await
            .unwrap_err()
            .into_message();

            assert_eq!(error, "local database unavailable");
            assert_eq!(dictionary.supplements.lock().len(), 1);
            assert!(providers.calls.lock().is_empty());
        });
    }

    #[test]
    fn missing_youdao_credentials_fail_before_provider_call() {
        tauri::async_runtime::block_on(async {
            let dictionary = FakeDictionary::default();
            let providers = FakeProviders::default();

            let error = resolve_translation(
                &dictionary,
                &providers,
                &TranslationConfig::default(),
                "hello world",
                |_| Ok(()),
                || false,
            )
            .await
            .unwrap_err()
            .into_message();

            assert_eq!(error, "翻译句子需要配置有道翻译 API，请在设置中配置");
            assert!(providers.calls.lock().is_empty());
        });
    }

    #[test]
    fn provider_failure_keeps_dictionary_context() {
        tauri::async_runtime::block_on(async {
            let dictionary = FakeDictionary {
                local: None,
                supplements: Mutex::new(VecDeque::from([Ok(None)])),
                local_error: None,
            };
            let providers = FakeProviders {
                youdao: Mutex::new(VecDeque::from([Err("provider unavailable".to_string())])),
                ..FakeProviders::default()
            };

            let config = TranslationConfig {
                youdao_app_key: "key".to_string(),
                youdao_app_secret: "secret".to_string(),
                ..TranslationConfig::default()
            };

            let error = resolve_translation(
                &dictionary,
                &providers,
                &config,
                "hello",
                |_| Ok(()),
                || false,
            )
            .await
            .unwrap_err()
            .into_message();

            assert_eq!(
                error,
                "未找到单词 \"hello\" 的释义；在线翻译回退失败: provider unavailable"
            );
        });
    }
}
