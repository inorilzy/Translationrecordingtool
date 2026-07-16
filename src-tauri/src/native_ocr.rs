use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    time::Instant,
};

use base64::{engine::general_purpose, Engine as _};
use ppocr_rs::{OcrLite, OcrOptions};
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

use crate::ocr_contracts::OcrRuntimeConfig;

const OCR_MODEL_RESOURCE_DIR: &str = "ocr-models";
const DEFAULT_PACKAGED_MODEL_PROFILE: &str = "small";
const NATIVE_ONNX_VERSION: &str = "1.20.1";

struct NativeOcrRuntime {
    profile: String,
    model_dir: PathBuf,
    engine: Mutex<OcrLite>,
}

static RUNTIME: LazyLock<Mutex<Option<NativeOcrRuntime>>> = LazyLock::new(|| Mutex::new(None));

pub fn engine_name() -> &'static str {
    "native_onnx"
}

pub fn onnx_runtime_version() -> &'static str {
    NATIVE_ONNX_VERSION
}

pub fn recognize_text(
    app: &AppHandle,
    config: &OcrRuntimeConfig,
    image_base64: &str,
) -> Result<crate::ocr_contracts::OcrRecognition, String> {
    ensure_initialized(app, config)?;
    let image = decode_image(image_base64)?;
    let image_width = image.width();
    let image_height = image.height();

    let mut guard = RUNTIME
        .lock()
        .map_err(|_| "原生 OCR 状态锁定失败".to_string())?;
    let runtime = guard
        .as_mut()
        .ok_or_else(|| "原生 OCR 尚未初始化".to_string())?;

    let started = Instant::now();
    let result = runtime
        .engine
        .lock()
        .map_err(|_| "原生 OCR 推理锁定失败".to_string())?
        .detect_with_options(
            &image,
            10,
            960,
            0.6,
            0.3,
            1.6,
            false,
            false,
            OcrOptions {
                use_doc_orientation: false,
                ..OcrOptions::default()
            },
        )
        .map_err(|error| format!("原生 OCR 识别失败: {}", error))?;

    info!(
        "原生 OCR 识别完成: {} 行，用时 {}ms",
        result.text_blocks.len(),
        started.elapsed().as_millis()
    );

    let mut blocks = result
        .text_blocks
        .into_iter()
        .filter(|block| !block.text.trim().is_empty())
        .map(|block| {
            let (x, y, width, height) = bounding_box(&block.box_points);
            crate::ocr_contracts::OcrTextBlock {
                text: block.text.trim().to_string(),
                x,
                y,
                width,
                height,
            }
        })
        .collect::<Vec<_>>();

    blocks.sort_by(|left, right| {
        left.y
            .partial_cmp(&right.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                left.x
                    .partial_cmp(&right.x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let text = blocks
        .iter()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    if text.trim().is_empty() {
        Err("OCR 未识别到文本".to_string())
    } else {
        Ok(crate::ocr_contracts::OcrRecognition {
            text,
            image_width,
            image_height,
            blocks,
        })
    }
}

pub fn ensure_initialized(app: &AppHandle, config: &OcrRuntimeConfig) -> Result<String, String> {
    let (profile, model_dir) =
        packaged_model_config(app, &config.model_profile).ok_or_else(|| {
            format!(
            "未找到原生 OCR profile={} 的本地模型。请先运行 npm run ocr:models:win -- -Profile {}",
            normalize_model_profile(&config.model_profile),
            normalize_model_profile(&config.model_profile)
        )
        })?;

    let mut guard = RUNTIME
        .lock()
        .map_err(|_| "原生 OCR 状态锁定失败".to_string())?;
    if let Some(runtime) = guard.as_ref() {
        if runtime.profile == profile && runtime.model_dir == model_dir {
            return Ok(format!("原生 ONNX OCR 已就绪: {}", model_dir.display()));
        }
    }

    let runtime = build_runtime(app, &profile, &model_dir)?;
    *guard = Some(runtime);
    Ok(format!("原生 ONNX OCR 已就绪: {}", model_dir.display()))
}

pub fn model_status(app: &AppHandle, model_profile: &str) -> (String, Option<PathBuf>) {
    let profile = normalize_model_profile(model_profile);
    let model_dir = packaged_model_dir(app, &profile);
    (profile, model_dir)
}

pub fn packaged_runtime_profile(app: &AppHandle) -> Option<String> {
    let (profile, _) = packaged_model_config(app, DEFAULT_PACKAGED_MODEL_PROFILE)?;
    ort_dll_candidates(app)
        .into_iter()
        .any(|path| path.is_file())
        .then_some(profile)
}

fn build_runtime(
    app: &AppHandle,
    profile: &str,
    model_dir: &Path,
) -> Result<NativeOcrRuntime, String> {
    set_ort_library_path(app);

    let det_path = model_dir.join("det").join("inference.onnx");
    let rec_path = model_dir.join("rec").join("inference.onnx");
    let rec_yml_path = model_dir.join("rec").join("inference.yml");
    let dict_path = ensure_dict_file(app, profile, &rec_yml_path)?;

    let started = Instant::now();
    let mut engine = OcrLite::new();
    engine
        .init_models_no_angle(
            path_to_str(&det_path)?,
            path_to_str(&rec_path)?,
            path_to_str(&dict_path)?,
            4,
        )
        .map_err(|error| format!("初始化原生 OCR 失败: {}", error))?;

    info!(
        "原生 OCR 初始化完成: profile={}, model={}, dict={}, 用时 {}ms",
        profile,
        model_dir.display(),
        dict_path.display(),
        started.elapsed().as_millis()
    );

    Ok(NativeOcrRuntime {
        profile: profile.to_string(),
        model_dir: model_dir.to_path_buf(),
        engine: Mutex::new(engine),
    })
}

fn set_ort_library_path(app: &AppHandle) {
    if std::env::var_os("ORT_DYLIB_PATH").is_some() {
        return;
    }

    for candidate in ort_dll_candidates(app) {
        if candidate.is_file() {
            std::env::set_var("ORT_DYLIB_PATH", &candidate);
            info!("使用 ONNX Runtime DLL: {}", candidate.display());
            return;
        }
    }

    warn!("未找到内置 onnxruntime.dll，将依赖系统 PATH 或 ort 默认加载逻辑");
}

fn ort_dll_candidates(app: &AppHandle) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("onnxruntime.dll"));
        candidates.push(resource_dir.join("binaries").join("onnxruntime.dll"));
        if let Some(parent) = resource_dir.parent() {
            candidates.push(parent.join("onnxruntime.dll"));
        }
    }
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            candidates.push(dir.join("onnxruntime.dll"));
            candidates.push(dir.join("binaries").join("onnxruntime.dll"));
        }
    }
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(
            current_dir
                .join("src-tauri")
                .join("binaries")
                .join("onnxruntime.dll"),
        );
        candidates.push(current_dir.join("binaries").join("onnxruntime.dll"));
    }
    if let Some(workspace_root) = workspace_root_from_current_dir() {
        candidates.push(
            workspace_root
                .join("src-tauri")
                .join("binaries")
                .join("onnxruntime.dll"),
        );
    }
    candidates
}

fn ensure_dict_file(
    app: &AppHandle,
    profile: &str,
    rec_yml_path: &Path,
) -> Result<PathBuf, String> {
    if !rec_yml_path.is_file() {
        return Err(format!(
            "未找到识别模型字典配置: {}",
            rec_yml_path.display()
        ));
    }

    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("无法获取应用缓存目录: {}", error))?
        .join("ocr-models")
        .join(profile);
    fs::create_dir_all(&cache_dir).map_err(|error| format!("创建 OCR 缓存目录失败: {}", error))?;

    let dict_path = cache_dir.join("dict.txt");
    let dict_is_current = match (fs::metadata(&dict_path), fs::metadata(rec_yml_path)) {
        (Ok(dict_meta), Ok(yml_meta)) => dict_meta.modified().ok() >= yml_meta.modified().ok(),
        _ => false,
    };
    if dict_is_current {
        return Ok(dict_path);
    }

    extract_dict_from_yml(rec_yml_path, &dict_path)?;
    Ok(dict_path)
}

fn extract_dict_from_yml(yml_path: &Path, dict_path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(yml_path)
        .map_err(|error| format!("读取 OCR 字典配置失败: {}", error))?;

    let mut chars: Vec<String> = Vec::new();
    let mut in_dict = false;

    for line in content.lines() {
        if !in_dict {
            if line.trim_start().starts_with("character_dict:") {
                in_dict = true;
            }
            continue;
        }

        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("- ") {
            let rest = rest.trim_end_matches('\r');
            let ch = if rest.starts_with('\'') && rest.ends_with('\'') && rest.len() >= 2 {
                rest[1..rest.len() - 1].replace("''", "'")
            } else {
                rest.to_string()
            };
            chars.push(ch);
        } else if !trimmed.is_empty() && !trimmed.starts_with('-') {
            break;
        }
    }

    if chars.is_empty() {
        return Err("未能从 inference.yml 提取 OCR 字典".to_string());
    }

    let tmp = dict_path.with_extension("tmp");
    {
        let mut file =
            fs::File::create(&tmp).map_err(|error| format!("创建 OCR 字典缓存失败: {}", error))?;
        for ch in &chars {
            writeln!(file, "{}", ch)
                .map_err(|error| format!("写入 OCR 字典缓存失败: {}", error))?;
        }
    }
    fs::rename(&tmp, dict_path).map_err(|error| format!("保存 OCR 字典缓存失败: {}", error))?;
    info!("已提取 OCR 字典: {} 项", chars.len());
    Ok(())
}

fn decode_image(image_base64: &str) -> Result<image::RgbImage, String> {
    let payload = image_base64
        .split_once(',')
        .map(|(_, data)| data)
        .unwrap_or(image_base64)
        .trim();
    let bytes = general_purpose::STANDARD
        .decode(payload)
        .or_else(|_| general_purpose::URL_SAFE.decode(payload))
        .map_err(|error| format!("OCR 图片 base64 解码失败: {}", error))?;
    let image = image::load_from_memory(&bytes)
        .map_err(|error| format!("OCR 图片解析失败: {}", error))?
        .to_rgb8();
    Ok(image)
}


fn bounding_box(points: &[ppocr_rs::Point]) -> (f64, f64, f64, f64) {
    let min_x = points.iter().map(|point| point.x).min().unwrap_or(0);
    let min_y = points.iter().map(|point| point.y).min().unwrap_or(0);
    let max_x = points.iter().map(|point| point.x).max().unwrap_or(min_x);
    let max_y = points.iter().map(|point| point.y).max().unwrap_or(min_y);
    (
        min_x as f64,
        min_y as f64,
        (max_x.saturating_sub(min_x)).max(1) as f64,
        (max_y.saturating_sub(min_y)).max(1) as f64,
    )
}


fn packaged_model_config(app: &AppHandle, model_profile: &str) -> Option<(String, PathBuf)> {
    let profile = normalize_model_profile(model_profile);
    packaged_model_dir(app, &profile).map(|model_dir| (profile, model_dir))
}

fn packaged_model_dir(app: &AppHandle, model_profile: &str) -> Option<PathBuf> {
    let profile = normalize_model_profile(model_profile);
    let mut candidates = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join(OCR_MODEL_RESOURCE_DIR).join(&profile));
    }
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(
            current_dir
                .join("resources")
                .join(OCR_MODEL_RESOURCE_DIR)
                .join(&profile),
        );
        candidates.push(
            current_dir
                .join("src-tauri")
                .join("resources")
                .join(OCR_MODEL_RESOURCE_DIR)
                .join(&profile),
        );
    }
    if let Some(workspace_root) = workspace_root_from_current_dir() {
        candidates.push(
            workspace_root
                .join("src-tauri")
                .join("resources")
                .join(OCR_MODEL_RESOURCE_DIR)
                .join(&profile),
        );
    }

    candidates.into_iter().find(|path| has_model_subdirs(path))
}

fn normalize_model_profile(model_profile: &str) -> String {
    match model_profile.trim() {
        "tiny" | "lite" => "tiny".to_string(),
        "medium" | "accurate" => "medium".to_string(),
        _ => "small".to_string(),
    }
}

fn has_model_subdirs(path: &Path) -> bool {
    path.join("det").join("inference.onnx").is_file()
        && path.join("rec").join("inference.onnx").is_file()
        && path.join("rec").join("inference.yml").is_file()
}

fn path_to_str(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("路径包含无效字符: {}", path.display()))
}

fn workspace_root_from_current_dir() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .and_then(|current_dir| workspace_root_from(&current_dir))
}

fn workspace_root_from(start: &Path) -> Option<PathBuf> {
    start.ancestors().find_map(|dir| {
        let has_package = dir.join("package.json").is_file();
        let has_ocr_models = dir
            .join("src-tauri")
            .join("resources")
            .join(OCR_MODEL_RESOURCE_DIR)
            .is_dir();
        if has_package && has_ocr_models {
            Some(dir.to_path_buf())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::extract_dict_from_yml;
    use std::{fs, path::PathBuf};
    use uuid::Uuid;

    #[test]
    fn extracts_character_dict_from_ppocr_yml() {
        let dir = std::env::temp_dir().join(format!("native-ocr-dict-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let yml = dir.join("inference.yml");
        let dict = dir.join("dict.txt");
        fs::write(
            &yml,
            "PostProcess:\n  character_dict:\n  - 'a'\n  - ''''\n  - 中\nOther:\n  name: done\n",
        )
        .unwrap();

        extract_dict_from_yml(&yml, &dict).unwrap();

        assert_eq!(fs::read_to_string(dict).unwrap(), "a\n'\n中\n");
        let _ = fs::remove_dir_all(PathBuf::from(dir));
    }
}
