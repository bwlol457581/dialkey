//! Best-effort: if a launch target is already open, bring that window forward.
//!
//! One `EnumWindows` pass at launch time — no timers, no process waiting.
//! Document matches use bounded title tokens + a small browser class deny-list.
//! Tabbed hosts (Excel, etc.) still have no tab-accurate guarantee.

use std::path::Path;
use std::time::Instant;

use tracing::{debug, warn};
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, BOOL, FALSE, HWND, LPARAM, MAX_PATH, TRUE};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW,
    PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, BringWindowToTop, EnumWindows, GetClassNameW, GetForegroundWindow,
    GetWindow, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsIconic, IsWindowVisible, SetForegroundWindow, SetWindowPos, ShowWindow, GWL_EXSTYLE,
    GW_OWNER, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SW_RESTORE, WS_EX_TOOLWINDOW,
};

use crate::core::focus_match::{
    is_document_focus_denied_class, process_image_matches, window_title_matches,
};

/// `AllowSetForegroundWindow(-1)` — any process may take foreground.
const ASFW_ANY: u32 = u32::MAX;

/// Bring `hwnd` above the current foreground window and try to give it focus.
///
/// Search / Settings are opened from a `NOACTIVATE` typing window or a posted
/// tray message, so DialKey is not the foreground process and
/// `SetForegroundWindow` alone is often denied (the window stays behind Excel).
/// Attach to the foreground thread, pop `HWND_TOPMOST` for one call, then drop
/// it so the window stays a normal overlapped window (Search is not always-on-top).
pub(super) unsafe fn bring_window_front(hwnd: HWND) {
    if hwnd.0.is_null() {
        return;
    }
    let _ = AllowSetForegroundWindow(ASFW_ANY);
    if IsIconic(hwnd).as_bool() {
        let _ = ShowWindow(hwnd, SW_RESTORE);
    }

    let fg = GetForegroundWindow();
    let this_tid = GetCurrentThreadId();
    let fg_tid = if fg.0.is_null() || fg == hwnd {
        0
    } else {
        GetWindowThreadProcessId(fg, None)
    };
    let attached =
        fg_tid != 0 && fg_tid != this_tid && AttachThreadInput(this_tid, fg_tid, TRUE).as_bool();

    let _ = BringWindowToTop(hwnd);
    let _ = SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
    let ok = SetForegroundWindow(hwnd);
    let _ = SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
    if attached {
        let _ = AttachThreadInput(this_tid, fg_tid, FALSE);
    }
    if !ok.as_bool() {
        warn!("SetForegroundWindow returned false");
    }
}

struct SearchCtx<'a> {
    path: &'a Path,
    match_exe: bool,
    found: Option<HWND>,
}

/// Returns true if an existing window was focused (caller should skip spawn).
pub fn try_focus_existing(path: &Path) -> bool {
    if path.as_os_str().is_empty() {
        return false;
    }

    let match_exe = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("exe"));

    let mut ctx = SearchCtx {
        path,
        match_exe,
        found: None,
    };

    let started = Instant::now();
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut SearchCtx as isize));
    }
    let scan_ms = started.elapsed().as_secs_f64() * 1000.0;

    let Some(hwnd) = ctx.found else {
        // Full pass cost (no early exit). Used for §10 / Appendix C weight notes.
        if scan_ms >= 250.0 {
            warn!(
                scan_ms = format!("{scan_ms:.3}"),
                match_exe, "focus_existing: no match (slow)"
            );
        } else {
            debug!(
                scan_ms = format!("{scan_ms:.3}"),
                match_exe, "focus_existing: no match"
            );
        }
        return false;
    };

    let focus_ok = unsafe { focus_hwnd(hwnd) };
    // Hit timing stays at debug; launch.rs already logs the skip at info.
    if scan_ms >= 250.0 {
        warn!(
            scan_ms = format!("{scan_ms:.3}"),
            focus_ok, "focus_existing: matched (slow)"
        );
    } else {
        debug!(
            scan_ms = format!("{scan_ms:.3}"),
            focus_ok, "focus_existing: matched"
        );
    }
    focus_ok
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut SearchCtx);

    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    // Skip owned / tool windows (tooltips, floating toolbars).
    if let Ok(owner) = GetWindow(hwnd, GW_OWNER) {
        if !owner.0.is_null() {
            return BOOL(1);
        }
    }
    let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    if ex & WS_EX_TOOLWINDOW.0 != 0 {
        return BOOL(1);
    }

    let title = window_title(hwnd);
    let mut hit = false;

    if !title.is_empty() && window_title_matches(&title, ctx.path) {
        // Document title hit: skip known browser hosts so Chrome/Edge download
        // titles do not steal focus and block the real shell open / Office host.
        if ctx.match_exe {
            hit = true;
        } else {
            let class = window_class(hwnd);
            if is_document_focus_denied_class(&class) {
                debug!(class, "focus_existing: skip denied class");
            } else {
                hit = true;
            }
        }
    }

    if !hit && ctx.match_exe {
        if let Some(image) = process_image(hwnd) {
            if process_image_matches(&image, ctx.path) {
                hit = true;
            }
        }
    }

    if hit {
        ctx.found = Some(hwnd);
        return BOOL(0); // stop enum
    }
    BOOL(1)
}

unsafe fn window_class(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let n = GetClassNameW(hwnd, &mut buf);
    if n <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..n as usize])
}

unsafe fn window_title(hwnd: HWND) -> String {
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len as usize) + 1];
    let n = GetWindowTextW(hwnd, &mut buf);
    if n <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..n as usize])
}

unsafe fn process_image(hwnd: HWND) -> Option<String> {
    let mut pid = 0u32;
    let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 {
        return None;
    }
    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let mut buf = vec![0u16; MAX_PATH as usize];
    let mut size = buf.len() as u32;
    let ok = QueryFullProcessImageNameW(
        handle,
        PROCESS_NAME_FORMAT(0),
        PWSTR(buf.as_mut_ptr()),
        &mut size,
    );
    let _ = CloseHandle(handle);
    if ok.is_err() || size == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..size as usize]))
}

unsafe fn focus_hwnd(hwnd: HWND) -> bool {
    bring_window_front(hwnd);
    // Count as handled even if focus was denied — avoids a redundant second instance
    // when the window was at least restored / brought up in the z-order.
    true
}
