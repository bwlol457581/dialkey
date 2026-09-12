//! Main Windows event loop (Phase 1 + Phase 2 host).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tracing::{debug, error, info, warn};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, CreateSolidBrush, DeleteObject, EndPaint, FillRect, GetStockObject,
    GetTextExtentPoint32W, InvalidateRect, LineTo, MoveToEx, RedrawWindow, SelectObject, SetBkMode,
    SetTextColor, TextOutW, DEFAULT_GUI_FONT, HDC, PAINTSTRUCT, PS_SOLID, RDW_ERASE,
    RDW_INVALIDATE, RDW_UPDATENOW, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetClientRect, GetCursorPos, GetMessageW, KillTimer, LoadCursorW, MessageBoxW, PostMessageW,
    PostQuitMessage, RegisterClassExW, SetTimer, SetWindowPos, SetWindowsHookExW, ShowWindow,
    TranslateMessage, UnhookWindowsHookEx, CS_HREDRAW, CS_VREDRAW, HHOOK, HWND_TOPMOST, IDC_ARROW,
    IDYES, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MB_ICONINFORMATION, MB_OK, MB_YESNO, MSG,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_SHOWNOACTIVATE, WH_KEYBOARD_LL,
    WM_CLOSE, WM_COPYDATA, WM_DESTROY, WM_HOTKEY, WM_KEYDOWN, WM_KEYUP, WM_PAINT, WM_SYSKEYDOWN,
    WM_SYSKEYUP, WM_TIMER, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP, WS_VISIBLE,
};

use crate::core::config::{
    list_slots_file_candidates, mouse_trigger_from_settings, normalize_slots_file_name,
    save_settings, slots_book_label, slots_file_is_writable, AppConfig,
};
use crate::core::i18n::{self, keys, t, tf};
use crate::core::keys::{
    digit_from_vk, KeyBindings, KeyRole, ResolvedKeys, KEY_SET_KEYBOARD, KEY_SET_NUMPAD,
    VK_CONTROL, VK_OEM_5, VK_SUBTRACT,
};
use crate::core::legend::{
    dial_area_ids, dial_head_display, LegendId, LegendSettings, DIAL_AREA_ROWS, LEGEND_ITEM_COUNT,
};
use crate::core::mode_map::build_mode_chord_map;
use crate::core::sequence::{Action, Mode, SequenceEvent, SequenceState};
use crate::platform::TrayIcon;

use super::autostart::WindowsAutoStart;
use super::hotkey::{hotkey_string_from_triggers, HotkeyTrigger};
use super::icon::load_class_icons;
use super::launch_ipc;
use super::launch_queue::{self, LaunchWorker};
use super::messages::{
    HOST_CLASS, TYPING_CLASS, WM_DK_KEY, WM_DK_KEYSET_CHORD, WM_DK_KEYSET_PICK,
    WM_DK_LAUNCH_FAILED, WM_DK_MODE_ARM, WM_DK_MODE_CHORD, WM_DK_MODE_PICK, WM_DK_QUIT,
    WM_DK_RELOAD, WM_DK_SEARCH, WM_DK_SEARCH_CLOSED, WM_DK_SETTINGS, WM_DK_SETTINGS_APPLIED,
    WM_DK_SETTINGS_CLOSED, WM_DK_TIMEOUT, WM_DK_TRAY, WM_DK_TRAY_DISPATCH, WM_DK_TRIGGER,
};
use super::mode_runtime;
use super::mouse_hook::{self, MouseHook};
use super::search;
use super::settings;
use super::tray::{register_taskbar_created_message, taskbar_created_msg, WindowsTray, WM_DK_HELP};

const WINDOW_TITLE: PCWSTR = w!("DialKey");
const TIMEOUT_TIMER_ID: usize = 1;
const FEEDBACK_TIMER_ID: usize = 2;
const TYPING_WIDTH: i32 = 420;
/// 10 dial rows + separator + 5 option slots + padding.
/// 11 dial rows (current + decade) + separator + 5 legend; 18px pitch + pad.
const TYPING_HEIGHT: i32 = 358;

pub(super) static HOST_HWND: AtomicIsize = AtomicIsize::new(0);
static TYPING_HWND: AtomicIsize = AtomicIsize::new(0);
static KB_HOOK_ALIVE: AtomicBool = AtomicBool::new(false);

pub(super) fn host_hwnd() -> HWND {
    HWND(HOST_HWND.load(Ordering::SeqCst) as *mut _)
}

static PRESSED_KEYS: OnceLock<Mutex<HashSet<u32>>> = OnceLock::new();

fn pressed_keys() -> &'static Mutex<HashSet<u32>> {
    PRESSED_KEYS.get_or_init(|| Mutex::new(HashSet::new()))
}

// ── Shared session view for painting ────────────────────────────────────

/// One typing-window row: optional key/id column + title (name / legend label).
#[derive(Clone)]
struct TypingRow {
    key: String,
    title: String,
}

struct ViewState {
    buffer: String,
    mode: Mode,
    /// Idle / MultiDigit: current + decade, then options.
    rows: Vec<TypingRow>,
    /// Index of the first body row after key rows (MultiDigit gap). `None` = no gap.
    body_from: Option<usize>,
    /// Acceptance feedback after a launch request (Phase 9-4).
    feedback: bool,
}

static VIEW: OnceLock<Mutex<ViewState>> = OnceLock::new();

fn view() -> &'static Mutex<ViewState> {
    VIEW.get_or_init(|| {
        Mutex::new(ViewState {
            buffer: String::new(),
            mode: Mode::Idle,
            rows: Vec::new(),
            body_from: None,
            feedback: false,
        })
    })
}

/// 0.16.4: launch duration. Warn near `LowLevelHooksTimeout` (~300 ms).
/// No keystroke contents and no paths.
fn log_launch_gate(kind: &'static str, launch_ms: u128, launch_ok: bool, feedback_ms: u32) {
    if launch_ms >= 250 {
        warn!(kind, launch_ms, ok = launch_ok, feedback_ms, "launch slow");
    } else {
        debug!(
            kind,
            launch_ms,
            ok = launch_ok,
            feedback_ms,
            "launch finished"
        );
    }
}

// ── RAII hooks ──────────────────────────────────────────────────────────

struct KeyboardHook {
    hook: HHOOK,
}

impl KeyboardHook {
    unsafe fn install() -> anyhow::Result<Self> {
        let module = GetModuleHandleW(None)?;
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0)?;
        KB_HOOK_ALIVE.store(true, Ordering::SeqCst);
        if let Ok(mut set) = pressed_keys().lock() {
            set.clear();
        }
        info!("keyboard hook installed");
        Ok(Self { hook })
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        unsafe {
            if let Err(e) = UnhookWindowsHookEx(self.hook) {
                error!("failed to uninstall keyboard hook: {e}");
            } else {
                info!("keyboard hook uninstalled");
            }
        }
        KB_HOOK_ALIVE.store(false, Ordering::SeqCst);
        if let Ok(mut set) = pressed_keys().lock() {
            set.clear();
        }
    }
}

// ── Hook callbacks ──────────────────────────────────────────────────────

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != 0 {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let msg = wparam.0 as u32;
    let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
    let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
    if !is_down && !is_up {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    if info.flags.contains(LLKHF_INJECTED) {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let vk = info.vkCode;
    let hwnd = HWND(TYPING_HWND.load(Ordering::SeqCst) as *mut _);
    if hwnd.0.is_null() {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    // Track press state for autorepeat suppression.
    if is_up {
        if let Ok(mut set) = pressed_keys().lock() {
            set.remove(&vk);
        }
        // Suppress key-up for keys we consume so the foreground app stays clean.
        if is_consumed_vk(vk) {
            return LRESULT(1);
        }
        return CallNextHookEx(None, code, wparam, lparam);
    }

    // key down
    if let Ok(mut set) = pressed_keys().lock() {
        if !set.insert(vk) {
            // Autorepeat — ignore, still swallow if it's a consumed key.
            if is_consumed_vk(vk) {
                return LRESULT(1);
            }
            return CallNextHookEx(None, code, wparam, lparam);
        }
    }

    if is_consumed_vk(vk) {
        let _ = PostMessageW(hwnd, WM_DK_KEY, WPARAM(vk as usize), LPARAM(0));
        return LRESULT(1);
    }

    CallNextHookEx(None, code, wparam, lparam)
}

fn is_consumed_vk(vk: u32) -> bool {
    let vk = vk as u16;
    if let Ok(k) = resolved_keys_slot().lock() {
        if k.is_start(vk)
            || vk == k.confirm
            || vk == k.cancel
            || k.is_search(vk)
            || k.is_open_workdir(vk)
            || vk == k.digit_back
            || k.is_wake(vk)
        {
            return true;
        }
    }
    // Digits after role keys so digitBack can own VK_RIGHT etc.
    digit_from_vk(vk).is_some()
}

static RESOLVED_KEYS: OnceLock<Mutex<ResolvedKeys>> = OnceLock::new();

fn resolved_keys_slot() -> &'static Mutex<ResolvedKeys> {
    RESOLVED_KEYS.get_or_init(|| {
        Mutex::new(ResolvedKeys {
            start: 0x6B,
            confirm: 0x0D,
            cancel: 0x1B,
            search: VK_OEM_5,
            open_workdir: VK_CONTROL,
            digit_back: 0x27,
            wake: VK_SUBTRACT,
            wake_enabled: false,
        })
    })
}

// ── Typing session ──────────────────────────────────────────────────────

struct TypingSession {
    hwnd: HWND,
    kb_hook: Option<KeyboardHook>,
    sequence: SequenceState,
    keys: ResolvedKeys,
    /// Full bindings (for legend display labels).
    bindings: KeyBindings,
    timeout_sec: u32,
    /// Waiting for `feedbackMs` timer before closing.
    feedback_pending: bool,
}

impl TypingSession {
    unsafe fn open(
        instance: windows::Win32::Foundation::HINSTANCE,
        config: &AppConfig,
    ) -> anyhow::Result<Self> {
        let bindings = config.settings.keys.clone();
        let keys = bindings.resolved();
        if let Ok(mut slot) = resolved_keys_slot().lock() {
            *slot = keys;
        }

        // Spec §9: primary top-right, window top-right anchored.
        let (x, y) = super::placement::typing_window_pos(TYPING_WIDTH, TYPING_HEIGHT);

        let hwnd = CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            TYPING_CLASS,
            WINDOW_TITLE,
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            TYPING_WIDTH,
            TYPING_HEIGHT,
            None,
            None,
            instance,
            None,
        )?;

        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

        TYPING_HWND.store(hwnd.0 as isize, Ordering::SeqCst);

        let kb_hook = KeyboardHook::install()?;
        let sequence =
            SequenceState::new(config.settings.max_digits, config.settings.instant.clone());
        let timeout_sec = config.settings.timeout_sec;

        if timeout_sec > 0 {
            SetTimer(hwnd, TIMEOUT_TIMER_ID, timeout_sec * 1000, None);
        }

        let mut session = Self {
            hwnd,
            kb_hook: Some(kb_hook),
            sequence,
            keys,
            bindings,
            timeout_sec,
            feedback_pending: false,
        };
        session.refresh_view(config);
        info!("typing window shown (NOACTIVATE + TOPMOST)");
        Ok(session)
    }

    fn refresh_view(&mut self, config: &AppConfig) {
        let key_rows = typing_key_rows(
            &self.bindings,
            &config.settings.legend,
            self.sequence.mode(),
        );
        let idle = self.sequence.mode() == Mode::Idle;
        let prefix = if idle { "" } else { self.sequence.buffer() };
        let mut rows = dial_area_rows(
            prefix,
            config.settings.max_digits,
            idle,
            &self.bindings.display_label(KeyRole::Start),
            &config.settings.instant,
            &config.slots,
        );
        rows.extend(key_rows);
        let body_from = Some(DIAL_AREA_ROWS);

        if let Ok(mut v) = view().lock() {
            v.buffer = self.sequence.buffer().to_string();
            v.mode = self.sequence.mode();
            v.rows = rows;
            v.body_from = body_from;
            v.feedback = false;
        }
        unsafe {
            let _ = InvalidateRect(self.hwnd, None, true);
        }
    }

    fn handle_key(&mut self, vk: u16, config: &AppConfig, exe_dir: &PathBuf) -> bool {
        // returns true if session should close
        if self.feedback_pending {
            // During acceptance display, only Cancel closes early.
            return vk == self.keys.cancel;
        }

        if self.keys.is_wake(vk) {
            return false;
        }

        let event = if vk == self.keys.cancel {
            SequenceEvent::Cancel
        } else if self.keys.is_start(vk) {
            SequenceEvent::Start
        } else if vk == self.keys.confirm {
            SequenceEvent::Confirm
        } else if self.keys.is_open_workdir(vk) {
            SequenceEvent::OpenWorkdir
        } else if vk == self.keys.digit_back {
            SequenceEvent::DigitBack
        } else if self.keys.is_search(vk) {
            SequenceEvent::Search
        } else if let Some(d) = digit_from_vk(vk) {
            SequenceEvent::Digit(d)
        } else {
            return false;
        };

        // Reset idle timeout on activity
        if self.timeout_sec > 0 {
            unsafe {
                let _ = KillTimer(self.hwnd, TIMEOUT_TIMER_ID);
                SetTimer(self.hwnd, TIMEOUT_TIMER_ID, self.timeout_sec * 1000, None);
            }
        }

        match self.sequence.handle(event) {
            Action::None => false,
            Action::Redraw => {
                if self.sequence.mode() == Mode::MultiDigit && self.sequence.buffer().is_empty() {
                    info!("entered multi-digit mode");
                }
                self.refresh_view(config);
                false
            }
            Action::Close => true,
            Action::OpenSearch => {
                // Close typing session; host loop opens Search.
                let host = HWND(HOST_HWND.load(Ordering::SeqCst) as *mut _);
                unsafe {
                    let _ = PostMessageW(host, WM_DK_SEARCH, WPARAM(0), LPARAM(0));
                }
                true
            }
            Action::Launch { id } => self.run_slot_launch(&id, config, exe_dir, false),
            Action::OpenWorkdirOnly { id } => self.run_slot_launch(&id, config, exe_dir, true),
        }
    }

    fn post_launch_failed() {
        let host = HWND(HOST_HWND.load(Ordering::SeqCst) as *mut _);
        unsafe {
            let _ = PostMessageW(host, WM_DK_LAUNCH_FAILED, WPARAM(0), LPARAM(0));
        }
    }

    /// Paint `Launching` (or failed) and flush so a slow spawn cannot leave `+10` on screen.
    fn apply_feedback_text(&mut self, id: &str, name: &str, ok: bool, folder_only: bool) {
        let title = match (folder_only, ok) {
            (false, true) => tf(keys::APP_TYPING_FEEDBACK, &[("id", id), ("name", name)]),
            (false, false) => tf(
                keys::APP_TYPING_FEEDBACK_FAILED,
                &[("id", id), ("name", name)],
            ),
            (true, true) => tf(
                keys::APP_TYPING_FEEDBACK_OPEN,
                &[("id", id), ("name", name)],
            ),
            (true, false) => tf(
                keys::APP_TYPING_FEEDBACK_OPEN_FAILED,
                &[("id", id), ("name", name)],
            ),
        };
        if let Ok(mut v) = view().lock() {
            v.feedback = true;
            v.body_from = None;
            v.rows = vec![TypingRow {
                key: String::new(),
                title,
            }];
        }
        unsafe {
            let _ = RedrawWindow(
                self.hwnd,
                None,
                None,
                RDW_ERASE | RDW_INVALIDATE | RDW_UPDATENOW,
            );
        }
    }

    /// Accept the request on the typing window first, then spawn (0.16.4).
    /// Returns true if the session should close now (`feedbackMs` == 0).
    fn run_slot_launch(
        &mut self,
        id: &str,
        config: &AppConfig,
        exe_dir: &PathBuf,
        folder_only: bool,
    ) -> bool {
        let ms = config.settings.feedback_ms;
        let name = config
            .slots
            .get(id)
            .map(|s| s.name.as_str())
            .unwrap_or("")
            .to_string();
        let kind = if folder_only {
            "open_workdir"
        } else {
            "launch"
        };
        if config.slots.get(id).is_none() {
            warn!("no slot registered for id {id}");
            Self::post_launch_failed();
            log_launch_gate(kind, 0, false, ms);
            if ms == 0 {
                return true;
            }
            self.begin_feedback(id, &name, ms, false, folder_only);
            return false;
        }
        if ms > 0 {
            self.begin_feedback(id, &name, ms, true, folder_only);
        } else {
            self.apply_feedback_text(id, &name, true, folder_only);
        }
        let launch_ok = match config.slots.get(id) {
            Some(slot) => match launch_queue::queue(
                slot.clone(),
                exe_dir.clone(),
                config.slots.clone(),
                folder_only,
                host_hwnd(),
            ) {
                Ok(()) => {
                    debug!(slot_id = %id, folder_only, "launch queued");
                    true
                }
                Err(e) => {
                    error!("queue launch failed for slot {id}: {e}");
                    false
                }
            },
            None => false,
        };
        if !launch_ok {
            Self::post_launch_failed();
            if ms > 0 {
                self.begin_feedback(id, &name, ms, false, folder_only);
            } else {
                self.apply_feedback_text(id, &name, false, folder_only);
            }
        }
        ms == 0
    }

    fn begin_feedback(&mut self, id: &str, name: &str, ms: u32, ok: bool, folder_only: bool) {
        self.feedback_pending = true;
        self.apply_feedback_text(id, name, ok, folder_only);
        unsafe {
            let _ = KillTimer(self.hwnd, TIMEOUT_TIMER_ID);
            let _ = KillTimer(self.hwnd, FEEDBACK_TIMER_ID);
            SetTimer(self.hwnd, FEEDBACK_TIMER_ID, ms, None);
        }
        info!(
            slot_id = %id,
            feedback_ms = ms,
            ok,
            "showing launch feedback"
        );
    }
}

impl Drop for TypingSession {
    fn drop(&mut self) {
        TYPING_HWND.store(0, Ordering::SeqCst);
        self.kb_hook = None;
        unsafe {
            if !self.hwnd.0.is_null() {
                let _ = KillTimer(self.hwnd, TIMEOUT_TIMER_ID);
                let _ = KillTimer(self.hwnd, FEEDBACK_TIMER_ID);
                let _ = DestroyWindow(self.hwnd);
                self.hwnd = HWND::default();
            }
        }
        if KB_HOOK_ALIVE.load(Ordering::SeqCst) {
            warn!("keyboard hook still marked alive after session drop");
        }
    }
}

// ── Window proc / paint ─────────────────────────────────────────────────

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            paint_window(hdc, hwnd);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DK_KEY => {
            let host = HWND(HOST_HWND.load(Ordering::SeqCst) as *mut _);
            let _ = PostMessageW(host, WM_DK_KEY, wparam, LPARAM(0));
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == TIMEOUT_TIMER_ID || wparam.0 == FEEDBACK_TIMER_ID => {
            let host = HWND(HOST_HWND.load(Ordering::SeqCst) as *mut _);
            let _ = PostMessageW(host, WM_DK_TIMEOUT, WPARAM(0), LPARAM(0));
            LRESULT(0)
        }
        WM_CLOSE => {
            let host = HWND(HOST_HWND.load(Ordering::SeqCst) as *mut _);
            let _ = PostMessageW(host, WM_DK_TIMEOUT, WPARAM(0), LPARAM(0));
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn paint_window(hdc: HDC, hwnd: HWND) {
    let mut rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut rect);
    if rect.right <= 0 || rect.bottom <= 0 {
        rect = RECT {
            left: 0,
            top: 0,
            right: TYPING_WIDTH,
            bottom: TYPING_HEIGHT,
        };
    }
    // Spec §9: post-it yellow #FFF9C4 (COLORREF is 0x00BBGGRR).
    let brush = CreateSolidBrush(COLORREF(0x00_C4_F9_FF));
    FillRect(hdc, &rect, brush);
    let _ = SetBkMode(hdc, TRANSPARENT);
    let _ = SetTextColor(hdc, COLORREF(0x00_22_22_22));
    let font = GetStockObject(DEFAULT_GUI_FONT);
    let old = SelectObject(hdc, font);

    let (rows, body_from) = if let Ok(v) = view().lock() {
        (v.rows.clone(), v.body_from)
    } else {
        (Vec::new(), None)
    };

    // Title column starts after the widest key (proportional font — char padding won't align).
    const PAD_X: i32 = 12;
    const KEY_GAP: i32 = 12;
    let mut key_max_w = 0i32;
    for row in &rows {
        if row.key.is_empty() {
            continue;
        }
        key_max_w = key_max_w.max(measure_hdc_text_width(hdc, &row.key));
    }
    let title_x = PAD_X + key_max_w + KEY_GAP;

    let mut y = 12i32;
    for (i, row) in rows.iter().enumerate() {
        if body_from == Some(i) || row.title.starts_with("--------") {
            let pen = CreatePen(PS_SOLID, 1, COLORREF(0x00_22_22_22));
            let old_pen = SelectObject(hdc, pen);
            let _ = MoveToEx(hdc, PAD_X, y + 8, None);
            let _ = LineTo(hdc, (rect.right - PAD_X).max(PAD_X + 40), y + 8);
            let _ = SelectObject(hdc, old_pen);
            let _ = DeleteObject(pen);
            y += 18;
            continue;
        }
        if row.key.is_empty() {
            text_out_at(hdc, PAD_X, y, &row.title);
        } else {
            text_out_at(hdc, PAD_X, y, &row.key);
            text_out_at(hdc, title_x, y, &row.title);
        }
        y += 18;
    }

    let _ = SelectObject(hdc, old);
    let _ = DeleteObject(brush);
}

unsafe fn measure_hdc_text_width(hdc: HDC, text: &str) -> i32 {
    let wide: Vec<u16> = text.encode_utf16().collect();
    if wide.is_empty() {
        return 0;
    }
    let mut size = SIZE::default();
    if GetTextExtentPoint32W(hdc, &wide, &mut size).as_bool() {
        size.cx
    } else {
        0
    }
}

unsafe fn text_out_at(hdc: HDC, x: i32, y: i32, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().collect();
    if wide.is_empty() {
        return;
    }
    let _ = TextOutW(hdc, x, y, &wide);
}

/// Current typed id, Start cue when Multi is armed, plus `{prefix}0`…`{prefix}9`.
fn dial_area_rows(
    prefix: &str,
    max_digits: usize,
    idle: bool,
    start_display: &str,
    instant: &HashMap<String, bool>,
    slots: &crate::core::slots::SlotRegistry,
) -> Vec<TypingRow> {
    dial_area_ids(prefix, max_digits)
        .into_iter()
        .enumerate()
        .map(|(i, id)| {
            if i == 0 {
                let key = dial_head_display(idle, prefix, start_display);
                let title = if prefix.is_empty() {
                    String::new()
                } else {
                    slots
                        .get(prefix)
                        .map(|s| s.name.clone())
                        .unwrap_or_default()
                };
                return TypingRow { key, title };
            }
            if id.is_empty() {
                return TypingRow {
                    key: String::new(),
                    title: String::new(),
                };
            }
            let title = if idle {
                if instant.get(&id).copied().unwrap_or(false) {
                    slots.get(&id).map(|s| s.name.clone()).unwrap_or_default()
                } else {
                    String::new()
                }
            } else {
                slots.get(&id).map(|s| s.name.clone()).unwrap_or_default()
            };
            TypingRow { key: id, title }
        })
        .collect()
}

/// Separator + visible options, padded to `LEGEND_ITEM_COUNT` so the window height is stable.
fn typing_key_rows(bindings: &KeyBindings, legend: &LegendSettings, mode: Mode) -> Vec<TypingRow> {
    let mut rows = vec![TypingRow {
        key: String::new(),
        title: "--------".into(),
    }];
    for id in legend.visible(mode) {
        rows.push(legend_option_row(bindings, id));
    }
    while rows.len() < 1 + LEGEND_ITEM_COUNT {
        rows.push(TypingRow {
            key: String::new(),
            title: String::new(),
        });
    }
    rows
}

fn legend_option_row(bindings: &KeyBindings, id: LegendId) -> TypingRow {
    let title = match id {
        LegendId::Start => t(keys::APP_TYPING_LEGEND_MULTI),
        LegendId::OpenWorkdir => t(keys::APP_TYPING_LEGEND_OPEN_WORKDIR),
        LegendId::Search => t(keys::APP_TYPING_LEGEND_SEARCH),
        LegendId::DigitBack => t(keys::APP_TYPING_LEGEND_DIGIT_BACK),
        LegendId::Cancel => t(keys::APP_TYPING_LEGEND_ESC),
    };
    TypingRow {
        key: bindings.display_label(id.key_role()),
        title,
    }
}

fn refresh_mode_switch(config: &AppConfig, tray: &mut WindowsTray) {
    let launch = mouse_trigger_from_settings(&config.settings);
    let ms = &config.settings.mode_switch;
    let same =
        !ms.mouse_button.is_empty() && launch.button.eq_ignore_ascii_case(ms.mouse_button.trim());
    let enabled = ms.chord_timeout_ms > 0 && !same;
    if same {
        warn!(
            launch = %launch.button,
            mode = %ms.mouse_button,
            "modeSwitch.mouseButton matches launch trigger; mode mouse disabled"
        );
    }
    mode_runtime::configure_mode_mouse(&ms.mouse_button, ms.suppress, enabled);
    let files = list_slots_file_candidates(&config.dir);
    let labels = mode_menu_labels(&config.dir, &files);
    let current = normalize_slots_file_name(&config.settings.slots_file);
    tray.set_mode_files(files, labels, current);
    refresh_key_sets(config, tray);
}

fn key_set_ui_label(id: &str) -> String {
    match id {
        KEY_SET_NUMPAD => t(keys::KEY_SET_NUMPAD),
        KEY_SET_KEYBOARD => t(keys::KEY_SET_KEYBOARD),
        other => other.to_string(),
    }
}

fn refresh_key_sets(config: &AppConfig, tray: &mut WindowsTray) {
    let keys = &config.settings.keys;
    let ids: Vec<String> = keys.sets.iter().map(|s| s.id.clone()).collect();
    let labels: Vec<String> = keys.sets.iter().map(|s| key_set_ui_label(&s.id)).collect();
    tray.set_key_sets(ids.clone(), labels, keys.active.clone());
    mode_runtime::set_keyset_chords(keys.chord_map(), ids);
}

fn switch_to_key_set(
    config: &mut AppConfig,
    id: &str,
    exe: &std::path::Path,
    tray: &mut WindowsTray,
    hotkey: &mut HotkeyTrigger,
    autostart: &WindowsAutoStart,
) {
    if settings::is_open() {
        return;
    }
    config.settings.keys.normalize();
    let already = config.settings.keys.active == id;
    if !already && !config.settings.keys.activate(id) {
        warn!(id, "key set switch ignored (unknown id)");
        return;
    }
    if let Err(e) = save_settings(&config.dir, &config.settings) {
        error!(error = %e, "failed to save settings after key set switch");
        tray.notify(
            &t(keys::TRAY_NOTIFY_TITLE),
            &t(keys::TRAY_NOTIFY_RELOAD_FAILED),
        );
        return;
    }
    if !already {
        match crate::core::load_app_config(exe, Some(&config.dir)) {
            Ok(new_cfg) => {
                *config = new_cfg;
                config.open_settings_on_start = false;
                apply_config_runtime(config, hotkey, autostart);
            }
            Err(e) => {
                error!(error = %e, "reload after key set switch failed");
                tray.notify(
                    &t(keys::TRAY_NOTIFY_TITLE),
                    &t(keys::TRAY_NOTIFY_RELOAD_FAILED),
                );
                return;
            }
        }
    }
    if let Ok(mut slot) = resolved_keys_slot().lock() {
        *slot = config.settings.keys.resolved();
    }
    refresh_mode_switch(config, tray);
    info!(id, "key set switched");
    tray.notify(
        &t(keys::TRAY_NOTIFY_TITLE),
        &tf(
            keys::TRAY_NOTIFY_KEY_SET,
            &[("name", &key_set_ui_label(&config.settings.keys.active))],
        ),
    );
}

fn mode_menu_labels(dir: &std::path::Path, files: &[String]) -> Vec<String> {
    files.iter().map(|f| slots_book_label(dir, f)).collect()
}

fn switch_to_slots_file(
    config: &mut AppConfig,
    basename: &str,
    exe: &std::path::Path,
    tray: &mut WindowsTray,
    hotkey: &mut HotkeyTrigger,
    autostart: &WindowsAutoStart,
    mouse_hook: &mut MouseHook,
) {
    let name = normalize_slots_file_name(basename);
    if name.is_empty() || !slots_file_is_writable(&name) {
        warn!(file = %basename, "mode switch ignored (invalid or read-only slots file)");
        return;
    }
    if config.settings.slots_file == name {
        info!(file = %name, "mode already active");
        tray.notify(
            &t(keys::TRAY_NOTIFY_TITLE),
            &tf(
                keys::TRAY_NOTIFY_MODE_SWITCHED,
                &[("file", &slots_book_label(&config.dir, &name))],
            ),
        );
        return;
    }
    config.settings.slots_file = name.clone();
    if let Err(e) = save_settings(&config.dir, &config.settings) {
        error!(error = %e, "failed to save settings after mode switch");
        tray.notify(
            &t(keys::TRAY_NOTIFY_TITLE),
            &t(keys::TRAY_NOTIFY_RELOAD_FAILED),
        );
        return;
    }
    match crate::core::load_app_config(exe, Some(&config.dir)) {
        Ok(new_cfg) => {
            *config = new_cfg;
            config.open_settings_on_start = false;
            apply_config_runtime(config, hotkey, autostart);
            refresh_mode_switch(config, tray);
            if let Err(e) = mouse_hook.reinstall() {
                error!("mouse hook reinstall after mode switch failed: {e}");
            }
            info!(file = %name, "mode switched");
            tray.notify(
                &t(keys::TRAY_NOTIFY_TITLE),
                &tf(
                    keys::TRAY_NOTIFY_MODE_SWITCHED,
                    &[("file", &slots_book_label(&config.dir, &name))],
                ),
            );
        }
        Err(e) => {
            error!(error = %e, "reload after mode switch failed");
            tray.notify(
                &t(keys::TRAY_NOTIFY_TITLE),
                &t(keys::TRAY_NOTIFY_RELOAD_FAILED),
            );
        }
    }
}

fn apply_mouse_trigger(config: &AppConfig) {
    let t = mouse_trigger_from_settings(&config.settings);
    let kind = match t.button.to_ascii_lowercase().as_str() {
        "x1" => 1u16,
        "middle" | "mbutton" => 2u16,
        _ => 0u16,
    };
    mouse_hook::configure(kind, t.suppress);
    info!(button = %t.button, suppress = t.suppress, "mouse trigger configured");
}

fn apply_config_runtime(
    config: &AppConfig,
    hotkey: &mut HotkeyTrigger,
    autostart: &WindowsAutoStart,
) {
    apply_mouse_trigger(config);
    let hk = hotkey_string_from_triggers(&config.settings.triggers);
    let _ = hotkey.install_from_string(&hk);
    if let Err(e) = autostart.apply(config.settings.autostart) {
        warn!("autostart apply failed: {e}");
    }
    install_ui_catalog(config);
    launch_ipc::set_ctx(exe_dir(), config.slots.clone());
}

fn install_ui_catalog(config: &AppConfig) {
    let exe = exe_dir();
    let os = super::locale::os_ui_language();
    let cat = i18n::resolve(
        &config.settings.ui.locale,
        &i18n::lang_dir_next_to_exe(&exe),
        os.as_deref(),
    );
    info!(
        locale = %cat.resolved_language(),
        fallback = cat.used_fallback(),
        "UI language catalog ready"
    );
    i18n::install(cat);
}

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn open_docs_url(url: &str) {
    if url.is_empty() {
        warn!("docsUrl is empty");
        return;
    }
    unsafe {
        let wide: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide.as_ptr()),
            None,
            None,
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
}

pub fn run(mut config: AppConfig) -> anyhow::Result<()> {
    // Best-effort "already open → foreground" (no polling; one EnumWindows per launch).
    crate::core::launch::set_focus_existing_handler(super::focus_existing::try_focus_existing);
    crate::core::launch::set_shell_open_handler(super::shell::shell_open);

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    apply_mouse_trigger(&config);
    let exe = exe_dir();
    let exe_path = std::env::current_exe().unwrap_or_else(|_| exe.join("dialkey.exe"));
    install_ui_catalog(&config);

    let autostart = WindowsAutoStart::new(&exe_path);
    if let Err(e) = autostart.apply(config.settings.autostart) {
        warn!("autostart apply failed: {e}");
    }

    unsafe {
        let instance = GetModuleHandleW(None)?;
        register_taskbar_created_message();

        // Typing window class
        let (app_icon, app_icon_sm) = load_class_icons();

        let typing_wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hIcon: app_icon,
            hIconSm: app_icon_sm,
            lpszClassName: TYPING_CLASS,
            ..Default::default()
        };
        if RegisterClassExW(&typing_wc) == 0 {
            anyhow::bail!("RegisterClassExW(typing) failed");
        }

        // Host message-only window (tray / hotkey / second-instance target)
        let host_wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(host_proc),
            hInstance: instance.into(),
            hIcon: app_icon,
            hIconSm: app_icon_sm,
            lpszClassName: HOST_CLASS,
            ..Default::default()
        };
        if RegisterClassExW(&host_wc) == 0 {
            anyhow::bail!("RegisterClassExW(host) failed");
        }

        // Top-level (not HWND_MESSAGE): TaskbarCreated is broadcast only to
        // top-level windows. TOOLWINDOW keeps it off the taskbar.
        let host = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            HOST_CLASS,
            w!("DialKeyHost"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            instance,
            None,
        )?;
        HOST_HWND.store(host.0 as isize, Ordering::SeqCst);

        let mut tray = WindowsTray::new(host);
        if let Err(e) = tray.add() {
            // Mouse/hotkey still work; do not abort the resident process.
            error!("tray icon unavailable: {e}");
        } else {
            tray.notify(
                &t(keys::TRAY_NOTIFY_TITLE),
                &tf(
                    keys::TRAY_NOTIFY_STARTED,
                    &[("version", env!("CARGO_PKG_VERSION"))],
                ),
            );
        }

        let mut hotkey = HotkeyTrigger::new(host);
        let hk = hotkey_string_from_triggers(&config.settings.triggers);
        hotkey.install_from_string(&hk)?;

        let _launch_worker = LaunchWorker::start()?;
        let mut mouse_hook = MouseHook::install()?;
        let mut session: Option<TypingSession> = None;
        launch_ipc::set_ctx(exe.clone(), config.slots.clone());
        refresh_mode_switch(&config, &mut tray);
        mode_runtime::start_hook_health_timer(host);

        info!("ready — tray / mouse / hotkey triggers active");
        info!("log file is beside the exe under logs\\dialkey.log");
        #[cfg(debug_assertions)]
        info!("debug: start key accepts VK_ADD and VK_OEM_PLUS; legacy slash search accepts VK_DIVIDE and VK_OEM_2");
        #[cfg(not(debug_assertions))]
        info!("legacy slash search accepts VK_DIVIDE and VK_OEM_2 when bound to either; default search is VK_OEM_5 (\\)");

        if config.open_settings_on_start {
            info!("first run — opening settings");
            let _ = PostMessageW(host, WM_DK_SETTINGS, WPARAM(0), LPARAM(0));
        }

        let mut msg = MSG::default();
        loop {
            let ok = GetMessageW(&mut msg, None, 0, 0);
            if !ok.as_bool() {
                break;
            }

            // Handle app messages whether posted to host or as thread messages.
            let is_ours = msg.hwnd == host || msg.hwnd.0.is_null();
            if is_ours {
                match msg.message {
                    WM_DK_TRIGGER | WM_HOTKEY => {
                        if settings::triggers_disabled() {
                            continue;
                        }
                        if settings::is_open() {
                            continue;
                        }
                        if search::is_open() {
                            info!("trigger while Search open — closing Search");
                            search::close_if_open();
                            continue;
                        }
                        if session.is_some() {
                            info!("trigger while open — closing");
                            session = None;
                        } else {
                            match TypingSession::open(instance.into(), &config) {
                                Ok(s) => session = Some(s),
                                Err(e) => {
                                    error!("failed to open typing window: {e}");
                                    tray.notify(
                                        &t(keys::TRAY_NOTIFY_TITLE),
                                        &t(keys::TRAY_NOTIFY_OPEN_TYPING_FAILED),
                                    );
                                }
                            }
                        }
                        continue;
                    }
                    WM_DK_KEY => {
                        let vk = msg.wParam.0 as u16;
                        if let Some(s) = session.as_mut() {
                            if s.handle_key(vk, &config, &exe) {
                                session = None;
                            }
                        }
                        continue;
                    }
                    WM_DK_TIMEOUT => {
                        info!("typing session closed (timeout/cancel)");
                        session = None;
                        continue;
                    }
                    WM_DK_TRAY_DISPATCH => {
                        tray.handle_callback(msg.wParam, msg.lParam);
                        continue;
                    }
                    m if m == taskbar_created_msg() && m != 0 => {
                        if let Err(e) = tray.reregister() {
                            error!("tray reregister failed: {e}");
                        }
                        continue;
                    }
                    WM_DK_RELOAD => {
                        info!("reloading configuration");
                        if session.is_some() {
                            session = None;
                        }
                        search::close_if_open();
                        settings::close_if_open();
                        match crate::core::load_app_config(&exe, Some(&config.dir)) {
                            Ok(new_cfg) => {
                                config = new_cfg;
                                config.open_settings_on_start = false;
                                apply_config_runtime(&config, &mut hotkey, &autostart);
                                refresh_mode_switch(&config, &mut tray);
                                tray.notify(
                                    &t(keys::TRAY_NOTIFY_TITLE),
                                    &t(keys::TRAY_NOTIFY_RELOADED),
                                );
                            }
                            Err(e) => {
                                error!("reload failed: {e}");
                                tray.notify(
                                    &t(keys::TRAY_NOTIFY_TITLE),
                                    &t(keys::TRAY_NOTIFY_RELOAD_FAILED),
                                );
                            }
                        }
                        continue;
                    }
                    WM_DK_MODE_ARM => {
                        if settings::triggers_disabled() || settings::is_open() {
                            continue;
                        }
                        mode_runtime::arm_mode_chord(
                            host,
                            config.settings.mode_switch.chord_timeout_ms,
                        );
                        continue;
                    }
                    WM_DK_MODE_CHORD => {
                        mode_runtime::disarm_mode_chord(host);
                        let vk = msg.wParam.0 as u32;
                        let Some(code) = mode_runtime::vk_to_chord(vk) else {
                            continue;
                        };
                        let candidates = list_slots_file_candidates(&config.dir);
                        let map =
                            build_mode_chord_map(&candidates, &config.settings.mode_switch.keys);
                        if let Some(file) = map.get(&code).cloned() {
                            switch_to_slots_file(
                                &mut config,
                                &file,
                                &exe,
                                &mut tray,
                                &mut hotkey,
                                &autostart,
                                &mut mouse_hook,
                            );
                        } else {
                            warn!(chord = %code, "no mode mapped to chord");
                        }
                        continue;
                    }
                    WM_DK_MODE_PICK => {
                        let idx = msg.wParam.0;
                        if let Some(file) = mode_runtime::mode_pick_file(idx) {
                            switch_to_slots_file(
                                &mut config,
                                &file,
                                &exe,
                                &mut tray,
                                &mut hotkey,
                                &autostart,
                                &mut mouse_hook,
                            );
                        }
                        continue;
                    }
                    WM_DK_KEYSET_CHORD => {
                        if settings::is_open() {
                            continue;
                        }
                        mode_runtime::disarm_mode_chord(host);
                        let vk = msg.wParam.0 as u32;
                        let Some(letter) = mode_runtime::vk_to_letter(vk) else {
                            continue;
                        };
                        if let Some(id) = mode_runtime::keyset_id_for_letter(letter) {
                            if session.is_some() {
                                session = None;
                            }
                            switch_to_key_set(
                                &mut config,
                                &id,
                                &exe,
                                &mut tray,
                                &mut hotkey,
                                &autostart,
                            );
                        }
                        continue;
                    }
                    WM_DK_KEYSET_PICK => {
                        if settings::is_open() {
                            continue;
                        }
                        let idx = msg.wParam.0;
                        if let Some(id) = mode_runtime::keyset_id_at(idx) {
                            if session.is_some() {
                                session = None;
                            }
                            switch_to_key_set(
                                &mut config,
                                &id,
                                &exe,
                                &mut tray,
                                &mut hotkey,
                                &autostart,
                            );
                        }
                        continue;
                    }
                    WM_TIMER => {
                        if mode_runtime::is_chord_timer(msg.wParam.0) {
                            mode_runtime::disarm_mode_chord(host);
                        } else if mode_runtime::is_health_timer(msg.wParam.0) {
                            let mut pt = POINT::default();
                            let _ = GetCursorPos(&mut pt);
                            let moved = mode_runtime::take_cursor_sample(pt.x, pt.y);
                            let beat = mode_runtime::mouse_hook_beat_ms();
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_millis() as u64)
                                .unwrap_or(0);
                            if moved && now.saturating_sub(beat) > 5_000 {
                                warn!(
                                    stale_ms = now.saturating_sub(beat),
                                    "mouse LL hook appears stalled; reinstalling"
                                );
                                if let Err(e) = mouse_hook.reinstall() {
                                    error!("mouse hook health reinstall failed: {e}");
                                }
                            }
                        }
                        continue;
                    }
                    WM_DK_HELP => {
                        // Help = About + pointer to the docs site (do not jump straight to browser).
                        let docs = config.settings.docs_url.trim();
                        let ver = env!("CARGO_PKG_VERSION");
                        let mut body = if docs.is_empty() {
                            tf(keys::HELP_ABOUT_NO_DOCS, &[("version", ver)])
                        } else {
                            tf(
                                keys::HELP_ABOUT_WITH_DOCS,
                                &[("version", ver), ("url", docs)],
                            )
                        };
                        // Spec §13-4: show active language pack beside version when used.
                        if let Some(p) = i18n::active().pack() {
                            let line = format!(
                                "Language pack: {} {} (target {})",
                                p.language, p.pack_version, p.target
                            );
                            if let Some(pos) = body.find('\n') {
                                body.insert_str(pos, &format!("\n{line}"));
                            } else {
                                body.push('\n');
                                body.push_str(&line);
                            }
                        }
                        let title: Vec<u16> = format!("{}\0", t(keys::HELP_DIALOG_TITLE))
                            .encode_utf16()
                            .collect();
                        let text: Vec<u16> = format!("{body}\0").encode_utf16().collect();
                        if docs.is_empty() {
                            // No docsUrl — About only (do not offer to open a missing site).
                            let _ = MessageBoxW(
                                host,
                                PCWSTR(text.as_ptr()),
                                PCWSTR(title.as_ptr()),
                                MB_OK | MB_ICONINFORMATION,
                            );
                        } else {
                            let answer = MessageBoxW(
                                host,
                                PCWSTR(text.as_ptr()),
                                PCWSTR(title.as_ptr()),
                                MB_YESNO | MB_ICONINFORMATION,
                            );
                            if answer == IDYES {
                                open_docs_url(docs);
                            }
                        }
                        continue;
                    }
                    WM_DK_SEARCH => {
                        if settings::is_open() {
                            info!("Search ignored — settings is open");
                            continue;
                        }
                        if session.is_some() {
                            session = None;
                        }
                        if let Err(e) = search::open(host, &config, &exe) {
                            error!("failed to open Search: {e}");
                            tray.notify(
                                &t(keys::TRAY_NOTIFY_TITLE),
                                &t(keys::TRAY_NOTIFY_OPEN_SEARCH_FAILED),
                            );
                        }
                        continue;
                    }
                    WM_DK_SEARCH_CLOSED => {
                        continue;
                    }
                    WM_DK_SETTINGS => {
                        if session.is_some() {
                            session = None;
                        }
                        search::close_if_open();
                        if let Err(e) = settings::open(host, &config) {
                            error!("failed to open settings: {e}");
                            tray.notify(
                                &t(keys::TRAY_NOTIFY_TITLE),
                                &t(keys::TRAY_NOTIFY_OPEN_SETTINGS_FAILED),
                            );
                        }
                        continue;
                    }
                    WM_DK_LAUNCH_FAILED => {
                        tray.notify(
                            &t(keys::TRAY_NOTIFY_TITLE),
                            &t(keys::TRAY_NOTIFY_LAUNCH_FAILED),
                        );
                        continue;
                    }
                    WM_DK_SETTINGS_APPLIED => {
                        if session.is_some() {
                            session = None;
                        }
                        match crate::core::load_app_config(&exe, Some(&config.dir)) {
                            Ok(new_cfg) => {
                                config = new_cfg;
                                config.open_settings_on_start = false;
                                apply_config_runtime(&config, &mut hotkey, &autostart);
                                refresh_mode_switch(&config, &mut tray);
                                info!("settings applied to running process");
                            }
                            Err(e) => {
                                error!("apply after settings save failed: {e}");
                                tray.notify(
                                    &t(keys::TRAY_NOTIFY_TITLE),
                                    &t(keys::TRAY_NOTIFY_APPLY_RELOAD_FAILED),
                                );
                            }
                        }
                        continue;
                    }
                    WM_DK_SETTINGS_CLOSED => {
                        // Locale Apply rebuilds Settings in the new language (hot refresh).
                        if let Some(page) = settings::take_reopen_request() {
                            if let Err(e) = settings::open_at_page(host, &config, page) {
                                error!("failed to reopen settings after language change: {e}");
                            }
                        }
                        continue;
                    }
                    WM_DK_QUIT => {
                        info!("quit requested");
                        mode_runtime::disarm_mode_chord(host);
                        mode_runtime::stop_hook_health_timer(host);
                        settings::close_if_open();
                        search::close_if_open();
                        PostQuitMessage(0);
                        continue;
                    }
                    _ => {}
                }
            }

            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        drop(session);
        mode_runtime::stop_hook_health_timer(host);
        HOST_HWND.store(0, Ordering::SeqCst);
        launch_ipc::clear_ctx();
    }

    Ok(())
}

/// Host WndProc.
///
/// Tray callbacks arrive via SendMessage (not the GetMessage queue), so we
/// re-post them for the main loop where `WindowsTray` is in scope.
/// `--launch` queues onto the launch worker; `SendMessage` returns once the
/// host has accepted the slot id (not after `ShellExecute` finishes).
unsafe extern "system" fn host_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_COPYDATA {
        return launch_ipc::handle_copydata(hwnd, wparam, lparam);
    }
    if msg == WM_DK_TRAY {
        let _ = PostMessageW(hwnd, WM_DK_TRAY_DISPATCH, wparam, lparam);
        return LRESULT(0);
    }
    if msg == WM_DESTROY {
        PostQuitMessage(0);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
