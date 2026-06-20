use std::{
    io::Cursor,
    sync::{Mutex, OnceLock},
};

#[cfg(not(windows))]
use std::thread;
#[cfg(not(windows))]
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use base64::{engine::general_purpose, Engine as _};
use screenshots::image::{DynamicImage, ImageOutputFormat};
#[cfg(not(windows))]
use screenshots::Screen;
use serde::{Deserialize, Serialize};
use tauri::Manager;
#[cfg(not(windows))]
use tauri::{Emitter, Listener, PhysicalPosition, PhysicalSize, Position, Size};

#[cfg(not(windows))]
const SELECTION_WINDOW_LABEL: &str = "screenshot-selection";
#[cfg(not(windows))]
const SELECTION_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(not(windows))]
const SELECTION_READY_TIMEOUT: Duration = Duration::from_secs(5);
const MIN_SELECTION_SIZE: f64 = 4.0;
#[cfg(not(windows))]
const OVERLAY_HIDE_DELAY: Duration = Duration::from_millis(120);

static SELECTION_PAYLOAD: OnceLock<Mutex<Option<SelectionStartPayload>>> = OnceLock::new();

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

#[cfg(not(windows))]
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "area", rename_all = "camelCase")]
enum SelectionEvent {
    Completed(CaptureArea),
    Cancelled,
}

#[cfg(not(windows))]
#[derive(Debug, Clone, Copy)]
struct VirtualDesktopBounds {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[cfg(not(windows))]
struct SelectionWindow {
    window: tauri::WebviewWindow,
    is_new: bool,
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
    let should_restore_main_window = should_restore_main_window_after_capture(&app);

    #[cfg(windows)]
    let result = select_and_capture_native_blocking();

    #[cfg(not(windows))]
    let result = select_and_capture_webview_blocking(&app);

    if should_restore_main_window {
        restore_main_window(&app);
    }
    result
}

#[cfg(windows)]
fn select_and_capture_native_blocking() -> Result<CaptureResult, String> {
    let area =
        windows_native_selection::select_area()?.ok_or_else(|| "已取消截图选择".to_string())?;
    let image_base64 = capture_area_as_png_data_url(area)?;
    Ok(CaptureResult { image_base64, area })
}

#[cfg(not(windows))]
fn select_and_capture_webview_blocking(app: &tauri::AppHandle) -> Result<CaptureResult, String> {
    let area = match wait_for_selection(&app) {
        Ok(area) => area,
        Err(error) => return Err(error),
    };

    thread::sleep(OVERLAY_HIDE_DELAY);
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

#[cfg(not(windows))]
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

#[cfg(not(windows))]
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

#[cfg(not(windows))]
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

#[cfg(not(windows))]
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

#[cfg(not(windows))]
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

fn selection_payload_slot() -> &'static Mutex<Option<SelectionStartPayload>> {
    SELECTION_PAYLOAD.get_or_init(|| Mutex::new(None))
}

#[cfg(not(windows))]
fn store_selection_payload(payload: SelectionStartPayload) -> Result<(), String> {
    *selection_payload_slot()
        .lock()
        .map_err(|_| "截图快照状态已损坏，请重试".to_string())? = Some(payload);
    Ok(())
}

#[cfg(not(windows))]
fn clear_selection_payload() {
    if let Ok(mut payload) = selection_payload_slot().lock() {
        *payload = None;
    }
}

#[cfg(not(windows))]
fn virtual_desktop_bounds() -> Result<VirtualDesktopBounds, String> {
    let screens = Screen::all().map_err(|e| format!("无法获取屏幕信息: {}", e))?;
    virtual_desktop_bounds_from_screens(&screens)
}

#[cfg(not(windows))]
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

#[cfg(not(windows))]
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

#[cfg(not(windows))]
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

fn normalize_area(area: CaptureArea) -> Result<CaptureArea, String> {
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

fn capture_area_as_png_data_url(area: CaptureArea) -> Result<String, String> {
    #[cfg(windows)]
    {
        return windows_native_selection::capture_area_as_png_data_url(area);
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

fn encode_png_data_url(image: &DynamicImage) -> Result<String, String> {
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, ImageOutputFormat::Png)
        .map_err(|e| format!("截图编码失败: {}", e))?;

    Ok(format!(
        "data:image/png;base64,{}",
        general_purpose::STANDARD.encode(bytes.into_inner())
    ))
}

#[cfg(not(windows))]
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

#[cfg(windows)]
mod windows_native_selection {
    use std::{
        cmp::{max, min},
        ffi::c_void,
        mem::{size_of, zeroed},
        ptr::{null, null_mut},
        slice,
        sync::mpsc,
    };

    use screenshots::image::{DynamicImage, ImageBuffer, Rgba};
    use windows_sys::Win32::{
        Foundation::{COLORREF, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM},
        Graphics::Gdi::{
            BitBlt, CreateCompatibleDC, CreateDIBSection, CreateSolidBrush, DeleteDC, DeleteObject,
            SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
            BLENDFUNCTION, CAPTUREBLT, DIB_RGB_COLORS, HBITMAP, HBRUSH, HDC, HGDIOBJ, RGBQUAD,
            SRCCOPY,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            HiDpi::{SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
            Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE, VK_RETURN},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
                GetSystemMetrics, GetWindowLongPtrW, LoadCursorW, PostQuitMessage,
                RegisterClassExW, SetCursor, SetForegroundWindow, SetLayeredWindowAttributes,
                SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, UnregisterClassW,
                UpdateLayeredWindow, CREATESTRUCTW, CS_DBLCLKS, GWLP_USERDATA, HCURSOR, IDC_CROSS,
                IDC_HAND, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE,
                LWA_ALPHA, MSG, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
                SM_YVIRTUALSCREEN, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW,
                SW_HIDE, SW_SHOW, ULW_ALPHA, WM_KEYDOWN, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
                WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_NCDESTROY, WM_RBUTTONDOWN,
                WM_SETCURSOR, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
                WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
            },
        },
    };

    use super::{encode_png_data_url, normalize_area, CaptureArea, MIN_SELECTION_SIZE};

    const INPUT_CLASS_NAME: &str = "TranslationToolNativeScreenshotInput";
    const DIM_CLASS_NAME: &str = "TranslationToolNativeScreenshotDim";
    const ACCENT_CLASS_NAME: &str = "TranslationToolNativeScreenshotAccent";
    const TOOLBAR_CLASS_NAME: &str = "TranslationToolNativeScreenshotToolbar";
    const WINDOW_TITLE: &str = "Screenshot Selection";
    const MIN_NATIVE_SELECTION_SIZE: i32 = 4;
    const INPUT_ALPHA: u8 = 1;
    const OVERLAY_ALPHA: u8 = 82;
    const BORDER_THICKNESS: i32 = 1;
    const HANDLE_VISUAL_SIZE: i32 = 6;
    const HANDLE_HIT_RADIUS: i32 = 9;
    const TOOLBAR_WIDTH: i32 = 70;
    const TOOLBAR_HEIGHT: i32 = 36;
    const TOOLBAR_PADDING: i32 = 4;
    const TOOLBAR_BUTTON_SIZE: i32 = 28;
    const TOOLBAR_BUTTON_GAP: i32 = 6;
    const TOOLBAR_MARGIN: i32 = 8;

    #[derive(Debug, Clone, Copy)]
    struct NativeBounds {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }

    #[derive(Debug, Clone, Copy, Default)]
    struct NativePoint {
        x: i32,
        y: i32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct NativeRect {
        left: i32,
        top: i32,
        width: i32,
        height: i32,
    }

    impl NativeRect {
        fn from_points(start: NativePoint, end: NativePoint) -> Self {
            let left = min(start.x, end.x);
            let top = min(start.y, end.y);
            Self {
                left,
                top,
                width: (start.x - end.x).abs(),
                height: (start.y - end.y).abs(),
            }
        }

        fn right(self) -> i32 {
            self.left + self.width
        }

        fn bottom(self) -> i32 {
            self.top + self.height
        }

        fn contains(self, point: NativePoint) -> bool {
            point.x >= self.left
                && point.x <= self.right()
                && point.y >= self.top
                && point.y <= self.bottom()
        }

        fn is_valid(self) -> bool {
            self.width >= MIN_NATIVE_SELECTION_SIZE && self.height >= MIN_NATIVE_SELECTION_SIZE
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ResizeHandle {
        Nw,
        N,
        Ne,
        E,
        Se,
        S,
        Sw,
        W,
    }

    impl ResizeHandle {
        fn has_north(self) -> bool {
            matches!(self, Self::Nw | Self::N | Self::Ne)
        }

        fn has_south(self) -> bool {
            matches!(self, Self::Sw | Self::S | Self::Se)
        }

        fn has_west(self) -> bool {
            matches!(self, Self::Nw | Self::W | Self::Sw)
        }

        fn has_east(self) -> bool {
            matches!(self, Self::Ne | Self::E | Self::Se)
        }
    }

    #[derive(Debug, Clone, Copy)]
    enum Interaction {
        Idle,
        Drawing {
            start: NativePoint,
        },
        Moving {
            start: NativePoint,
            rect: NativeRect,
        },
        Resizing {
            start: NativePoint,
            rect: NativeRect,
            handle: ResizeHandle,
        },
    }

    #[derive(Clone, Copy)]
    struct NativeCursors {
        cross: HCURSOR,
        hand: HCURSOR,
        move_all: HCURSOR,
        size_we: HCURSOR,
        size_ns: HCURSOR,
        size_nwse: HCURSOR,
        size_nesw: HCURSOR,
    }

    struct OverlayBitmap {
        mem_dc: HDC,
        bitmap: HBITMAP,
        old_bitmap: HGDIOBJ,
        bits: *mut c_void,
        width: i32,
        height: i32,
    }

    impl Drop for OverlayBitmap {
        fn drop(&mut self) {
            unsafe {
                if !self.mem_dc.is_null() && !self.old_bitmap.is_null() {
                    SelectObject(self.mem_dc, self.old_bitmap);
                }
                if !self.bitmap.is_null() {
                    DeleteObject(self.bitmap as HGDIOBJ);
                }
                if !self.mem_dc.is_null() {
                    DeleteDC(self.mem_dc);
                }
            }
        }
    }

    struct OverlayVisuals {
        dim_windows: [HWND; 4],
        border_windows: [HWND; 4],
        handle_windows: [HWND; 8],
        toolbar_window: HWND,
        toolbar_bitmap: OverlayBitmap,
    }

    impl Drop for OverlayVisuals {
        fn drop(&mut self) {
            unsafe {
                for hwnd in self
                    .dim_windows
                    .iter()
                    .chain(self.border_windows.iter())
                    .chain(self.handle_windows.iter())
                {
                    if !hwnd.is_null() {
                        DestroyWindow(*hwnd);
                    }
                }

                if !self.toolbar_window.is_null() {
                    DestroyWindow(self.toolbar_window);
                }
            }
        }
    }

    impl OverlayVisuals {
        unsafe fn new(
            hinstance: HINSTANCE,
            dim_class_name: *const u16,
            accent_class_name: *const u16,
            toolbar_class_name: *const u16,
        ) -> Result<Self, String> {
            let dim_windows = create_window_array::<4>(hinstance, dim_class_name, OVERLAY_ALPHA)?;
            let border_windows = create_window_array::<4>(hinstance, accent_class_name, 255)?;
            let handle_windows = create_window_array::<8>(hinstance, accent_class_name, 255)?;
            let toolbar_window = create_bitmap_window(hinstance, toolbar_class_name)?;
            let toolbar_bitmap = OverlayBitmap::new(TOOLBAR_WIDTH, TOOLBAR_HEIGHT)?;

            Ok(Self {
                dim_windows,
                border_windows,
                handle_windows,
                toolbar_window,
                toolbar_bitmap,
            })
        }
    }

    struct NativeSelectionState {
        bounds: NativeBounds,
        visuals: Option<OverlayVisuals>,
        cursors: NativeCursors,
        selection: Option<NativeRect>,
        interaction: Interaction,
        last_point: NativePoint,
        toolbar_rect: Option<NativeRect>,
        result_sender: Option<mpsc::Sender<Result<Option<CaptureArea>, String>>>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ToolbarButton {
        Cancel,
        Confirm,
    }

    #[derive(Clone, Copy)]
    struct Color {
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    }

    pub fn select_area() -> Result<Option<CaptureArea>, String> {
        let previous_dpi_context =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

        let result = unsafe { run_selection_window() };

        if !previous_dpi_context.is_null() {
            unsafe {
                SetThreadDpiAwarenessContext(previous_dpi_context);
            }
        }

        result
    }

    pub fn capture_area_as_png_data_url(area: CaptureArea) -> Result<String, String> {
        let area = normalize_area(area)?;
        unsafe { capture_area_with_gdi(area) }
    }

    unsafe fn run_selection_window() -> Result<Option<CaptureArea>, String> {
        let bounds = NativeBounds::from_system_metrics()?;
        let hinstance = GetModuleHandleW(null()) as HINSTANCE;
        let input_class_name = to_wide(INPUT_CLASS_NAME);
        let dim_class_name = to_wide(DIM_CLASS_NAME);
        let accent_class_name = to_wide(ACCENT_CLASS_NAME);
        let toolbar_class_name = to_wide(TOOLBAR_CLASS_NAME);
        let window_title = to_wide(WINDOW_TITLE);

        let dim_brush = CreateSolidBrush(colorref(0, 0, 0));
        let accent_brush = CreateSolidBrush(colorref(31, 143, 255));

        register_input_class(hinstance, input_class_name.as_ptr());
        register_static_class(hinstance, dim_class_name.as_ptr(), dim_brush);
        register_static_class(hinstance, accent_class_name.as_ptr(), accent_brush);
        register_static_class(hinstance, toolbar_class_name.as_ptr(), null_mut());

        let (tx, rx) = mpsc::channel();
        let visuals = OverlayVisuals::new(
            hinstance,
            dim_class_name.as_ptr(),
            accent_class_name.as_ptr(),
            toolbar_class_name.as_ptr(),
        )?;
        let state = Box::new(NativeSelectionState::new(bounds, visuals, tx));
        let state_ptr = Box::into_raw(state);

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            input_class_name.as_ptr(),
            window_title.as_ptr(),
            WS_POPUP | WS_VISIBLE,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            null_mut(),
            null_mut(),
            hinstance,
            state_ptr as *const c_void,
        );

        if hwnd.is_null() {
            drop(Box::from_raw(state_ptr));
            return Err("无法创建截图选择窗口".to_string());
        }

        SetLayeredWindowAttributes(hwnd, 0, INPUT_ALPHA, LWA_ALPHA);

        if let Some(state) = state_from_hwnd(hwnd) {
            render_overlay(hwnd, state);
        }

        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);

        let mut message: MSG = zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        UnregisterClassW(input_class_name.as_ptr(), hinstance);
        UnregisterClassW(dim_class_name.as_ptr(), hinstance);
        UnregisterClassW(accent_class_name.as_ptr(), hinstance);
        UnregisterClassW(toolbar_class_name.as_ptr(), hinstance);
        if !dim_brush.is_null() {
            DeleteObject(dim_brush as HGDIOBJ);
        }
        if !accent_brush.is_null() {
            DeleteObject(accent_brush as HGDIOBJ);
        }

        rx.try_recv().unwrap_or(Ok(None))
    }

    unsafe fn register_input_class(hinstance: HINSTANCE, class_name: *const u16) {
        let mut window_class: WNDCLASSEXW = zeroed();
        window_class.cbSize = size_of::<WNDCLASSEXW>() as u32;
        window_class.style = CS_DBLCLKS;
        window_class.lpfnWndProc = Some(window_proc);
        window_class.hInstance = hinstance;
        window_class.hCursor = LoadCursorW(null_mut(), IDC_CROSS);
        window_class.lpszClassName = class_name;
        RegisterClassExW(&window_class);
    }

    unsafe fn register_static_class(hinstance: HINSTANCE, class_name: *const u16, brush: HBRUSH) {
        let mut window_class: WNDCLASSEXW = zeroed();
        window_class.cbSize = size_of::<WNDCLASSEXW>() as u32;
        window_class.lpfnWndProc = Some(DefWindowProcW);
        window_class.hInstance = hinstance;
        window_class.hCursor = LoadCursorW(null_mut(), IDC_CROSS);
        window_class.hbrBackground = brush;
        window_class.lpszClassName = class_name;
        RegisterClassExW(&window_class);
    }

    unsafe fn create_window_array<const N: usize>(
        hinstance: HINSTANCE,
        class_name: *const u16,
        alpha: u8,
    ) -> Result<[HWND; N], String> {
        let mut windows = [null_mut(); N];
        for hwnd in windows.iter_mut() {
            *hwnd = create_visual_window(hinstance, class_name, alpha)?;
        }
        Ok(windows)
    }

    unsafe fn create_visual_window(
        hinstance: HINSTANCE,
        class_name: *const u16,
        alpha: u8,
    ) -> Result<HWND, String> {
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
            class_name,
            null(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            hinstance,
            null_mut(),
        );

        if hwnd.is_null() {
            return Err("无法创建截图选择视觉层".to_string());
        }

        SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA);
        ShowWindow(hwnd, SW_HIDE);
        Ok(hwnd)
    }

    unsafe fn create_bitmap_window(
        hinstance: HINSTANCE,
        class_name: *const u16,
    ) -> Result<HWND, String> {
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
            class_name,
            null(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            hinstance,
            null_mut(),
        );

        if hwnd.is_null() {
            return Err("无法创建截图选择工具条".to_string());
        }

        ShowWindow(hwnd, SW_HIDE);
        Ok(hwnd)
    }

    fn colorref(r: u8, g: u8, b: u8) -> COLORREF {
        (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
    }

    impl NativeBounds {
        unsafe fn from_system_metrics() -> Result<Self, String> {
            let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);

            if width <= 0 || height <= 0 {
                return Err("屏幕区域无效，无法启动截图选择".to_string());
            }

            Ok(Self {
                x,
                y,
                width,
                height,
            })
        }
    }

    impl NativeSelectionState {
        unsafe fn new(
            bounds: NativeBounds,
            visuals: OverlayVisuals,
            result_sender: mpsc::Sender<Result<Option<CaptureArea>, String>>,
        ) -> Self {
            Self {
                bounds,
                visuals: Some(visuals),
                cursors: NativeCursors::load(),
                selection: None,
                interaction: Interaction::Idle,
                last_point: NativePoint::default(),
                toolbar_rect: None,
                result_sender: Some(result_sender),
            }
        }

        fn send_result(&mut self, result: Result<Option<CaptureArea>, String>) {
            if let Some(sender) = self.result_sender.take() {
                let _ = sender.send(result);
            }
        }
    }

    impl OverlayBitmap {
        unsafe fn new(width: i32, height: i32) -> Result<Self, String> {
            let screen_dc = windows_sys::Win32::Graphics::Gdi::GetDC(null_mut());
            if screen_dc.is_null() {
                return Err("无法获取屏幕绘图上下文".to_string());
            }

            let mem_dc = CreateCompatibleDC(screen_dc);
            if mem_dc.is_null() {
                windows_sys::Win32::Graphics::Gdi::ReleaseDC(null_mut(), screen_dc);
                return Err("无法创建截图选择绘图上下文".to_string());
            }

            let mut bitmap_info = bitmap_info(width, height);
            let mut bits: *mut c_void = null_mut();
            let bitmap = CreateDIBSection(
                screen_dc,
                &mut bitmap_info,
                DIB_RGB_COLORS,
                &mut bits,
                null_mut::<c_void>() as HANDLE,
                0,
            );
            windows_sys::Win32::Graphics::Gdi::ReleaseDC(null_mut(), screen_dc);

            if bitmap.is_null() || bits.is_null() {
                DeleteDC(mem_dc);
                return Err("无法创建截图选择透明图层".to_string());
            }

            let old_bitmap = SelectObject(mem_dc, bitmap as HGDIOBJ);

            Ok(Self {
                mem_dc,
                bitmap,
                old_bitmap,
                bits,
                width,
                height,
            })
        }

        unsafe fn pixels_mut(&mut self) -> &mut [u8] {
            slice::from_raw_parts_mut(
                self.bits as *mut u8,
                (self.width * self.height * 4) as usize,
            )
        }
    }

    impl NativeCursors {
        unsafe fn load() -> Self {
            Self {
                cross: LoadCursorW(null_mut(), IDC_CROSS),
                hand: LoadCursorW(null_mut(), IDC_HAND),
                move_all: LoadCursorW(null_mut(), IDC_SIZEALL),
                size_we: LoadCursorW(null_mut(), IDC_SIZEWE),
                size_ns: LoadCursorW(null_mut(), IDC_SIZENS),
                size_nwse: LoadCursorW(null_mut(), IDC_SIZENWSE),
                size_nesw: LoadCursorW(null_mut(), IDC_SIZENESW),
            }
        }

        fn for_handle(self, handle: ResizeHandle) -> HCURSOR {
            match handle {
                ResizeHandle::N | ResizeHandle::S => self.size_ns,
                ResizeHandle::E | ResizeHandle::W => self.size_we,
                ResizeHandle::Nw | ResizeHandle::Se => self.size_nwse,
                ResizeHandle::Ne | ResizeHandle::Sw => self.size_nesw,
            }
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_NCCREATE => {
                let create = lparam as *const CREATESTRUCTW;
                if create.is_null() {
                    return 0;
                }

                let state_ptr = (*create).lpCreateParams as *mut NativeSelectionState;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
                1
            }
            WM_NCDESTROY => {
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut NativeSelectionState;
                if !state_ptr.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    let mut state = Box::from_raw(state_ptr);
                    state.send_result(Ok(None));
                }
                PostQuitMessage(0);
                0
            }
            WM_LBUTTONDOWN => {
                if let Some(state) = state_from_hwnd(hwnd) {
                    let point = clamp_point(lparam_point(lparam), state.bounds);
                    state.last_point = point;
                    if let Some(button) = toolbar_button_at(state, point) {
                        match button {
                            ToolbarButton::Cancel => {
                                state.send_result(Ok(None));
                                DestroyWindow(hwnd);
                            }
                            ToolbarButton::Confirm => complete_selection(hwnd, state),
                        }
                        return 0;
                    }
                    begin_interaction(state, point);
                    SetCapture(hwnd);
                    SetCursor(cursor_for_state(state));
                    render_overlay(hwnd, state);
                }
                0
            }
            WM_MOUSEMOVE => {
                if let Some(state) = state_from_hwnd(hwnd) {
                    let point = clamp_point(lparam_point(lparam), state.bounds);
                    let previous_selection = state.selection;
                    state.last_point = point;
                    update_interaction(state, point);
                    SetCursor(cursor_for_state(state));
                    if state.selection != previous_selection {
                        render_overlay(hwnd, state);
                    }
                }
                0
            }
            WM_LBUTTONUP => {
                if let Some(state) = state_from_hwnd(hwnd) {
                    ReleaseCapture();
                    if matches!(state.interaction, Interaction::Drawing { .. })
                        && !state.selection.map(NativeRect::is_valid).unwrap_or(false)
                    {
                        state.selection = None;
                    }
                    state.interaction = Interaction::Idle;
                    SetCursor(cursor_for_state(state));
                    render_overlay(hwnd, state);
                }
                0
            }
            WM_LBUTTONDBLCLK => {
                if let Some(state) = state_from_hwnd(hwnd) {
                    complete_selection(hwnd, state);
                }
                0
            }
            WM_RBUTTONDOWN => {
                if let Some(state) = state_from_hwnd(hwnd) {
                    state.send_result(Ok(None));
                    DestroyWindow(hwnd);
                }
                0
            }
            WM_KEYDOWN => {
                if let Some(state) = state_from_hwnd(hwnd) {
                    match wparam as u16 {
                        key if key == VK_ESCAPE => {
                            state.send_result(Ok(None));
                            DestroyWindow(hwnd);
                        }
                        key if key == VK_RETURN => complete_selection(hwnd, state),
                        _ => {}
                    }
                }
                0
            }
            WM_SETCURSOR => {
                if let Some(state) = state_from_hwnd(hwnd) {
                    SetCursor(cursor_for_state(state));
                    return 1;
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    unsafe fn state_from_hwnd(hwnd: HWND) -> Option<&'static mut NativeSelectionState> {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut NativeSelectionState;
        ptr.as_mut()
    }

    fn begin_interaction(state: &mut NativeSelectionState, point: NativePoint) {
        if let Some(rect) = state.selection {
            if let Some(handle) = hit_resize_handle(rect, point) {
                state.interaction = Interaction::Resizing {
                    start: point,
                    rect,
                    handle,
                };
                return;
            }

            if rect.contains(point) {
                state.interaction = Interaction::Moving { start: point, rect };
                return;
            }
        }

        state.selection = Some(NativeRect {
            left: point.x,
            top: point.y,
            width: 0,
            height: 0,
        });
        state.interaction = Interaction::Drawing { start: point };
    }

    fn update_interaction(state: &mut NativeSelectionState, point: NativePoint) {
        match state.interaction {
            Interaction::Idle => {}
            Interaction::Drawing { start } => {
                state.selection = Some(clamp_rect(
                    NativeRect::from_points(start, point),
                    state.bounds,
                ));
            }
            Interaction::Moving { start, rect } => {
                let dx = point.x - start.x;
                let dy = point.y - start.y;
                state.selection = Some(move_rect(rect, dx, dy, state.bounds));
            }
            Interaction::Resizing {
                start,
                rect,
                handle,
            } => {
                let dx = point.x - start.x;
                let dy = point.y - start.y;
                state.selection = Some(resize_rect(rect, dx, dy, handle, state.bounds));
            }
        }
    }

    unsafe fn complete_selection(hwnd: HWND, state: &mut NativeSelectionState) {
        let Some(rect) = state.selection else {
            return;
        };

        if !rect.is_valid() {
            state.selection = None;
            render_overlay(hwnd, state);
            return;
        }

        let area = CaptureArea {
            x: (state.bounds.x + rect.left) as f64,
            y: (state.bounds.y + rect.top) as f64,
            width: rect.width as f64,
            height: rect.height as f64,
        };

        state.send_result(Ok(Some(area)));
        DestroyWindow(hwnd);
    }

    fn move_rect(rect: NativeRect, dx: i32, dy: i32, bounds: NativeBounds) -> NativeRect {
        NativeRect {
            left: (rect.left + dx).clamp(0, max(bounds.width - rect.width, 0)),
            top: (rect.top + dy).clamp(0, max(bounds.height - rect.height, 0)),
            ..rect
        }
    }

    fn resize_rect(
        rect: NativeRect,
        dx: i32,
        dy: i32,
        handle: ResizeHandle,
        bounds: NativeBounds,
    ) -> NativeRect {
        let mut left = rect.left;
        let mut top = rect.top;
        let mut right = rect.right();
        let mut bottom = rect.bottom();

        if handle.has_west() {
            left += dx;
        }
        if handle.has_east() {
            right += dx;
        }
        if handle.has_north() {
            top += dy;
        }
        if handle.has_south() {
            bottom += dy;
        }

        left = left.clamp(0, bounds.width);
        right = right.clamp(0, bounds.width);
        top = top.clamp(0, bounds.height);
        bottom = bottom.clamp(0, bounds.height);

        if right - left < MIN_NATIVE_SELECTION_SIZE {
            if handle.has_west() {
                left = (right - MIN_NATIVE_SELECTION_SIZE).max(0);
            } else {
                right = (left + MIN_NATIVE_SELECTION_SIZE).min(bounds.width);
            }
        }

        if bottom - top < MIN_NATIVE_SELECTION_SIZE {
            if handle.has_north() {
                top = (bottom - MIN_NATIVE_SELECTION_SIZE).max(0);
            } else {
                bottom = (top + MIN_NATIVE_SELECTION_SIZE).min(bounds.height);
            }
        }

        NativeRect {
            left,
            top,
            width: max(right - left, MIN_NATIVE_SELECTION_SIZE),
            height: max(bottom - top, MIN_NATIVE_SELECTION_SIZE),
        }
    }

    fn clamp_rect(rect: NativeRect, bounds: NativeBounds) -> NativeRect {
        let left = rect.left.clamp(0, bounds.width);
        let top = rect.top.clamp(0, bounds.height);
        let right = rect.right().clamp(0, bounds.width);
        let bottom = rect.bottom().clamp(0, bounds.height);

        NativeRect {
            left,
            top,
            width: max(right - left, 0),
            height: max(bottom - top, 0),
        }
    }

    fn clamp_point(point: NativePoint, bounds: NativeBounds) -> NativePoint {
        NativePoint {
            x: point.x.clamp(0, bounds.width),
            y: point.y.clamp(0, bounds.height),
        }
    }

    fn lparam_point(lparam: LPARAM) -> NativePoint {
        NativePoint {
            x: (lparam & 0xffff) as u16 as i16 as i32,
            y: ((lparam >> 16) & 0xffff) as u16 as i16 as i32,
        }
    }

    fn hit_resize_handle(rect: NativeRect, point: NativePoint) -> Option<ResizeHandle> {
        let center_x = rect.left + rect.width / 2;
        let center_y = rect.top + rect.height / 2;
        let handles = [
            (ResizeHandle::Nw, rect.left, rect.top),
            (ResizeHandle::N, center_x, rect.top),
            (ResizeHandle::Ne, rect.right(), rect.top),
            (ResizeHandle::E, rect.right(), center_y),
            (ResizeHandle::Se, rect.right(), rect.bottom()),
            (ResizeHandle::S, center_x, rect.bottom()),
            (ResizeHandle::Sw, rect.left, rect.bottom()),
            (ResizeHandle::W, rect.left, center_y),
        ];

        handles.iter().find_map(|(handle, x, y)| {
            if (point.x - *x).abs() <= HANDLE_HIT_RADIUS
                && (point.y - *y).abs() <= HANDLE_HIT_RADIUS
            {
                Some(*handle)
            } else {
                None
            }
        })
    }

    fn cursor_for_state(state: &NativeSelectionState) -> HCURSOR {
        match state.interaction {
            Interaction::Moving { .. } => state.cursors.move_all,
            Interaction::Resizing { handle, .. } => state.cursors.for_handle(handle),
            Interaction::Drawing { .. } => state.cursors.cross,
            Interaction::Idle => {
                if toolbar_button_at(state, state.last_point).is_some() {
                    return state.cursors.hand;
                }
                if let Some(rect) = state.selection {
                    if let Some(handle) = hit_resize_handle(rect, state.last_point) {
                        return state.cursors.for_handle(handle);
                    }
                    if rect.contains(state.last_point) {
                        return state.cursors.move_all;
                    }
                }
                state.cursors.cross
            }
        }
    }

    unsafe fn render_overlay(_hwnd: HWND, state: &mut NativeSelectionState) {
        layout_visuals(state);
    }

    unsafe fn layout_visuals(state: &mut NativeSelectionState) {
        let bounds = state.bounds;
        let Some(visuals) = state.visuals.as_mut() else {
            return;
        };

        let Some(rect) = state
            .selection
            .filter(|rect| rect.width > 0 && rect.height > 0)
        else {
            state.toolbar_rect = None;
            for hwnd in visuals
                .dim_windows
                .iter()
                .chain(visuals.border_windows.iter())
                .chain(visuals.handle_windows.iter())
            {
                move_window(*hwnd, 0, 0, 0, 0, false);
            }
            move_window(visuals.toolbar_window, 0, 0, 0, 0, false);
            return;
        };

        let left = rect.left;
        let top = rect.top;
        let right = rect.right();
        let bottom = rect.bottom();
        let vw = bounds.width;
        let vh = bounds.height;

        let dim_rects = [
            NativeRect {
                left: 0,
                top: 0,
                width: vw,
                height: top,
            },
            NativeRect {
                left: 0,
                top: bottom,
                width: vw,
                height: vh - bottom,
            },
            NativeRect {
                left: 0,
                top,
                width: left,
                height: rect.height,
            },
            NativeRect {
                left: right,
                top,
                width: vw - right,
                height: rect.height,
            },
        ];

        for (hwnd, dim_rect) in visuals.dim_windows.iter().zip(dim_rects) {
            move_window_relative(
                *hwnd,
                bounds,
                dim_rect,
                dim_rect.width > 0 && dim_rect.height > 0,
            );
        }

        let t = BORDER_THICKNESS;
        let border_rects = [
            NativeRect {
                left,
                top,
                width: rect.width,
                height: t,
            },
            NativeRect {
                left,
                top: bottom - t,
                width: rect.width,
                height: t,
            },
            NativeRect {
                left,
                top,
                width: t,
                height: rect.height,
            },
            NativeRect {
                left: right - t,
                top,
                width: t,
                height: rect.height,
            },
        ];

        for (hwnd, border_rect) in visuals.border_windows.iter().zip(border_rects) {
            move_window_relative(*hwnd, bounds, border_rect, true);
        }

        let handle_half = HANDLE_VISUAL_SIZE / 2;
        for (hwnd, point) in visuals.handle_windows.iter().zip(handle_points(rect)) {
            move_window(
                *hwnd,
                bounds.x + point.x - handle_half,
                bounds.y + point.y - handle_half,
                HANDLE_VISUAL_SIZE,
                HANDLE_VISUAL_SIZE,
                true,
            );
        }

        let toolbar_rect = toolbar_rect_for_selection(rect, bounds);
        state.toolbar_rect = Some(toolbar_rect);
        move_window_relative(visuals.toolbar_window, bounds, toolbar_rect, true);
        draw_toolbar(visuals, bounds, toolbar_rect);
    }

    unsafe fn move_window_relative(hwnd: HWND, bounds: NativeBounds, rect: NativeRect, show: bool) {
        move_window(
            hwnd,
            bounds.x + rect.left,
            bounds.y + rect.top,
            rect.width,
            rect.height,
            show,
        );
    }

    unsafe fn move_window(hwnd: HWND, x: i32, y: i32, width: i32, height: i32, show: bool) {
        let flags =
            SWP_NOZORDER | SWP_NOACTIVATE | if show { SWP_SHOWWINDOW } else { SWP_HIDEWINDOW };
        SetWindowPos(hwnd, null_mut(), x, y, width.max(0), height.max(0), flags);
    }

    fn handle_points(rect: NativeRect) -> [NativePoint; 8] {
        let center_x = rect.left + rect.width / 2;
        let center_y = rect.top + rect.height / 2;
        [
            NativePoint {
                x: rect.left,
                y: rect.top,
            },
            NativePoint {
                x: center_x,
                y: rect.top,
            },
            NativePoint {
                x: rect.right(),
                y: rect.top,
            },
            NativePoint {
                x: rect.right(),
                y: center_y,
            },
            NativePoint {
                x: rect.right(),
                y: rect.bottom(),
            },
            NativePoint {
                x: center_x,
                y: rect.bottom(),
            },
            NativePoint {
                x: rect.left,
                y: rect.bottom(),
            },
            NativePoint {
                x: rect.left,
                y: center_y,
            },
        ]
    }

    fn toolbar_rect_for_selection(selection: NativeRect, bounds: NativeBounds) -> NativeRect {
        let max_left = (bounds.width - TOOLBAR_WIDTH).max(0);
        let mut left = (selection.right() - TOOLBAR_WIDTH).clamp(0, max_left);
        let mut top = selection.bottom() + TOOLBAR_MARGIN;

        if top + TOOLBAR_HEIGHT > bounds.height {
            top = selection.top - TOOLBAR_MARGIN - TOOLBAR_HEIGHT;
        }

        if top < 0 {
            top = (selection.top + TOOLBAR_MARGIN).min((bounds.height - TOOLBAR_HEIGHT).max(0));
            left = (selection.left + TOOLBAR_MARGIN).clamp(0, max_left);
        }

        NativeRect {
            left,
            top: top.clamp(0, (bounds.height - TOOLBAR_HEIGHT).max(0)),
            width: TOOLBAR_WIDTH,
            height: TOOLBAR_HEIGHT,
        }
    }

    fn toolbar_button_at(
        state: &NativeSelectionState,
        point: NativePoint,
    ) -> Option<ToolbarButton> {
        let rect = state.toolbar_rect?;
        if !rect.contains(point) {
            return None;
        }

        let local_x = point.x - rect.left;
        let local_y = point.y - rect.top;
        if local_y < TOOLBAR_PADDING || local_y >= TOOLBAR_PADDING + TOOLBAR_BUTTON_SIZE {
            return None;
        }

        if local_x >= TOOLBAR_PADDING && local_x < TOOLBAR_PADDING + TOOLBAR_BUTTON_SIZE {
            return Some(ToolbarButton::Cancel);
        }

        let confirm_left = TOOLBAR_PADDING + TOOLBAR_BUTTON_SIZE + TOOLBAR_BUTTON_GAP;
        if local_x >= confirm_left && local_x < confirm_left + TOOLBAR_BUTTON_SIZE {
            return Some(ToolbarButton::Confirm);
        }

        None
    }

    unsafe fn draw_toolbar(visuals: &mut OverlayVisuals, bounds: NativeBounds, rect: NativeRect) {
        let width = visuals.toolbar_bitmap.width;
        let height = visuals.toolbar_bitmap.height;
        let pixels = visuals.toolbar_bitmap.pixels_mut();
        clear_pixels(pixels);

        fill_rect(
            pixels,
            width,
            height,
            0,
            0,
            width,
            height,
            Color {
                r: 24,
                g: 30,
                b: 38,
                a: 232,
            },
        );

        draw_toolbar_button(
            pixels,
            width,
            height,
            TOOLBAR_PADDING,
            TOOLBAR_PADDING,
            ToolbarButton::Cancel,
        );
        draw_toolbar_button(
            pixels,
            width,
            height,
            TOOLBAR_PADDING + TOOLBAR_BUTTON_SIZE + TOOLBAR_BUTTON_GAP,
            TOOLBAR_PADDING,
            ToolbarButton::Confirm,
        );

        let dest = POINT {
            x: bounds.x + rect.left,
            y: bounds.y + rect.top,
        };
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let source = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };

        UpdateLayeredWindow(
            visuals.toolbar_window,
            null_mut(),
            &dest,
            &size,
            visuals.toolbar_bitmap.mem_dc,
            &source,
            0 as COLORREF,
            &blend,
            ULW_ALPHA,
        );
    }

    fn draw_toolbar_button(
        pixels: &mut [u8],
        canvas_width: i32,
        canvas_height: i32,
        left: i32,
        top: i32,
        button: ToolbarButton,
    ) {
        let bg = match button {
            ToolbarButton::Cancel => Color {
                r: 50,
                g: 58,
                b: 68,
                a: 245,
            },
            ToolbarButton::Confirm => Color {
                r: 31,
                g: 143,
                b: 255,
                a: 255,
            },
        };
        let fg = Color {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };

        fill_rect(
            pixels,
            canvas_width,
            canvas_height,
            left,
            top,
            TOOLBAR_BUTTON_SIZE,
            TOOLBAR_BUTTON_SIZE,
            bg,
        );

        match button {
            ToolbarButton::Cancel => {
                draw_line(
                    pixels,
                    canvas_width,
                    canvas_height,
                    left + 9,
                    top + 9,
                    left + 19,
                    top + 19,
                    2,
                    fg,
                );
                draw_line(
                    pixels,
                    canvas_width,
                    canvas_height,
                    left + 19,
                    top + 9,
                    left + 9,
                    top + 19,
                    2,
                    fg,
                );
            }
            ToolbarButton::Confirm => {
                draw_line(
                    pixels,
                    canvas_width,
                    canvas_height,
                    left + 7,
                    top + 15,
                    left + 12,
                    top + 20,
                    2,
                    fg,
                );
                draw_line(
                    pixels,
                    canvas_width,
                    canvas_height,
                    left + 12,
                    top + 20,
                    left + 21,
                    top + 9,
                    2,
                    fg,
                );
            }
        }
    }

    fn draw_line(
        pixels: &mut [u8],
        canvas_width: i32,
        canvas_height: i32,
        mut x0: i32,
        mut y0: i32,
        x1: i32,
        y1: i32,
        thickness: i32,
        color: Color,
    ) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            fill_rect(
                pixels,
                canvas_width,
                canvas_height,
                x0 - thickness / 2,
                y0 - thickness / 2,
                thickness,
                thickness,
                color,
            );

            if x0 == x1 && y0 == y1 {
                break;
            }

            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    fn clear_pixels(pixels: &mut [u8]) {
        pixels.fill(0);
    }

    fn fill_rect(
        pixels: &mut [u8],
        canvas_width: i32,
        canvas_height: i32,
        left: i32,
        top: i32,
        width: i32,
        height: i32,
        color: Color,
    ) {
        let start_x = left.clamp(0, canvas_width);
        let start_y = top.clamp(0, canvas_height);
        let end_x = (left + width).clamp(0, canvas_width);
        let end_y = (top + height).clamp(0, canvas_height);
        if start_x >= end_x || start_y >= end_y || canvas_width <= 0 {
            return;
        }

        let color = premultiplied_bgra(color);

        for y in start_y..end_y {
            let row_start = ((y * canvas_width + start_x) * 4) as usize;
            let row_end = ((y * canvas_width + end_x) * 4) as usize;
            if row_end > pixels.len() || row_start >= row_end {
                continue;
            }

            for pixel in pixels[row_start..row_end].chunks_exact_mut(4) {
                pixel.copy_from_slice(&color);
            }
        }
    }

    fn premultiplied_bgra(color: Color) -> [u8; 4] {
        [
            premultiply(color.b, color.a),
            premultiply(color.g, color.a),
            premultiply(color.r, color.a),
            color.a,
        ]
    }

    fn premultiply(channel: u8, alpha: u8) -> u8 {
        ((channel as u16 * alpha as u16 + 127) / 255) as u8
    }

    unsafe fn capture_area_with_gdi(area: CaptureArea) -> Result<String, String> {
        let x = area.x.round() as i32;
        let y = area.y.round() as i32;
        let width = area.width.round().max(MIN_SELECTION_SIZE) as i32;
        let height = area.height.round().max(MIN_SELECTION_SIZE) as i32;

        if width <= 0 || height <= 0 {
            return Err("截图区域太小，请重新框选".to_string());
        }

        let screen_dc = windows_sys::Win32::Graphics::Gdi::GetDC(null_mut());
        if screen_dc.is_null() {
            return Err("无法获取屏幕绘图上下文".to_string());
        }

        let mem_dc = CreateCompatibleDC(screen_dc);
        if mem_dc.is_null() {
            windows_sys::Win32::Graphics::Gdi::ReleaseDC(null_mut(), screen_dc);
            return Err("无法创建截图绘图上下文".to_string());
        }

        let mut bitmap_info = bitmap_info(width, height);
        let mut bits: *mut c_void = null_mut();
        let bitmap = CreateDIBSection(
            screen_dc,
            &mut bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            null_mut::<c_void>() as HANDLE,
            0,
        );

        if bitmap.is_null() || bits.is_null() {
            DeleteDC(mem_dc);
            windows_sys::Win32::Graphics::Gdi::ReleaseDC(null_mut(), screen_dc);
            return Err("无法创建截图位图".to_string());
        }

        let old_bitmap = SelectObject(mem_dc, bitmap as HGDIOBJ);
        let ok = BitBlt(
            mem_dc,
            0,
            0,
            width,
            height,
            screen_dc,
            x,
            y,
            SRCCOPY | CAPTUREBLT,
        );

        let pixel_count = (width * height) as usize;
        let rgba = if ok != 0 {
            let bgra = slice::from_raw_parts(bits as *const u8, pixel_count * 4);
            let mut rgba = Vec::with_capacity(pixel_count * 4);
            for pixel in bgra.chunks_exact(4) {
                rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
            }
            Some(rgba)
        } else {
            None
        };

        if !old_bitmap.is_null() {
            SelectObject(mem_dc, old_bitmap);
        }
        DeleteObject(bitmap as HGDIOBJ);
        DeleteDC(mem_dc);
        windows_sys::Win32::Graphics::Gdi::ReleaseDC(null_mut(), screen_dc);

        let rgba = rgba.ok_or_else(|| "截取屏幕失败".to_string())?;
        let image = ImageBuffer::<Rgba<u8>, _>::from_raw(width as u32, height as u32, rgba)
            .ok_or_else(|| "截图像素转换失败".to_string())?;

        encode_png_data_url(&DynamicImage::ImageRgba8(image))
    }

    fn bitmap_info(width: i32, height: i32) -> BITMAPINFO {
        BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [RGBQUAD {
                rgbBlue: 0,
                rgbGreen: 0,
                rgbRed: 0,
                rgbReserved: 0,
            }],
        }
    }

    fn to_wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_area, CaptureArea};

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
}
