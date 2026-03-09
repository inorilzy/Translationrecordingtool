use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_logger(log_dir: PathBuf) -> Result<WorkerGuard, String> {
    fs::create_dir_all(&log_dir).map_err(|e| format!("创建日志目录失败: {}", e))?;

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("app")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .map_err(|e| format!("创建日志文件失败: {}", e))?;

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stdout))
        .init();

    Ok(guard)
}

#[tauri::command]
pub fn get_log_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("获取日志目录失败: {}", e))?;

    if !log_dir.exists() {
        return Ok(vec![]);
    }

    let mut files = vec![];
    for entry in fs::read_dir(&log_dir).map_err(|e| format!("读取日志目录失败: {}", e))? {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("log") {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                files.push(name.to_string());
            }
        }
    }

    files.sort_by(|a, b| b.cmp(a)); // 最新的在前
    Ok(files)
}

#[tauri::command]
pub fn read_log_file(app: tauri::AppHandle, filename: String) -> Result<String, String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("获取日志目录失败: {}", e))?;

    let file_path = log_dir.join(&filename);

    // 安全检查：确保文件在日志目录内
    if !file_path.starts_with(&log_dir) {
        return Err("非法的文件路径".to_string());
    }

    if !file_path.exists() {
        return Err("日志文件不存在".to_string());
    }

    fs::read_to_string(&file_path).map_err(|e| format!("读取日志文件失败: {}", e))
}

#[tauri::command]
pub fn get_log_dir_path(app: tauri::AppHandle) -> Result<String, String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("获取日志目录失败: {}", e))?;

    Ok(log_dir.to_string_lossy().to_string())
}
