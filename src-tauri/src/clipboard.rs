// 工单09：划词翻译已取消“UI Automation 失败后自动模拟 Ctrl+C 并恢复剪贴板”的回退。
// 原先仅供该回退使用的快照备份、所有权 token、恢复事务实现已删除；
// 本模块只保留用户主动触发的剪贴板读写能力。

pub fn read_clipboard(app: &tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    app.clipboard()
        .read_text()
        .map_err(|e| format!("读取剪贴板失败: {}", e))
}

pub fn write_clipboard(app: &tauri::AppHandle, text: &str) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    app.clipboard()
        .write_text(text)
        .map_err(|e| format!("写入剪贴板失败: {}", e))
}

pub fn clipboard_sequence_number() -> Option<u32> {
    #[cfg(target_os = "windows")]
    {
        Some(unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() })
    }

    #[cfg(target_os = "macos")]
    {
        // NSPasteboard.changeCount is the macOS analogue of the Win32
        // clipboard sequence number: it increments on every content change.
        Some(objc2_app_kit::NSPasteboard::generalPasteboard().changeCount() as u32)
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        None
    }
}

pub fn read_clipboard_after_update(
    app: &tauri::AppHandle,
    baseline_sequence: Option<u32>,
    max_attempts: usize,
    retry_delay_ms: u64,
) -> Result<String, String> {
    wait_for_clipboard_update(
        || clipboard_sequence_number(),
        || read_clipboard(app),
        baseline_sequence,
        max_attempts,
        Some(retry_delay_ms),
    )
}

fn wait_for_clipboard_update<S, R>(
    mut sequence_reader: S,
    mut read_once: R,
    baseline_sequence: Option<u32>,
    max_attempts: usize,
    retry_delay_ms: Option<u64>,
) -> Result<String, String>
where
    S: FnMut() -> Option<u32>,
    R: FnMut() -> Result<String, String>,
{
    let attempts = max_attempts.max(1);
    let mut last_error = "读取剪贴板失败: 剪贴板尚未更新".to_string();

    for attempt in 0..attempts {
        let clipboard_updated = match (baseline_sequence, sequence_reader()) {
            (Some(before), Some(after)) => after != before,
            _ => true,
        };

        if !clipboard_updated {
            last_error = "读取剪贴板失败: 剪贴板尚未更新".to_string();
        } else {
            match read_once() {
                Ok(text) if !text.trim().is_empty() => return Ok(text.trim().to_string()),
                Ok(_) => {
                    last_error = "读取剪贴板失败: 剪贴板为空".to_string();
                }
                Err(error) => {
                    last_error = error;
                }
            }
        }

        if attempt + 1 < attempts {
            if let Some(delay_ms) = retry_delay_ms {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
        }
    }

    Err(last_error)
}

#[cfg(test)]
mod tests {
    use super::wait_for_clipboard_update;
    use std::cell::Cell;

    #[test]
    fn waits_for_clipboard_sequence_before_accepting_text() {
        let sequences = [7_u32, 7, 8];
        let sequence_checks = Cell::new(0);
        let reads = Cell::new(0);

        let result = wait_for_clipboard_update(
            || {
                let current = sequence_checks.get();
                sequence_checks.set(current + 1);
                let index = current.min(sequences.len() - 1);
                Some(sequences[index])
            },
            || {
                let current = reads.get();
                reads.set(current + 1);
                match current {
                    0 => Ok("address".to_string()),
                    _ => Ok("unexpected".to_string()),
                }
            },
            Some(7),
            3,
            None,
        );

        assert_eq!(result.as_deref(), Ok("address"));
        assert_eq!(sequence_checks.get(), 3);
        assert_eq!(reads.get(), 1);
    }

    #[test]
    fn returns_error_when_clipboard_never_updates() {
        let sequence = Cell::new(12_u32);
        let reads = Cell::new(0);

        let result = wait_for_clipboard_update(
            || Some(sequence.get()),
            || {
                reads.set(reads.get() + 1);
                Ok("previous".to_string())
            },
            Some(12),
            3,
            None,
        );

        assert_eq!(result, Err("读取剪贴板失败: 剪贴板尚未更新".to_string()));
        assert_eq!(reads.get(), 0);
    }
}
