use std::{
    future::Future,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use parking_lot::RwLock;
use tauri::AppHandle;

use crate::{
    database::{open_translations_connection, save_translation_in_connection, TranslationRecord},
    local_dictionary::{self, FreeDictionarySupplement, OfflineDictionaryEntry},
    ocr_contracts::{OcrRecognition, OcrRuntimeConfig, OcrTextBlock},
    settings::SettingsRecord,
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
    ) -> impl Future<Output = Result<OcrRecognition, String>> + Send + 'a;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OverlayTextBlock {
    pub source_text: String,
    pub translated_text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImageOverlayTranslation {
    pub image_base64: String,
    pub image_width: u32,
    pub image_height: u32,
    pub blocks: Vec<OverlayTextBlock>,
    pub record: TranslationRecord,
}

pub fn align_overlay_blocks(blocks: &[OcrTextBlock], translated_text: &str) -> Vec<OverlayTextBlock> {
    if blocks.is_empty() {
        return Vec::new();
    }

    let lines = translated_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    // Per-line mapping is only trustworthy when the provider kept line breaks.
    if lines.len() == blocks.len() {
        return blocks
            .iter()
            .zip(lines.into_iter())
            .map(|(block, translated)| OverlayTextBlock {
                source_text: block.text.clone(),
                translated_text: translated,
                x: block.x,
                y: block.y,
                width: block.width,
                height: block.height,
            })
            .collect();
    }

    // Otherwise cover the union of all text boxes with the whole translation,
    // instead of mis-assigning lines and leaking untranslated originals.
    let min_x = blocks.iter().map(|b| b.x).fold(f64::INFINITY, f64::min);
    let min_y = blocks.iter().map(|b| b.y).fold(f64::INFINITY, f64::min);
    let max_x = blocks
        .iter()
        .map(|b| b.x + b.width)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = blocks
        .iter()
        .map(|b| b.y + b.height)
        .fold(f64::NEG_INFINITY, f64::max);
    let source_text = blocks
        .iter()
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    vec![OverlayTextBlock {
        source_text,
        translated_text: translated_text.trim().to_string(),
        x: min_x,
        y: min_y,
        width: (max_x - min_x).max(1.0),
        height: (max_y - min_y).max(1.0),
    }]
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

        let overlay = self
            .translate_image_overlay(image_base64, report, is_cancelled)
            .await?;
        Ok(overlay.record)
    }

    pub async fn translate_image_overlay<F, C>(
        &self,
        image_base64: &str,
        report: &mut F,
        is_cancelled: &C,
    ) -> Result<ImageOverlayTranslation, String>
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
            Ok(result) => result,
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

        let recognized_text = recognized.text.trim();
        if recognized_text.is_empty() || recognized.blocks.is_empty() {
            let message = "OCR 未识别到文本".to_string();
            report(WorkflowStage::Failed {
                message: message.clone(),
            });
            return Err(message);
        }

        let record = self
            .translate_text_with_config(
                recognized_text,
                &settings.translation,
                report,
                is_cancelled,
            )
            .await?;

        Ok(ImageOverlayTranslation {
            image_base64: image_base64.to_string(),
            image_width: recognized.image_width,
            image_height: recognized.image_height,
            blocks: align_overlay_blocks(&recognized.blocks, &record.translated_text),
            record,
        })
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
    fn translate<'a>(
        &'a self,
        text: &'a str,
        config: &'a TranslationConfig,
    ) -> impl Future<Output = Result<TranslationContent, String>> + Send + 'a {
        async move {
            match config.provider.trim().to_ascii_lowercase().as_str() {
                "microsoft" => {
                    if config.microsoft_key.trim().is_empty() {
                        return Err("使用微软翻译需要配置 Microsoft Translator Key".to_string());
                    }
                    translator::translate_with_microsoft(
                        text,
                        &config.microsoft_key,
                        &config.microsoft_region,
                    )
                    .await
                }
                "google" => {
                    if config.google_api_key.trim().is_empty() {
                        return Err(
                            "使用 Google 翻译需要配置 API Key，请在设置中配置".to_string(),
                        );
                    }
                    translator::translate_with_google(text, &config.google_api_key).await
                }
                _ => {
                    if config.youdao_app_key.trim().is_empty()
                        || config.youdao_app_secret.trim().is_empty()
                    {
                        return Err("翻译句子需要配置有道翻译 API，请在设置中配置".to_string());
                    }
                    translator::translate_text(
                        text,
                        &config.youdao_app_key,
                        &config.youdao_app_secret,
                    )
                    .await
                }
            }
        }
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
    settings: Arc<RwLock<SettingsRecord>>,
}

impl RuntimeSettingsSource for ManagedRuntimeSettings {
    fn translation_config(&self) -> TranslationConfig {
        self.settings.read().translation_config()
    }

    fn screenshot_settings(&self) -> ScreenshotRequestSettings {
        let settings = self.settings.read();
        ScreenshotRequestSettings {
            translation: settings.translation_config(),
            ocr: settings.ocr_runtime_config(),
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
    ) -> impl Future<Output = Result<OcrRecognition, String>> + Send + 'a {
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
    settings: Arc<RwLock<SettingsRecord>>,
) -> AppTranslationWorkflow {
    TranslationWorkflow::new(
        AppDictionaryGateway { app: app.clone() },
        AppProviderGateway,
        AppTranslationRepository { app: app.clone() },
        ManagedRuntimeSettings { settings },
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
        fn translate<'a>(
            &'a self,
            _text: &'a str,
            config: &'a TranslationConfig,
        ) -> impl Future<Output = Result<TranslationContent, String>> + Send + 'a {
            let provider = config.provider.trim().to_ascii_lowercase();
            let result = match provider.as_str() {
                "microsoft" => {
                    if config.microsoft_key.trim().is_empty() {
                        Err("使用微软翻译需要配置 Microsoft Translator Key".to_string())
                    } else {
                        self.calls.lock().push("microsoft".to_string());
                        self.microsoft
                            .lock()
                            .pop_front()
                            .unwrap_or_else(|| Err("unexpected microsoft call".to_string()))
                    }
                }
                "google" => {
                    if config.google_api_key.trim().is_empty() {
                        Err("使用 Google 翻译需要配置 API Key，请在设置中配置".to_string())
                    } else {
                        self.calls.lock().push("google".to_string());
                        self.google
                            .lock()
                            .pop_front()
                            .unwrap_or_else(|| Err("unexpected google call".to_string()))
                    }
                }
                _ => {
                    if config.youdao_app_key.trim().is_empty()
                        || config.youdao_app_secret.trim().is_empty()
                    {
                        Err("翻译句子需要配置有道翻译 API，请在设置中配置".to_string())
                    } else {
                        self.calls.lock().push("youdao".to_string());
                        self.youdao
                            .lock()
                            .pop_front()
                            .unwrap_or_else(|| Err("unexpected youdao call".to_string()))
                    }
                }
            };
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
        result: Mutex<VecDeque<Result<OcrRecognition, String>>>,
    }

    fn fake_ocr_text(text: &str) -> OcrRecognition {
        let blocks = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .enumerate()
            .map(|(index, line)| OcrTextBlock {
                text: line.trim().to_string(),
                x: 10.0,
                y: (index as f64) * 24.0 + 10.0,
                width: 120.0,
                height: 20.0,
            })
            .collect::<Vec<_>>();
        OcrRecognition {
            text: text.to_string(),
            image_width: 200,
            image_height: 200,
            blocks,
        }
    }

    impl OcrGateway for FakeOcr {
        fn recognize<'a>(
            &'a self,
            _config: &'a OcrRuntimeConfig,
            _image_base64: &'a str,
        ) -> impl Future<Output = Result<OcrRecognition, String>> + Send + 'a {
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
        ) -> impl Future<Output = Result<OcrRecognition, String>> + Send + 'a {
            self.translation.write().provider = "microsoft".to_string();
            let result = fake_ocr_text(&self.result);
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
            {
                let mut translation = workflow.settings.translation.write();
                translation.provider = "microsoft".to_string();
                translation.microsoft_key = "key".to_string();
            }
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
                FakeOcr { result: Mutex::new(VecDeque::from([Ok(fake_ocr_text("  screenshot text  "))])), },
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
                FakeOcr { result: Mutex::new(VecDeque::from([Ok(fake_ocr_text("  recognized text  "))])), },
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
                FakeOcr { result: Mutex::new(VecDeque::from([Ok(fake_ocr_text("   "))])), },
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
    #[test]
    fn align_overlay_blocks_merges_on_mismatch() {
        let blocks = vec![
            OcrTextBlock { text: "hello".into(), x: 10.0, y: 10.0, width: 40.0, height: 12.0 },
            OcrTextBlock { text: "world".into(), x: 10.0, y: 30.0, width: 60.0, height: 12.0 },
        ];
        let overlays = align_overlay_blocks(&blocks, "你好世界");
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].translated_text, "你好世界");
        assert_eq!(overlays[0].x, 10.0);
        assert_eq!(overlays[0].y, 10.0);
        assert_eq!(overlays[0].width, 60.0);
        assert_eq!(overlays[0].height, 32.0);
    }


    #[test]
    fn align_overlay_blocks_matches_line_count() {
        let blocks = vec![
            OcrTextBlock { text: "hello".into(), x: 1.0, y: 2.0, width: 30.0, height: 10.0 },
            OcrTextBlock { text: "world".into(), x: 1.0, y: 20.0, width: 30.0, height: 10.0 },
        ];
        let overlays = align_overlay_blocks(&blocks, "你好\n世界");
        assert_eq!(overlays.len(), 2);
        assert_eq!(overlays[0].translated_text, "你好");
        assert_eq!(overlays[1].translated_text, "世界");
    }
}
