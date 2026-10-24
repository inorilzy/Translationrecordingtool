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

use crate::ocr_contracts::{
    normalize_model_profile, OcrEngineSnapshot, OcrRuntimeConfig, SUPPORTED_OCR_MODEL_PROFILE,
};

const OCR_MODEL_RESOURCE_DIR: &str = "ocr-models";
// 工单 21：打包默认 profile 与产品唯一正式 profile 同源，避免两处常量漂移。
const DEFAULT_PACKAGED_MODEL_PROFILE: &str = SUPPORTED_OCR_MODEL_PROFILE;
const NATIVE_ONNX_VERSION: &str = "1.20.1";

#[cfg(target_os = "windows")]
const ORT_LIBRARY_FILE_NAME: &str = "onnxruntime.dll";
#[cfg(target_os = "macos")]
const ORT_LIBRARY_FILE_NAME: &str = "onnxruntime.dylib";
#[cfg(all(unix, not(target_os = "macos")))]
const ORT_LIBRARY_FILE_NAME: &str = "libonnxruntime.so";

struct NativeOcrRuntime {
    profile: String,
    model_dir: PathBuf,
    engine: Mutex<OcrLite>,
}

static RUNTIME: LazyLock<Mutex<Option<NativeOcrRuntime>>> = LazyLock::new(|| Mutex::new(None));

// Lifecycle diagnostics, separate from the inference locks: a status query
// never waits for a running recognition, and never holds RUNTIME/engine while
// reading these. `initialized` may describe an older engine kept after a failed
// switch; `last_failure_profile` says which profile that error belongs to.
#[derive(Default)]
struct Lifecycle {
    initialized: Option<(String, PathBuf)>,
    in_progress: bool,
    last_failure: Option<(String, String)>,
}

static LIFECYCLE: LazyLock<Mutex<Lifecycle>> = LazyLock::new(|| Mutex::new(Lifecycle::default()));

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
    let profile = normalize_model_profile(&config.model_profile);
    let model_dir = packaged_model_dir(app, &profile).ok_or_else(|| {
        format!(
            "未找到原生 OCR profile={} 的本地模型。请先运行 npm run ocr:models -- --profile {}",
            profile, profile
        )
    })?;

    let mut guard = RUNTIME
        .lock()
        .map_err(|_| "原生 OCR 状态锁定失败".to_string())?;
    let running = guard
        .as_ref()
        .filter(|runtime| runtime.profile == profile && runtime.model_dir == model_dir)
        .is_some();
    let outcome = initialize_locked(&profile, &model_dir, running, || {
        build_runtime(app, &profile, &model_dir)
    });
    match outcome {
        Ok(InitOutcome::Reused) => {}
        Ok(InitOutcome::Built(runtime)) => *guard = Some(runtime),
        // The previous engine (if any) stays installed; only diagnostics change.
        Err(error) => return Err(error),
    }
    Ok(format!("原生 ONNX OCR 已就绪: {}", model_dir.display()))
}

enum InitOutcome<T> {
    Reused,
    Built(T),
}

/// Reuse-or-build decision shared by production and tests. Lifecycle is only
/// updated through short locks; RUNTIME is never taken here.
fn initialize_locked<T, B>(
    profile: &str,
    model_dir: &Path,
    running: bool,
    build: B,
) -> Result<InitOutcome<T>, String>
where
    B: FnOnce() -> Result<T, String>,
{
    if running {
        // Reusing the live engine means nothing failed; an older error for
        // another profile is no longer relevant.
        lifecycle(|state| {
            state.last_failure = None;
            state.initialized = Some((profile.to_string(), model_dir.to_path_buf()));
        });
        return Ok(InitOutcome::Reused);
    }
    struct ProgressGuard;
    impl Drop for ProgressGuard {
        fn drop(&mut self) {
            lifecycle(|state| state.in_progress = false);
        }
    }
    lifecycle(|state| {
        state.in_progress = true;
        if state.last_failure.as_ref().is_some_and(|(failed, _)| failed == profile) {
            state.last_failure = None;
        }
    });
    let _progress = ProgressGuard;
    let outcome = build();
    match outcome {
        Ok(runtime) => {
            lifecycle(|state| {
                state.last_failure = None;
                state.initialized = Some((profile.to_string(), model_dir.to_path_buf()));
            });
            Ok(InitOutcome::Built(runtime))
        }
        Err(error) => {
            // Keep any previously working engine and only record the failure.
            lifecycle(|state| state.last_failure = Some((profile.to_string(), error.clone())));
            Err(error)
        }
    }
}

pub fn engine_snapshot(app: &AppHandle, model_profile: &str) -> OcrEngineSnapshot {
    let (profile, model_dir) = model_status(app, model_profile);
    engine_snapshot_for(&profile, model_dir.as_deref())
}

/// Pure status projection over the lifecycle facts. It never touches RUNTIME,
/// so a long-running recognition cannot hide an otherwise healthy engine.
pub fn engine_snapshot_for(profile: &str, model_dir: Option<&Path>) -> OcrEngineSnapshot {
    let state = lifecycle(|state| Lifecycle {
        initialized: state.initialized.clone(),
        in_progress: state.in_progress,
        last_failure: state.last_failure.clone(),
    });
    let initialized_matches = state
        .initialized
        .as_ref()
        .map_or(false, |(ready, path)| ready == profile && model_dir == Some(path.as_path()));
    let last_error = state
        .last_failure
        .as_ref()
        .filter(|(failed, _)| failed == profile)
        .map(|(_, error)| error.clone());
    OcrEngineSnapshot {
        models_available: model_dir.is_some(),
        initialized_profile: initialized_matches.then(|| profile.to_string()),
        last_error,
        initializing: state.in_progress,
        last_failure_profile: state.last_failure.map(|(failed, _)| failed),
    }
}

fn lifecycle<R>(access: impl FnOnce(&mut Lifecycle) -> R) -> R {
    access(&mut LIFECYCLE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()))
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    struct TestState {
        previous: Lifecycle,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl Drop for TestState {
        fn drop(&mut self) {
            lifecycle(|state| *state = std::mem::take(&mut self.previous));
        }
    }

    fn reset() -> TestState {
        let lock = TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = lifecycle(std::mem::take);
        TestState { previous, _lock: lock }
    }

    #[test]
    fn status_keeps_healthy_engine_without_reading_inference_state() {
        let _state = reset();
        let dir = PathBuf::from("C:/models/small");
        lifecycle(|state| {
            state.initialized = Some(("small".to_string(), dir.clone()));
        });
        let status = engine_snapshot_for("small", Some(&dir));
        assert_eq!(status.initialized_profile.as_deref(), Some("small"));
        let contract = crate::ocr_contracts::OcrServiceStatus::from_snapshot(
            &status,
            "small".to_string(),
            Some(dir.display().to_string()),
        );
        assert_eq!(contract.state, crate::ocr_contracts::OcrRuntimeState::Ready);
    }

    #[test]
    fn failed_attempt_does_not_clear_a_working_engine() {
        // 纯生命周期测试：直接调用 initialize_locked，不经 profile 归一；
        // "medium" 只是“另一 profile 键”的测试标签，不代表产品支持 medium（工单 21）。
        let _state = reset();
        let old = PathBuf::from("C:/models/small");
        let attempted = PathBuf::from("C:/models/medium");
        lifecycle(|state| state.initialized = Some(("small".to_string(), old.clone())));
        let result = initialize_locked::<(), _>("medium", &attempted, false, || {
            Err("injected init failure".to_string())
        });
        assert!(result.is_err());
        let status = engine_snapshot_for("small", Some(&old));
        assert_eq!(status.initialized_profile.as_deref(), Some("small"));
        assert_eq!(status.last_failure_profile.as_deref(), Some("medium"));
        assert_eq!(status.last_error, None, "failure belongs to medium");
        let medium = engine_snapshot_for("medium", Some(&attempted));
        assert_eq!(medium.last_error.as_deref(), Some("injected init failure"));
    }

    #[test]
    fn retry_success_clears_failure_and_marks_initialized() {
        let _state = reset();
        let dir = PathBuf::from("C:/models/small");
        assert!(initialize_locked::<(), _>("small", &dir, false, || Err("first failure".into())).is_err());
        assert_eq!(
            engine_snapshot_for("small", Some(&dir))
                .last_error
                .as_deref(),
            Some("first failure")
        );
        // Run the same production build branch with a DLL-free fake engine.
        let built = initialize_locked("small", &dir, false, || Ok(42_u32));
        assert!(matches!(built, Ok(InitOutcome::Built(42))));
        let status = engine_snapshot_for("small", Some(&dir));
        assert_eq!(status.last_error, None);
        assert_eq!(status.initialized_profile.as_deref(), Some("small"));
    }

    #[test]
    fn in_progress_is_reported_and_cleared() {
        let _state = reset();
        let dir = PathBuf::from("C:/models/small");
        assert!(initialize_locked::<(), _>("small", &dir, false, || {
            Err("previous attempt failed".to_string())
        }).is_err());
        let failed = engine_snapshot_for("small", Some(&dir));
        assert_eq!(crate::ocr_contracts::OcrServiceStatus::from_snapshot(
            &failed, "small".to_string(), None,
        ).state, crate::ocr_contracts::OcrRuntimeState::Failed);
        let worker_dir = dir.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            initialize_locked("small", &worker_dir, false, || {
                started_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok(())
            })
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let during = engine_snapshot_for("small", Some(&dir));
        let contract = crate::ocr_contracts::OcrServiceStatus::from_snapshot(
            &during, "small".to_string(), None,
        );
        release_tx.send(()).unwrap();
        assert!(matches!(handle.join().unwrap(), Ok(InitOutcome::Built(()))));
        assert_eq!(contract.state, crate::ocr_contracts::OcrRuntimeState::Initializing);
        let after = engine_snapshot_for("small", Some(&dir));
        assert!(!after.initializing);
        assert_eq!(crate::ocr_contracts::OcrServiceStatus::from_snapshot(
            &after, "small".to_string(), None,
        ).state, crate::ocr_contracts::OcrRuntimeState::Ready);
    }

    #[test]
    fn build_panic_clears_in_progress() {
        let _state = reset();
        let dir = PathBuf::from("C:/models/small");
        let result = std::panic::catch_unwind(|| {
            initialize_locked::<(), _>("small", &dir, false, || panic!("injected panic"))
        });
        assert!(result.is_err());
        assert!(!engine_snapshot_for("small", Some(&dir)).initializing);
    }

    #[test]
    fn concurrent_status_queries_never_block_behind_initialization() {
        let _state = reset();
        let dir = PathBuf::from("C:/models/small");
        // Hold the heavy inference lock the way recognize_text does; status
        // must still answer from the lifecycle record without waiting.
        let runtime = RUNTIME.lock().unwrap();
        lifecycle(|state| state.initialized = Some(("small".to_string(), dir.clone())));
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            tx.send(engine_snapshot_for("small", Some(&dir))).unwrap();
        });
        let status = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("status must not wait for the inference lock");
        assert_eq!(status.initialized_profile.as_deref(), Some("small"));
        drop(runtime);
        handle.join().unwrap();
    }
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
            info!("使用 ONNX Runtime 动态库: {}", candidate.display());
            return;
        }
    }

    warn!(
        "未找到内置 {}，将依赖系统 PATH 或 ort 默认加载逻辑",
        ORT_LIBRARY_FILE_NAME
    );
}

fn ort_dll_candidates(app: &AppHandle) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let library_name = ORT_LIBRARY_FILE_NAME;
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join(library_name));
        candidates.push(resource_dir.join("binaries").join(library_name));
        if let Some(parent) = resource_dir.parent() {
            candidates.push(parent.join(library_name));
        }
    }
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            candidates.push(dir.join(library_name));
            candidates.push(dir.join("binaries").join(library_name));
        }
    }
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(
            current_dir
                .join("src-tauri")
                .join("binaries")
                .join(library_name),
        );
        candidates.push(current_dir.join("binaries").join(library_name));
    }
    if let Some(workspace_root) = workspace_root_from_current_dir() {
        candidates.push(
            workspace_root
                .join("src-tauri")
                .join("binaries")
                .join(library_name),
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
        // Tauri preserves the source-relative path for `bundle.resources`, so
        // packaged builds normally place models under `Resources/resources/`.
        // Older layouts (and the dev tree) use `Resources/ocr-models`; accept
        // both instead of depending on one packaging convention.
        candidates.push(resource_dir.join(OCR_MODEL_RESOURCE_DIR).join(&profile));
        candidates.push(
            resource_dir
                .join("resources")
                .join(OCR_MODEL_RESOURCE_DIR)
                .join(&profile),
        );
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

// 工单 21：本地 normalize_model_profile（tiny/lite→tiny、medium/accurate→medium）已删除，
// 统一复用 ocr_contracts::normalize_model_profile，运行时与 settings 加载/更新规则一致。

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
