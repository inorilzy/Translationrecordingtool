/// Popup window management: creation, positioning, showing/hiding.
use std::sync::{Arc, RwLock};

use serde::Serialize;
use tauri::{Emitter, Listener, Manager};
use tracing::{info, warn};

use crate::app_state::{mark_popup_ready, PopupRuntimeState};

// ─── Window Constants ────────────────────────────────────────────────────────

pub const POPUP_WIDTH: i32 = 420;
pub const POPUP_HEIGHT: i32 = 380;
pub const OFFSET: i32 = 15; // 鼠标偏移量
pub const MARGIN: i32 = 20; // 屏幕边缘留白
const RECT_ANCHOR_GAP: i32 = 12;

// ─── Pure Positioning Helper (Testable) ──────────────────────────────────────

/// Minimal monitor info needed for positioning — decoupled from Tauri's Monitor type.
#[derive(Debug, Clone, Copy)]
pub struct MonitorInfo {
    pub width: i32,
    pub height: i32,
    pub x: i32,
    pub y: i32,
}

/// Computed popup position with metadata about edge adjustments.
#[derive(Debug, Clone, Copy)]
pub struct PopupPositionInfo {
    pub x: i32,
    pub y: i32,
    pub adjusted_for_edge: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum PopupAnchor {
    Point {
        x: i32,
        y: i32,
    },
    Rect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadingPopupPayload {
    message: String,
}

impl PopupAnchor {
    fn reference_point(self) -> (i32, i32) {
        match self {
            Self::Point { x, y } => (x, y),
            Self::Rect {
                x,
                y,
                width,
                height,
            } => (x + width / 2, y + height / 2),
        }
    }
}

/// Calculate popup position near the cursor, with screen-edge awareness.
/// Pure function — no I/O, no Tauri dependencies — suitable for unit testing.
pub fn calculate_popup_position(
    cursor_pos: (i32, i32),
    popup_width: i32,
    popup_height: i32,
    offset: i32,
    margin: i32,
    monitor: Option<MonitorInfo>,
) -> PopupPositionInfo {
    calculate_popup_position_for_anchor(
        PopupAnchor::Point {
            x: cursor_pos.0,
            y: cursor_pos.1,
        },
        popup_width,
        popup_height,
        offset,
        margin,
        monitor,
    )
}

pub fn calculate_popup_position_for_anchor(
    anchor: PopupAnchor,
    popup_width: i32,
    popup_height: i32,
    offset: i32,
    margin: i32,
    monitor: Option<MonitorInfo>,
) -> PopupPositionInfo {
    let (anchor_x, anchor_y) = anchor.reference_point();
    let mut adjusted_for_edge = false;

    let Some(m) = monitor else {
        return PopupPositionInfo {
            x: anchor_x + offset,
            y: anchor_y + offset,
            adjusted_for_edge,
        };
    };

    let usable_left = m.x + margin;
    let usable_top = m.y + margin;
    let usable_right = m.x + m.width - margin;
    let usable_bottom = m.y + m.height - margin;

    let candidates = popup_position_candidates(anchor, popup_width, popup_height, offset);
    let preferred_position = candidates
        .first()
        .map(|&(x, y, _)| (x, y))
        .unwrap_or((anchor_x + offset, anchor_y + offset));
    let (raw_x, raw_y, selected_preference) = candidates
        .into_iter()
        .min_by_key(|&(x, y, preference)| {
            overflow_score(
                x,
                y,
                popup_width,
                popup_height,
                usable_left,
                usable_top,
                usable_right,
                usable_bottom,
            ) * 100
                + preference
        })
        .unwrap_or((anchor_x + offset, anchor_y + offset, 0));

    let x = raw_x.clamp(usable_left, (usable_right - popup_width).max(usable_left));
    let y = raw_y.clamp(usable_top, (usable_bottom - popup_height).max(usable_top));
    adjusted_for_edge = selected_preference != 0
        || (raw_x, raw_y) != preferred_position
        || x != raw_x
        || y != raw_y;

    PopupPositionInfo {
        x,
        y,
        adjusted_for_edge,
    }
}

fn popup_position_candidates(
    anchor: PopupAnchor,
    popup_width: i32,
    popup_height: i32,
    offset: i32,
) -> Vec<(i32, i32, i32)> {
    match anchor {
        PopupAnchor::Point { x, y } => vec![
            (x + offset, y + offset, 0),
            (x - popup_width - offset, y + offset, 1),
            (x + offset, y - popup_height - offset, 2),
            (x - popup_width - offset, y - popup_height - offset, 3),
        ],
        PopupAnchor::Rect {
            x,
            y,
            width,
            height,
        } => {
            let right = x + width;
            let bottom = y + height;
            let center_x = x + width / 2;
            let center_y = y + height / 2;
            vec![
                (center_x - popup_width / 2, bottom + RECT_ANCHOR_GAP, 0),
                (
                    center_x - popup_width / 2,
                    y - popup_height - RECT_ANCHOR_GAP,
                    1,
                ),
                (right + RECT_ANCHOR_GAP, center_y - popup_height / 2, 2),
                (
                    x - popup_width - RECT_ANCHOR_GAP,
                    center_y - popup_height / 2,
                    3,
                ),
                (right + RECT_ANCHOR_GAP, y, 4),
                (x - popup_width - RECT_ANCHOR_GAP, y, 5),
            ]
        }
    }
}

fn overflow_score(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
) -> i32 {
    let overflow_left = (left - x).max(0);
    let overflow_top = (top - y).max(0);
    let overflow_right = (x + width - right).max(0);
    let overflow_bottom = (y + height - bottom).max(0);
    overflow_left + overflow_top + overflow_right + overflow_bottom
}

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

/// Position the popup near the anchor, with screen-edge awareness.
fn set_popup_position(window: &tauri::WebviewWindow, anchor: PopupAnchor) -> Result<(), String> {
    let monitor_info = monitor_for_anchor(window, anchor);

    let pos = calculate_popup_position_for_anchor(
        anchor,
        POPUP_WIDTH,
        POPUP_HEIGHT,
        OFFSET,
        MARGIN,
        monitor_info,
    );

    if pos.adjusted_for_edge {
        info!(
            "弹窗位置已调整（靠近屏幕边缘）: 锚点{:?} -> 窗口({}, {})",
            anchor, pos.x, pos.y
        );
    } else {
        info!("弹窗位置: 锚点{:?} -> 窗口({}, {})", anchor, pos.x, pos.y);
    }

    window
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: pos.x,
            y: pos.y,
        }))
        .map_err(|e: tauri::Error| e.to_string())
}

fn monitor_for_anchor(window: &tauri::WebviewWindow, anchor: PopupAnchor) -> Option<MonitorInfo> {
    let monitors = match window.available_monitors() {
        Ok(monitors) => monitors,
        Err(e) => {
            warn!("获取显示器列表失败: {}, 使用默认偏移", e);
            return None;
        }
    };

    let (anchor_x, anchor_y) = anchor.reference_point();
    monitors
        .iter()
        .map(|monitor| {
            let size = monitor.size();
            let pos = monitor.position();
            MonitorInfo {
                width: size.width as i32,
                height: size.height as i32,
                x: pos.x,
                y: pos.y,
            }
        })
        .min_by_key(|monitor| monitor_distance_score(*monitor, anchor_x, anchor_y))
}

fn monitor_distance_score(monitor: MonitorInfo, x: i32, y: i32) -> i32 {
    let left = monitor.x;
    let top = monitor.y;
    let right = monitor.x + monitor.width;
    let bottom = monitor.y + monitor.height;
    let dx = if x < left {
        left - x
    } else if x > right {
        x - right
    } else {
        0
    };
    let dy = if y < top {
        top - y
    } else if y > bottom {
        y - bottom
    } else {
        0
    };
    dx + dy
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
            .decorations(false)
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
    anchor: PopupAnchor,
    action: F,
) -> Result<(), String>
where
    F: Fn(&tauri::WebviewWindow) + Send + Sync + 'static,
{
    use crate::app_state::{is_active_popup_request, is_popup_ready};

    let window = ensure_popup_window(app, popup_state)?;
    set_popup_position(&window, anchor)?;

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

        let _ = set_popup_position(&window_clone, anchor);
        deferred_action(&window_clone);
    });

    Ok(())
}

/// Show a loading state in the popup.
pub fn show_loading_popup(
    app: &tauri::AppHandle,
    popup_state: &Arc<RwLock<PopupRuntimeState>>,
    request_id: u64,
    anchor: PopupAnchor,
) -> Result<(), String> {
    show_loading_popup_with_message(app, popup_state, request_id, anchor, "翻译中...")
}

/// Show or update a loading state in the popup with stage-specific text.
pub fn show_loading_popup_with_message(
    app: &tauri::AppHandle,
    popup_state: &Arc<RwLock<PopupRuntimeState>>,
    request_id: u64,
    anchor: PopupAnchor,
    message: impl Into<String>,
) -> Result<(), String> {
    use crate::app_state::is_active_popup_request;

    if !is_active_popup_request(popup_state, request_id) {
        return Ok(());
    }

    let payload = LoadingPopupPayload {
        message: message.into(),
    };

    with_popup_ready(app, popup_state, request_id, anchor, move |window| {
        let _ = window.emit("translation-started", &payload);
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
    anchor: PopupAnchor,
    should_show: bool,
) -> Result<(), String> {
    use crate::app_state::is_active_popup_request;

    if !is_active_popup_request(popup_state, request_id) {
        return Ok(());
    }

    with_popup_ready(app, popup_state, request_id, anchor, move |window| {
        let _ = window.emit(event_name, &translation);
        if should_show {
            let _ = window.show();
        }
    })
}

pub fn point_anchor(cursor_pos: (i32, i32)) -> PopupAnchor {
    PopupAnchor::Point {
        x: cursor_pos.0,
        y: cursor_pos.1,
    }
}

pub fn rect_anchor(area: crate::screenshot::CaptureArea) -> PopupAnchor {
    PopupAnchor::Rect {
        x: area.x.round() as i32,
        y: area.y.round() as i32,
        width: area.width.round() as i32,
        height: area.height.round() as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        calculate_popup_position, calculate_popup_position_for_anchor, monitor_distance_score,
        MonitorInfo, PopupAnchor,
    };

    fn monitor() -> MonitorInfo {
        MonitorInfo {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        }
    }

    #[test]
    fn point_anchor_prefers_bottom_right_when_space_allows() {
        let pos = calculate_popup_position((300, 200), 420, 380, 15, 20, Some(monitor()));

        assert_eq!(pos.x, 315);
        assert_eq!(pos.y, 215);
        assert!(!pos.adjusted_for_edge);
    }

    #[test]
    fn point_anchor_flips_away_from_screen_edges() {
        let pos = calculate_popup_position((1880, 1040), 420, 380, 15, 20, Some(monitor()));

        assert_eq!(pos.x, 1445);
        assert_eq!(pos.y, 645);
        assert!(pos.adjusted_for_edge);
    }

    #[test]
    fn rect_anchor_prefers_below_center_for_screenshot_results() {
        let pos = calculate_popup_position_for_anchor(
            PopupAnchor::Rect {
                x: 760,
                y: 300,
                width: 360,
                height: 160,
            },
            420,
            380,
            15,
            20,
            Some(monitor()),
        );

        assert_eq!(pos.x, 730);
        assert_eq!(pos.y, 472);
        assert!(!pos.adjusted_for_edge);
    }

    #[test]
    fn rect_anchor_uses_above_when_screenshot_is_near_bottom() {
        let pos = calculate_popup_position_for_anchor(
            PopupAnchor::Rect {
                x: 980,
                y: 770,
                width: 360,
                height: 180,
            },
            420,
            380,
            15,
            20,
            Some(monitor()),
        );

        assert_eq!(pos.x, 950);
        assert_eq!(pos.y, 378);
        assert!(pos.adjusted_for_edge);
    }

    #[test]
    fn monitor_distance_score_is_zero_when_anchor_is_inside_monitor() {
        assert_eq!(monitor_distance_score(monitor(), 1200, 800), 0);
        assert!(monitor_distance_score(monitor(), 2500, 800) > 0);
    }
}
