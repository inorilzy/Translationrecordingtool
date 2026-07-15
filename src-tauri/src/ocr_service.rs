use std::{
    env,
    fs::OpenOptions,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, RwLock},
    thread,
    time::Duration,
};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

use crate::ocr_contracts::{
    engine_label, normalize_engine, OcrHealthStatus, OcrRuntimeConfig, OcrServiceStatus,
    OCR_DEVICE, OCR_LANG, PADDLE_OCR_VERSION, PPOCR_VERSION, RAPID_OCR_VERSION,
    SIDECAR_ONNXRUNTIME_VERSION,
};

const OCR_HOST: &str = "127.0.0.1";
const OCR_SIDECAR_STEM: &str = "paddle-ocr-server";
const OCR_MODEL_RESOURCE_DIR: &str = "ocr-models";

#[derive(Default)]
pub struct OcrServiceState {
    child: Mutex<Option<Child>>,
    last_error: RwLock<Option<String>>,
}

pub async fn ensure_running(app: &AppHandle, config: &OcrRuntimeConfig) -> Result<String, String> {
    if let Ok(message) =
        check_service_config(&config.endpoint, &config.engine, &config.model_profile).await
    {
        clear_last_error(app);
        return Ok(message);
    }

    stop_process_if_managed(app)?;

    if let Err(error) = start_process_if_needed(app, config) {
        set_last_error(app, error.clone());
        return Err(error);
    }

    match wait_until_healthy(
        &config.endpoint,
        &config.engine,
        &config.model_profile,
        Duration::from_secs(90),
    )
    .await
    {
        Ok(message) => {
            clear_last_error(app);
            Ok(message)
        }
        Err(error) => {
            set_last_error(app, error.clone());
            Err(error)
        }
    }
}

pub async fn warmup(app: &AppHandle, config: &OcrRuntimeConfig) -> Result<String, String> {
    ensure_running(app, config).await?;
    warmup_service(&config.endpoint).await
}

pub async fn restart(app: &AppHandle, config: &OcrRuntimeConfig) -> Result<String, String> {
    stop_process(app)?;
    ensure_running(app, config).await?;
    warmup(app, config).await
}

pub async fn status(app: &AppHandle, config: &OcrRuntimeConfig) -> OcrServiceStatus {
    let engine = normalize_engine(&config.engine);
    let (model_profile, model_dir) = sidecar_model_status(app, config);
    let sidecar_path = packaged_sidecar_path(app);
    let log_path = log_path(app, engine).ok();
    let (rapidocr_version, paddleocr_version, onnxruntime_version) = engine_versions(engine);
    let (running, message, last_error) =
        match check_service_config(&config.endpoint, engine, &model_profile).await {
            Ok(message) => (true, message, None),
            Err(error) => (
                false,
                "OCR 服务未运行".to_string(),
                last_error(app).or(Some(error)),
            ),
        };

    OcrServiceStatus {
        running,
        endpoint: config.endpoint.clone(),
        message,
        last_error,
        engine: engine.to_string(),
        model_profile,
        model_dir: model_dir.as_ref().map(|path| path.display().to_string()),
        sidecar_path: sidecar_path.as_ref().map(|path| path.display().to_string()),
        log_path: log_path.as_ref().map(|path| path.display().to_string()),
        preload_on_startup: config.preload_on_startup,
        rapidocr_version,
        paddleocr_version,
        ppocr_version: PPOCR_VERSION,
        onnxruntime_version,
        lang: OCR_LANG,
        device: OCR_DEVICE,
    }
}

fn engine_versions(engine: &str) -> (&'static str, &'static str, &'static str) {
    match normalize_engine(engine) {
        "rapidocr" => (RAPID_OCR_VERSION, "-", SIDECAR_ONNXRUNTIME_VERSION),
        _ => ("-", PADDLE_OCR_VERSION, SIDECAR_ONNXRUNTIME_VERSION),
    }
}

fn sidecar_model_status(app: &AppHandle, config: &OcrRuntimeConfig) -> (String, Option<PathBuf>) {
    match normalize_engine(&config.engine) {
        "rapidocr" => ("embedded".to_string(), None),
        _ => {
            let profile = normalize_model_profile(&config.model_profile);
            let model_dir = if profile == "official" {
                None
            } else {
                packaged_model_dir(app, &profile)
            };
            (profile, model_dir)
        }
    }
}

fn start_process_if_needed(app: &AppHandle, config: &OcrRuntimeConfig) -> Result<(), String> {
    let engine = normalize_engine(&config.engine);
    if !matches!(engine, "paddleocr" | "rapidocr") {
        return Err(format!(
            "暂不支持 OCR 引擎 {}，当前可用引擎为 paddleocr、rapidocr",
            config.engine
        ));
    }

    let state = app.state::<Arc<OcrServiceState>>();
    let mut child_guard = state
        .child
        .lock()
        .map_err(|_| "OCR 服务状态锁定失败".to_string())?;

    if let Some(child) = child_guard.as_mut() {
        match child.try_wait() {
            Ok(None) => {
                info!("OCR 服务进程已在启动中");
                return Ok(());
            }
            Ok(Some(status)) => {
                warn!("OCR 服务进程已退出: {}", status);
                *child_guard = None;
            }
            Err(error) => {
                warn!("检查 OCR 服务进程失败: {}", error);
                *child_guard = None;
            }
        }
    }

    let mut command = build_command(app, config)?;
    configure_logs(app, engine, &mut command)?;

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let child = command
        .spawn()
        .map_err(|error| format!("启动 OCR 服务失败: {}", error))?;

    clear_last_error(app);
    info!("已启动 OCR 服务进程: {}", child.id());
    *child_guard = Some(child);
    Ok(())
}

pub(crate) fn stop_process(app: &AppHandle) -> Result<(), String> {
    stop_process_if_managed(app)
}

fn stop_process_if_managed(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<Arc<OcrServiceState>>();
    let mut child_guard = state
        .child
        .lock()
        .map_err(|_| "OCR 服务状态锁定失败".to_string())?;

    if let Some(mut child) = child_guard.take() {
        let _ = child.kill();
        let _ = child.wait();
    }

    Ok(())
}

fn build_command(app: &AppHandle, config: &OcrRuntimeConfig) -> Result<Command, String> {
    let url = reqwest::Url::parse(config.endpoint.trim())
        .map_err(|error| format!("OCR HTTP 地址无效: {}", error))?;
    let host = url.host_str().unwrap_or(OCR_HOST).to_string();
    let port = url.port().unwrap_or(8866).to_string();
    let engine = normalize_engine(&config.engine);
    let (model_profile, model_dir, allow_official_model_download) =
        resolve_sidecar_model_config(app, engine, &config.model_profile)?;
    let mut resolved_config = config.clone();
    resolved_config.engine = engine.to_string();
    resolved_config.model_profile = model_profile.clone();

    if cfg!(debug_assertions) {
        let mut command = npm_command();
        command.args(["run", ocr_server_script_name(engine)]);
        command.arg("--");
        command.args(ocr_server_args(
            &host,
            &port,
            &resolved_config,
            &model_profile,
            model_dir.as_ref(),
            allow_official_model_download,
        ));
        if let Some(workspace_root) = workspace_root_from_current_dir() {
            command.current_dir(workspace_root);
        }
        return Ok(command);
    }

    if let Some(sidecar_path) = packaged_sidecar_path(app) {
        let mut command = Command::new(sidecar_path);
        command.args(ocr_server_args(
            &host,
            &port,
            &resolved_config,
            &model_profile,
            model_dir.as_ref(),
            allow_official_model_download,
        ));
        return Ok(command);
    }

    let script_path = packaged_script_path(app)?;
    let uv_path = find_command_on_path("uv").ok_or_else(|| {
        format!(
            "未找到内置 OCR sidecar，也未找到 uv。请先运行 npm run ocr:sidecar:win 生成 src-tauri/binaries/{}-x86_64-pc-windows-msvc.exe，或安装 uv 后重试。",
            OCR_SIDECAR_STEM
        )
    })?;

    let mut command = Command::new(uv_path);
    command.args([
        "run",
        "--python",
        "3.11",
        "--with",
        ocr_engine_runtime_requirement(engine),
        "--with",
        ocr_engine_core_requirement(engine),
        "--with",
        "numpy<2",
        "python",
    ]);
    command.arg(script_path);
    command.args(ocr_server_args(
        &host,
        &port,
        &resolved_config,
        &model_profile,
        model_dir.as_ref(),
        allow_official_model_download,
    ));
    Ok(command)
}

fn ocr_server_args(
    host: &str,
    port: &str,
    config: &OcrRuntimeConfig,
    model_profile: &str,
    model_dir: Option<&PathBuf>,
    allow_official_model_download: bool,
) -> Vec<String> {
    let mut args = vec![
        "--host".to_string(),
        host.to_string(),
        "--port".to_string(),
        port.to_string(),
        "--lang".to_string(),
        OCR_LANG.to_string(),
        "--device".to_string(),
        OCR_DEVICE.to_string(),
        "--engine".to_string(),
        config.engine.clone(),
        "--model-profile".to_string(),
        model_profile.to_string(),
    ];

    if let Some(model_dir) = model_dir {
        args.push("--model-dir".to_string());
        args.push(model_dir.display().to_string());
    }
    if allow_official_model_download {
        args.push("--allow-official-model-download".to_string());
    }

    args
}

fn ocr_server_script_name(engine: &str) -> &'static str {
    match normalize_engine(engine) {
        "rapidocr" => "ocr:server:rapid",
        _ => "ocr:server:paddle",
    }
}

fn ocr_engine_runtime_requirement(engine: &str) -> &'static str {
    match normalize_engine(engine) {
        "rapidocr" => "rapidocr-onnxruntime==1.4.4",
        _ => "paddleocr==3.7.0",
    }
}

fn ocr_engine_core_requirement(_engine: &str) -> &'static str {
    "onnxruntime==1.27.0"
}

fn resolve_sidecar_model_config(
    app: &AppHandle,
    engine: &str,
    model_profile: &str,
) -> Result<(String, Option<PathBuf>, bool), String> {
    if normalize_engine(engine) == "rapidocr" {
        return Ok(("embedded".to_string(), None, false));
    }

    let profile = normalize_model_profile(model_profile);
    if profile == "official" {
        return Ok((profile, None, true));
    }

    let model_dir = packaged_model_dir(app, &profile).ok_or_else(|| {
        format!(
            "未找到 PaddleOCR profile={} 的本地模型。请先运行 npm run ocr:models:win -- -Profile {}，或显式选择 official 配置允许下载官方模型",
            profile, profile
        )
    })?;
    Ok((profile, Some(model_dir), false))
}

const DEFAULT_PACKAGED_MODEL_PROFILE: &str = "small";

fn packaged_model_dir(app: &AppHandle, model_profile: &str) -> Option<PathBuf> {
    let profile = normalize_model_profile(model_profile);
    let mut candidates = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join(OCR_MODEL_RESOURCE_DIR).join(&profile));
    }
    if let Ok(current_dir) = env::current_dir() {
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
    match model_profile.trim().to_ascii_lowercase().as_str() {
        "tiny" | "lite" => "tiny".to_string(),
        "medium" | "accurate" => "medium".to_string(),
        "official" | "download" => "official".to_string(),
        "embedded" | "bundled" => "embedded".to_string(),
        _ => "small".to_string(),
    }
}

fn has_model_subdirs(path: &PathBuf) -> bool {
    model_dir_has_files(&path.join("det")) && model_dir_has_files(&path.join("rec"))
}

fn model_dir_has_files(path: &PathBuf) -> bool {
    path.is_dir()
        && path
            .read_dir()
            .map(|mut entries| {
                entries.any(|entry| entry.map(|item| item.path().is_file()).unwrap_or(false))
            })
            .unwrap_or(false)
}

pub fn packaged_runtime_profile(app: &AppHandle) -> Option<String> {
    packaged_model_dir(app, DEFAULT_PACKAGED_MODEL_PROFILE)
        .map(|_| DEFAULT_PACKAGED_MODEL_PROFILE.to_string())
}

pub fn has_packaged_sidecar(app: &AppHandle) -> bool {
    packaged_sidecar_path(app).is_some()
}

fn packaged_sidecar_path(app: &AppHandle) -> Option<PathBuf> {
    let sidecar_file_names = sidecar_file_names();
    let mut candidates = sidecar_path_candidates(&sidecar_file_names);

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.extend(
            sidecar_file_names
                .iter()
                .map(|name| resource_dir.join(name)),
        );
        candidates.extend(
            sidecar_file_names
                .iter()
                .map(|name| resource_dir.join("binaries").join(name)),
        );
        if let Some(parent) = resource_dir.parent() {
            candidates.extend(sidecar_file_names.iter().map(|name| parent.join(name)));
        }
    }

    candidates.into_iter().find(|path| path.exists())
}

fn sidecar_path_candidates(sidecar_file_names: &[String]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(current_exe) = env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            candidates.extend(sidecar_file_names.iter().map(|name| exe_dir.join(name)));
        }
    }

    if let Ok(current_dir) = env::current_dir() {
        candidates.extend(
            sidecar_file_names
                .iter()
                .map(|name| current_dir.join("src-tauri").join("binaries").join(name)),
        );
        candidates.extend(
            sidecar_file_names
                .iter()
                .map(|name| current_dir.join("binaries").join(name)),
        );
    }

    candidates
}

fn sidecar_file_names() -> Vec<String> {
    let suffix = env::consts::EXE_SUFFIX;
    let with_suffix = |stem: &str| {
        if suffix.is_empty() {
            stem.to_string()
        } else {
            format!("{}{}", stem, suffix)
        }
    };

    let mut names = vec![with_suffix(OCR_SIDECAR_STEM)];
    if let Some(target) = sidecar_target_triple() {
        names.push(with_suffix(&format!("{}-{}", OCR_SIDECAR_STEM, target)));
    }
    names
}

fn sidecar_target_triple() -> Option<&'static str> {
    match (env::consts::OS, env::consts::ARCH) {
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
        ("windows", "aarch64") => Some("aarch64-pc-windows-msvc"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        _ => None,
    }
}

fn find_command_on_path(command: &str) -> Option<PathBuf> {
    let path_value = env::var_os("PATH")?;
    let extensions = executable_extensions();

    env::split_paths(&path_value).find_map(|dir| {
        extensions
            .iter()
            .map(|extension| dir.join(format!("{}{}", command, extension)))
            .find(|candidate| candidate.is_file())
    })
}

fn executable_extensions() -> Vec<&'static str> {
    if cfg!(windows) {
        vec![".exe", ".cmd", ".bat", ""]
    } else {
        vec![""]
    }
}

fn npm_command() -> Command {
    if cfg!(windows) {
        Command::new("npm.cmd")
    } else {
        Command::new("npm")
    }
}

fn workspace_root_from_current_dir() -> Option<PathBuf> {
    env::current_dir()
        .ok()
        .and_then(|current_dir| workspace_root_from(&current_dir))
}

fn workspace_root_from(start: &Path) -> Option<PathBuf> {
    start.ancestors().find_map(|dir| {
        let has_package = dir.join("package.json").is_file();
        let has_ocr_script = dir.join("scripts").join("paddle_ocr_server.py").is_file();
        if has_package && has_ocr_script {
            Some(dir.to_path_buf())
        } else {
            None
        }
    })
}

fn packaged_script_path(app: &AppHandle) -> Result<PathBuf, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| format!("无法获取应用资源目录: {}", error))?;
    let candidates = [
        resource_dir.join("paddle_ocr_server.py"),
        resource_dir.join("scripts").join("paddle_ocr_server.py"),
    ];
    candidates
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| {
            "未找到内置 OCR 服务脚本。请确认打包资源包含 paddle_ocr_server.py".to_string()
        })
}

fn configure_logs(app: &AppHandle, engine: &str, command: &mut Command) -> Result<(), String> {
    let log_path = log_path(app, engine)?;
    let Some(log_dir) = log_path.parent() else {
        return Err("无法获取 OCR 日志目录".to_string());
    };
    std::fs::create_dir_all(log_dir).map_err(|error| format!("无法创建日志目录: {}", error))?;
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|error| format!("无法写入 OCR 日志: {}", error))?;
    let stderr = log_file
        .try_clone()
        .map_err(|error| format!("无法复制 OCR 日志句柄: {}", error))?;
    command.stdout(Stdio::from(log_file));
    command.stderr(Stdio::from(stderr));
    Ok(())
}

pub fn log_path(app: &AppHandle, engine: &str) -> Result<PathBuf, String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|error| format!("无法获取日志目录: {}", error))?;
    Ok(log_dir.join(log_file_name(engine)))
}

fn log_file_name(engine: &str) -> &'static str {
    match normalize_engine(engine) {
        "rapidocr" => "rapidocr-service.log",
        _ => "paddleocr-service.log",
    }
}

async fn wait_until_healthy(
    endpoint: &str,
    engine: &str,
    model_profile: &str,
    timeout: Duration,
) -> Result<String, String> {
    let started = std::time::Instant::now();
    let mut last_error = None;

    while started.elapsed() < timeout {
        match check_service_config(endpoint, engine, model_profile).await {
            Ok(message) => return Ok(message),
            Err(error) => last_error = Some(error),
        }
        async_sleep(Duration::from_millis(700)).await;
    }

    Err(format!(
        "OCR 服务启动超时。最后错误: {}",
        last_error.unwrap_or_else(|| "未知错误".to_string())
    ))
}

async fn async_sleep(duration: Duration) {
    tauri::async_runtime::spawn_blocking(move || thread::sleep(duration))
        .await
        .ok();
}

fn set_last_error(app: &AppHandle, error: String) {
    if let Some(state) = app.try_state::<Arc<OcrServiceState>>() {
        if let Ok(mut last_error) = state.last_error.write() {
            *last_error = Some(error);
        }
    }
}

fn clear_last_error(app: &AppHandle) {
    if let Some(state) = app.try_state::<Arc<OcrServiceState>>() {
        if let Ok(mut last_error) = state.last_error.write() {
            *last_error = None;
        }
    }
}

fn last_error(app: &AppHandle) -> Option<String> {
    app.try_state::<Arc<OcrServiceState>>()
        .and_then(|state| state.last_error.read().ok().and_then(|error| error.clone()))
}

#[derive(Debug, Serialize)]
struct OcrRequest<'a> {
    image: &'a str,
}

pub async fn recognize_text(endpoint: &str, image_base64: &str) -> Result<String, String> {
    validate_endpoint(endpoint)?;
    let payload = image_base64
        .split_once(',')
        .map(|(_, data)| data)
        .unwrap_or(image_base64);

    let response = crate::http_client::shared_client()
        .post(endpoint)
        .header("Content-Type", "application/json")
        .json(&OcrRequest { image: payload })
        .send()
        .await
        .map_err(|error| format!("OCR 请求失败: {}", error))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("OCR 返回错误: {} {}", status, body));
    }

    let value: Value = response
        .json()
        .await
        .map_err(|error| format!("OCR 响应解析失败: {}", error))?;
    extract_text(&value).ok_or_else(|| "OCR 未返回可识别文本".to_string())
}

pub async fn check_service_config(
    endpoint: &str,
    expected_engine: &str,
    expected_model_profile: &str,
) -> Result<String, String> {
    let health = check_service_health(endpoint).await?;
    validate_health_config(&health, expected_engine, expected_model_profile)?;

    Ok(format!(
        "{} 服务正常: {}",
        engine_label(expected_engine),
        health_url_from_endpoint(endpoint)?
    ))
}

fn validate_health_config(
    health: &OcrHealthStatus,
    expected_engine: &str,
    expected_model_profile: &str,
) -> Result<(), String> {
    let actual_engine = health.engine.as_deref().unwrap_or("unknown");
    let expected_engine = normalize_engine(expected_engine);
    if normalize_engine(actual_engine) != expected_engine {
        return Err(format!(
            "OCR 服务引擎不匹配：当前为 {}，设置中选择的是 {}",
            engine_label(actual_engine),
            engine_label(expected_engine)
        ));
    }

    let expected_profile = if expected_engine == "rapidocr" {
        "embedded".to_string()
    } else {
        normalize_model_profile(expected_model_profile)
    };
    let actual_profile = health
        .model_profile
        .as_deref()
        .unwrap_or("unknown")
        .trim()
        .to_ascii_lowercase();
    if actual_profile != expected_profile {
        return Err(format!(
            "OCR 服务 profile 不匹配：当前为 {}，设置中选择的是 {}",
            actual_profile, expected_profile
        ));
    }

    let expected_source = if expected_engine == "rapidocr" {
        "embedded"
    } else if expected_profile == "official" {
        "official-download"
    } else {
        "local"
    };
    let actual_source = health.model_source.as_deref().unwrap_or("unknown");
    if actual_source != expected_source {
        return Err(format!(
            "OCR 服务 model source 不匹配：当前为 {}，期望 {}",
            actual_source, expected_source
        ));
    }

    Ok(())
}
pub async fn check_service_health(endpoint: &str) -> Result<OcrHealthStatus, String> {
    validate_endpoint(endpoint)?;
    let health_url = health_url_from_endpoint(endpoint)?;
    let response = crate::http_client::shared_client()
        .get(&health_url)
        .send()
        .await
        .map_err(|error| format!("OCR 服务连接失败: {}", error))?;

    if !response.status().is_success() {
        return Err(format!("OCR 服务健康检查失败: {}", response.status()));
    }

    response
        .json::<OcrHealthStatus>()
        .await
        .map_err(|error| format!("OCR 服务健康检查响应解析失败: {}", error))
}
pub async fn warmup_service(endpoint: &str) -> Result<String, String> {
    validate_endpoint(endpoint)?;
    let warmup_url = sibling_url_from_endpoint(endpoint, "/warmup")?;
    let response = crate::http_client::shared_client()
        .post(&warmup_url)
        .send()
        .await
        .map_err(|error| format!("OCR 预热请求失败: {}", error))?;

    if !response.status().is_success() {
        return Err(format!("OCR 预热失败: {}", response.status()));
    }

    Ok(format!("OCR 预热完成: {}", warmup_url))
}

fn validate_endpoint(endpoint: &str) -> Result<(), String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return Err("请先在设置中配置 OCR HTTP 地址".to_string());
    }
    if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
        return Err("OCR HTTP 地址必须以 http:// 或 https:// 开头".to_string());
    }
    Ok(())
}

fn health_url_from_endpoint(endpoint: &str) -> Result<String, String> {
    sibling_url_from_endpoint(endpoint, "/health")
}

fn sibling_url_from_endpoint(endpoint: &str, path: &str) -> Result<String, String> {
    let mut url = reqwest::Url::parse(endpoint.trim())
        .map_err(|error| format!("OCR HTTP 地址无效: {}", error))?;
    url.set_path(path);
    url.set_query(None);
    Ok(url.to_string())
}

fn extract_text(value: &Value) -> Option<String> {
    value
        .get("text")
        .and_then(Value::as_str)
        .and_then(clean_text)
        .or_else(|| value.get("result").and_then(extract_text_from_node))
        .or_else(|| value.get("data").and_then(extract_text_from_node))
        .or_else(|| extract_text_from_node(value))
}

fn extract_text_from_node(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str().and_then(clean_text) {
        return Some(text);
    }

    if let Some(items) = value.as_array() {
        let lines = items
            .iter()
            .filter_map(extract_text_from_node)
            .collect::<Vec<_>>();
        if !lines.is_empty() {
            return Some(lines.join("\n"));
        }
    }

    if let Some(object) = value.as_object() {
        for key in [
            "text",
            "recText",
            "rec_texts",
            "transcription",
            "label",
            "value",
            "words",
            "result",
            "results",
            "data",
            "ocrResults",
            "texts",
        ] {
            if let Some(text) = object.get(key).and_then(extract_text_from_node) {
                return Some(text);
            }
        }
    }

    None
}

fn clean_text(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_paddle_style_nested_lines() {
        let value = serde_json::json!({
            "result": [
                { "recText": "first line" },
                { "recText": "second line" }
            ]
        });

        assert_eq!(
            extract_text(&value),
            Some("first line\nsecond line".to_string())
        );
    }

    #[test]
    fn extracts_legacy_ocr_collection_and_word_keys() {
        let value = serde_json::json!({
            "ocrResults": [
                { "words": "hello" },
                { "words": "world" }
            ]
        });

        assert_eq!(extract_text(&value), Some("hello\nworld".to_string()));
    }

    #[test]
    fn extracts_previous_results_and_paddle_v3_keys() {
        let results = serde_json::json!({
            "results": [{ "rec_texts": ["first", "second"] }]
        });
        let texts = serde_json::json!({ "texts": ["hello", "world"] });

        assert_eq!(extract_text(&results), Some("first\nsecond".to_string()));
        assert_eq!(extract_text(&texts), Some("hello\nworld".to_string()));
    }

    #[test]
    fn sidecar_file_names_include_plain_and_target_specific_names() {
        let names = sidecar_file_names();

        if cfg!(windows) {
            assert!(names.contains(&"paddle-ocr-server.exe".to_string()));
        } else {
            assert!(names.contains(&"paddle-ocr-server".to_string()));
        }

        if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            assert!(names.contains(&"paddle-ocr-server-x86_64-pc-windows-msvc.exe".to_string()));
        }
    }

    #[test]
    fn executable_extensions_match_platform() {
        let extensions = executable_extensions();

        if cfg!(windows) {
            assert!(extensions.contains(&".exe"));
            assert!(extensions.contains(&".cmd"));
            assert!(extensions.contains(&".bat"));
        }

        assert!(extensions.contains(&""));
    }

    #[test]
    fn picks_debug_script_for_engine() {
        assert_eq!(ocr_server_script_name("paddleocr"), "ocr:server:paddle");
        assert_eq!(ocr_server_script_name("rapidocr"), "ocr:server:rapid");
    }

    #[test]
    fn picks_runtime_requirements_for_engine() {
        assert_eq!(
            ocr_engine_runtime_requirement("paddleocr"),
            "paddleocr==3.7.0"
        );
        assert_eq!(
            ocr_engine_core_requirement("paddleocr"),
            "onnxruntime==1.27.0"
        );
        assert_eq!(
            ocr_engine_runtime_requirement("rapidocr"),
            "rapidocr-onnxruntime==1.4.4"
        );
        assert_eq!(
            ocr_engine_core_requirement("rapidocr"),
            "onnxruntime==1.27.0"
        );
    }

    #[test]
    fn rejects_running_sidecar_with_stale_model_profile() {
        let health = OcrHealthStatus {
            ok: true,
            engine: Some("paddleocr".to_string()),
            lang: Some("ch".to_string()),
            device: Some("cpu".to_string()),
            model_profile: Some("small".to_string()),
            model_dir: Some("C:/models/small".to_string()),
            model_source: Some("local".to_string()),
        };

        let error = validate_health_config(&health, "paddleocr", "official").unwrap_err();

        assert!(error.contains("profile"));
        assert!(error.contains("official"));
    }

    #[test]
    fn rejects_running_sidecar_with_unverified_model_source() {
        let health = OcrHealthStatus {
            ok: true,
            engine: Some("paddleocr".to_string()),
            lang: Some("ch".to_string()),
            device: Some("cpu".to_string()),
            model_profile: Some("official".to_string()),
            model_dir: None,
            model_source: None,
        };

        let error = validate_health_config(&health, "paddleocr", "official").unwrap_err();

        assert!(error.contains("model source"));
        assert!(error.contains("official-download"));
    }

    #[test]
    fn builds_rapidocr_args_with_embedded_models() {
        let config = OcrRuntimeConfig {
            endpoint: "http://127.0.0.1:8867/ocr".to_string(),
            engine: "rapidocr".to_string(),
            model_profile: "embedded".to_string(),
            preload_on_startup: true,
        };

        let args = ocr_server_args("127.0.0.1", "8867", &config, "embedded", None, false);

        assert!(args.windows(2).any(|pair| pair == ["--engine", "rapidocr"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--model-profile", "embedded"]));
        assert!(args.windows(2).any(|pair| pair == ["--port", "8867"]));
        assert!(!args.contains(&"--allow-official-model-download".to_string()));
    }

    #[test]
    fn paddle_official_profile_explicitly_allows_download() {
        let config = OcrRuntimeConfig {
            endpoint: "http://127.0.0.1:8866/ocr".to_string(),
            engine: "paddleocr".to_string(),
            model_profile: "official".to_string(),
            preload_on_startup: true,
        };

        let args = ocr_server_args("127.0.0.1", "8866", &config, "official", None, true);

        assert!(args.contains(&"--allow-official-model-download".to_string()));
        assert!(!args.contains(&"--model-dir".to_string()));
    }

    #[test]
    fn uses_engine_specific_log_file_names() {
        assert_eq!(log_file_name("paddleocr"), "paddleocr-service.log");
        assert_eq!(log_file_name("rapidocr"), "rapidocr-service.log");
    }

    #[test]
    fn finds_workspace_root_from_nested_path() {
        let current_dir = env::current_dir().unwrap();
        let workspace_root = workspace_root_from(&current_dir).unwrap();

        assert!(workspace_root.join("package.json").is_file());
        assert!(workspace_root
            .join("scripts")
            .join("paddle_ocr_server.py")
            .is_file());
    }
}
