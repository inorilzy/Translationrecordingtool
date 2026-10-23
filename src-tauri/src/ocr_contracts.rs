use serde::{Deserialize, Serialize};

pub const OCR_LANG: &str = "ch";
pub const OCR_DEVICE: &str = "cpu";
pub const PPOCR_VERSION: &str = "PP-OCRv6";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrRuntimeConfig {
    pub endpoint: String,
    pub engine: String,
    pub model_profile: String,
    pub preload_on_startup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrPoint {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrTextBlock {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OcrRecognition {
    pub text: String,
    pub image_width: u32,
    pub image_height: u32,
    pub blocks: Vec<OcrTextBlock>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OcrRuntimeState {
    ModelsUnavailable,
    NotInitialized,
    Initializing,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OcrServiceStatus {
    /// True only when an engine for the requested profile is initialized.
    pub initialized: bool,
    pub state: OcrRuntimeState,
    pub message: String,
    pub last_error: Option<String>,
    pub initialized_profile: Option<String>,
    pub engine: String,
    pub model_profile: String,
    pub model_dir: Option<String>,
    pub preload_on_startup: bool,
    pub ppocr_version: String,
    pub onnxruntime_version: String,
    pub lang: String,
    pub device: String,
}

/// Facts about the engine lifecycle, independent of how files are located.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrEngineSnapshot {
    pub models_available: bool,
    pub initialized_profile: Option<String>,
    /// Failure attributed to the queried profile only.
    pub last_error: Option<String>,
    pub initializing: bool,
    /// Profile the recorded failure belongs to, even if it is not the queried
    /// one (a failed switch can leave an older engine running).
    pub last_failure_profile: Option<String>,
}

impl OcrServiceStatus {
    pub fn from_snapshot(
        snapshot: &OcrEngineSnapshot,
        model_profile: String,
        model_dir: Option<String>,
    ) -> Self {
        let initialized = snapshot.initialized_profile.as_deref() == Some(model_profile.as_str());
        let (state, message) = if !snapshot.models_available {
            (
                OcrRuntimeState::ModelsUnavailable,
                "未找到 PP-OCRv6 ONNX 模型目录".to_string(),
            )
        } else if initialized {
            (OcrRuntimeState::Ready, "原生 ONNX OCR 已就绪".to_string())
        } else if let Some(error) = &snapshot.last_error {
            (
                OcrRuntimeState::Failed,
                format!("原生 ONNX OCR 初始化失败: {}", error),
            )
        } else if snapshot.initializing {
            (
                OcrRuntimeState::Initializing,
                "原生 ONNX OCR 正在初始化".to_string(),
            )
        } else {
            (
                OcrRuntimeState::NotInitialized,
                "原生 ONNX OCR 尚未初始化".to_string(),
            )
        };
        let last_error = match state {
            OcrRuntimeState::ModelsUnavailable => Some("请先下载 PP-OCRv6 ONNX 模型".to_string()),
            OcrRuntimeState::Failed => snapshot.last_error.clone(),
            OcrRuntimeState::Initializing => None,
            OcrRuntimeState::NotInitialized => {
                Some("OCR 引擎将在首次截图识别或手动预热时初始化".to_string())
            }
            OcrRuntimeState::Ready => None,
        };
        Self {
            initialized,
            state,
            message,
            last_error,
            initialized_profile: snapshot.initialized_profile.clone(),
            engine: String::new(),
            model_profile,
            model_dir,
            preload_on_startup: false,
            ppocr_version: PPOCR_VERSION.to_string(),
            onnxruntime_version: String::new(),
            lang: OCR_LANG.to_string(),
            device: OCR_DEVICE.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(models: bool, initialized: Option<&str>, error: Option<&str>) -> OcrEngineSnapshot {
        OcrEngineSnapshot {
            models_available: models,
            initialized_profile: initialized.map(str::to_string),
            last_error: error.map(str::to_string),
            initializing: false,
            last_failure_profile: error.map(|_| "small".to_string()),
        }
    }

    #[test]
    fn existing_model_files_alone_are_not_reported_as_running() {
        let status = OcrServiceStatus::from_snapshot(
            &snapshot(true, None, None),
            "small".to_string(),
            Some("C:/models/small".to_string()),
        );
        assert!(!status.initialized);
        assert_eq!(status.state, OcrRuntimeState::NotInitialized);
        assert!(status.last_error.is_some());
    }

    #[test]
    fn initialized_engine_reports_ready_only_for_matching_profile() {
        let ready = OcrServiceStatus::from_snapshot(
            &snapshot(true, Some("small"), None),
            "small".to_string(),
            None,
        );
        assert!(ready.initialized);
        assert_eq!(ready.state, OcrRuntimeState::Ready);
        assert_eq!(ready.last_error, None);

        // "medium" 在此仅作为“与已初始化 profile 不同的查询键”的纯生命周期测试数据，
        // 不代表产品支持 medium（产品 profile 由 normalize_model_profile 归一为 small）。
        let other = OcrServiceStatus::from_snapshot(
            &snapshot(true, Some("small"), None),
            "medium".to_string(),
            None,
        );
        assert!(!other.initialized);
        assert_eq!(other.state, OcrRuntimeState::NotInitialized);
        assert_eq!(other.initialized_profile.as_deref(), Some("small"));
    }

    #[test]
    fn init_failure_surfaces_last_error_even_with_models_present() {
        let status = OcrServiceStatus::from_snapshot(
            &snapshot(true, None, Some("init_models_no_angle failed")),
            "small".to_string(),
            Some("C:/models/small".to_string()),
        );
        assert!(!status.initialized);
        assert_eq!(status.state, OcrRuntimeState::Failed);
        assert_eq!(
            status.last_error.as_deref(),
            Some("init_models_no_angle failed")
        );
        assert!(status.message.contains("初始化失败"));
    }

    #[test]
    fn initializing_is_reported_distinctly_from_not_initialized() {
        let status = OcrServiceStatus::from_snapshot(
            &OcrEngineSnapshot {
                models_available: true,
                initialized_profile: None,
                last_error: None,
                initializing: true,
                last_failure_profile: None,
            },
            "small".to_string(),
            None,
        );
        assert_eq!(status.state, OcrRuntimeState::Initializing);
        assert!(!status.initialized);
        assert_eq!(status.last_error, None);
    }

    #[test]
    fn failure_for_another_profile_is_not_attributed_to_the_query() {
        let status = OcrServiceStatus::from_snapshot(
            &OcrEngineSnapshot {
                models_available: true,
                initialized_profile: Some("small".to_string()),
                last_error: None,
                initializing: false,
                last_failure_profile: Some("medium".to_string()),
            },
            "small".to_string(),
            None,
        );
        // The healthy small engine stays Ready despite a failed medium switch.
        // （"medium" 仅为纯生命周期测试中的另一 profile 标签，不扩大产品支持。）
        assert_eq!(status.state, OcrRuntimeState::Ready);
        assert_eq!(status.initialized_profile.as_deref(), Some("small"));
    }

    #[test]
    fn missing_models_are_distinct_from_failed_or_uninitialized_engine() {
        let status = OcrServiceStatus::from_snapshot(
            &snapshot(false, None, Some("stale old error")),
            "small".to_string(),
            None,
        );
        assert!(!status.initialized);
        assert_eq!(status.state, OcrRuntimeState::ModelsUnavailable);
        assert_eq!(
            status.last_error.as_deref(),
            Some("请先下载 PP-OCRv6 ONNX 模型")
        );
    }

    /// 工单 21：所有旧 profile 值都迁移到唯一正式 profile small。
    #[test]
    fn every_legacy_or_unknown_profile_normalizes_to_small() {
        let cases = [
            "", "   ", "small", " small ", "SMALL", "tiny", "lite", "medium", "accurate",
            "Medium", "large", "server", "unknown-profile",
        ];
        for raw in cases {
            assert_eq!(
                normalize_model_profile(raw),
                SUPPORTED_OCR_MODEL_PROFILE,
                "profile {raw:?} 应归一为 small"
            );
        }
        assert_eq!(SUPPORTED_OCR_MODEL_PROFILE, "small");
    }
}

pub fn is_native_engine(engine: &str) -> bool {
    normalize_engine(engine) == "native_onnx"
}

/// Product runtime only supports in-process ONNX OCR.
/// Legacy paddle/rapid values are migrated to `native_onnx`.
pub fn normalize_engine(engine: &str) -> &'static str {
    match engine.trim().to_ascii_lowercase().as_str() {
        ""
        | "native"
        | "native_onnx"
        | "onnx"
        | "onnxruntime"
        | "ppocr-rs"
        | "paddle"
        | "paddleocr"
        | "rapid"
        | "rapidocr"
        | "rapidocr_onnxruntime" => "native_onnx",
        _ => "unknown",
    }
}

pub fn engine_label(engine: &str) -> &'static str {
    match normalize_engine(engine) {
        "native_onnx" => "原生 ONNX OCR",
        _ => "未知 OCR",
    }
}

/// 产品唯一正式支持的 OCR 模型 profile（工单 21 决策）。
pub const SUPPORTED_OCR_MODEL_PROFILE: &str = "small";

/// OCR profile 的唯一归一入口：settings 加载、设置更新与 native_ocr 运行时都经由此函数。
/// 旧值（空值、`tiny`/`lite`、`medium`/`accurate`、大小写/空白变体及任何未知值）
/// 一律迁移为 `small`；不保留也不新增其他 profile 的模型支持。
pub fn normalize_model_profile(_profile: &str) -> String {
    SUPPORTED_OCR_MODEL_PROFILE.to_string()
}
