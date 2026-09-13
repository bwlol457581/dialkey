//! System tray icon with TaskbarCreated recovery.

use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use tracing::{error, info, warn};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NIM_SETVERSION, NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyIcon, DestroyMenu, GetCursorPos, GetSystemMetrics,
    PostMessageW, RegisterWindowMessageW, SetForegroundWindow, TrackPopupMenu, HICON, HMENU,
    MF_CHECKED, MF_POPUP, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, SM_CXSMICON, SM_CYSMICON,
    TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP, WM_CONTEXTMENU, WM_LBUTTONDBLCLK,
    WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WM_USER,
};

use crate::core::i18n::{keys, t, tf};
use crate::platform::TrayIcon;

use super::icon::load_app_icon;
use super::messages::{
    TRAY_ICON_ID, WM_DK_KEYSET_PICK, WM_DK_MODE_PICK, WM_DK_QUIT, WM_DK_RELOAD, WM_DK_SEARCH,
    WM_DK_SETTINGS, WM_DK_TRAY, WM_DK_TRIGGER,
};
use super::mode_runtime;
use super::wide::{pcwstr, to_wide};

pub const WM_DK_HELP: u32 = WM_APP + 13;

/// NOTIFYICON_VERSION_4 callback codes
const NIN_SELECT: u32 = WM_USER + 0;
const NIN_KEYSELECT: u32 = WM_USER + 1;

const IDM_RUN: usize = 1001;
const IDM_SEARCH: usize = 1002;
const IDM_SETTINGS: usize = 1003;
const IDM_RELOAD: usize = 1004;
const IDM_HELP: usize = 1005;
const IDM_QUIT: usize = 1006;
const IDM_MODE_BASE: usize = 1100;
const IDM_KEYSET_BASE: usize = 1200;

static TASKBAR_CREATED_MSG: AtomicU32 = AtomicU32::new(0);

pub fn taskbar_created_msg() -> u32 {
    TASKBAR_CREATED_MSG.load(Ordering::SeqCst)
}

pub fn register_taskbar_created_message() {
    unsafe {
        let id = RegisterWindowMessageW(w!("TaskbarCreated"));
        TASKBAR_CREATED_MSG.store(id, Ordering::SeqCst);
        info!(msg_id = id, "registered TaskbarCreated message");
    }
}

pub struct WindowsTray {
    hwnd: HWND,
    icon: HICON,
    added: bool,
    /// Slot JSON basenames for the Mode submenu (refreshed before each menu).
    mode_files: Vec<String>,
    /// Display labels aligned with `mode_files` (`meta.displayName` or basename).
    mode_labels: Vec<String>,
    /// Current `slotsFile` basename (checked in the Mode submenu).
    mode_current: String,
    keyset_ids: Vec<String>,
    keyset_labels: Vec<String>,
    keyset_current: String,
}

impl WindowsTray {
    pub fn new(hwnd: HWND) -> Self {
        let icon = unsafe { create_tray_icon() };
        Self {
            hwnd,
            icon,
            added: false,
            mode_files: Vec::new(),
            mode_labels: Vec::new(),
            mode_current: String::new(),
            keyset_ids: Vec::new(),
            keyset_labels: Vec::new(),
            keyset_current: String::new(),
        }
    }

    pub fn set_mode_files(&mut self, files: Vec<String>, labels: Vec<String>, current: String) {
        mode_runtime::set_mode_pick_files(files.clone());
        self.mode_files = files;
        self.mode_labels = labels;
        self.mode_current = current;
    }

    pub fn set_key_sets(&mut self, ids: Vec<String>, labels: Vec<String>, current: String) {
        self.keyset_ids = ids;
        self.keyset_labels = labels;
        self.keyset_current = current;
    }

    fn nid(&self) -> NOTIFYICONDATAW {
        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = self.hwnd;
        nid.uID = TRAY_ICON_ID;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        nid.uCallbackMessage = WM_DK_TRAY;
        nid.hIcon = self.icon;
        let tip = to_wide(&tf(
            keys::TRAY_TOOLTIP,
            &[("version", env!("CARGO_PKG_VERSION"))],
        ));
        for (i, ch) in tip.iter().take(nid.szTip.len()).enumerate() {
            nid.szTip[i] = *ch;
        }
        nid
    }

    fn add_with_retry(&mut self) -> anyhow::Result<()> {
        for attempt in 1..=5 {
            unsafe {
                let mut nid = self.nid();
                // Clear a stale icon from a previous crash (same hwnd/uID).
                let _ = Shell_NotifyIconW(NIM_DELETE, &mut nid);

                if Shell_NotifyIconW(NIM_ADD, &mut nid).as_bool() {
                    nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
                    if !Shell_NotifyIconW(NIM_SETVERSION, &mut nid).as_bool() {
                        warn!("NIM_SETVERSION failed; continuing with legacy callbacks");
                    }
                    self.added = true;
                    info!(attempt, "tray icon added");
                    return Ok(());
                }
            }
            warn!(attempt, "Shell_NotifyIcon NIM_ADD failed; retrying");
            thread::sleep(Duration::from_millis(300));
        }
        error!("failed to add tray icon after retries");
        anyhow::bail!("Shell_NotifyIcon NIM_ADD failed")
    }

    pub fn remove(&mut self) {
        if !self.added {
            return;
        }
        unsafe {
            let mut nid = self.nid();
            let _ = Shell_NotifyIconW(NIM_DELETE, &mut nid);
        }
        self.added = false;
    }

    pub fn handle_callback(&self, wparam: WPARAM, lparam: LPARAM) {
        // VERSION_4: LOWORD(lParam) = event; cursor often in wParam.
        // Legacy: lParam is WM_*BUTTON*.
        let event = (lparam.0 as u32) & 0xFFFF;
        info!(event, "tray callback");
        match event {
            NIN_SELECT | NIN_KEYSELECT | WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
                info!("tray left-click: open Search");
                unsafe {
                    // Grant the foreground lock before the posted open (same as the menu).
                    let _ = SetForegroundWindow(self.hwnd);
                    let _ = PostMessageW(self.hwnd, WM_DK_SEARCH, WPARAM(0), LPARAM(0));
                }
            }
            WM_CONTEXTMENU | WM_RBUTTONUP => {
                let _ = wparam;
                show_context_menu(
                    self.hwnd,
                    &self.mode_files,
                    &self.mode_labels,
                    &self.mode_current,
                    &self.keyset_ids,
                    &self.keyset_labels,
                    &self.keyset_current,
                );
            }
            _ => {}
        }
    }

    pub fn notify(&self, title: &str, text: &str) {
        if self.added {
            balloon(self.hwnd, title, text);
        }
    }
}

impl TrayIcon for WindowsTray {
    fn add(&mut self) -> anyhow::Result<()> {
        self.add_with_retry()
    }

    fn reregister(&mut self) -> anyhow::Result<()> {
        info!("TaskbarCreated — re-registering tray icon");
        self.added = false;
        self.add_with_retry()
    }
}

impl Drop for WindowsTray {
    fn drop(&mut self) {
        self.remove();
        unsafe {
            if !self.icon.0.is_null() {
                let _ = DestroyIcon(self.icon);
            }
        }
    }
}

/// Dial-pad tray icon from the embedded `assets/dialkey.ico` (spec §9).
unsafe fn create_tray_icon() -> HICON {
    let cx = GetSystemMetrics(SM_CXSMICON);
    let cy = GetSystemMetrics(SM_CYSMICON);
    let icon = load_app_icon(cx, cy);
    if icon.0.is_null() {
        error!("failed to load tray icon from resources");
    }
    icon
}

fn show_context_menu(
    hwnd: HWND,
    mode_files: &[String],
    mode_labels: &[String],
    current: &str,
    keyset_ids: &[String],
    keyset_labels: &[String],
    keyset_current: &str,
) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            error!("CreatePopupMenu failed");
            return;
        };
        let run = to_wide(&t(keys::TRAY_MENU_RUN));
        let search = to_wide(&t(keys::TRAY_MENU_SEARCH));
        let settings = to_wide(&t(keys::TRAY_MENU_SETTINGS));
        let reload = to_wide(&t(keys::TRAY_MENU_RELOAD));
        let help = to_wide(&t(keys::TRAY_MENU_HELP));
        let quit = to_wide(&t(keys::TRAY_MENU_QUIT));
        let mode_label = to_wide(&t(keys::TRAY_MENU_MODE));
        let keys_label = to_wide(&t(keys::TRAY_MENU_KEYS));
        let _ = AppendMenuW(menu, MF_STRING, IDM_RUN, pcwstr(&run));
        let _ = AppendMenuW(menu, MF_STRING, IDM_SEARCH, pcwstr(&search));
        let _ = AppendMenuW(menu, MF_STRING, IDM_SETTINGS, pcwstr(&settings));

        let mut mode_sub = HMENU::default();
        let mut keyset_sub = HMENU::default();
        if !mode_files.is_empty() {
            if let Ok(sub) = CreatePopupMenu() {
                mode_sub = sub;
                let cur = current.trim();
                for (i, file) in mode_files.iter().enumerate() {
                    let text = mode_labels.get(i).unwrap_or(file);
                    let label = to_wide(text);
                    let checked = !cur.is_empty() && file.eq_ignore_ascii_case(cur);
                    let flags = if checked {
                        MF_STRING | MF_CHECKED
                    } else {
                        MF_STRING | MF_UNCHECKED
                    };
                    let _ = AppendMenuW(sub, flags, IDM_MODE_BASE + i, pcwstr(&label));
                }
                let _ = AppendMenuW(
                    menu,
                    MF_POPUP | MF_STRING,
                    mode_sub.0 as usize,
                    pcwstr(&mode_label),
                );
            }
        }

        if !keyset_ids.is_empty() {
            if let Ok(sub) = CreatePopupMenu() {
                keyset_sub = sub;
                let cur = keyset_current.trim();
                for (i, id) in keyset_ids.iter().enumerate() {
                    let text = keyset_labels.get(i).unwrap_or(id);
                    let label = to_wide(text);
                    let checked = !cur.is_empty() && id == cur;
                    let flags = if checked {
                        MF_STRING | MF_CHECKED
                    } else {
                        MF_STRING | MF_UNCHECKED
                    };
                    let _ = AppendMenuW(sub, flags, IDM_KEYSET_BASE + i, pcwstr(&label));
                }
                let _ = AppendMenuW(
                    menu,
                    MF_POPUP | MF_STRING,
                    keyset_sub.0 as usize,
                    pcwstr(&keys_label),
                );
            }
        }

        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, IDM_RELOAD, pcwstr(&reload));
        let _ = AppendMenuW(menu, MF_STRING, IDM_HELP, pcwstr(&help));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, IDM_QUIT, pcwstr(&quit));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
            pt.x,
            pt.y,
            0,
            hwnd,
            None,
        );
        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
        if !mode_sub.0.is_null() {
            let _ = DestroyMenu(mode_sub);
        }
        if !keyset_sub.0.is_null() {
            let _ = DestroyMenu(keyset_sub);
        }
        let _ = DestroyMenu(menu);

        let cmd = cmd.0 as usize;
        match cmd {
            IDM_RUN => {
                let _ = PostMessageW(hwnd, WM_DK_TRIGGER, WPARAM(0), LPARAM(0));
            }
            IDM_SEARCH => {
                let _ = PostMessageW(hwnd, WM_DK_SEARCH, WPARAM(0), LPARAM(0));
            }
            IDM_SETTINGS => {
                let _ = PostMessageW(hwnd, WM_DK_SETTINGS, WPARAM(0), LPARAM(0));
            }
            IDM_RELOAD => {
                let _ = PostMessageW(hwnd, WM_DK_RELOAD, WPARAM(0), LPARAM(0));
            }
            IDM_HELP => {
                let _ = PostMessageW(hwnd, WM_DK_HELP, WPARAM(0), LPARAM(0));
            }
            IDM_QUIT => {
                let _ = PostMessageW(hwnd, WM_DK_QUIT, WPARAM(0), LPARAM(0));
            }
            id if id >= IDM_MODE_BASE && id < IDM_MODE_BASE + mode_files.len() => {
                let _ = PostMessageW(hwnd, WM_DK_MODE_PICK, WPARAM(id - IDM_MODE_BASE), LPARAM(0));
            }
            id if id >= IDM_KEYSET_BASE && id < IDM_KEYSET_BASE + keyset_ids.len() => {
                let _ = PostMessageW(
                    hwnd,
                    WM_DK_KEYSET_PICK,
                    WPARAM(id - IDM_KEYSET_BASE),
                    LPARAM(0),
                );
            }
            _ => {}
        }
    }
}

fn balloon(hwnd: HWND, title: &str, text: &str) {
    unsafe {
        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ICON_ID;
        nid.uFlags = NIF_INFO;
        nid.Anonymous.uTimeout = 4000;
        let t: Vec<u16> = format!("{title}\0").encode_utf16().collect();
        let b: Vec<u16> = format!("{text}\0").encode_utf16().collect();
        for (i, ch) in t.iter().take(nid.szInfoTitle.len()).enumerate() {
            nid.szInfoTitle[i] = *ch;
        }
        for (i, ch) in b.iter().take(nid.szInfo.len()).enumerate() {
            nid.szInfo[i] = *ch;
        }
        let _ = Shell_NotifyIconW(NIM_MODIFY, &mut nid);
    }
}
