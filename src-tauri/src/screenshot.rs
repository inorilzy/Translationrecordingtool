use std::{
    io::Cursor,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        LazyLock, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose, Engine as _};
use screenshots::image::{DynamicImage, ImageOutputFormat};
use serde::{Deserialize, Serialize};
use tauri::Manager;

pub(crate) const MIN_SELECTION_SIZE: f64 = 4.0;

pub(crate) static SELECTION_PAYLOAD: LazyLock<Mutex<Option<SelectionStartPayload>>> =
    LazyLock::new(|| Mutex::new(None));
static SELECTION_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static SELECTION_STARTED_MS: AtomicU64 = AtomicU64::new(0);
const SELECTION_STALE_MS: u64 = 90_000;

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

struct SelectionSessionGuard;

impl SelectionSessionGuard {
    fn try_acquire() -> Result<Self, String> {
        for _ in 0..2 {
            match SELECTION_IN_PROGRESS.compare_exchange(
                false,
                true,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    SELECTION_STARTED_MS.store(now_millis(), Ordering::Release);
                    return Ok(Self);
                }
                Err(_) => {
                    let started = SELECTION_STARTED_MS.load(Ordering::Acquire);
                    let age = now_millis().saturating_sub(started);
                    if started > 0 && age > SELECTION_STALE_MS {
                        // Recover from a stuck native selection session.
                        SELECTION_IN_PROGRESS.store(false, Ordering::Release);
                        SELECTION_STARTED_MS.store(0, Ordering::Release);
                        continue;
                    }
                    return Err("已有截图选择进行中，请先按 ESC 取消当前截图".to_string());
                }
            }
        }

        Err("已有截图选择进行中，请先按 ESC 取消当前截图".to_string())
    }
}

impl Drop for SelectionSessionGuard {
    fn drop(&mut self) {
        SELECTION_IN_PROGRESS.store(false, Ordering::Release);
        SELECTION_STARTED_MS.store(0, Ordering::Release);
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureArea {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub struct CaptureResult {
    pub image_base64: String,
    pub area: CaptureArea,
}




#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionScreenPreview {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    image: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionStartPayload {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    screens: Vec<SelectionScreenPreview>,
}

pub fn get_screenshot_selection_payload() -> Result<SelectionStartPayload, String> {
    selection_payload_slot()
        .lock()
        .map_err(|_| "截图快照状态已损坏，请重试".to_string())?
        .clone()
        .ok_or_else(|| "截图快照尚未准备好，请重试".to_string())
}

pub async fn select_and_capture(app: tauri::AppHandle) -> Result<String, String> {
    Ok(select_and_capture_with_area(app).await?.image_base64)
}

pub async fn select_and_capture_with_area(app: tauri::AppHandle) -> Result<CaptureResult, String> {
    tauri::async_runtime::spawn_blocking(move || select_and_capture_blocking(app))
        .await
        .map_err(|e| format!("截图任务执行失败: {}", e))?
}

fn select_and_capture_blocking(app: tauri::AppHandle) -> Result<CaptureResult, String> {
    let _session = SelectionSessionGuard::try_acquire()?;
    let should_restore_main_window = should_restore_main_window_after_capture(&app);

    #[cfg(windows)]
    let result = select_and_capture_native_blocking();

    #[cfg(not(windows))]
    let result = webview::select_and_capture_with_area(app);

    if should_restore_main_window {
        restore_main_window(&app);
    }
    result
}

#[cfg(windows)]
fn select_and_capture_native_blocking() -> Result<CaptureResult, String> {
    let area =
        windows::select_area()?.ok_or_else(|| "已取消截图选择".to_string())?;
    let image_base64 = capture_area_as_png_data_url(area)?;
    Ok(CaptureResult { image_base64, area })
}


fn restore_main_window(app: &tauri::AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
}

fn should_restore_main_window_after_capture(app: &tauri::AppHandle) -> bool {
    app.get_webview_window("main")
        .map(|main| {
            main.is_visible().unwrap_or(false)
                && !main.is_minimized().unwrap_or(false)
                && main.is_focused().unwrap_or(false)
        })
        .unwrap_or(false)
}






pub(crate) fn selection_payload_slot() -> &'static Mutex<Option<SelectionStartPayload>> {
    &SELECTION_PAYLOAD
}







pub(crate) fn normalize_area(area: CaptureArea) -> Result<CaptureArea, String> {
    if !area.x.is_finite()
        || !area.y.is_finite()
        || !area.width.is_finite()
        || !area.height.is_finite()
        || area.width < MIN_SELECTION_SIZE
        || area.height < MIN_SELECTION_SIZE
    {
        return Err("截图区域太小，请重新框选".to_string());
    }

    Ok(area)
}

pub(crate) fn capture_area_as_png_data_url(area: CaptureArea) -> Result<String, String> {
    #[cfg(windows)]
    {
        return windows::capture_area_as_png_data_url(area);
    }

    #[cfg(not(windows))]
    {
        let x = area.x.round() as i32;
        let y = area.y.round() as i32;
        let width = area.width.round().max(MIN_SELECTION_SIZE) as u32;
        let height = area.height.round().max(MIN_SELECTION_SIZE) as u32;

        let screen =
            Screen::from_point(x, y).map_err(|e| format!("无法定位截图所在屏幕: {}", e))?;
        let relative_x = x - screen.display_info.x;
        let relative_y = y - screen.display_info.y;
        let image = screen
            .capture_area(relative_x, relative_y, width, height)
            .map_err(|e| format!("截取屏幕失败: {}", e))?;

        encode_png_data_url(&DynamicImage::ImageRgba8(image))
    }
}

pub(crate) fn encode_png_data_url(image: &DynamicImage) -> Result<String, String> {
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageOutputFormat::Png)
        .map_err(|e| format!("截图编码失败: {}", e))?;

    Ok(format!(
        "data:image/png;base64,{}",
        general_purpose::STANDARD.encode(bytes.into_inner())
    ))
}


#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
mod webview;


#[cfg(test)]
mod tests {
    use super::{normalize_area, CaptureArea, SelectionSessionGuard, SELECTION_IN_PROGRESS};
    use std::sync::atomic::Ordering;

    #[test]
    fn accepts_valid_capture_area() {
        let area = normalize_area(CaptureArea {
            x: 10.0,
            y: 20.0,
            width: 120.0,
            height: 80.0,
        })
        .unwrap();

        assert_eq!(area.width, 120.0);
        assert_eq!(area.height, 80.0);
    }

    #[test]
    fn rejects_tiny_capture_area() {
        assert!(normalize_area(CaptureArea {
            x: 10.0,
            y: 20.0,
            width: 2.0,
            height: 80.0,
        })
        .is_err());
    }

    #[test]
    fn blocks_concurrent_screenshot_selection_sessions() {
        SELECTION_IN_PROGRESS.store(false, Ordering::Release);

        let first = SelectionSessionGuard::try_acquire().expect("first session should start");
        let second_error = match SelectionSessionGuard::try_acquire() {
            Ok(_) => panic!("second session should be blocked"),
            Err(error) => error,
        };
        assert!(second_error.contains("已有截图选择进行中"));

        drop(first);
        let third = SelectionSessionGuard::try_acquire().expect("session should restart after drop");
        drop(third);
    }
}
