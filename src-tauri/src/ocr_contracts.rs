use serde::{Deserialize, Serialize};

pub const OCR_LANG: &str = "ch";
pub const OCR_DEVICE: &str = "cpu";
pub const PADDLE_OCR_VERSION: &str = "3.7.0";
pub const PPOCR_VERSION: &str = "PP-OCRv6";
pub const SIDECAR_ONNXRUNTIME_VERSION: &str = "1.27.0";
pub const RAPID_OCR_VERSION: &str = "1.4.4";

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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OcrServiceStatus {
    pub running: bool,
    pub endpoint: String,
    pub message: String,
    pub last_error: Option<String>,
    pub engine: String,
    pub model_profile: String,
    pub model_dir: Option<String>,
    pub sidecar_path: Option<String>,
    pub log_path: Option<String>,
    pub preload_on_startup: bool,
    pub rapidocr_version: &'static str,
    pub paddleocr_version: &'static str,
    pub ppocr_version: &'static str,
    pub onnxruntime_version: &'static str,
    pub lang: &'static str,
    pub device: &'static str,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OcrHealthStatus {
    pub ok: bool,
    pub engine: Option<String>,
    pub lang: Option<String>,
    pub device: Option<String>,
    pub model_profile: Option<String>,
    pub model_dir: Option<String>,
    pub model_source: Option<String>,
}

pub fn is_native_engine(engine: &str) -> bool {
    normalize_engine(engine) == "native_onnx"
}

/// Product runtime only supports in-process ONNX OCR.
/// Legacy paddle/rapid values are migrated to `native_onnx`.
pub fn normalize_engine(engine: &str) -> &'static str {
    match engine.trim().to_ascii_lowercase().as_str() {
        "" | "native" | "native_onnx" | "onnx" | "onnxruntime" | "ppocr-rs" | "paddle"
        | "paddleocr" | "rapid" | "rapidocr" | "rapidocr_onnxruntime" => "native_onnx",
        _ => "unknown",
    }
}

pub fn engine_label(engine: &str) -> &'static str {
    match normalize_engine(engine) {
        "native_onnx" => "原生 ONNX OCR",
        _ => "未知 OCR",
    }
}

pub fn normalize_model_profile(_profile: &str) -> String {
    "small".to_string()
}
