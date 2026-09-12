//! Shell-open a file or folder (long UNC / MAX_PATH-safe).

use std::io;
use std::path::Path;
use std::time::Instant;

use tracing::{debug, warn};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{
    ShellExecuteExW, SEE_MASK_ASYNCOK, SEE_MASK_NOZONECHECKS, SHELLEXECUTEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::core::winpath::path_for_win32;

use super::wide::to_wide;

/// Open `file` with the Windows shell. `workdir` is optional (`lpDirectory`).
/// Tries the extended-length form first, then the original path.
///
/// `SEE_MASK_ASYNCOK` returns without waiting on DDE / association servers so
/// the caller (launch worker) is not stuck for seconds on Excel / UNC.
pub fn shell_open(file: &Path, workdir: Option<&Path>) -> io::Result<()> {
    if try_execute(file, workdir, true) {
        return Ok(());
    }
    if try_execute(file, workdir, false) {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        format!("ShellExecute failed for {}", file.display()),
    ))
}

fn try_execute(file: &Path, workdir: Option<&Path>, extended: bool) -> bool {
    let file_s = if extended {
        path_for_win32(file).to_string_lossy().into_owned()
    } else {
        file.to_string_lossy().into_owned()
    };
    if file_s.is_empty() {
        return false;
    }
    let file_w = to_wide(&file_s);
    let dir_owned = workdir.filter(|d| !d.as_os_str().is_empty()).map(|d| {
        if extended {
            path_for_win32(d).to_string_lossy().into_owned()
        } else {
            d.to_string_lossy().into_owned()
        }
    });
    let dir_w = dir_owned.as_ref().map(|s| to_wide(s));
    let started = Instant::now();
    let ok = unsafe {
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_ASYNCOK | SEE_MASK_NOZONECHECKS,
            hwnd: HWND::default(),
            lpVerb: w!("open"),
            lpFile: PCWSTR(file_w.as_ptr()),
            lpParameters: PCWSTR::null(),
            lpDirectory: match &dir_w {
                Some(dw) => PCWSTR(dw.as_ptr()),
                None => PCWSTR::null(),
            },
            nShow: SW_SHOWNORMAL.0 as i32,
            ..Default::default()
        };
        ShellExecuteExW(&mut info).is_ok()
    };
    let elapsed_ms = started.elapsed().as_millis();
    if elapsed_ms >= 250 {
        warn!(extended, elapsed_ms, ok, "ShellExecute slow");
    } else {
        debug!(extended, elapsed_ms, ok, "ShellExecute");
    }
    ok
}
