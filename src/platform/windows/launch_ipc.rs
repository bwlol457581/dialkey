//! CLI `--launch` hand-off to the resident host via `WM_COPYDATA`.
//!
//! No always-on server: the second process finds `DialKeyHost` and SendMessage's.
//! Slot id is a string (`"01"` != `"1"`), so WPARAM alone is not enough.

use std::path::PathBuf;
use std::sync::Mutex;

use tracing::{error, info, warn};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, PostMessageW, SendMessageW, WM_COPYDATA,
};

use crate::core::slots::{is_valid_slot_id, SlotRegistry};

use super::messages::{COPYDATA_LAUNCH, HOST_CLASS};

/// Snapshot used by the host while handling `--launch` IPC.
struct LaunchIpcCtx {
    exe_dir: PathBuf,
    slots: SlotRegistry,
}

static LAUNCH_CTX: Mutex<Option<LaunchIpcCtx>> = Mutex::new(None);

/// Exit / `SendMessage` result codes for `--launch` (automation-friendly).
pub const LAUNCH_OK: isize = 1;
pub const LAUNCH_UNKNOWN: isize = 2;
pub const LAUNCH_FAILED: isize = 3;
pub const LAUNCH_BAD_REQUEST: isize = 0;

/// Refresh after load / Reload / Settings Apply.
pub fn set_ctx(exe_dir: PathBuf, slots: SlotRegistry) {
    if let Ok(mut g) = LAUNCH_CTX.lock() {
        *g = Some(LaunchIpcCtx { exe_dir, slots });
    }
}

pub fn clear_ctx() {
    if let Ok(mut g) = LAUNCH_CTX.lock() {
        *g = None;
    }
}

/// `host_proc` handler for `WM_COPYDATA`. Returns a `LAUNCH_*` code.
pub unsafe fn handle_copydata(_hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let _ = wparam;
    if lparam.0 == 0 {
        return LRESULT(LAUNCH_BAD_REQUEST);
    }
    let cds = &*(lparam.0 as *const COPYDATASTRUCT);
    if cds.dwData != COPYDATA_LAUNCH {
        return LRESULT(LAUNCH_BAD_REQUEST);
    }
    if cds.cbData == 0 || cds.lpData.is_null() {
        return LRESULT(LAUNCH_BAD_REQUEST);
    }
    let bytes = std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize);
    let id = match std::str::from_utf8(bytes) {
        Ok(s) => s.trim(),
        Err(_) => {
            warn!("--launch IPC: slot id is not UTF-8");
            return LRESULT(LAUNCH_BAD_REQUEST);
        }
    };
    if !is_valid_slot_id(id) {
        warn!(slot_id = %id, "--launch IPC: invalid slot id");
        return LRESULT(LAUNCH_BAD_REQUEST);
    }

    let guard = match LAUNCH_CTX.lock() {
        Ok(g) => g,
        Err(_) => {
            error!("--launch IPC: launch context lock poisoned");
            return LRESULT(LAUNCH_FAILED);
        }
    };
    let Some(ctx) = guard.as_ref() else {
        error!("--launch IPC: no launch context (host not ready)");
        return LRESULT(LAUNCH_FAILED);
    };
    let Some(slot) = ctx.slots.get(id) else {
        warn!(slot_id = %id, "--launch IPC: unknown slot");
        return LRESULT(LAUNCH_UNKNOWN);
    };

    let slot = slot.clone();
    let exe_dir = ctx.exe_dir.clone();
    let registry = ctx.slots.clone();
    drop(guard);
    match super::launch_queue::queue(slot, exe_dir, registry, false, _hwnd) {
        Ok(()) => {
            info!(slot_id = %id, "queued via CLI IPC");
            LRESULT(LAUNCH_OK)
        }
        Err(e) => {
            error!(slot_id = %id, error = %e, "--launch IPC: queue failed");
            LRESULT(LAUNCH_FAILED)
        }
    }
}

#[derive(Debug)]
pub enum SignalLaunchError {
    /// No resident host window (caller should one-shot).
    NotRunning,
    /// Host returned unknown / failed / bad request.
    Host(isize),
}

/// Send `--launch` to the resident host. `NotRunning` → caller does one-shot.
pub fn try_signal_launch(id: &str) -> Result<(), SignalLaunchError> {
    unsafe {
        let Ok(hwnd) = FindWindowW(HOST_CLASS, None) else {
            return Err(SignalLaunchError::NotRunning);
        };
        if hwnd.0.is_null() {
            return Err(SignalLaunchError::NotRunning);
        }
        let mut bytes = id.as_bytes().to_vec();
        let cds = COPYDATASTRUCT {
            dwData: COPYDATA_LAUNCH,
            cbData: bytes.len() as u32,
            lpData: bytes.as_mut_ptr() as *mut _,
        };
        let result = SendMessageW(
            hwnd,
            WM_COPYDATA,
            WPARAM(0),
            LPARAM(&cds as *const COPYDATASTRUCT as isize),
        );
        match result.0 {
            LAUNCH_OK => Ok(()),
            other => Err(SignalLaunchError::Host(other)),
        }
    }
}

/// Ask the resident host to reload config (`WM_DK_RELOAD`).
pub fn try_signal_reload() -> Result<(), SignalLaunchError> {
    use super::messages::WM_DK_RELOAD;
    unsafe {
        let Ok(hwnd) = FindWindowW(HOST_CLASS, None) else {
            return Err(SignalLaunchError::NotRunning);
        };
        if hwnd.0.is_null() {
            return Err(SignalLaunchError::NotRunning);
        }
        match PostMessageW(hwnd, WM_DK_RELOAD, WPARAM(0), LPARAM(0)) {
            Ok(()) => Ok(()),
            Err(_) => Err(SignalLaunchError::Host(LAUNCH_FAILED)),
        }
    }
}
