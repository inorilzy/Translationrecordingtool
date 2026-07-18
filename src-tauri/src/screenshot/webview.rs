//! Non-Windows WebView screenshot selection adapter.
#![cfg(not(windows))]

use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use base64::{engine::general_purpose, Engine as _};
use screenshots::image::{DynamicImage, ImageOutputFormat};
use screenshots::Screen;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use tauri::{Emitter, Listener, Manager, PhysicalPosition, PhysicalSize, Position, Size};

use super::{
    encode_png_data_url, normalize_area, CaptureArea, CaptureResult, SelectionScreenPreview,
    SelectionStartPayload, MIN_SELECTION_SIZE, selection_payload_slot,
};

const SELECTION_WINDOW_LABEL: &str = "screenshot-selection";
const SELECTION_TIMEOUT: Duration = Duration::from_secs(60);
const SELECTION_READY_TIMEOUT: Duration = Duration::from_secs(5);
const OVERLAY_HIDE_DELAY: Duration = Duration::from_millis(120);

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "area", rename_all = "camelCase")]
enum SelectionEvent {
    Completed(CaptureArea),
    Cancelled,
}

#[derive(Debug, Clone, Copy)]
struct VirtualDesktopBounds {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

struct SelectionWindow {
    window: tauri::WebviewWindow,
    is_new: bool,
}

pub fn select_and_capture_with_area(app: tauri::AppHandle) -> Result<CaptureResult, String> {
    let area = match wait_for_selection(&app) {
        Ok(area) => area,
        Err(error) => return Err(error),
    };

    thread::sleep(OVERLAY_HIDE_DELAY);
    let image_base64 = super::capture_area_as_png_data_url(area)?;
    Ok(CaptureResult { image_base64, area })
}

fn wait_for_selection(app: &tauri::AppHandle) -> Result<CaptureArea, String> {
    let selection_payload = create_selection_start_payload()?;
    store_selection_payload(selection_payload)?;

    let selection_window = ensure_selection_window(app)?;
    let window = selection_window.window;
    let ready_result = if selection_window.is_new {
        wait_for_selection_window_ready(&window)
    } else {
        reload_selection_window(&window)
    };

    if let Err(error) = ready_result {
        let _ = window.hide();
        clear_selection_payload();
        return Err(error);
    }

    let (tx, rx) = mpsc::channel::<SelectionEvent>();

    let completed_tx = tx.clone();
    let completed_listener = window.listen("screenshot-selection-completed", move |event| {
        let payload = event.payload();
        match serde_json::from_str::<CaptureArea>(payload) {
            Ok(area) => {
                let _ = completed_tx.send(SelectionEvent::Completed(area));
            }
            Err(_) => {
                let _ = completed_tx.send(SelectionEvent::Cancelled);
            }
        }
    });

    let cancelled_tx = tx.clone();
    let cancelled_listener = window.listen("screenshot-selection-cancelled", move |_event| {
        let _ = cancelled_tx.send(SelectionEvent::Cancelled);
    });

    if let Err(error) = window.show().and_then(|_| window.set_focus()) {
        window.unlisten(completed_listener);
        window.unlisten(cancelled_listener);
        let _ = window.hide();
        clear_selection_payload();
        return Err(error.to_string());
    }

    let started_at = Instant::now();
    let result = loop {
        let remaining = SELECTION_TIMEOUT
            .checked_sub(started_at.elapsed())
            .unwrap_or_default();

        if remaining.is_zero() {
            break Err("截图已超时，请重试".to_string());
        }

        match rx.recv_timeout(remaining) {
            Ok(SelectionEvent::Completed(area)) => break normalize_area(area),
            Ok(SelectionEvent::Cancelled) => break Err("已取消截图选择".to_string()),
            Err(mpsc::RecvTimeoutError::Timeout) => break Err("截图已超时，请重试".to_string()),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break Err("截图选择窗口已关闭".to_string())
            }
        }
    };

    window.unlisten(completed_listener);
    window.unlisten(cancelled_listener);
    let _ = window.hide();
    clear_selection_payload();

    result
}

fn ensure_selection_window(app: &tauri::AppHandle) -> Result<SelectionWindow, String> {
    let bounds = virtual_desktop_bounds()?;

    if let Some(window) = app.get_webview_window(SELECTION_WINDOW_LABEL) {
        position_selection_window(&window, bounds)?;
        return Ok(SelectionWindow {
            window,
            is_new: false,
        });
    }

    let window = tauri::WebviewWindowBuilder::new(
        app,
        SELECTION_WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("截图 OCR")
    .position(bounds.x as f64, bounds.y as f64)
    .inner_size(bounds.width as f64, bounds.height as f64)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .transparent(false)
    .visible(false)
    .initialization_script("window.location.hash = '#/screenshot-selection';")
    .build()
    .map_err(|e| e.to_string())?;

    position_selection_window(&window, bounds)?;

    Ok(SelectionWindow {
        window,
        is_new: true,
    })
}

fn position_selection_window(
    window: &tauri::WebviewWindow,
    bounds: VirtualDesktopBounds,
) -> Result<(), String> {
    window
        .set_position(Position::Physical(PhysicalPosition {
            x: bounds.x,
            y: bounds.y,
        }))
        .map_err(|e| e.to_string())?;
    window
        .set_size(Size::Physical(PhysicalSize {
            width: bounds.width,
            height: bounds.height,
        }))
        .map_err(|e| e.to_string())
}

fn wait_for_selection_window_ready(window: &tauri::WebviewWindow) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<()>();
    let listener = window.listen("screenshot-selection-ready", move |_event| {
        let _ = tx.send(());
    });

    let result = rx
        .recv_timeout(SELECTION_READY_TIMEOUT)
        .map_err(|_| "截图选择窗口启动超时，请重试".to_string());

    window.unlisten(listener);
    result
}

fn reload_selection_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<()>();
    let listener = window.listen("screenshot-selection-ready", move |_event| {
        let _ = tx.send(());
    });

    let emit_result = window
        .emit("screenshot-selection-reload", ())
        .map_err(|e| e.to_string());

    let result = emit_result.and_then(|_| {
        rx.recv_timeout(SELECTION_READY_TIMEOUT)
            .map_err(|_| "截图选择窗口刷新超时，请重试".to_string())
    });

    window.unlisten(listener);
    result
}

fn store_selection_payload(payload: SelectionStartPayload) -> Result<(), String> {
    *selection_payload_slot()
        .lock()
        .map_err(|_| "截图快照状态已损坏，请重试".to_string())? = Some(payload);
    Ok(())
}

fn clear_selection_payload() {
    if let Ok(mut payload) = selection_payload_slot().lock() {
        *payload = None;
    }
}

fn virtual_desktop_bounds() -> Result<VirtualDesktopBounds, String> {
    let screens = Screen::all().map_err(|e| format!("无法获取屏幕信息: {}", e))?;
    virtual_desktop_bounds_from_screens(&screens)
}

fn virtual_desktop_bounds_from_screens(screens: &[Screen]) -> Result<VirtualDesktopBounds, String> {
    let bounds = screens
        .iter()
        .map(|screen| {
            let info = screen.display_info;

            (
                info.x,
                info.y,
                info.x.saturating_add(info.width as i32),
                info.y.saturating_add(info.height as i32),
            )
        })
        .reduce(|acc, item| {
            (
                acc.0.min(item.0),
                acc.1.min(item.1),
                acc.2.max(item.2),
                acc.3.max(item.3),
            )
        })
        .ok_or_else(|| "未检测到可截图的屏幕".to_string())?;

    if bounds.2 <= bounds.0 || bounds.3 <= bounds.1 {
        return Err("屏幕区域无效，无法启动截图选择".to_string());
    }

    Ok(VirtualDesktopBounds {
        x: bounds.0,
        y: bounds.1,
        width: (bounds.2 - bounds.0) as u32,
        height: (bounds.3 - bounds.1) as u32,
    })
}

fn create_selection_start_payload() -> Result<SelectionStartPayload, String> {
    let screens = Screen::all().map_err(|e| format!("无法获取屏幕信息: {}", e))?;
    let bounds = virtual_desktop_bounds_from_screens(&screens)?;

    let previews = screens
        .iter()
        .map(capture_screen_preview)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SelectionStartPayload {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
        screens: previews,
    })
}

fn capture_screen_preview(screen: &Screen) -> Result<SelectionScreenPreview, String> {
    let info = screen.display_info;
    let image = screen
        .capture()
        .map_err(|e| format!("截取屏幕预览失败: {}", e))?;
    let image = DynamicImage::ImageRgba8(image);
    let image = encode_jpeg_data_url(&image, 78)?;

    Ok(SelectionScreenPreview {
        x: info.x,
        y: info.y,
        width: info.width,
        height: info.height,
        image,
    })
}

fn encode_jpeg_data_url(image: &DynamicImage, quality: u8) -> Result<String, String> {
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageOutputFormat::Jpeg(quality))
        .map_err(|e| format!("截图预览编码失败: {}", e))?;

    Ok(format!(
        "data:image/jpeg;base64,{}",
        general_purpose::STANDARD.encode(bytes.into_inner())
    ))
}

