//! Mode-switch chord arming + mouse-hook heartbeat (§4 / §6 / §10).

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicU16, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::{error, info};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, KillTimer, PostMessageW, SetTimer, SetWindowsHookExW, UnhookWindowsHookEx,
    HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

use super::messages::{HOST_CLASS, WM_DK_KEYSET_CHORD, WM_DK_MODE_ARM, WM_DK_MODE_CHORD};
use windows::Win32::UI::WindowsAndMessaging::FindWindowW;

const MODE_CHORD_TIMER_ID: usize = 90;
const HOOK_HEALTH_TIMER_ID: usize = 91;
const HOOK_HEALTH_INTERVAL_MS: u32 = 5_000;

static MODE_MOUSE_KIND: AtomicU16 = AtomicU16::new(1); // default x1
static MODE_MOUSE_SUPPRESS: AtomicBool = AtomicBool::new(true);
static MODE_ENABLED: AtomicBool = AtomicBool::new(true);
static MODE_ARMED: AtomicBool = AtomicBool::new(false);
static MODE_CHORD_HOOK: AtomicIsize = AtomicIsize::new(0);

static MOUSE_HOOK_BEAT_MS: AtomicU64 = AtomicU64::new(0);
static LAST_CURSOR_X: AtomicI32 = AtomicI32::new(i32::MIN);
static LAST_CURSOR_Y: AtomicI32 = AtomicI32::new(i32::MIN);

static MODE_PICK_FILES: Mutex<Vec<String>> = Mutex::new(Vec::new());
static KEYSET_CHORDS: Mutex<Vec<(char, String)>> = Mutex::new(Vec::new());
static KEYSET_IDS: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn set_mode_pick_files(files: Vec<String>) {
    if let Ok(mut g) = MODE_PICK_FILES.lock() {
        *g = files;
    }
}

pub fn mode_pick_file(index: usize) -> Option<String> {
    MODE_PICK_FILES.lock().ok()?.get(index).cloned()
}

pub fn set_keyset_chords(chords: Vec<(char, String)>, ids: Vec<String>) {
    if let Ok(mut g) = KEYSET_CHORDS.lock() {
        *g = chords;
    }
    if let Ok(mut g) = KEYSET_IDS.lock() {
        *g = ids;
    }
}

pub fn keyset_id_for_letter(letter: char) -> Option<String> {
    let c = letter.to_ascii_lowercase();
    KEYSET_CHORDS
        .lock()
        .ok()?
        .iter()
        .find(|(ch, _)| *ch == c)
        .map(|(_, id)| id.clone())
}

pub fn keyset_id_at(index: usize) -> Option<String> {
    KEYSET_IDS.lock().ok()?.get(index).cloned()
}

pub fn vk_to_letter(vk: u32) -> Option<char> {
    match vk {
        0x41..=0x5A => Some((b'a' + (vk - 0x41) as u8) as char),
        _ => None,
    }
}

pub fn configure_mode_mouse(button: &str, suppress: bool, enabled: bool) {
    let kind = match button.to_ascii_lowercase().as_str() {
        "x1" => 1u16,
        "middle" | "mbutton" => 2,
        "x2" => 0,
        _ => 1,
    };
    MODE_MOUSE_KIND.store(kind, Ordering::SeqCst);
    MODE_MOUSE_SUPPRESS.store(suppress, Ordering::SeqCst);
    MODE_ENABLED.store(enabled, Ordering::SeqCst);
    info!(button, suppress, enabled, "mode-switch mouse configured");
}

pub fn mode_mouse_kind() -> u16 {
    MODE_MOUSE_KIND.load(Ordering::SeqCst)
}

pub fn mode_mouse_suppress() -> bool {
    MODE_MOUSE_SUPPRESS.load(Ordering::SeqCst)
}

pub fn mode_mouse_enabled() -> bool {
    MODE_ENABLED.load(Ordering::SeqCst)
}

pub fn note_mouse_hook_activity() {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    MOUSE_HOOK_BEAT_MS.store(ms, Ordering::SeqCst);
}

pub fn mouse_hook_beat_ms() -> u64 {
    MOUSE_HOOK_BEAT_MS.load(Ordering::SeqCst)
}

pub fn take_cursor_sample(x: i32, y: i32) -> bool {
    let px = LAST_CURSOR_X.swap(x, Ordering::SeqCst);
    let py = LAST_CURSOR_Y.swap(y, Ordering::SeqCst);
    px != i32::MIN && (px != x || py != y)
}

pub fn arm_mode_chord(host: HWND, timeout_ms: u32) {
    if timeout_ms == 0 || !mode_mouse_enabled() {
        return;
    }
    disarm_mode_chord(host);
    MODE_ARMED.store(true, Ordering::SeqCst);
    unsafe {
        let _ = SetTimer(host, MODE_CHORD_TIMER_ID, timeout_ms, None);
        if let Err(e) = install_chord_hook() {
            error!("mode chord keyboard hook failed: {e}");
            MODE_ARMED.store(false, Ordering::SeqCst);
            let _ = KillTimer(host, MODE_CHORD_TIMER_ID);
        } else {
            info!(timeout_ms, "mode chord armed");
        }
    }
}

pub fn disarm_mode_chord(host: HWND) {
    MODE_ARMED.store(false, Ordering::SeqCst);
    uninstall_chord_hook();
    unsafe {
        let _ = KillTimer(host, MODE_CHORD_TIMER_ID);
    }
}

pub fn is_mode_armed() -> bool {
    MODE_ARMED.load(Ordering::SeqCst)
}

pub fn start_hook_health_timer(host: HWND) {
    unsafe {
        let _ = SetTimer(host, HOOK_HEALTH_TIMER_ID, HOOK_HEALTH_INTERVAL_MS, None);
    }
}

pub fn stop_hook_health_timer(host: HWND) {
    unsafe {
        let _ = KillTimer(host, HOOK_HEALTH_TIMER_ID);
    }
}

pub fn is_chord_timer(wparam: usize) -> bool {
    wparam == MODE_CHORD_TIMER_ID
}

pub fn is_health_timer(wparam: usize) -> bool {
    wparam == HOOK_HEALTH_TIMER_ID
}

/// Map VK to mode chord code (`"0"`…`"9"`).
pub fn vk_to_chord(vk: u32) -> Option<String> {
    match vk {
        0x30..=0x39 => Some(((b'0' + (vk - 0x30) as u8) as char).to_string()),
        0x60..=0x69 => Some(((b'0' + (vk - 0x60) as u8) as char).to_string()),
        _ => None,
    }
}

unsafe fn install_chord_hook() -> windows::core::Result<()> {
    let module = GetModuleHandleW(None)?;
    let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(mode_chord_keyboard_proc), module, 0)?;
    MODE_CHORD_HOOK.store(hook.0 as isize, Ordering::SeqCst);
    Ok(())
}

fn uninstall_chord_hook() {
    let h = MODE_CHORD_HOOK.swap(0, Ordering::SeqCst);
    if h != 0 {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(h as _));
        }
    }
}

unsafe extern "system" fn mode_chord_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code != 0 || !MODE_ARMED.load(Ordering::SeqCst) {
        return CallNextHookEx(None, code, wparam, lparam);
    }
    let msg = wparam.0 as u32;
    if msg != WM_KEYDOWN && msg != WM_SYSKEYDOWN {
        return CallNextHookEx(None, code, wparam, lparam);
    }
    let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    if info.flags.contains(LLKHF_INJECTED) {
        return CallNextHookEx(None, code, wparam, lparam);
    }
    if vk_to_chord(info.vkCode).is_some() {
        let host = FindWindowW(HOST_CLASS, None).unwrap_or_default();
        if !host.0.is_null() {
            let _ = PostMessageW(
                host,
                WM_DK_MODE_CHORD,
                WPARAM(info.vkCode as usize),
                LPARAM(0),
            );
        }
        return LRESULT(1);
    }
    if let Some(letter) = vk_to_letter(info.vkCode) {
        if keyset_id_for_letter(letter).is_some() {
            let host = FindWindowW(HOST_CLASS, None).unwrap_or_default();
            if !host.0.is_null() {
                let _ = PostMessageW(
                    host,
                    WM_DK_KEYSET_CHORD,
                    WPARAM(info.vkCode as usize),
                    LPARAM(0),
                );
            }
            return LRESULT(1);
        }
    }
    // Unrelated key cancels arm (do not swallow).
    let host = FindWindowW(HOST_CLASS, None).unwrap_or_default();
    if !host.0.is_null() {
        disarm_mode_chord(host);
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// Post MODE_ARM to host if mode mouse is enabled.
pub fn post_mode_arm() {
    if !mode_mouse_enabled() {
        return;
    }
    unsafe {
        let host = FindWindowW(HOST_CLASS, None).unwrap_or_default();
        if !host.0.is_null() {
            let _ = PostMessageW(host, WM_DK_MODE_ARM, WPARAM(0), LPARAM(0));
        }
    }
}
