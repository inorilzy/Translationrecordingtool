use std::{
    future::Future,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use parking_lot::RwLock;
use tauri::AppHandle;

use crate::{
    app_state::AppConfig,
    database::{open_translations_connection, save_translation_in_connection, TranslationRecord},
    local_dictionary::{self, FreeDictionarySupplement, OfflineDictionaryEntry},
    ocr_contracts::OcrRuntimeConfig,
    translation_domain::{TranslationConfig, TranslationContent, TranslationResult},
    translation_flow::{
        self, DictionaryGateway, ProviderGateway, ResolutionError, ResolutionStage,
    },
    translator,
};

pub trait TranslationRepository: Send + Sync {
    fn save_new(&self, result: &TranslationResult) -> Result<TranslationRecord, String>;

    fn update(
        &self,
        existing: &TranslationRecord,
        result: &TranslationResult,
    ) -> Result<TranslationRecord, String>;
}

#[derive(Clone)]
pub struct ScreenshotRequestSettings {
    pub translation: TranslationConfig,
    pub ocr: OcrRuntimeConfig,
}

pub trait RuntimeSettingsSource: Send + Sync {
    fn translation_config(&self) -> TranslationConfig;
    fn screenshot_settings(&self) -> ScreenshotRequestSettings;
}

pub trait OcrGateway: Send + Sync {
    fn recognize<'a>(
        &'a self,
        config: &'a OcrRuntimeConfig,
        image_base64: &'a str,
    ) -> impl Future<Output = Result<String, String>> + Send + 'a;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowStage {
    OcrInProgress,
    InputAccepted { text: String },
    LocalResultAvailable(TranslationRecord),
    EnrichmentAvailable(TranslationRecord),
    RemoteTranslationInProgress,
    Completed(TranslationRecord),
    Cancelled,
    Failed { message: String },
}

pub struct TranslationWorkflow<D, P, R, S, O> {
    dictionary: D,
    providers: P,
    repository: R,
    settings: S,
    ocr: O,
}

impl<D, P, R, S, O> TranslationWorkflow<D, P, R, S, O>
where
    D: DictionaryGateway,
    P: ProviderGateway,
    R: TranslationRepository,
    S: RuntimeSettingsSource,
    O: OcrGateway,
{
    pub fn new(dictionary: D, providers: P, repository: R, settings: S, ocr: O) -> Self {
        Self {
            dictionary,
            providers,
            repository,
            settings,
            ocr,
        }
    }

    pub async fn translate_text<F, C>(
        &self,
        text: &str,
        report: &mut F,
        is_cancelled: &C,
    ) -> Result<TranslationRecord, String>
    where
        F: FnMut(WorkflowStage),
        C: Fn() -> bool,
    {
        let config = self.settings.translation_config();
        self.translate_text_with_config(text, &config, report, is_cancelled)
            .await
    }

    async fn translate_text_with_config<F, C>(
        &self,
        text: &str,
        config: &TranslationConfig,
        report: &mut F,
        is_cancelled: &C,
    ) -> Result<TranslationRecord, String>
    where
        F: FnMut(WorkflowStage),
        C: Fn() -> bool,
    {
        let mut persisted: Option<TranslationRecord> = None;

        let result = translation_flow::resolve_translation(
            &self.dictionary,
            &self.providers,
            config,
            text,
            |stage| {
                match stage {
                    ResolutionStage::InputAccepted { text } => {
                        report(WorkflowStage::InputAccepted { text });
                    }
                    ResolutionStage::LocalResultAvailable(result) => {
                        let record = self.repository.save_new(&result)?;
                        report(WorkflowStage::LocalResultAvailable(record.clone()));
                        persisted = Some(record);
                    }
                    ResolutionStage::EnrichmentAvailable(result) => {
                        let record = match persisted.as_ref() {
                            Some(existing) => self.repository.update(existing, &result)?,
                            None => self.repository.save_new(&result)?,
                        };
                        report(WorkflowStage::EnrichmentAvailable(record.clone()));
                        persisted = Some(record);
                    }
                    ResolutionStage::RemoteTranslationInProgress => {
                        report(WorkflowStage::RemoteTranslationInProgress);
                    }
                    ResolutionStage::Completed(result) => {
                        let record = match persisted.as_ref() {
                            Some(existing) if existing.has_same_content_as(&result) => {
                                existing.clone()
                            }
                            Some(existing) => self.repository.update(existing, &result)?,
                            None => self.repository.save_new(&result)?,
                        };
                        report(WorkflowStage::Completed(record.clone()));
                        persisted = Some(record);
                    }
                }
                Ok(())
            },
            is_cancelled,
        )
        .await;

        match result {
            Ok(_) => persisted.ok_or_else(|| {
                let message = "翻译完成但未生成持久化记录".to_string();
                report(WorkflowStage::Failed {
                    message: message.clone(),
                });
                message
            }),
            Err(ResolutionError::Cancelled) => {
                report(WorkflowStage::Cancelled);
                Err(ResolutionError::Cancelled.into_message())
            }
            Err(ResolutionError::Failed(message)) => {
                report(WorkflowStage::Failed {
                    message: message.clone(),
                });
                Err(message)
            }
        }
    }

    pub async fn translate_image<F, C>(
        &self,
        image_base64: &str,
        report: &mut F,
        is_cancelled: &C,
    ) -> Result<TranslationRecord, String>
    where
        F: FnMut(WorkflowStage),
        C: Fn() -> bool,
    {
        if is_cancelled() {
            report(WorkflowStage::Cancelled);
            return Err(ResolutionError::Cancelled.into_message());
        }

        let settings = self.settings.screenshot_settings();
        report(WorkflowStage::OcrInProgress);
        let recognized = match self.ocr.recognize(&settings.ocr, image_base64).await {
            Ok(text) => text,
            Err(message) => {
                report(WorkflowStage::Failed {
                    message: message.clone(),
                });
                return Err(message);
            }
        };

        if is_cancelled() {
            report(WorkflowStage::Cancelled);
            return Err(ResolutionError::Cancelled.into_message());
        }

        let recognized = recognized.trim();
        if recognized.is_empty() {
            let message = "OCR 未识别到文本".to_string();
            report(WorkflowStage::Failed {
                message: message.clone(),
            });
            return Err(message);
        }

        self.translate_text_with_config(recognized, &settings.translation, report, is_cancelled)
            .await
    }
}

#[derive(Clone)]
pub struct AppDictionaryGateway {
    app: AppHandle,
}

impl DictionaryGateway for AppDictionaryGateway {
    fn lookup_local(&self, text: &str) -> Result<Option<OfflineDictionaryEntry>, String> {
        local_dictionary::lookup_word(&self.app, text)
    }

    fn fetch_supplement<'a>(
        &'a self,
        text: &'a str,
    ) -> impl Future<Output = Result<Option<FreeDictionarySupplement>, String>> + Send + 'a {
        async move { translator::fetch_free_dictionary_supplement(text).await }
    }
}

#[derive(Clone, Copy, Default)]
pub struct AppProviderGateway;

impl ProviderGateway for AppProviderGateway {
    fn translate_youdao<'a>(
        &'a self,
        text: &'a str,
        app_key: &'a str,
        app_secret: &'a str,
    ) -> impl Future<Output = Result<TranslationContent, String>> + Send + 'a {
        async move { translator::translate_text(text, app_key, app_secret).await }
    }

    fn translate_microsoft<'a>(
        &'a self,
        text: &'a str,
        key: &'a str,
        region: &'a str,
    ) -> impl Future<Output = Result<TranslationContent, String>> + Send + 'a {
        async move { translator::translate_with_microsoft(text, key, region).await }
    }

    fn translate_google<'a>(
        &'a self,
        text: &'a str,
        api_key: &'a str,
    ) -> impl Future<Output = Result<TranslationContent, String>> + Send + 'a {
        async move { translator::translate_with_google(text, api_key).await }
    }
}

#[derive(Clone)]
pub struct AppTranslationRepository {
    app: AppHandle,
}

impl TranslationRepository for AppTranslationRepository {
    fn save_new(&self, result: &TranslationResult) -> Result<TranslationRecord, String> {
        let connection = open_translations_connection(&self.app)?;
        let record = TranslationRecord::from_result(result.clone(), current_timestamp());
        save_translation_in_connection(&connection, &record, true)
    }

    fn update(
        &self,
        existing: &TranslationRecord,
        result: &TranslationResult,
    ) -> Result<TranslationRecord, String> {
        let connection = open_translations_connection(&self.app)?;
        let record = existing.with_result(result.clone());
        save_translation_in_connection(&connection, &record, false)
    }
}

#[derive(Clone)]
pub struct ManagedRuntimeSettings {
    config: Arc<RwLock<AppConfig>>,
}

impl RuntimeSettingsSource for ManagedRuntimeSettings {
    fn translation_config(&self) -> TranslationConfig {
        self.config.read().translation_runtime_config()
    }

    fn screenshot_settings(&self) -> ScreenshotRequestSettings {
        let config = self.config.read();
        ScreenshotRequestSettings {
            translation: config.translation_runtime_config(),
            ocr: config.ocr_runtime_config(),
        }
    }
}

#[derive(Clone)]
pub struct AppOcrGateway {
    app: AppHandle,
}

impl OcrGateway for AppOcrGateway {
    fn recognize<'a>(
        &'a self,
        config: &'a OcrRuntimeConfig,
        image_base64: &'a str,
    ) -> impl Future<Output = Result<String, String>> + Send + 'a {
        async move { crate::ocr::recognize_text_with_config(&self.app, config, image_base64).await }
    }
}

pub type AppTranslationWorkflow = TranslationWorkflow<
    AppDictionaryGateway,
    AppProviderGateway,
    AppTranslationRepository,
    ManagedRuntimeSettings,
    AppOcrGateway,
>;

pub fn create_app_workflow(
    app: AppHandle,
    config: Arc<RwLock<AppConfig>>,
) -> AppTranslationWorkflow {
    TranslationWorkflow::new(
        AppDictionaryGateway { app: app.clone() },
        AppProviderGateway,
        AppTranslationRepository { app: app.clone() },
        ManagedRuntimeSettings { config },
        AppOcrGateway { app },
    )
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after Unix epoch")
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::{
        collections::VecDeque,
        sync::atomic::{AtomicBool, Ordering},
    };

    #[derive(Default)]
    struct FakeDictionary {
        local: Option<OfflineDictionaryEntry>,
        supplements: Mutex<VecDeque<Result<Option<FreeDictionarySupplement>, String>>>,
        cancel_after_fetch: Option<Arc<AtomicBool>>,
    }

    impl DictionaryGateway for FakeDictionary {
        fn lookup_local(&self, _text: &str) -> Result<Option<OfflineDictionaryEntry>, String> {
            Ok(self.local.clone())
        }

        fn fetch_supplement<'a>(
            &'a self,
            _text: &'a str,
        ) -> impl Future<Output = Result<Option<FreeDictionarySupplement>, String>> + Send + 'a
        {
            let result = self.supplements.lock().pop_front().unwrap_or(Ok(None));
            let cancel = self.cancel_after_fetch.clone();
            async move {
                if let Some(cancel) = cancel {
                    cancel.store(true, Ordering::SeqCst);
                }
                result
            }
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
            let result = self.youdao.lock().pop_front().unwrap();
            async move { result }
        }

        fn translate_microsoft<'a>(
            &'a self,
            _text: &'a str,
            _key: &'a str,
            _region: &'a str,
        ) -> impl Future<Output = Result<TranslationContent, String>> + Send + 'a {
            self.calls.lock().push("microsoft".to_string());
            let result = self.microsoft.lock().pop_front().unwrap();
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

    #[derive(Default)]
    struct FakeRepository {
        saves: Mutex<usize>,
        updates: Mutex<usize>,
    }

    impl TranslationRepository for FakeRepository {
        fn save_new(&self, result: &TranslationResult) -> Result<TranslationRecord, String> {
            *self.saves.lock() += 1;
            let mut record = TranslationRecord::from_result(result.clone(), 100);
            record.id = Some(1);
            Ok(record)
        }

        fn update(
            &self,
            existing: &TranslationRecord,
            result: &TranslationResult,
        ) -> Result<TranslationRecord, String> {
            *self.updates.lock() += 1;
            Ok(existing.with_result(result.clone()))
        }
    }

    struct FakeSettings {
        translation: RwLock<TranslationConfig>,
        ocr: OcrRuntimeConfig,
    }

    impl RuntimeSettingsSource for FakeSettings {
        fn translation_config(&self) -> TranslationConfig {
            self.translation.read().clone()
        }

        fn screenshot_settings(&self) -> ScreenshotRequestSettings {
            ScreenshotRequestSettings {
                translation: self.translation.read().clone(),
                ocr: self.ocr.clone(),
            }
        }
    }

    struct FakeOcr {
        result: Mutex<VecDeque<Result<String, String>>>,
    }

    impl OcrGateway for FakeOcr {
        fn recognize<'a>(
            &'a self,
            _config: &'a OcrRuntimeConfig,
            _image_base64: &'a str,
        ) -> impl Future<Output = Result<String, String>> + Send + 'a {
            let result = self.result.lock().pop_front().unwrap();
            async move { result }
        }
    }

    struct SharedSettings {
        translation: Arc<RwLock<TranslationConfig>>,
        ocr: OcrRuntimeConfig,
    }

    impl RuntimeSettingsSource for SharedSettings {
        fn translation_config(&self) -> TranslationConfig {
            self.translation.read().clone()
        }

        fn screenshot_settings(&self) -> ScreenshotRequestSettings {
            ScreenshotRequestSettings {
                translation: self.translation.read().clone(),
                ocr: self.ocr.clone(),
            }
        }
    }

    struct SettingsMutatingOcr {
        translation: Arc<RwLock<TranslationConfig>>,
        result: String,
    }

    impl OcrGateway for SettingsMutatingOcr {
        fn recognize<'a>(
            &'a self,
            _config: &'a OcrRuntimeConfig,
            _image_base64: &'a str,
        ) -> impl Future<Output = Result<String, String>> + Send + 'a {
            self.translation.write().provider = "microsoft".to_string();
            let result = self.result.clone();
            async move { Ok(result) }
        }
    }
    #[test]
    fn image_translation_uses_one_settings_snapshot() {
        tauri::async_runtime::block_on(async {
            let translation = Arc::new(RwLock::new(TranslationConfig {
                provider: "youdao".to_string(),
                youdao_app_key: "key".to_string(),
                youdao_app_secret: "secret".to_string(),
                microsoft_key: "key".to_string(),
                ..TranslationConfig::default()
            }));
            let workflow = TranslationWorkflow::new(
                FakeDictionary {
                    local: None,
                    supplements: Mutex::new(VecDeque::from([Ok(None)])),
                    cancel_after_fetch: None,
                },
                FakeProviders {
                    youdao: Mutex::new(VecDeque::from([Ok(content("有道"))])),
                    microsoft: Mutex::new(VecDeque::from([Ok(content("微软"))])),
                    ..FakeProviders::default()
                },
                FakeRepository::default(),
                SharedSettings {
                    translation: translation.clone(),
                    ocr: OcrRuntimeConfig {
                        endpoint: "http://127.0.0.1:8866/ocr".to_string(),
                        engine: "native_onnx".to_string(),
                        model_profile: "small".to_string(),
                        preload_on_startup: true,
                    },
                },
                SettingsMutatingOcr {
                    translation,
                    result: "screenshot text".to_string(),
                },
            );

            workflow
                .translate_image("image", &mut |_| {}, &|| false)
                .await
                .unwrap();

            assert_eq!(workflow.providers.calls.lock().as_slice(), ["youdao"]);
        });
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
            audio_url: None,
            explains: vec!["interjection. greeting".to_string()],
            examples: vec!["Hello, world!".to_string()],
            synonyms: vec!["hi".to_string()],
        }
    }

    fn content(text: &str) -> TranslationContent {
        TranslationContent {
            translated_text: text.to_string(),
            ..TranslationContent::default()
        }
    }

    fn settings(provider: &str) -> FakeSettings {
        FakeSettings {
            translation: RwLock::new(TranslationConfig {
                provider: provider.to_string(),
                youdao_app_key: "key".to_string(),
                youdao_app_secret: "secret".to_string(),
                ..TranslationConfig::default()
            }),
            ocr: OcrRuntimeConfig {
                endpoint: "http://127.0.0.1:8866/ocr".to_string(),
                engine: "native_onnx".to_string(),
                model_profile: "small".to_string(),
                preload_on_startup: true,
            },
        }
    }

    #[test]
    fn local_enrichment_persists_once_and_updates_without_incrementing() {
        tauri::async_runtime::block_on(async {
            let workflow = TranslationWorkflow::new(
                FakeDictionary {
                    local: Some(local_entry()),
                    supplements: Mutex::new(VecDeque::from([Ok(Some(supplement()))])),
                    cancel_after_fetch: None,
                },
                FakeProviders::default(),
                FakeRepository::default(),
                settings("youdao"),
                FakeOcr {
                    result: Mutex::new(VecDeque::new()),
                },
            );
            let mut stages = Vec::new();

            let record = workflow
                .translate_text("hello", &mut |stage| stages.push(stage), &|| false)
                .await
                .unwrap();

            assert_eq!(record.access_count, 1);
            assert_eq!(*workflow.repository.saves.lock(), 1);
            assert_eq!(*workflow.repository.updates.lock(), 1);
            assert!(matches!(stages[1], WorkflowStage::LocalResultAvailable(_)));
            assert!(matches!(stages[2], WorkflowStage::EnrichmentAvailable(_)));
            assert!(matches!(stages[3], WorkflowStage::Completed(_)));
        });
    }

    #[test]
    fn equal_local_and_completed_results_do_not_update_persistence() {
        tauri::async_runtime::block_on(async {
            let workflow = TranslationWorkflow::new(
                FakeDictionary {
                    local: Some(local_entry()),
                    supplements: Mutex::new(VecDeque::from([Ok(None)])),
                    cancel_after_fetch: None,
                },
                FakeProviders::default(),
                FakeRepository::default(),
                settings("youdao"),
                FakeOcr {
                    result: Mutex::new(VecDeque::new()),
                },
            );

            workflow
                .translate_text("hello", &mut |_| {}, &|| false)
                .await
                .unwrap();

            assert_eq!(*workflow.repository.saves.lock(), 1);
            assert_eq!(*workflow.repository.updates.lock(), 0);
        });
    }

    #[test]
    fn workflow_reads_provider_settings_for_each_request() {
        tauri::async_runtime::block_on(async {
            let workflow = TranslationWorkflow::new(
                FakeDictionary {
                    local: None,
                    supplements: Mutex::new(VecDeque::from([Ok(None), Ok(None)])),
                    cancel_after_fetch: None,
                },
                FakeProviders {
                    youdao: Mutex::new(VecDeque::from([Ok(content("有道"))])),
                    microsoft: Mutex::new(VecDeque::from([Ok(content("微软"))])),
                    ..FakeProviders::default()
                },
                FakeRepository::default(),
                settings("youdao"),
                FakeOcr {
                    result: Mutex::new(VecDeque::new()),
                },
            );

            workflow
                .translate_text("hello", &mut |_| {}, &|| false)
                .await
                .unwrap();
            workflow.settings.translation.write().provider = "microsoft".to_string();
            workflow
                .translate_text("world", &mut |_| {}, &|| false)
                .await
                .unwrap();

            assert_eq!(
                workflow.providers.calls.lock().as_slice(),
                ["youdao", "microsoft"]
            );
        });
    }

    #[test]
    fn stale_request_cannot_publish_enrichment_or_completion() {
        tauri::async_runtime::block_on(async {
            let cancelled = Arc::new(AtomicBool::new(false));
            let workflow = TranslationWorkflow::new(
                FakeDictionary {
                    local: Some(local_entry()),
                    supplements: Mutex::new(VecDeque::from([Ok(Some(supplement()))])),
                    cancel_after_fetch: Some(cancelled.clone()),
                },
                FakeProviders::default(),
                FakeRepository::default(),
                settings("youdao"),
                FakeOcr {
                    result: Mutex::new(VecDeque::new()),
                },
            );
            let mut stages = Vec::new();

            let error = workflow
                .translate_text("hello", &mut |stage| stages.push(stage), &|| {
                    cancelled.load(Ordering::SeqCst)
                })
                .await
                .unwrap_err();

            assert_eq!(error, "翻译请求已取消");
            assert!(matches!(stages[1], WorkflowStage::LocalResultAvailable(_)));
            assert!(matches!(stages.last(), Some(WorkflowStage::Cancelled)));
            assert_eq!(*workflow.repository.updates.lock(), 0);
        });
    }

    #[test]
    fn image_translation_reports_ocr_backfill_and_persists_result() {
        tauri::async_runtime::block_on(async {
            let workflow = TranslationWorkflow::new(
                FakeDictionary {
                    local: None,
                    supplements: Mutex::new(VecDeque::from([Ok(None)])),
                    cancel_after_fetch: None,
                },
                FakeProviders {
                    youdao: Mutex::new(VecDeque::from([Ok(content("截图翻译"))])),
                    ..FakeProviders::default()
                },
                FakeRepository::default(),
                settings("youdao"),
                FakeOcr {
                    result: Mutex::new(VecDeque::from([Ok("  screenshot text  ".to_string())])),
                },
            );
            let mut stages = Vec::new();

            let record = workflow
                .translate_image("image", &mut |stage| stages.push(stage), &|| false)
                .await
                .unwrap();

            assert_eq!(record.source_text, "screenshot text");
            assert!(matches!(stages[0], WorkflowStage::OcrInProgress));
            assert_eq!(
                stages[1],
                WorkflowStage::InputAccepted {
                    text: "screenshot text".to_string()
                }
            );
        });
    }

    #[test]
    fn provider_failure_keeps_ocr_backfill_stage() {
        tauri::async_runtime::block_on(async {
            let workflow = TranslationWorkflow::new(
                FakeDictionary {
                    local: None,
                    supplements: Mutex::new(VecDeque::from([Ok(None)])),
                    cancel_after_fetch: None,
                },
                FakeProviders {
                    youdao: Mutex::new(VecDeque::from([Err("provider unavailable".to_string())])),
                    ..FakeProviders::default()
                },
                FakeRepository::default(),
                settings("youdao"),
                FakeOcr {
                    result: Mutex::new(VecDeque::from([Ok("  recognized text  ".to_string())])),
                },
            );
            let mut stages = Vec::new();

            let error = workflow
                .translate_image("image", &mut |stage| stages.push(stage), &|| false)
                .await
                .unwrap_err();

            assert_eq!(error, "provider unavailable");
            assert_eq!(
                stages[1],
                WorkflowStage::InputAccepted {
                    text: "recognized text".to_string()
                }
            );
            assert!(matches!(
                stages.last(),
                Some(WorkflowStage::Failed { message }) if message == "provider unavailable"
            ));
        });
    }

    #[test]
    fn empty_ocr_output_fails_before_dictionary_or_provider_work() {
        tauri::async_runtime::block_on(async {
            let workflow = TranslationWorkflow::new(
                FakeDictionary {
                    local: None,
                    supplements: Mutex::new(VecDeque::from([Ok(None)])),
                    cancel_after_fetch: None,
                },
                FakeProviders::default(),
                FakeRepository::default(),
                settings("youdao"),
                FakeOcr {
                    result: Mutex::new(VecDeque::from([Ok("   ".to_string())])),
                },
            );
            let mut stages = Vec::new();

            let error = workflow
                .translate_image("image", &mut |stage| stages.push(stage), &|| false)
                .await
                .unwrap_err();

            assert_eq!(error, "OCR 未识别到文本");
            assert_eq!(*workflow.repository.saves.lock(), 0);
            assert_eq!(workflow.dictionary.supplements.lock().len(), 1);
            assert!(workflow.providers.calls.lock().is_empty());
            assert!(matches!(
                stages.last(),
                Some(WorkflowStage::Failed { message }) if message == "OCR 未识别到文本"
            ));
        });
    }
}
