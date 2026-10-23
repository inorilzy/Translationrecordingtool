use std::{
    io::Cursor,
    sync::{
        atomic::{AtomicU64, Ordering},
        LazyLock, Mutex,
    },
};

use base64::{engine::general_purpose, Engine as _};
use screenshots::image::{DynamicImage, ImageOutputFormat};
#[cfg(not(windows))]
use screenshots::Screen;
use serde::{Deserialize, Serialize};
use tauri::Manager;

pub(crate) const MIN_SELECTION_SIZE: f64 = 4.0;

pub(crate) static SELECTION_PAYLOAD: LazyLock<Mutex<Option<SelectionStartPayload>>> =
    LazyLock::new(|| Mutex::new(None));
// Only the winning owner may clear the slot (compare-and-swap on this token),
// so a late Drop cannot release a session it no longer owns.
static SELECTION_OWNER: AtomicU64 = AtomicU64::new(0);

struct SelectionSessionGuard {
    owner: u64,
    slot: &'static AtomicU64,
}

impl SelectionSessionGuard {
    fn try_acquire() -> Result<Self, String> {
        Self::acquire(&SELECTION_OWNER)
    }

    // A live session is never released by elapsed time: the Windows native
    // selection loop has no matching timeout and must first exit on its own
    // (confirm or ESC), which drops this guard. An explicit cancellation
    // protocol would have to confirm that before allowing another session.
    fn acquire(owner: &'static AtomicU64) -> Result<Self, String> {
        // 0 means free; once taken, the slot is only released by the owner's
        // Drop. Allocate a unique token by CAS 0 -> 1; a concurrent claimant
        // either sees 1 (busy) or wins the slot with a different token.
        if owner
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("已有截图选择进行中，请先按 ESC 取消当前截图".to_string());
        }
        let token = owner.fetch_add(1, Ordering::AcqRel) + 1;
        Ok(Self {
            owner: token,
            slot: owner,
        })
    }
}

impl Drop for SelectionSessionGuard {
    fn drop(&mut self) {
        let _ = self
            .slot
            .compare_exchange(self.owner, 0, Ordering::AcqRel, Ordering::Acquire);
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
    let result = webview::select_and_capture_with_area(app.clone());

    if should_restore_main_window {
        restore_main_window(&app);
    }
    result
}

#[cfg(windows)]
fn select_and_capture_native_blocking() -> Result<CaptureResult, String> {
    let area = windows::select_area()?.ok_or_else(|| "已取消截图选择".to_string())?;
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
    use super::{normalize_area, CaptureArea, SelectionSessionGuard};
    use std::sync::atomic::{AtomicU64, Ordering};

    // Each test uses its own `static` owner slot, so neither the process-global
    // guard nor another test's session can leak into it.
    macro_rules! scratch_owner {
        ($name:ident) => {
            static $name: AtomicU64 = AtomicU64::new(0);
        };
    }
    scratch_owner!(CONCURRENT_OWNER);
    scratch_owner!(LONG_LIVED_OWNER);
    scratch_owner!(STALE_DROP_OWNER);
    scratch_owner!(DROP_OWNER);

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
        let owner = &CONCURRENT_OWNER;

        let first = SelectionSessionGuard::acquire(owner).expect("first session should start");
        let second_error = match SelectionSessionGuard::acquire(owner) {
            Ok(_) => panic!("second session should be blocked"),
            Err(error) => error,
        };
        assert!(second_error.contains("已有截图选择进行中"));

        drop(first);
        let third =
            SelectionSessionGuard::acquire(owner).expect("session should restart after drop");
        drop(third);
    }

    #[test]
    fn long_lived_session_keeps_blocking_other_requests() {
        let owner = &LONG_LIVED_OWNER;
        let live = SelectionSessionGuard::acquire(owner).unwrap();
        // Simulate a session kept open (e.g. user paused over the overlay) and
        // unrelated requests arriving: elapsed time grants no escape hatch.
        for _ in 0..100 {
            assert!(SelectionSessionGuard::acquire(owner).is_err());
        }
        drop(live);
        assert!(SelectionSessionGuard::acquire(owner).is_ok());
    }

    #[test]
    fn stale_guard_drop_cannot_release_a_newer_owner() {
        let owner = &STALE_DROP_OWNER;
        let old = SelectionSessionGuard::acquire(owner).unwrap();
        let old_token = old.owner;
        // Simulate a forced reassignment (e.g. a future explicit-cancel
        // protocol) that hands the slot to a new generation.
        let replacement_token = 1000;
        owner.store(replacement_token, Ordering::Release);
        let replacement = SelectionSessionGuard {
            owner: replacement_token,
            slot: owner,
        };
        assert_ne!(replacement.owner, old_token);
        drop(old);
        // The stale Drop must not free the newer owner's slot, and must not
        // manufacture a token that a third request could accidentally own.
        assert_eq!(owner.load(Ordering::Acquire), replacement_token);
        assert!(SelectionSessionGuard::acquire(owner).is_err());
        drop(replacement);
        assert_eq!(owner.load(Ordering::Acquire), 0);
        assert!(SelectionSessionGuard::acquire(owner).is_ok());
    }

    #[test]
    fn dropped_guard_does_not_clear_an_unrelated_new_session() {
        let owner = &DROP_OWNER;
        let first = SelectionSessionGuard::acquire(owner).unwrap();
        assert_ne!(first.owner, 0, "free marker 0 must never be an owner token");
        drop(first);
        let second = SelectionSessionGuard::acquire(owner).unwrap();
        assert_eq!(owner.load(Ordering::Acquire), second.owner);
        drop(second);
        assert_eq!(owner.load(Ordering::Acquire), 0);
    }
}
