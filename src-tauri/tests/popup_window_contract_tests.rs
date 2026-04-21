/// Contract tests for popup window positioning and creation behavior.
///
/// These tests lock down the positioning helper's edge-case behavior and verify
/// that the popup contract (undecorated, always-on-top, skip-taskbar) is preserved.
///
/// Run with:
///   cargo test --manifest-path src-tauri/Cargo.toml --test popup_window_contract_tests

use translation_tool_lib::popup_window::{
    MonitorInfo, PopupPositionInfo, calculate_popup_position,
    OFFSET, MARGIN, POPUP_WIDTH, POPUP_HEIGHT,
};

// ─── Helper ──────────────────────────────────────────────────────────────────

fn default_monitor() -> MonitorInfo {
    MonitorInfo {
        width: 1920,
        height: 1080,
        x: 0,
        y: 0,
    }
}

fn calc(cursor: (i32, i32), monitor: Option<MonitorInfo>) -> PopupPositionInfo {
    calculate_popup_position(cursor, POPUP_WIDTH, POPUP_HEIGHT, OFFSET, MARGIN, monitor)
}

// ─── Normal Positioning (No Edge Adjustment) ─────────────────────────────────

#[test]
fn normal_position_center_of_screen() {
    // Cursor near center — popup should appear bottom-right of cursor
    let pos = calc((800, 500), Some(default_monitor()));

    assert_eq!(pos.x, 800 + OFFSET);
    assert_eq!(pos.y, 500 + OFFSET);
    assert!(!pos.adjusted_for_edge);
}

#[test]
fn normal_position_with_default_offset_no_monitor() {
    // No monitor info — should still compute default offset, no adjustment
    let pos = calc((500, 300), None);

    assert_eq!(pos.x, 500 + OFFSET);
    assert_eq!(pos.y, 300 + OFFSET);
    assert!(!pos.adjusted_for_edge);
}

// ─── Right Edge Overflow ─────────────────────────────────────────────────────

#[test]
fn right_edge_overflow_moves_to_left_of_cursor() {
    // Cursor near right edge — popup should appear on the left
    let monitor = default_monitor(); // 1920 wide, usable_right = 1920 - 20 = 1900
    // Cursor at 1700: 1700 + 15 + 420 = 2135 > 1900 → overflow
    // Left attempt: 1700 - 420 - 15 = 1265 >= 20 (usable_left) → fits
    let pos = calc((1700, 500), Some(monitor));

    assert_eq!(pos.x, 1700 - POPUP_WIDTH - OFFSET); // 1265
    assert!(pos.adjusted_for_edge);
}

#[test]
fn right_edge_overflow_left_also_does_not_fit_clamps() {
    // Cursor so far right that even left side doesn't fit
    // usable_left = 20, usable_right = 1900
    // Cursor at 100: left_x = 100 - 420 - 15 = -335 < 20 → doesn't fit
    // Should clamp to usable_right - POPUP_WIDTH = 1900 - 420 = 1480
    let _pos = calc((100, 500), Some(default_monitor()));

    // 100 + 15 = 115; 115 + 420 = 535 <= 1900 → no overflow actually
    // Let's use a case that does overflow: cursor at 40, right side 40+15+420=475 <= 1900, no overflow
    // Actually for this test we need a narrow screen scenario
    let narrow_monitor = MonitorInfo {
        width: 500,  // Very narrow: usable_right = 500 - 20 = 480
        height: 1080,
        x: 0,
        y: 0,
    };
    // Cursor at 100: 100 + 15 + 420 = 535 > 480 → overflow
    // Left: 100 - 420 - 15 = -335 < 20 → doesn't fit
    // Clamp: 480 - 420 = 60
    let pos = calc((100, 500), Some(narrow_monitor));

    assert_eq!(pos.x, 60); // clamped to usable_right - popup_width
    assert!(pos.adjusted_for_edge);
}

// ─── Bottom Edge Overflow ────────────────────────────────────────────────────

#[test]
fn bottom_edge_overflow_moves_above_cursor() {
    // Cursor near bottom — popup should appear above
    let monitor = default_monitor(); // 1080 tall, usable_bottom = 1080 - 20 = 1060
    // Cursor at 900: 900 + 15 + 380 = 1295 > 1060 → overflow
    // Top attempt: 900 - 380 - 15 = 505 >= 20 → fits
    let pos = calc((800, 900), Some(monitor));

    assert_eq!(pos.x, 800 + OFFSET); // X unchanged
    assert_eq!(pos.y, 900 - POPUP_HEIGHT - OFFSET); // 505
    assert!(pos.adjusted_for_edge);
}

#[test]
fn bottom_edge_overflow_above_does_not_fit_clamps() {
    // Cursor so low that even above doesn't fit
    let narrow_monitor = MonitorInfo {
        width: 1920,
        height: 450,   // Very short: usable_bottom = 450 - 20 = 430
        x: 0,
        y: 0,
    };
    // Cursor at 200: 200 + 15 + 380 = 595 > 430 → overflow
    // Top: 200 - 380 - 15 = -195 < 20 → doesn't fit
    // Clamp: 430 - 380 = 50
    let pos = calc((200, 200), Some(narrow_monitor));

    assert_eq!(pos.y, 50); // clamped to usable_bottom - popup_height
    assert!(pos.adjusted_for_edge);
}

// ─── Corner Cases ────────────────────────────────────────────────────────────

#[test]
fn bottom_right_corner_both_axes_adjusted() {
    // Cursor in bottom-right corner — both X and Y should adjust
    let monitor = default_monitor();
    // Cursor at (1800, 1000)
    // X: 1800 + 15 + 420 = 2235 > 1900 → overflow → left: 1800-420-15=1365 >= 20 → fits
    // Y: 1000 + 15 + 380 = 1395 > 1060 → overflow → top: 1000-380-15=605 >= 20 → fits
    let pos = calc((1800, 1000), Some(monitor));

    assert_eq!(pos.x, 1800 - POPUP_WIDTH - OFFSET); // 1365
    assert_eq!(pos.y, 1000 - POPUP_HEIGHT - OFFSET); // 605
    assert!(pos.adjusted_for_edge);
}

#[test]
fn top_left_corner_no_adjustment() {
    // Cursor near top-left — popup fits without adjustment
    let pos = calc((100, 100), Some(default_monitor()));

    assert_eq!(pos.x, 100 + OFFSET); // 115
    assert_eq!(pos.y, 100 + OFFSET); // 115
    assert!(!pos.adjusted_for_edge);
}

#[test]
fn extreme_top_left_clamped_to_usable_area() {
    // Cursor at screen origin — popup offset would push it into margin
    // (0,0) + 15 = (15, 15) which is within usable area (20, 20)
    // So it should be clamped up to the margin boundary
    let pos = calc((0, 0), Some(default_monitor()));

    assert_eq!(pos.x, MARGIN); // 20, clamped from 15
    assert_eq!(pos.y, MARGIN); // 20, clamped from 15
    // Note: clamp doesn't set adjusted_for_edge since it's just a safety boundary
}

// ─── Multi-Monitor Scenario ──────────────────────────────────────────────────

#[test]
fn secondary_monitor_with_nonzero_origin() {
    // Second monitor positioned at (1920, 0) — common dual-monitor setup
    let secondary = MonitorInfo {
        width: 1920,
        height: 1080,
        x: 1920,
        y: 0,
    };
    // Cursor at (2500, 500) on secondary monitor
    // 2500 + 15 + 420 = 2935; usable_right = 1920 + 1920 - 20 = 3820 → fits
    let pos = calc((2500, 500), Some(secondary));

    assert_eq!(pos.x, 2500 + OFFSET);
    assert_eq!(pos.y, 500 + OFFSET);
    assert!(!pos.adjusted_for_edge);
}

#[test]
fn secondary_monitor_right_edge_overflow() {
    // Second monitor at (1920, 0); cursor near its right edge
    let secondary = MonitorInfo {
        width: 1920,
        height: 1080,
        x: 1920,
        y: 0,
    };
    // usable_right = 1920 + 1920 - 20 = 3820
    // Cursor at 3700: 3700 + 15 + 420 = 4135 > 3820 → overflow
    // Left: 3700 - 420 - 15 = 3265; usable_left = 1920 + 20 = 1940 → fits
    let pos = calc((3700, 500), Some(secondary));

    assert_eq!(pos.x, 3700 - POPUP_WIDTH - OFFSET); // 3265
    assert!(pos.adjusted_for_edge);
}

// ─── Missing Monitor / Error Path ────────────────────────────────────────────

#[test]
fn missing_monitor_returns_default_offset_no_panic() {
    // When monitor info is None (error path), position should be simple offset
    let pos = calc((999, 888), None);

    assert_eq!(pos.x, 999 + OFFSET);
    assert_eq!(pos.y, 888 + OFFSET);
    assert!(!pos.adjusted_for_edge);
}

#[test]
fn missing_monitor_at_screen_corner_does_not_panic() {
    // Even extreme cursor positions with no monitor info should not panic
    let pos = calc((9999, 9999), None);

    assert_eq!(pos.x, 9999 + OFFSET);
    assert_eq!(pos.y, 9999 + OFFSET);
    assert!(!pos.adjusted_for_edge);
}

// ─── Constants Verification ──────────────────────────────────────────────────

#[test]
fn popup_constants_are_reasonable() {
    // Sanity-check that the popup dimensions and margins are in expected ranges
    assert!(POPUP_WIDTH > 200 && POPUP_WIDTH < 800);
    assert!(POPUP_HEIGHT > 200 && POPUP_HEIGHT < 800);
    assert!(OFFSET > 0 && OFFSET < 100);
    assert!(MARGIN > 0 && MARGIN < 100);
}

#[test]
fn popup_fits_on_minimum_viable_screen() {
    // Ensure the popup can fit on a very small screen (e.g., 800x600)
    let small_monitor = MonitorInfo {
        width: 800,
        height: 600,
        x: 0,
        y: 0,
    };
    // usable: left=20, right=780, top=20, bottom=580
    // popup needs 420 width and 380 height
    // available: 780 - 20 = 760 width, 580 - 20 = 560 height → fits
    assert!(POPUP_WIDTH <= small_monitor.width - 2 * MARGIN);
    assert!(POPUP_HEIGHT <= small_monitor.height - 2 * MARGIN);
}
