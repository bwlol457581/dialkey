//! Resident `WH_MOUSE_LL` on its own message-loop thread.
//!
//! Low-level hooks are delivered to the installing thread. If that thread is
//! the UI loop, a blocking `ShellExecute` (~300 ms) lets Windows silently
//! remove the hook. The pump thread never waits on launches.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU16, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::Context;
use tracing::{error, info};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    PostMessageW, PostQuitMessage, RegisterClassExW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, HHOOK, HWND_MESSAGE, MSG, MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_CLOSE,
    WM_DESTROY, WM_MBUTTONDOWN, WM_QUIT, WM_XBUTTONDOWN, WNDCLASSEXW, WS_POPUP, XBUTTON1, XBUTTON2,
};

use super::app::host_hwnd;
use super::messages::WM_DK_TRIGGER;
use super::mode_runtime;
use super::settings;

const PUMP_CLASS: PCWSTR = w!("DialKeyMouseHookPump");
const WM_HOOK_REINSTALL: u32 = 0x0400 + 1; // WM_APP + 1 on this window only

static MOUSE_SUPPRESS: AtomicBool = AtomicBool::new(true);
/// 0 = x2, 1 = x1, 2 = middle
static MOUSE_KIND: AtomicU16 = AtomicU16::new(0);
static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);
static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);

pub fn configure(kind: u16, suppress: bool) {
    MOUSE_KIND.store(kind, Ordering::SeqCst);
    MOUSE_SUPPRESS.store(suppress, Ordering::SeqCst);
}

pub struct MouseHook {
    hwnd: HWND,
    join: Option<JoinHandle<()>>,
}

impl MouseHook {
    pub fn install() -> anyhow::Result<Self> {
        let (tx, rx) = mpsc::channel::<anyhow::Result<isize>>();
        let join = thread::Builder::new()
            .name("dialkey-mouse-hook".into())
            .spawn(move || pump_thread(tx))
            .context("spawn mouse-hook thread")?;
        let hwnd_bits = rx
            .recv_timeout(Duration::from_secs(5))
            .context("mouse-hook thread did not start")??;
        Ok(Self {
            hwnd: HWND(hwnd_bits as *mut _),
            join: Some(join),
        })
    }

    pub fn reinstall(&self) -> anyhow::Result<()> {
        if self.hwnd.0.is_null() {
            anyhow::bail!("mouse hook pump hwnd is null");
        }
        unsafe {
            PostMessageW(self.hwnd, WM_HOOK_REINSTALL, WPARAM(0), LPARAM(0))
                .context("post mouse-hook reinstall")?;
        }
        Ok(())
    }
}

impl Drop for MouseHook {
    fn drop(&mut self) {
        if !self.hwnd.0.is_null() {
            unsafe {
                let _ = PostMessageW(self.hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn pump_thread(ready: mpsc::Sender<anyhow::Result<isize>>) {
    unsafe {
        if let Err(e) = pump_thread_inner(&ready) {
            let _ = ready.send(Err(e));
        }
    }
}

unsafe fn pump_thread_inner(ready: &mpsc::Sender<anyhow::Result<isize>>) -> anyhow::Result<()> {
    let instance = GetModuleHandleW(None)?;
    if !CLASS_REGISTERED.swap(true, Ordering::SeqCst) {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(pump_proc),
            hInstance: instance.into(),
            lpszClassName: PUMP_CLASS,
            ..Default::default()
        };
        if RegisterClassExW(&wc) == 0 {
            anyhow::bail!("RegisterClassExW(mouse-hook pump) failed");
        }
    }

    let hwnd = CreateWindowExW(
        Default::default(),
        PUMP_CLASS,
        w!("DialKeyMouseHook"),
        WS_POPUP,
        0,
        0,
        0,
        0,
        HWND_MESSAGE,
        None,
        instance,
        None,
    )?;

    match install_ll() {
        Ok(hook) => HOOK_HANDLE.store(hook.0 as isize, Ordering::SeqCst),
        Err(e) => {
            let _ = DestroyWindow(hwnd);
            return Err(e);
        }
    }

    let _ = ready.send(Ok(hwnd.0 as isize));
    info!("mouse hook installed (dedicated thread)");

    let mut msg = MSG::default();
    loop {
        let ok = GetMessageW(&mut msg, None, 0, 0);
        if !ok.as_bool() || msg.message == WM_QUIT {
            break;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    unhook_ll();
    Ok(())
}

unsafe fn install_ll() -> anyhow::Result<HHOOK> {
    let module = GetModuleHandleW(None)?;
    let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0)?;
    mode_runtime::note_mouse_hook_activity();
    Ok(hook)
}

fn unhook_ll() {
    let h = HOOK_HANDLE.swap(0, Ordering::SeqCst);
    if h != 0 {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(h as _));
        }
        info!("mouse hook uninstalled");
    }
}

unsafe extern "system" fn pump_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_HOOK_REINSTALL => {
            unhook_ll();
            match install_ll() {
                Ok(hook) => {
                    HOOK_HANDLE.store(hook.0 as isize, Ordering::SeqCst);
                    info!("mouse LL hook reinstalled (health check)");
                }
                Err(e) => error!("mouse hook health reinstall failed: {e}"),
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == 0 {
        mode_runtime::note_mouse_hook_activity();

        let xbutton = if wparam.0 == WM_XBUTTONDOWN as usize {
            let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
            Some(((info.mouseData >> 16) & 0xFFFF) as u16)
        } else {
            None
        };

        if mode_runtime::mode_mouse_enabled() {
            let mode_hit = match mode_runtime::mode_mouse_kind() {
                2 => wparam.0 == WM_MBUTTONDOWN as usize,
                1 => xbutton == Some(XBUTTON1),
                _ => xbutton == Some(XBUTTON2),
            };
            if mode_hit {
                if settings::triggers_disabled() {
                    return CallNextHookEx(None, code, wparam, lparam);
                }
                mode_runtime::post_mode_arm();
                if mode_runtime::mode_mouse_suppress() {
                    return LRESULT(1);
                }
                return CallNextHookEx(None, code, wparam, lparam);
            }
        }

        let kind = MOUSE_KIND.load(Ordering::SeqCst);
        let hit = match kind {
            2 => wparam.0 == WM_MBUTTONDOWN as usize,
            1 => xbutton == Some(XBUTTON1),
            _ => xbutton == Some(XBUTTON2),
        };
        if hit {
            if settings::triggers_disabled() {
                return CallNextHookEx(None, code, wparam, lparam);
            }
            let host = host_hwnd();
            if !host.0.is_null() {
                let _ = PostMessageW(host, WM_DK_TRIGGER, WPARAM(0), LPARAM(0));
            }
            if MOUSE_SUPPRESS.load(Ordering::SeqCst) {
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}
