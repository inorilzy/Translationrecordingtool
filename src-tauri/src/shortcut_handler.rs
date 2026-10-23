use std::sync::Arc;

use parking_lot::RwLock;
use tauri::{Emitter, Manager};
use tracing::{error, info, warn};

use crate::{
    app_state::{is_active_popup_request, next_popup_request_id, PopupRuntimeState},
    popup_window::{
        close_popup_window, get_cursor_position, point_anchor, rect_anchor,
        show_loading_popup_with_message, show_popup_failure, show_popup_translation, PopupAnchor,
    },
    settings::SettingsRecord,
    translation_workflow::{AppTranslationWorkflow, WorkflowStage},
};

/// UI Automation 无法给出选文时的弹窗提示。
/// 工单09：不再自动模拟 Ctrl+C 读取/恢复剪贴板，改由用户手动复制后走现有剪贴板翻译。
const MANUAL_COPY_HINT: &str = "未能直接读取选中文本（当前应用可能不支持 UI Automation）。\
请手动复制后在主窗口使用“剪贴板翻译”，或在设置中开启“UI Automation 读取失败时，自动复制并翻译”。";

/// 已开启剪贴板回退但模拟复制本身出错时的提示。
const FALLBACK_FAILED_HINT: &str = "未能读取选中文本：UI Automation 不可用，且自动复制失败。\
请手动复制后在主窗口使用“剪贴板翻译”。";

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

pub fn handle_shortcut(app: tauri::AppHandle, popup_state: Arc<RwLock<PopupRuntimeState>>) {
    tauri::async_runtime::spawn(async move {
        info!("开始执行选中文本翻译流程");
        let anchor = point_anchor(get_cursor_position());
        // UI Automation remains the default path. The clipboard fallback is an
        // explicit opt-in and is the only path that injects Ctrl+C or touches
        // the clipboard; it never restores previous clipboard contents.
        let fallback_enabled = app
            .state::<Arc<RwLock<SettingsRecord>>>()
            .read()
            .clipboard_fallback_enabled;
        let uia_result = read_selection_for_popup(crate::selection_reader::read_selected_text);
        let outcome = match uia_result {
            Ok(Some(text)) => Ok(Some(text)),
            // UIA 读不到或选区为空时，若用户开启了回退，则尝试复制。
            // 部分应用（如 Chrome PDF）支持 TextPattern 却报告空选区，所以空选区也要走回退。
            _ if fallback_enabled => {
                info!("UI Automation 未读到选区，尝试剪贴板回退");
                crate::selection_fallback::copy_selection_and_read(&app).map_err(|error| {
                    warn!("剪贴板回退失败: {}", error);
                    FALLBACK_FAILED_HINT
                })
            }
            other => other,
        };

        let text = match outcome {
            Ok(Some(text)) => text,
            Ok(None) => {
                // 什么都没选中：静默忽略，不弹出翻译窗口。
                info!("未检测到选中文本，忽略本次划词快捷键");
                return;
            }
            Err(message) => {
                let request_id = next_popup_request_id(&popup_state);
                let _ = show_popup_failure(&app, &popup_state, request_id, anchor, message);
                return;
            }
        };
        let request_id = next_popup_request_id(&popup_state);

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
                log_translation_failure("选中文本", &error);
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
                log_translation_failure("截图 OCR", &error);
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
        WorkflowStage::Cancelled => {
            close_if_active(app, popup_state, request_id);
        }
        WorkflowStage::Failed { message } => {
            if let Err(popup_error) =
                show_popup_failure(app, popup_state, request_id, anchor, message)
            {
                error!("显示翻译失败状态失败: {}", popup_error);
            }
        }
    }
}

/// 划词入口的 UI Automation 读取：本函数自身绝不回退到模拟按键或剪贴板。
/// 成功返回去空白文本；控件支持读取但选区为空返回 `Ok(None)`（视为未选中，静默忽略）；
/// 控件不支持读取返回手动复制提示。
/// 日志只记录字符数与 UIA 错误（UIA 错误为系统/控件诊断，不含选中内容）。
fn read_selection_for_popup<R>(read_ui_automation: R) -> Result<Option<String>, &'static str>
where
    R: FnOnce() -> Result<String, String>,
{
    match read_ui_automation() {
        Ok(text) if !text.trim().is_empty() => {
            let text = text.trim().to_string();
            info!(
                "UI Automation 读取到选中文本，字符数: {}",
                text.chars().count()
            );
            Ok(Some(text))
        }
        Ok(_) => {
            info!("UI Automation 未读取到选中文本，视为未选中");
            Ok(None)
        }
        Err(error) => {
            warn!("UI Automation 读取选中文本失败，提示用户手动复制: {}", error);
            Err(MANUAL_COPY_HINT)
        }
    }
}

/// 翻译失败消息可能拼入原文（例如“未找到单词 "xxx" 的释义”），因此这里只记录范围与长度。
/// 具体原因已通过弹窗展示给用户；provider 层另有已去除 URL/响应体的错误日志。
fn log_translation_failure(scope: &str, message: &str) {
    error!(
        "{}翻译失败，错误消息字符数: {}",
        scope,
        message.chars().count()
    );
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
    use crate::logger::capture_logs_for_test;
    use std::cell::Cell;

    const SELECTION_MARKER: &str = "SYNTHETIC_SELECTION_MARKER_7f3a";

    #[test]
    fn ui_automation_success_returns_trimmed_text() {
        let calls = Cell::new(0);
        let result = read_selection_for_popup(|| {
            calls.set(calls.get() + 1);
            Ok("  direct text  ".to_string())
        });
        assert_eq!(result, Ok(Some("direct text".to_string())));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn empty_ui_automation_result_means_nothing_selected() {
        for empty in ["", "   ", "\r\n\t"] {
            assert_eq!(read_selection_for_popup(|| Ok(empty.to_string())), Ok(None));
        }
    }

    #[test]
    fn ui_automation_failure_asks_for_manual_copy() {
        assert_eq!(
            read_selection_for_popup(|| Err("当前控件不支持直接读取选中文本".to_string())),
            Err(MANUAL_COPY_HINT)
        );
    }

    #[test]
    fn manual_copy_hint_points_to_existing_clipboard_translation() {
        assert!(MANUAL_COPY_HINT.contains("手动复制"));
        assert!(MANUAL_COPY_HINT.contains("剪贴板翻译"));
        assert!(FALLBACK_FAILED_HINT.contains("剪贴板翻译"));
    }

    /// 默认划词路径静态守卫：不得出现模拟按键或剪贴板备份/恢复实现。
    /// 关键字用 concat! 拼接，避免本测试自身命中；只检查 #[cfg(test)] 之前的生产代码。
    #[test]
    fn default_selection_path_has_no_key_injection_or_clipboard_restore() {
        let forbidden = [
            concat!("en", "igo"),
            concat!("Send", "Input"),
            concat!("keybd", "_event"),
            concat!("Key::", "Control"),
            concat!("backup_", "clipboard"),
            concat!("restore_", "clipboard"),
            concat!("Empty", "Clipboard"),
            concat!("SetClipboard", "Data"),
            concat!("Clipboard", "Fallback"),
        ];
        let sources = [
            ("shortcut_handler.rs", include_str!("shortcut_handler.rs")),
            ("clipboard.rs", include_str!("clipboard.rs")),
            ("selection_reader.rs", include_str!("selection_reader.rs")),
            ("translation_workflow.rs", include_str!("translation_workflow.rs")),
            ("lib.rs", include_str!("lib.rs")),
        ];
        for (name, source) in sources {
            let production = source.split("#[cfg(test)]").next().unwrap();
            for token in forbidden {
                assert!(!production.contains(token), "{name} 仍包含 {token}");
            }
        }
    }

    /// 剪贴板回退是显式 opt-in：只允许在新模块里模拟 Ctrl+C，
    /// 且永远不备份/恢复旧剪贴板内容。
    #[test]
    fn clipboard_fallback_is_opt_in_and_never_restores_clipboard() {
        let source = include_str!("selection_fallback.rs");
        assert!(source.contains("Key::Control"));
        assert!(source.contains("Unicode('c')"));
        for forbidden in [
            concat!("backup_", "clipboard"),
            concat!("restore_", "clipboard"),
            concat!("Empty", "Clipboard"),
            concat!("SetClipboard", "Data"),
            concat!("write_", "clipboard"),
        ] {
            assert!(!source.contains(forbidden), "回退模块包含 {forbidden}");
        }

        let handler = include_str!("shortcut_handler.rs");
        let production = handler.split("#[cfg(test)]").next().unwrap();
        assert!(production.contains("clipboard_fallback_enabled"));
        assert!(production.contains("if fallback_enabled"));
        assert!(production.contains("selection_fallback::copy_selection_and_read"));
    }

    #[test]
    fn selection_logs_omit_synthetic_marker() {
        let logs = capture_logs_for_test(|| {
            let text = format!("  {SELECTION_MARKER} private text  ");
            assert!(read_selection_for_popup(|| Ok(text)).is_ok());
            assert_eq!(read_selection_for_popup(|| Ok(String::new())), Ok(None));
            assert!(read_selection_for_popup(|| Err("uia failed".into())).is_err());
            log_translation_failure(
                "选中文本",
                &format!("未找到单词 \"{SELECTION_MARKER}\" 的释义"),
            );
        });
        // 确认日志确实被捕获，避免“空日志”导致假通过。
        assert!(logs.contains("字符数"));
        assert!(logs.contains("视为未选中"));
        assert!(logs.contains("提示用户手动复制"));
        assert!(!logs.contains(SELECTION_MARKER), "{logs}");
    }
}
