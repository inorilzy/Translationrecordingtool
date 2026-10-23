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

/// `Ok(None)` 表示目标应用没有响应 Ctrl+C 或复制出空内容，
/// 通常就是“当前没有选中任何文本”，调用方应静默忽略。
pub fn copy_selection_and_read(app: &tauri::AppHandle) -> Result<Option<String>, String> {
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

    let text = match crate::clipboard::read_clipboard_after_update(
        app,
        Some(baseline_sequence),
        CLIPBOARD_WAIT_ATTEMPTS,
        CLIPBOARD_WAIT_DELAY_MS,
    ) {
        Ok(text) => text,
        Err(error) => {
            // 剪贴板序号没变：目标应用没有复制任何东西，视为未选中文本。
            if crate::clipboard::clipboard_sequence_number() == Some(baseline_sequence) {
                info!("剪贴板回退未检测到复制内容，视为未选中文本");
                return Ok(None);
            }
            // 序号变了但内容为空（例如复制了图片）：同样视为没有可翻译文本。
            if error.contains("剪贴板为空") {
                info!("剪贴板回退复制内容为空，视为未选中文本");
                return Ok(None);
            }
            return Err(error);
        }
    };

    let text = text.trim().to_string();
    if text.is_empty() {
        return Ok(None);
    }

    info!(
        "剪贴板回退读取到选中文本，字符数: {}",
        text.chars().count()
    );
    Ok(Some(text))
}
