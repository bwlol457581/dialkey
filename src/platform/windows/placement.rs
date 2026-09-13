//! Window placement helpers (§9 窓表示).

use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

/// Gap from work-area edges at 96 DPI (spec: slight inset). Scales with monitor DPI.
const INSET_AT_96: i32 = 20;

unsafe fn work_area_primary() -> Option<RECT> {
    let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(monitor, &mut mi).as_bool() {
        Some(mi.rcWork)
    } else {
        None
    }
}

unsafe fn work_area_at(pt: POINT) -> Option<RECT> {
    let monitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(monitor, &mut mi).as_bool() {
        Some(mi.rcWork)
    } else {
        None
    }
}

unsafe fn inset_for_work_area(wa: RECT) -> i32 {
    let monitor = MonitorFromRect(&wa, MONITOR_DEFAULTTOPRIMARY);
    let mut dpi_x = 96u32;
    let mut dpi_y = 96u32;
    if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok() {
        ((INSET_AT_96 as i64) * (dpi_x as i64) / 96) as i32
    } else {
        INSET_AT_96
    }
}

fn clamp_top_left(mut x: i32, mut y: i32, width: i32, height: i32, wa: RECT) -> (i32, i32) {
    if x + width > wa.right {
        x = wa.right - width;
    }
    if y + height > wa.bottom {
        y = wa.bottom - height;
    }
    if x < wa.left {
        x = wa.left;
    }
    if y < wa.top {
        y = wa.top;
    }
    (x, y)
}

/// Typing window: primary work-area top-right − inset → window top-right.
pub unsafe fn typing_window_pos(width: i32, height: i32) -> (i32, i32) {
    let Some(wa) = work_area_primary() else {
        return (0, 0);
    };
    let inset = inset_for_work_area(wa);
    let x = wa.right - inset - width;
    let y = wa.top + inset;
    clamp_top_left(x, y, width, height, wa)
}

/// Search window: primary work-area bottom-right − inset → window bottom-right.
pub unsafe fn search_window_pos(width: i32, height: i32) -> (i32, i32) {
    let Some(wa) = work_area_primary() else {
        return (0, 0);
    };
    let inset = inset_for_work_area(wa);
    let x = wa.right - inset - width;
    let y = wa.bottom - inset - height;
    clamp_top_left(x, y, width, height, wa)
}

/// Settings: cursor monitor work-area top-left + inset → window top-left.
pub unsafe fn settings_window_pos(pt: POINT, width: i32, height: i32) -> (i32, i32) {
    let Some(wa) = work_area_at(pt) else {
        return (pt.x, pt.y);
    };
    let inset = inset_for_work_area(wa);
    let x = wa.left + inset;
    let y = wa.top + inset;
    clamp_top_left(x, y, width, height, wa)
}
