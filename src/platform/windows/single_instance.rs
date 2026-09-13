//! Single-instance mutex. A second process asks the first to open the window.

use windows::core::w;
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, LPARAM, WPARAM,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW};

use super::messages::{HOST_CLASS, WM_DK_TRIGGER};

pub struct SingleInstance {
    handle: HANDLE,
}

#[derive(Debug)]
pub struct AlreadyRunning;

impl SingleInstance {
    /// Acquire the mutex. If another instance owns it, signal it to open
    /// the typing window and return `Err(AlreadyRunning)`.
    pub fn acquire() -> Result<Self, AlreadyRunning> {
        unsafe {
            let handle = CreateMutexW(None, true, w!("Local\\DialKey-SingleInstance"))
                .map_err(|_| AlreadyRunning)?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                signal_primary();
                return Err(AlreadyRunning);
            }
            Ok(Self { handle })
        }
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

fn signal_primary() {
    unsafe {
        let Ok(hwnd) = FindWindowW(HOST_CLASS, None) else {
            return;
        };
        if hwnd.0.is_null() {
            return;
        }
        let _ = PostMessageW(hwnd, WM_DK_TRIGGER, WPARAM(0), LPARAM(0));
    }
}
