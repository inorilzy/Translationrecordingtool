use std::path::PathBuf;

use tauri::AppHandle;
use tracing::{info, warn};

use crate::{
    native_ocr,
    ocr_contracts::{
        is_native_engine, OcrRuntimeConfig, OcrServiceStatus, OCR_DEVICE, OCR_LANG, PPOCR_VERSION,
        RAPID_OCR_VERSION,
    },
    ocr_service,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrAdapterKind {
    Native,
    CompatibilitySidecar,
}

pub fn adapter_kind(engine: &str) -> OcrAdapterKind {
    if is_native_engine(engine) {
        OcrAdapterKind::Native
    } else {
        OcrAdapterKind::CompatibilitySidecar
    }
}

pub fn spawn_startup_check(app: AppHandle, config: OcrRuntimeConfig) {
    tauri::async_runtime::spawn(async move {
        if !config.preload_on_startup {
            info!("OCR 启动预热已关闭");
            return;
        }

        if let Err(error) = ensure_running(&app, &config).await {
            warn!("自动启动 OCR 服务失败: {}", error);
            return;
        }

        if let Err(error) = warmup(&app, &config).await {
            warn!("自动预热 OCR 服务失败: {}", error);
        }
    });
}

pub async fn recognize_text_with_config(
    app: &AppHandle,
    config: &OcrRuntimeConfig,
    image_base64: &str,
) -> Result<String, String> {
    match adapter_kind(&config.engine) {
        OcrAdapterKind::Native => {
            let app = app.clone();
            let config = config.clone();
            let image_base64 = image_base64.to_string();
            tauri::async_runtime::spawn_blocking(move || {
                native_ocr::recognize_text(&app, &config, &image_base64)
            })
            .await
            .map_err(|error| format!("原生 OCR 任务失败: {}", error))?
        }
        OcrAdapterKind::CompatibilitySidecar => {
            ocr_service::ensure_running(app, config).await?;
            ocr_service::recognize_text(&config.endpoint, image_base64).await
        }
    }
}

pub async fn ensure_running(app: &AppHandle, config: &OcrRuntimeConfig) -> Result<String, String> {
    match adapter_kind(&config.engine) {
        OcrAdapterKind::Native => {
            let app = app.clone();
            let config = config.clone();
            tauri::async_runtime::spawn_blocking(move || {
                native_ocr::ensure_initialized(&app, &config)
            })
            .await
            .map_err(|error| format!("原生 OCR 初始化任务失败: {}", error))?
        }
        OcrAdapterKind::CompatibilitySidecar => ocr_service::ensure_running(app, config).await,
    }
}

pub async fn warmup(app: &AppHandle, config: &OcrRuntimeConfig) -> Result<String, String> {
    match adapter_kind(&config.engine) {
        OcrAdapterKind::Native => ensure_running(app, config).await,
        OcrAdapterKind::CompatibilitySidecar => ocr_service::warmup(app, config).await,
    }
}

pub async fn restart(app: &AppHandle, config: &OcrRuntimeConfig) -> Result<String, String> {
    match adapter_kind(&config.engine) {
        OcrAdapterKind::Native => {
            ocr_service::stop_process(app)?;
            ensure_running(app, config).await
        }
        OcrAdapterKind::CompatibilitySidecar => ocr_service::restart(app, config).await,
    }
}

pub async fn status(app: &AppHandle, config: &OcrRuntimeConfig) -> OcrServiceStatus {
    match adapter_kind(&config.engine) {
        OcrAdapterKind::Native => native_status(app, config),
        OcrAdapterKind::CompatibilitySidecar => ocr_service::status(app, config).await,
    }
}

pub fn log_path(app: &AppHandle, config: &OcrRuntimeConfig) -> Result<PathBuf, String> {
    match adapter_kind(&config.engine) {
        OcrAdapterKind::Native => Err("原生 ONNX OCR 不使用 sidecar 日志文件".to_string()),
        OcrAdapterKind::CompatibilitySidecar => ocr_service::log_path(app, &config.engine),
    }
}

fn native_status(app: &AppHandle, config: &OcrRuntimeConfig) -> OcrServiceStatus {
    let (model_profile, model_dir) = native_ocr::model_status(app, &config.model_profile);
    let running = model_dir.is_some();
    OcrServiceStatus {
        running,
        endpoint: "in-process".to_string(),
        message: if running {
            "原生 ONNX OCR 可用，无需 Python sidecar".to_string()
        } else {
            "未找到 PP-OCRv6 ONNX 模型目录".to_string()
        },
        last_error: if running {
            None
        } else {
            Some("请先下载 PP-OCRv6 ONNX 模型".to_string())
        },
        engine: native_ocr::engine_name().to_string(),
        model_profile,
        model_dir: model_dir.as_ref().map(|path| path.display().to_string()),
        sidecar_path: None,
        log_path: None,
        preload_on_startup: config.preload_on_startup,
        rapidocr_version: RAPID_OCR_VERSION,
        paddleocr_version: "-",
        ppocr_version: PPOCR_VERSION,
        onnxruntime_version: native_ocr::onnx_runtime_version(),
        lang: OCR_LANG,
        device: OCR_DEVICE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_native_adapter_for_supported_native_aliases() {
        for engine in ["native", "native_onnx", "onnx", "onnxruntime", "ppocr-rs"] {
            assert_eq!(adapter_kind(engine), OcrAdapterKind::Native);
        }
    }

    #[test]
    fn selects_compatibility_sidecar_for_paddle_and_rapid() {
        for engine in ["paddleocr", "rapidocr"] {
            assert_eq!(adapter_kind(engine), OcrAdapterKind::CompatibilitySidecar);
        }
    }
}
