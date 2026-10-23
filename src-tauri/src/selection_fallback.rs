//! Opt-in fallback for applications that do not expose the current selection
//! through UI Automation (for example Chrome PDF pages or canvas-rendered text).
//!
//! This sends Ctrl+C to the foreground window and translates what the target
//! application put on the clipboard. It is only reached when the user enables
//! `clipboard_fallback_enabled` in settings. The previous clipboard contents are
//! intentionally not restored: the old restore transaction could overwrite a
//! copy the user made concurrently. The setting is off by default and the UI
//! warns that the clipboard is replaced.

use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};
use tracing::info;

const CLIPBOARD_WAIT_ATTEMPTS: usize = 24;
const CLIPBOARD_WAIT_DELAY_MS: u64 = 25;

pub fn copy_selection_and_read(app: &tauri::AppHandle) -> Result<String, String> {
    let baseline_sequence = crate::clipboard::clipboard_sequence_number()
        .ok_or_else(|| "当前平台不支持剪贴板回退".to_string())?;

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|error| format!("初始化键盘模拟失败: {}", error))?;

    enigo
        .key(Key::Control, Press)
        .map_err(|error| format!("模拟 Ctrl+C 失败: {}", error))?;
    let click_result = enigo.key(Key::Unicode('c'), Click);
    let release_result = enigo.key(Key::Control, Release);
    click_result.map_err(|error| format!("模拟 Ctrl+C 失败: {}", error))?;
    release_result.map_err(|error| format!("释放 Ctrl 键失败: {}", error))?;

    let text = crate::clipboard::read_clipboard_after_update(
        app,
        Some(baseline_sequence),
        CLIPBOARD_WAIT_ATTEMPTS,
        CLIPBOARD_WAIT_DELAY_MS,
    )?;

    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("剪贴板为空".to_string());
    }

    info!(
        "剪贴板回退读取到选中文本，字符数: {}",
        text.chars().count()
    );
    Ok(text)
}
