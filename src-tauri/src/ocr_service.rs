use std::{
    fs::OpenOptions,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, RwLock},
    thread,
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tracing::{info, warn};

const OCR_HOST: &str = "127.0.0.1";
const OCR_LANG: &str = "ch";
const OCR_DEVICE: &str = "cpu";
const PADDLE_OCR_VERSION: &str = "2.7.3";
const PADDLEPADDLE_VERSION: &str = "2.6.2";

#[derive(Default)]
pub struct OcrServiceState {
    child: Mutex<Option<Child>>,
    last_error: RwLock<Option<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrServiceStatus {
    pub running: bool,
    pub endpoint: String,
    pub message: String,
    pub last_error: Option<String>,
    pub paddleocr_version: &'static str,
    pub paddlepaddle_version: &'static str,
    pub lang: &'static str,
    pub device: &'static str,
}

pub fn spawn_startup_check(app: AppHandle, endpoint: String) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = ensure_running(&app, &endpoint).await {
            set_last_error(&app, error.clone());
            warn!("自动启动 OCR 服务失败: {}", error);
        }
    });
}

pub async fn ensure_running(app: &AppHandle, endpoint: &str) -> Result<String, String> {
    if let Ok(message) = crate::ocr::check_service(endpoint).await {
        clear_last_error(app);
        return Ok(message);
    }

    if let Err(error) = start_process_if_needed(app, endpoint) {
        set_last_error(app, error.clone());
        return Err(error);
    }

    match wait_until_healthy(endpoint, Duration::from_secs(90)).await {
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

pub async fn status(app: &AppHandle, endpoint: &str) -> OcrServiceStatus {
    match crate::ocr::check_service(endpoint).await {
        Ok(message) => OcrServiceStatus {
            running: true,
            endpoint: endpoint.to_string(),
            message,
            last_error: None,
            paddleocr_version: PADDLE_OCR_VERSION,
            paddlepaddle_version: PADDLEPADDLE_VERSION,
            lang: OCR_LANG,
            device: OCR_DEVICE,
        },
        Err(error) => OcrServiceStatus {
            running: false,
            endpoint: endpoint.to_string(),
            message: "Paddle OCR 服务未运行".to_string(),
            last_error: last_error(app).or(Some(error)),
            paddleocr_version: PADDLE_OCR_VERSION,
            paddlepaddle_version: PADDLEPADDLE_VERSION,
            lang: OCR_LANG,
            device: OCR_DEVICE,
        },
    }
}

fn start_process_if_needed(app: &AppHandle, endpoint: &str) -> Result<(), String> {
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

    let mut command = build_command(app, endpoint)?;
    configure_logs(app, &mut command)?;

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let child = command
        .spawn()
        .map_err(|error| format!("启动 Paddle OCR 服务失败: {}", error))?;

    clear_last_error(app);
    info!("已启动 Paddle OCR 服务进程: {}", child.id());
    *child_guard = Some(child);
    Ok(())
}

fn build_command(app: &AppHandle, endpoint: &str) -> Result<Command, String> {
    let url = reqwest::Url::parse(endpoint.trim())
        .map_err(|error| format!("Paddle OCR HTTP 地址无效: {}", error))?;
    let host = url.host_str().unwrap_or(OCR_HOST).to_string();
    let port = url.port().unwrap_or(8866).to_string();

    if cfg!(debug_assertions) {
        let mut command = npm_command();
        command.args(["run", "ocr:server"]);
        if let Ok(current_dir) = std::env::current_dir() {
            command.current_dir(current_dir);
        }
        return Ok(command);
    }

    let script_path = packaged_script_path(app)?;
    let mut command = Command::new("uv");
    command.args([
        "run",
        "--python",
        "3.11",
        "--with",
        &format!("paddleocr=={}", PADDLE_OCR_VERSION),
        "--with",
        &format!("paddlepaddle=={}", PADDLEPADDLE_VERSION),
        "--with",
        "numpy<2",
        "python",
    ]);
    command.arg(script_path);
    command.args(["--host", &host, "--port", &port, "--lang", OCR_LANG, "--device", OCR_DEVICE]);
    Ok(command)
}

fn npm_command() -> Command {
    if cfg!(windows) {
        Command::new("npm.cmd")
    } else {
        Command::new("npm")
    }
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

fn configure_logs(app: &AppHandle, command: &mut Command) -> Result<(), String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|error| format!("无法获取日志目录: {}", error))?;
    std::fs::create_dir_all(&log_dir)
        .map_err(|error| format!("无法创建日志目录: {}", error))?;
    let log_path = log_dir.join("paddle-ocr-service.log");
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

async fn wait_until_healthy(endpoint: &str, timeout: Duration) -> Result<String, String> {
    let started = std::time::Instant::now();
    let mut last_error = None;

    while started.elapsed() < timeout {
        match crate::ocr::check_service(endpoint).await {
            Ok(message) => return Ok(message),
            Err(error) => last_error = Some(error),
        }
        async_sleep(Duration::from_millis(700)).await;
    }

    Err(format!(
        "Paddle OCR 服务启动超时。最后错误: {}",
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
