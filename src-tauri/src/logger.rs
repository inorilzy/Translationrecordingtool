use std::fs;
use std::path::{Component, Path, PathBuf};
use tauri::Manager;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
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
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
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

    read_log_file_in_dir(&log_dir, &filename)
}

fn read_log_file_in_dir(log_dir: &Path, filename: &str) -> Result<String, String> {
    let invalid = || "日志文件不存在或路径非法".to_string();
    let name = Path::new(filename);
    let mut components = name.components();
    // Reject Windows separators/prefixes on every platform, including ADS syntax.
    if filename.contains(['/', '\\', ':'])
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || name.extension().and_then(|value| value.to_str()) != Some("log")
    {
        return Err(invalid());
    }

    // 根目录必须已存在且为目录；否则与非法/缺失文件统一失败。
    let root = fs::canonicalize(log_dir).map_err(|_| invalid())?;
    if !root.is_dir() {
        return Err(invalid());
    }
    let target = fs::canonicalize(root.join(name)).map_err(|_| invalid())?;
    if target.parent() != Some(root.as_path()) || !target.is_file() {
        return Err(invalid());
    }

    // This constrains resolved paths, not concurrent local reparse-point races.
    fs::read_to_string(target).map_err(|_| "读取日志文件失败".to_string())
}

#[tauri::command]
pub fn get_log_dir_path(app: tauri::AppHandle) -> Result<String, String> {
    let log_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("获取日志目录失败: {}", e))?;

    Ok(log_dir.to_string_lossy().to_string())
}

/// 测试专用：在当前线程临时安装一个 TRACE 级订阅者，把日志写入内存并返回文本。
/// 只捕获被测闭包内同一线程产生的日志，不读取、不写入任何真实日志文件。
#[cfg(test)]
pub(crate) fn capture_logs_for_test(operation: impl FnOnce()) -> String {
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MemoryWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for MemoryWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let buffer = MemoryWriter::default();
    let writer = buffer.clone();
    let subscriber = fmt::Subscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, operation);
    let bytes = buffer.0.lock().unwrap().clone();
    String::from_utf8(bytes).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_helper_records_trace_and_debug_levels() {
        // 自检：确保后续“marker 不存在”断言不是因为捕获器没收到 debug 日志而假通过。
        let logs = capture_logs_for_test(|| {
            tracing::debug!("SYNTHETIC_CAPTURE_DEBUG");
            tracing::trace!("SYNTHETIC_CAPTURE_TRACE");
        });
        assert!(logs.contains("SYNTHETIC_CAPTURE_DEBUG"));
        assert!(logs.contains("SYNTHETIC_CAPTURE_TRACE"));
    }

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("translation-log-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(root.join("logs")).unwrap();
            fs::write(root.join("outside.log"), "SYNTHETIC_OUTSIDE").unwrap();
            fs::write(root.join("logs/app.log"), "SYNTHETIC_LOG").unwrap();
            Self(root)
        }

        fn logs(&self) -> PathBuf {
            self.0.join("logs")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn log_reader_accepts_only_existing_log_basename() {
        let fixture = Fixture::new();
        assert_eq!(
            read_log_file_in_dir(&fixture.logs(), "app.log").unwrap(),
            "SYNTHETIC_LOG"
        );
        fs::create_dir(fixture.logs().join("directory.log")).unwrap();
        fs::write(fixture.logs().join("wrong.txt"), "SYNTHETIC_WRONG").unwrap();
        fs::create_dir(fixture.logs().join("nested")).unwrap();
        fs::write(fixture.logs().join("nested/app.log"), "SYNTHETIC_NESTED").unwrap();
        let outside = fixture.0.join("outside.log");
        let missing_error = read_log_file_in_dir(&fixture.logs(), "missing.log").unwrap_err();
        for filename in [
            "",
            ".",
            "..",
            "../outside.log",
            "..\\outside.log",
            "/outside.log",
            "C:\\outside.log",
            "C:outside.log",
            "\\\\server\\share\\outside.log",
            "\\\\?\\C:\\outside.log",
            "nested/app.log",
            "nested\\app.log",
            "./app.log",
            "wrong.txt",
            "app.LOG",
            "app.log.",
            "app.log ",
            "app.log:stream.log",
            "directory.log",
            outside.to_str().unwrap(),
        ] {
            assert_eq!(
                read_log_file_in_dir(&fixture.logs(), filename).unwrap_err(),
                missing_error,
                "{filename}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn log_reader_rejects_symlink_outside_directory() {
        let fixture = Fixture::new();
        std::os::unix::fs::symlink(
            fixture.0.join("outside.log"),
            fixture.logs().join("link.log"),
        )
        .unwrap();
        assert!(read_log_file_in_dir(&fixture.logs(), "link.log").is_err());
    }
}
