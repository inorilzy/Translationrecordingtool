use std::path::PathBuf;

use tauri::AppHandle;
use tracing::{info, warn};

use crate::{
    native_ocr,
    ocr_contracts::{OcrRuntimeConfig, OcrServiceStatus},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrAdapterKind {
    Native,
}

pub fn adapter_kind(engine: &str) -> OcrAdapterKind {
    // Product runtime is native ONNX only. Legacy engine names are normalized
    // upstream; unknown values still fail closed through native status/init.
    let _ = engine;
    OcrAdapterKind::Native
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
) -> Result<crate::ocr_contracts::OcrRecognition, String> {
    let _ = adapter_kind(&config.engine);
    let app = app.clone();
    let config = config.clone();
    let image_base64 = image_base64.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        native_ocr::recognize_text(&app, &config, &image_base64)
    })
    .await
    .map_err(|error| format!("原生 OCR 任务失败: {}", error))?
}

pub async fn ensure_running(app: &AppHandle, config: &OcrRuntimeConfig) -> Result<String, String> {
    let _ = adapter_kind(&config.engine);
    let app = app.clone();
    let config = config.clone();
    tauri::async_runtime::spawn_blocking(move || native_ocr::ensure_initialized(&app, &config))
        .await
        .map_err(|error| format!("原生 OCR 初始化任务失败: {}", error))?
}

pub async fn warmup(app: &AppHandle, config: &OcrRuntimeConfig) -> Result<String, String> {
    ensure_running(app, config).await
}

pub async fn restart(app: &AppHandle, config: &OcrRuntimeConfig) -> Result<String, String> {
    ensure_running(app, config).await
}

pub async fn status(app: &AppHandle, config: &OcrRuntimeConfig) -> OcrServiceStatus {
    native_status(app, config)
}

pub fn log_path(_app: &AppHandle, _config: &OcrRuntimeConfig) -> Result<PathBuf, String> {
    Err("原生 ONNX OCR 不使用 sidecar 日志文件".to_string())
}

fn native_status(app: &AppHandle, config: &OcrRuntimeConfig) -> OcrServiceStatus {
    let (model_profile, model_dir) = native_ocr::model_status(app, &config.model_profile);
    let mut status = OcrServiceStatus::from_snapshot(
        &native_ocr::engine_snapshot(app, &config.model_profile),
        model_profile,
        model_dir.as_ref().map(|path| path.display().to_string()),
    );
    status.engine = native_ocr::engine_name().to_string();
    status.preload_on_startup = config.preload_on_startup;
    status.onnxruntime_version = native_ocr::onnx_runtime_version().to_string();
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_selects_native_adapter() {
        for engine in [
            "native",
            "native_onnx",
            "onnx",
            "paddleocr",
            "rapidocr",
            "unknown",
        ] {
            assert_eq!(adapter_kind(engine), OcrAdapterKind::Native);
        }
    }
}
