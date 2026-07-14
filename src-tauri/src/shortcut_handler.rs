use std::sync::Arc;

use parking_lot::RwLock;
use tauri::{Emitter, Manager};
use tracing::{error, info, warn};

use crate::{
    app_state::{
        is_active_popup_request, next_popup_request_id, PopupRuntimeState,
    },
    popup_window::{
        close_popup_window, get_cursor_position, point_anchor, rect_anchor,
        show_loading_popup_with_message, show_popup_translation, PopupAnchor,
    },
    translation_workflow::{AppTranslationWorkflow, WorkflowStage},
};

#[derive(Debug, PartialEq, Eq)]
enum SelectionTextSource {
    UiAutomation,
    ClipboardFallback,
}

pub fn register_shortcut_handler(
    app: &tauri::AppHandle,
    shortcut_value: &str,
    popup_state: Arc<RwLock<PopupRuntimeState>>,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    let shortcut: Shortcut = shortcut_value
        .parse()
        .map_err(|error| format!("无效的快捷键格式: {}", error))?;

    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                handle_shortcut(app.clone(), popup_state.clone());
            }
        })
        .map_err(|error| format!("注册快捷键失败: {}", error))
}

pub fn register_screenshot_shortcut_handler(
    app: &tauri::AppHandle,
    shortcut_value: &str,
    popup_state: Arc<RwLock<PopupRuntimeState>>,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    let shortcut: Shortcut = shortcut_value
        .parse()
        .map_err(|error| format!("无效的截图快捷键格式: {}", error))?;

    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                handle_screenshot_shortcut(app.clone(), popup_state.clone());
            }
        })
        .map_err(|error| format!("注册截图快捷键失败: {}", error))
}

pub fn handle_shortcut(
    app: tauri::AppHandle,
    popup_state: Arc<RwLock<PopupRuntimeState>>,
) {
    tauri::async_runtime::spawn(async move {
        info!("开始执行选中文本翻译流程");
        let request_id = next_popup_request_id(&popup_state);
        let anchor = point_anchor(get_cursor_position());
        let text = match read_selected_text_with_fallback(&app) {
            Ok((text, source)) => {
                info!("读取到选中文本({:?}): {}", source, text);
                text
            }
            Err(error) => {
                error!("读取选中文本失败: {}", error);
                close_if_active(&app, &popup_state, request_id);
                return;
            }
        };

        let workflow = app.state::<AppTranslationWorkflow>();
        let mut visible_result = false;
        let mut report = |stage| {
            present_popup_stage(
                &app,
                &popup_state,
                request_id,
                anchor,
                &mut visible_result,
                stage,
                false,
            );
        };
        let is_cancelled = || !is_active_popup_request(&popup_state, request_id);

        if let Err(error) = workflow
            .translate_text(&text, &mut report, &is_cancelled)
            .await
        {
            if is_active_popup_request(&popup_state, request_id) {
                error!("选中文本翻译失败: {}", error);
            }
        }
    });
}

pub fn handle_screenshot_shortcut(
    app: tauri::AppHandle,
    popup_state: Arc<RwLock<PopupRuntimeState>>,
) {
    tauri::async_runtime::spawn(async move {
        info!("开始执行截图 OCR 翻译流程");
        let request_id = next_popup_request_id(&popup_state);
        let capture = match crate::screenshot::select_and_capture_with_area(app.clone()).await {
            Ok(capture) => capture,
            Err(error) => {
                warn!("截图选择取消或失败: {}", error);
                close_if_active(&app, &popup_state, request_id);
                return;
            }
        };

        if !is_active_popup_request(&popup_state, request_id) {
            return;
        }

        let anchor = rect_anchor(capture.area);
        let _ = show_loading_popup_with_message(
            &app,
            &popup_state,
            request_id,
            anchor,
            "正在准备 OCR...",
        );

        let workflow = app.state::<AppTranslationWorkflow>();
        let mut visible_result = false;
        let mut report = |stage| {
            present_popup_stage(
                &app,
                &popup_state,
                request_id,
                anchor,
                &mut visible_result,
                stage,
                true,
            );
        };
        let is_cancelled = || !is_active_popup_request(&popup_state, request_id);

        if let Err(error) = workflow
            .translate_image(&capture.image_base64, &mut report, &is_cancelled)
            .await
        {
            if is_active_popup_request(&popup_state, request_id) {
                error!("截图 OCR 翻译失败: {}", error);
            }
        }
    });
}

fn present_popup_stage(
    app: &tauri::AppHandle,
    popup_state: &Arc<RwLock<PopupRuntimeState>>,
    request_id: u64,
    anchor: PopupAnchor,
    visible_result: &mut bool,
    stage: WorkflowStage,
    backfill_ocr_text: bool,
) {
    if !is_active_popup_request(popup_state, request_id) {
        return;
    }

    match stage {
        WorkflowStage::OcrInProgress => {
            let _ = show_loading_popup_with_message(
                app,
                popup_state,
                request_id,
                anchor,
                "OCR 识别中...",
            );
        }
        WorkflowStage::InputAccepted { text } => {
            if backfill_ocr_text {
                if let Some(main_window) = app.get_webview_window("main") {
                    let _ = main_window.emit("ocr-source-text", text);
                }
            }
        }
        WorkflowStage::LocalResultAvailable(record) => {
            *visible_result = true;
            let _ = show_popup_translation(
                app,
                popup_state,
                request_id,
                "translation-result",
                record,
                anchor,
                true,
            );
        }
        WorkflowStage::EnrichmentAvailable(record) => {
            *visible_result = true;
            let _ = show_popup_translation(
                app,
                popup_state,
                request_id,
                "translation-update",
                record,
                anchor,
                false,
            );
        }
        WorkflowStage::RemoteTranslationInProgress => {
            if !*visible_result {
                let _ = show_loading_popup_with_message(
                    app,
                    popup_state,
                    request_id,
                    anchor,
                    "翻译中...",
                );
            }
        }
        WorkflowStage::Completed(record) => {
            if !*visible_result {
                *visible_result = true;
                let _ = show_popup_translation(
                    app,
                    popup_state,
                    request_id,
                    "translation-result",
                    record,
                    anchor,
                    true,
                );
            }
        }
        WorkflowStage::Cancelled | WorkflowStage::Failed { .. } => {
            close_if_active(app, popup_state, request_id);
        }
    }
}

fn read_selected_text_with_fallback(
    app: &tauri::AppHandle,
) -> Result<(String, SelectionTextSource), String> {
    select_text_source(crate::selection_reader::read_selected_text(), || {
        copy_selection_and_read_clipboard(app)
    })
}

fn select_text_source<F>(
    ui_automation_result: Result<String, String>,
    clipboard_fallback: F,
) -> Result<(String, SelectionTextSource), String>
where
    F: FnOnce() -> Result<String, String>,
{
    match ui_automation_result {
        Ok(text) if !text.trim().is_empty() => Ok((
            text.trim().to_string(),
            SelectionTextSource::UiAutomation,
        )),
        Ok(_) => {
            warn!("UI Automation 未读取到选中文本，回退到剪贴板复制");
            clipboard_fallback().map(|text| {
                (
                    text.trim().to_string(),
                    SelectionTextSource::ClipboardFallback,
                )
            })
        }
        Err(error) => {
            warn!(
                "UI Automation 读取选中文本失败，回退到剪贴板复制: {}",
                error
            );
            clipboard_fallback().map(|text| {
                (
                    text.trim().to_string(),
                    SelectionTextSource::ClipboardFallback,
                )
            })
        }
    }
}

fn copy_selection_and_read_clipboard(app: &tauri::AppHandle) -> Result<String, String> {
    let previous_clipboard = crate::clipboard::read_clipboard(app).ok();
    let baseline_clipboard_sequence = crate::clipboard::clipboard_sequence_number();

    std::thread::sleep(std::time::Duration::from_millis(50));
    use enigo::{Enigo, Key, Keyboard, Settings};
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|error| format!("初始化键盘模拟失败: {}", error))?;
    enigo.key(Key::Control, enigo::Direction::Press).ok();
    enigo.key(Key::Unicode('c'), enigo::Direction::Click).ok();
    enigo.key(Key::Control, enigo::Direction::Release).ok();

    let selected_text =
        crate::clipboard::read_clipboard_after_update(app, baseline_clipboard_sequence, 6, 80);

    if let Some(previous_text) = previous_clipboard {
        if let Err(error) = crate::clipboard::write_clipboard(app, &previous_text) {
            warn!("恢复原剪贴板失败: {}", error);
        }
    }

    selected_text
}

fn close_if_active(
    app: &tauri::AppHandle,
    popup_state: &Arc<RwLock<PopupRuntimeState>>,
    request_id: u64,
) {
    if is_active_popup_request(popup_state, request_id) {
        let _ = close_popup_window(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn ui_automation_result_is_preferred_without_touching_clipboard() {
        let fallback_calls = AtomicUsize::new(0);

        let result = select_text_source(Ok("  direct text  ".to_string()), || {
            fallback_calls.fetch_add(1, Ordering::SeqCst);
            Ok("clipboard text".to_string())
        })
        .unwrap();

        assert_eq!(result, ("direct text".to_string(), SelectionTextSource::UiAutomation));
        assert_eq!(fallback_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn empty_ui_automation_result_uses_clipboard_fallback() {
        let result = select_text_source(Ok("   ".to_string()), || {
            Ok("  clipboard text  ".to_string())
        })
        .unwrap();

        assert_eq!(
            result,
            (
                "clipboard text".to_string(),
                SelectionTextSource::ClipboardFallback
            )
        );
    }

    #[test]
    fn ui_automation_failure_uses_clipboard_fallback() {
        let result = select_text_source(Err("unsupported".to_string()), || {
            Ok("clipboard text".to_string())
        })
        .unwrap();

        assert_eq!(result.1, SelectionTextSource::ClipboardFallback);
    }
}
