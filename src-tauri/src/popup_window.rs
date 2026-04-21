/// Popup window management: creation, positioning, showing/hiding.
use std::sync::{Arc, RwLock};

use tauri::{Emitter, Listener, Manager};
use tracing::{info, warn};

use crate::app_state::{mark_popup_ready, PopupRuntimeState};

// ─── Window Constants ────────────────────────────────────────────────────────

const POPUP_WIDTH: i32 = 420;
const POPUP_HEIGHT: i32 = 380;
const OFFSET: i32 = 15;    // 鼠标偏移量
const MARGIN: i32 = 20;    // 屏幕边缘留白

// ─── Cursor Position ─────────────────────────────────────────────────────────

pub fn get_cursor_position() -> (i32, i32) {
    use mouse_position::mouse_position::Mouse;
    match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => (x, y),
        Mouse::Error => (100, 100),
    }
}

// ─── Window Management ───────────────────────────────────────────────────────

/// Close the popup window (hide it).
pub fn close_popup_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("popup") {
        window.hide().map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

/// Position the popup near the cursor, with screen-edge awareness.
fn set_popup_position(window: &tauri::WebviewWindow, cursor_pos: (i32, i32)) -> Result<(), String> {
    let mut x = cursor_pos.0 + OFFSET;
    let mut y = cursor_pos.1 + OFFSET;

    match window.current_monitor() {
        Ok(Some(monitor)) => {
            let screen_size = monitor.size();
            let screen_position = monitor.position();

            let screen_width = screen_size.width as i32;
            let screen_height = screen_size.height as i32;
            let screen_x = screen_position.x;
            let screen_y = screen_position.y;

            let usable_right = screen_x + screen_width - MARGIN;
            let usable_bottom = screen_y + screen_height - MARGIN;
            let usable_left = screen_x + MARGIN;
            let usable_top = screen_y + MARGIN;

            // X 轴：优先右侧，不够则左侧
            if x + POPUP_WIDTH > usable_right {
                let left_x = cursor_pos.0 - POPUP_WIDTH - OFFSET;
                if left_x >= usable_left {
                    x = left_x;
                } else {
                    x = usable_right - POPUP_WIDTH;
                }
            }

            // Y 轴：优先下方，不够则上方
            if y + POPUP_HEIGHT > usable_bottom {
                let top_y = cursor_pos.1 - POPUP_HEIGHT - OFFSET;
                if top_y >= usable_top {
                    y = top_y;
                } else {
                    y = usable_bottom - POPUP_HEIGHT;
                }
            }

            x = x.max(usable_left).min(usable_right - POPUP_WIDTH);
            y = y.max(usable_top).min(usable_bottom - POPUP_HEIGHT);

            info!("弹窗位置: 鼠标({}, {}) -> 窗口({}, {})", cursor_pos.0, cursor_pos.1, x, y);
        }
        Ok(None) => {
            warn!("无法获取显示器信息，使用默认偏移");
        }
        Err(e) => {
            warn!("获取显示器信息失败: {}, 使用默认偏移", e);
        }
    }

    window
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }))
        .map_err(|e: tauri::Error| e.to_string())
}

/// Build a new popup window.
pub fn build_popup_window(
    app: &tauri::AppHandle,
    popup_state: Arc<RwLock<PopupRuntimeState>>,
) -> Result<tauri::WebviewWindow, String> {
    let window =
        tauri::WebviewWindowBuilder::new(app, "popup", tauri::WebviewUrl::App("index.html".into()))
            .title("翻译")
            .inner_size(420.0, 380.0)
            .decorations(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .visible(false)
            .initialization_script("window.location.hash = '#/popup';")
            .build()
            .map_err(|e| e.to_string())?;

    let popup_state_clone = popup_state.clone();
    window.listen("popup-ready", move |_event| {
        mark_popup_ready(&popup_state_clone, true);
    });

    Ok(window)
}

/// Get existing popup or create a new one.
pub fn ensure_popup_window(
    app: &tauri::AppHandle,
    popup_state: &Arc<RwLock<PopupRuntimeState>>,
) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window("popup") {
        return Ok(window);
    }

    mark_popup_ready(popup_state, false);
    build_popup_window(app, popup_state.clone())
}

/// Safely execute an action on the popup, deferring if it's not yet ready.
pub fn with_popup_ready<F>(
    app: &tauri::AppHandle,
    popup_state: &Arc<RwLock<PopupRuntimeState>>,
    request_id: u64,
    cursor_pos: (i32, i32),
    action: F,
) -> Result<(), String>
where
    F: Fn(&tauri::WebviewWindow) + Send + Sync + 'static,
{
    use crate::app_state::{is_active_popup_request, is_popup_ready};

    let window = ensure_popup_window(app, popup_state)?;
    set_popup_position(&window, cursor_pos)?;

    if is_popup_ready(popup_state) {
        action(&window);
        return Ok(());
    }

    let popup_state_clone = popup_state.clone();
    let window_clone = window.clone();
    let action = Arc::new(action);
    let deferred_action = action.clone();

    window.once("popup-ready", move |_event| {
        if !is_active_popup_request(&popup_state_clone, request_id) {
            return;
        }

        let _ = set_popup_position(&window_clone, cursor_pos);
        deferred_action(&window_clone);
    });

    Ok(())
}

/// Show a loading state in the popup.
pub fn show_loading_popup(
    app: &tauri::AppHandle,
    popup_state: &Arc<RwLock<PopupRuntimeState>>,
    request_id: u64,
    cursor_pos: (i32, i32),
) -> Result<(), String> {
    use crate::app_state::is_active_popup_request;

    if !is_active_popup_request(popup_state, request_id) {
        return Ok(());
    }

    with_popup_ready(app, popup_state, request_id, cursor_pos, |window| {
        let _ = window.emit("translation-started", ());
        let _ = window.show();
    })
}

/// Emit a translation result/update to the popup and optionally show it.
pub fn show_popup_translation(
    app: &tauri::AppHandle,
    popup_state: &Arc<RwLock<PopupRuntimeState>>,
    request_id: u64,
    event_name: &'static str,
    translation: crate::database::Translation,
    cursor_pos: (i32, i32),
    should_show: bool,
) -> Result<(), String> {
    use crate::app_state::is_active_popup_request;

    if !is_active_popup_request(popup_state, request_id) {
        return Ok(());
    }

    with_popup_ready(app, popup_state, request_id, cursor_pos, move |window| {
        let _ = window.emit(event_name, &translation);
        if should_show {
            let _ = window.show();
        }
    })
}
