//! Focused Settings window — key rebinding, slots, options.
//!
//! Triggers are disabled while this window is open (spec §7).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use tracing::{error, info};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{
    GetLastError, COLORREF, ERROR_CLASS_ALREADY_EXISTS, HWND, LPARAM, LRESULT, POINT, RECT, SIZE,
    WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    FillRect, GetDC, GetMonitorInfoW, GetStockObject, GetSysColorBrush, GetTextExtentPoint32W,
    MonitorFromPoint, MonitorFromWindow, RedrawWindow, ReleaseDC, SelectObject, SetBkColor,
    SetTextColor, COLOR_WINDOW, DEFAULT_GUI_FONT, HBRUSH, HFONT, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE, RDW_UPDATENOW,
    WHITE_BRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForMonitor, GetDpiForSystem, GetDpiForWindow, MDT_EFFECTIVE_DPI,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, IsWindowEnabled, SetFocus,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CallWindowProcW, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, GetClassNameW, GetClientRect, GetCursorPos, GetMessageW, GetParent,
    GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, IsDialogMessageW, LoadCursorW,
    MessageBoxW, PostMessageW, RegisterClassExW, SendMessageW, SetForegroundWindow, SetParent,
    SetWindowLongPtrW, SetWindowPos, SetWindowTextW, SetWindowsHookExW, ShowWindow,
    TranslateMessage, UnhookWindowsHookEx, GWLP_USERDATA, GWLP_WNDPROC, HHOOK, HWND_BOTTOM,
    HWND_MESSAGE, HWND_TOP, IDC_ARROW, IDNO, IDYES, KBDLLHOOKSTRUCT, LLKHF_INJECTED,
    MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_YESNOCANCEL, MINMAXINFO, MSG, MSLLHOOKSTRUCT,
    SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOCOPYBITS, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, SW_SHOWNORMAL, WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC,
    WM_DESTROY, WM_DPICHANGED, WM_DROPFILES, WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONDOWN,
    WM_MBUTTONDOWN, WM_NOTIFY, WM_RBUTTONDOWN, WM_SETFONT, WM_SIZE, WM_USER, WM_XBUTTONDOWN,
    WNDCLASSEXW, WS_BORDER, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_MINIMIZEBOX,
    WS_OVERLAPPED, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_THICKFRAME, WS_VISIBLE, WS_VSCROLL,
    XBUTTON1, XBUTTON2,
};

use crate::core::chain::{is_chain_path, validate_chains, ChainBlockError};
use crate::core::config::{
    backup_config_json, default_shipped_settings, hotkey_from_settings, list_slots_file_candidates,
    load_slots_for_settings, mouse_trigger_from_settings, normalize_slots_file_name,
    path_is_inside, read_slots_display_name, resolve_slots_path, restore_config_json,
    save_settings, save_slots, set_hotkey_trigger, set_mouse_trigger, slots_book_label,
    slots_file_is_writable, AppConfig, ConfigError, RestoreSlotsScope, Settings,
};
use crate::core::drop_path::{
    dropped_item_kind, payload_for_dropped_item, text_for_slot_drop, url_from_internet_shortcut,
    DroppedItemKind, SlotDropField,
};
use crate::core::i18n::{self, keys, t, tf};
use crate::core::keys::{
    apply_captured_vk, apply_captured_wake, validate_display_labels, validate_key_bindings,
    validate_key_set_chords, KeyBindingError, KeyRole, KEY_SET_KEYBOARD, KEY_SET_NUMPAD,
};
use crate::core::launch::is_http_url;
use crate::core::legend::{LegendId, LEGEND_ITEM_COUNT};
use crate::core::mode_map::{build_mode_chord_map, chord_code_at};
use crate::core::search::{SearchMode, SearchSettings};
use crate::core::slots::{is_valid_slot_id, leading_zero_conflicts, Slot, SlotRegistry};
use crate::core::winpath::{path_exists, path_is_dir};

use super::icon::load_class_icons;
use super::messages::{SETTINGS_CLASS, WM_DK_SETTINGS_APPLIED, WM_DK_SETTINGS_CLOSED};
use super::wide::{pcwstr, to_wide};

/// Child host for one Settings tab. Hiding the panel hides every control on it
/// (sibling ShowWindow was letting Options bleed onto Keys).
const SETTINGS_PAGE_CLASS: PCWSTR = w!("DialKeySettingsPage");

// ── Trigger gate (checked by the host mouse/hotkey path) ────────────────

static TRIGGERS_DISABLED: AtomicBool = AtomicBool::new(false);

pub fn triggers_disabled() -> bool {
    TRIGGERS_DISABLED.load(Ordering::SeqCst)
}

fn set_triggers_disabled(v: bool) {
    TRIGGERS_DISABLED.store(v, Ordering::SeqCst);
    info!(disabled = v, "settings: triggers gate");
}

// ── Control IDs ─────────────────────────────────────────────────────────

const IDC_TAB_SLOTS: isize = 1001;
const IDC_TAB_KEYS: isize = 1002;
const IDC_TAB_TRIGGERS: isize = 1003;
const IDC_TAB_SEARCH: isize = 1004;
const IDC_TAB_GENERAL: isize = 1005;
const IDC_TAB_MODE: isize = 1006;

const IDC_SLOT_LIST: isize = 1100;
const IDC_SLOT_ADD: isize = 1101;
const IDC_SLOT_DELETE: isize = 1102;
const IDC_SLOT_SWAP: isize = 1103;
const IDC_SLOT_ID: isize = 1110;
const IDC_SLOT_NAME: isize = 1111;
const IDC_SLOT_PATH: isize = 1112;
const IDC_SLOT_DESC: isize = 1113;
const IDC_SLOT_WORKDIR: isize = 1115;
const IDC_SLOT_APPLY: isize = 1116;
const IDC_SLOT_WARN: isize = 1117;
const IDC_SLOT_OPEN_WORKDIR: isize = 1118;
const IDC_SLOT_INSTANT: isize = 1119;
const IDC_SLOT_FOCUS_EXISTING: isize = 1120;
const IDC_SLOT_CURRENT: isize = 1121;

const IDC_CAP_START: isize = 1210;
const IDC_CAP_CONFIRM: isize = 1211;
const IDC_CAP_CANCEL: isize = 1212;
const IDC_CAP_SEARCH: isize = 1213;
const IDC_CAP_OPEN_WORKDIR: isize = 1214;
const IDC_CAP_DIGIT_BACK: isize = 1215;
const IDC_WAKE: isize = 1217;
const IDC_KEY_SET: isize = 1218;
const IDC_KEY_CHORD: isize = 1219;
const IDC_CAP_WAKE: isize = 1226;
const IDC_START_LABEL: isize = 1216;
const IDC_CONFIRM_LABEL: isize = 1230;
const IDC_CANCEL_LABEL: isize = 1231;
const IDC_SEARCH_LABEL: isize = 1232;
const IDC_OPEN_WORKDIR_LABEL: isize = 1233;
const IDC_DIGIT_BACK_LABEL: isize = 1234;
const IDC_MOUSE_BTN: isize = 1220;
const IDC_MOUSE_SUPPRESS: isize = 1221;
const IDC_HOTKEY: isize = 1222;
const IDC_RESET_KEYS: isize = 1223;
const IDC_RESET_TRIGGERS: isize = 1227;
const IDC_CAPTURE_STATUS: isize = 1224;
const IDC_CAP_MOUSE: isize = 1225;

const IDC_LEGEND_IDLE_0: isize = 1600;
const IDC_LEGEND_MULTI_0: isize = 1610;
const IDC_LEGEND_IDLE_UP: isize = 1620;
const IDC_LEGEND_IDLE_DOWN: isize = 1621;
const IDC_LEGEND_MULTI_UP: isize = 1622;
const IDC_LEGEND_MULTI_DOWN: isize = 1623;

const IDC_SEARCH_MODE: isize = 1320;
const IDC_CASE_SENSITIVE: isize = 1321;
const IDC_TIMEOUT: isize = 1322;
const IDC_LOG_LEVEL: isize = 1323;
const IDC_AUTOSTART: isize = 1324;
const IDC_MAX_DIGITS: isize = 1325;
const IDC_UI_LOCALE: isize = 1326;
const IDC_FEEDBACK_MS: isize = 1327;
const IDC_SLOTS_FILE: isize = 1328;
const IDC_OPEN_CONFIG_FOLDER: isize = 1329;
const IDC_BACKUP_CONFIG: isize = 1330;
const IDC_BACKUP_FOLDER: isize = 1331;
const IDC_BACKUP_BROWSE: isize = 1332;
const IDC_BACKUP_RESTORE: isize = 1333;

const IDC_SAVE: isize = 1400;
const IDC_CANCEL: isize = 1401;
const IDC_APPLY: isize = 1402;
const IDC_ADVANCED: isize = 1403;

const IDC_MODE_MOUSE: isize = 1500;
const IDC_MODE_SUPPRESS: isize = 1501;
const IDC_MODE_TIMEOUT: isize = 1502;
const IDC_MODE_CHORD_LIST: isize = 1503;
const IDC_MODE_CHORD_FILE: isize = 1505;
const IDC_MODE_SET_OVERRIDE: isize = 1506;
const IDC_MODE_CLEAR_OVERRIDE: isize = 1507;
const IDC_MODE_STATUS: isize = 1508;
const IDC_MODE_CURRENT: isize = 1509;
const IDC_MODE_LETTER_KEYBOARD: isize = 1510;
const LBN_DBLCLK: u16 = 2;

const LBN_SELCHANGE: u16 = 1;
const CBN_SELCHANGE: u16 = 1;
const BN_CLICKED: u16 = 0;
const EN_SETFOCUS: u16 = 0x0100;
const EN_CHANGE: u16 = 0x0300;
const ES_AUTOHSCROLL: u32 = 0x0080;
const ES_READONLY: u32 = 0x0800;
const SS_NOPREFIX: u32 = 0x0080;
const BACKUP_STATUS_H: i32 = 40;
const ES_AUTOVSCROLL: u32 = 0x0040;
const ES_MULTILINE: u32 = 0x0004;
const ES_WANTRETURN: u32 = 0x1000;
const ES_NUMBER: u32 = 0x2000;
const BS_PUSHBUTTON: u32 = 0x0000_0000;
const BS_AUTORADIOBUTTON: u32 = 0x0000_0009;
/// Radio/check that looks like a sticky push button (selected tab stays down).
const BS_PUSHLIKE: u32 = 0x0000_1000;
const TAB_STICKY: u32 = BS_AUTORADIOBUTTON | BS_PUSHLIKE;
const BS_AUTOCHECKBOX: u32 = 0x0000_0003;
const WS_GROUP: u32 = 0x0002_0000;
const BS_MULTILINE: u32 = 0x0000_2000;
const BS_GROUPBOX: u32 = 0x0000_0007;
const CBS_DROPDOWNLIST: u32 = 0x0003;
const LBS_NOTIFY: u32 = 0x0001;
const LBS_USETABSTOPS: u32 = 0x0080;
const LB_SETTABSTOPS: u32 = 0x0192;

/// Soft minimum when resizing by hand; always capped to the work area.
const SETTINGS_MIN_CLIENT_W: i32 = 640;
const SETTINGS_MIN_CLIENT_H: i32 = 400;
/// Provisional open size before measuring tab content (then sized to fit).
const SETTINGS_PROVISIONAL_W: i32 = 900;
const SETTINGS_PROVISIONAL_H: i32 = 700;
/// Settings spacing (one rhythm for every tab):
/// - `LABEL_STACK_GAP` — label to the control under it
/// - `FIELD_BLOCK_GAP` — sibling rows (Action keys, checkbox lists, stacked fields)
/// - `KEYS_GROUP_GAP` — adjacent group boxes
/// - `KEYS_COL_GAP` — horizontal gap on a row (same 12 as footer buttons)
const KEYS_COL_GAP: i32 = 12;
/// Gap between Action keys and Displayed options groups.
const KEYS_GROUP_GAP: i32 = 8;
const SETTINGS_WINDOW_STYLE: WINDOW_STYLE = WINDOW_STYLE(
    WS_OVERLAPPED.0 | WS_CAPTION.0 | WS_SYSMENU.0 | WS_THICKFRAME.0 | WS_MINIMIZEBOX.0,
);
const SLOTS_LIST_W: i32 = 260;
const SLOTS_LIST_FORM_GAP: i32 = 20;
const SLOTS_ID_COL_W: i32 = 120;
/// Widest tab need: Slots list + form (other tabs follow panel width).
const SETTINGS_CONTENT_W: i32 = 12 + SLOTS_LIST_W + SLOTS_LIST_FORM_GAP + 500 + 12;
const FOOTER_BTN_H: i32 = 28;
const FOOTER_BOTTOM_PAD: i32 = 12;
/// Minimum footer button width; actual width grows with measured label text.
const FOOTER_BTN_W_MIN: i32 = 72;
const FOOTER_BTN_PAD_X: i32 = 24;
const FOOTER_BTN_GAP: i32 = 12;
const FOOTER_RIGHT_PAD: i32 = 20;
/// Gap between a label and the control under it (stacked form).
const LABEL_STACK_GAP: i32 = 4;
/// Gap after a stacked field block.
const FIELD_BLOCK_GAP: i32 = 12;
/// Space reserved for tab buttons (five tabs; panels must not cover this).
const TAB_BAND_H: i32 = 42;
/// Space reserved for Save/Apply/Cancel (panels must not cover this).
/// Gap above the footer row so Slots actions sit on the same baseline, not under it.
const FOOTER_BAND_H: i32 = FOOTER_BOTTOM_PAD + FOOTER_BTN_H + 12;
/// Description edit height (grows with free panel space; shrinks if short).
const SLOTS_DESC_H_DEFAULT: i32 = 72;
const SLOTS_DESC_H_MIN: i32 = 48;
const SLOTS_DESC_H_MAX: i32 = 160;
const SLOTS_FORM_LABEL_H: i32 = 26;
const SLOTS_FORM_EDIT_H: i32 = 24;
const SLOTS_UPDATE_BTN_H: i32 = 26;
const SLOTS_WARN_H: i32 = 56;
const SLOTS_OPEN_WORKDIR_H: i32 = 22;
const LB_SETTOPINDEX: u32 = 0x0197;
const LB_GETITEMHEIGHT: u32 = 0x01A1;
/// Capture status under the Typing window group (3 lines: capturing / hint / reserved).
const CAPTURE_STATUS_H: i32 = 56;
/// Mode chord list: 10 rows (codes 0–9). Height fits those rows; do not stretch.
const MODE_CHORD_ROWS: i32 = 10;
const MODE_CHORD_LIST_H_MIN: i32 = 72;
const MODE_CHORD_LIST_H_FALLBACK: i32 = 16 * MODE_CHORD_ROWS + 4;

const BM_GETCHECK: u32 = 0x00F0;
const BM_SETCHECK: u32 = 0x00F1;
const BST_CHECKED: isize = 1;
const BST_UNCHECKED: isize = 0;

const CB_ADDSTRING: u32 = 0x0143;
const CB_RESETCONTENT: u32 = 0x014B;
const CB_GETCURSEL: u32 = 0x0147;
const CB_SETCURSEL: u32 = 0x014E;
const LB_ADDSTRING: u32 = 0x0180;
const LB_RESETCONTENT: u32 = 0x0184;
const LB_SETCURSEL: u32 = 0x0186;
const LB_GETCURSEL: u32 = 0x0188;

const WM_DK_CAPTURE_KEY: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 40;
const WM_DK_CAPTURE_MOUSE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 41;

static SETTINGS_HWND: AtomicIsize = AtomicIsize::new(0);
static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);
static PAGE_CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);
static CAPTURE_HWND: AtomicIsize = AtomicIsize::new(0);
/// After Apply with a locale change: destroy + reopen so chrome is rebuilt in the new language.
static REOPEN_AFTER_APPLY: AtomicBool = AtomicBool::new(false);
static REOPEN_PAGE: AtomicU8 = AtomicU8::new(0);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Slots,
    Mode,
    Keys,
    Triggers,
    Search,
    General,
}

impl Page {
    fn as_u8(self) -> u8 {
        match self {
            Page::Slots => 0,
            Page::Mode => 1,
            Page::Keys => 2,
            Page::Triggers => 3,
            Page::Search => 4,
            Page::General => 5,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            1 => Page::Mode,
            2 => Page::Keys,
            3 => Page::Triggers,
            4 => Page::Search,
            5 => Page::General,
            _ => Page::Slots,
        }
    }

    fn is_advanced(self) -> bool {
        matches!(
            self,
            Page::Mode | Page::Keys | Page::Triggers | Page::Search
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptureTarget {
    Key(KeyRole),
    Wake,
    Mouse,
}

struct CaptureHook {
    kb: Option<HHOOK>,
    mouse: Option<HHOOK>,
}

impl CaptureHook {
    fn none() -> Self {
        Self {
            kb: None,
            mouse: None,
        }
    }
}

impl Drop for CaptureHook {
    fn drop(&mut self) {
        unsafe {
            if let Some(h) = self.kb.take() {
                let _ = UnhookWindowsHookEx(h);
            }
            if let Some(h) = self.mouse.take() {
                let _ = UnhookWindowsHookEx(h);
            }
        }
        CAPTURE_HWND.store(0, Ordering::SeqCst);
    }
}

struct SettingsState {
    host: HWND,
    draft_settings: Settings,
    draft_slots: SlotRegistry,
    config_dir: PathBuf,
    settings_writable: bool,
    slots_writable: bool,
    /// Absolute path of the slots file loaded when the dialog opened.
    slots_path_at_open: PathBuf,
    /// Slots snapshot for the book currently being edited (dirty check).
    slots_baseline: SlotRegistry,
    page: Page,
    // Tab buttons
    tab_slots: HWND,
    tab_keys: HWND,
    tab_triggers: HWND,
    tab_search: HWND,
    tab_mode: HWND,
    tab_general: HWND,
    // Slot page
    slot_list: HWND,
    slot_ids: Vec<String>,
    edit_id: HWND,
    edit_name: HWND,
    edit_path: HWND,
    edit_desc: HWND,
    edit_workdir: HWND,
    path_edit_prev: isize,
    workdir_edit_prev: isize,
    chk_open_workdir: HWND,
    /// Focus already-open window before spawn (slot `focusExisting`, default on).
    chk_focus_existing: HWND,
    /// Instant fire for single-digit IDs 0–9 (writes `settings.instant`).
    chk_instant_fire: HWND,
    warn_label: HWND,
    lbl_slots_current: HWND,
    /// Slots actions — children of the main dialog (footer row with Save/Apply/Cancel).
    btn_add: HWND,
    btn_delete: HWND,
    btn_swap: HWND,
    btn_slot_apply: HWND,
    // Keys page
    lbl_start: HWND,
    edit_start_label: HWND,
    edit_confirm_label: HWND,
    edit_cancel_label: HWND,
    edit_search_label: HWND,
    edit_open_workdir_label: HWND,
    edit_digit_back_label: HWND,
    lbl_confirm: HWND,
    lbl_cancel: HWND,
    lbl_search: HWND,
    lbl_open_workdir_key: HWND,
    lbl_digit_back: HWND,
    btn_cap_start: HWND,
    btn_cap_confirm: HWND,
    btn_cap_cancel: HWND,
    btn_cap_search: HWND,
    btn_cap_open_workdir: HWND,
    btn_cap_digit_back: HWND,
    chk_wake: HWND,
    lbl_wake_cap: HWND,
    lbl_wake: HWND,
    btn_cap_wake: HWND,
    lbl_key_set: HWND,
    combo_key_set: HWND,
    lbl_key_chord: HWND,
    combo_key_chord: HWND,
    lbl_key_chord_kb: HWND,
    combo_key_chord_kb: HWND,
    suppress_key_set_change: bool,
    capture_status: HWND,
    // Typing-window legend (Idle / Narrow down)
    g_legend: HWND,
    lbl_legend_idle: HWND,
    lbl_legend_multi: HWND,
    chk_legend_idle: [HWND; LEGEND_ITEM_COUNT],
    chk_legend_multi: [HWND; LEGEND_ITEM_COUNT],
    btn_legend_idle_up: HWND,
    btn_legend_idle_down: HWND,
    btn_legend_multi_up: HWND,
    btn_legend_multi_down: HWND,
    legend_idle_sel: usize,
    legend_multi_sel: usize,
    // Triggers page
    combo_mouse: HWND,
    btn_cap_mouse: HWND,
    chk_suppress: HWND,
    edit_hotkey: HWND,
    btn_reset_keys: HWND,
    btn_reset_triggers: HWND,
    lbl_triggers_status: HWND,
    // Search page
    combo_search: HWND,
    chk_case: HWND,
    // Mode page (§4 / §6 / §7 — leader mouse + chord overrides)
    lbl_mode_mouse: HWND,
    combo_mode_mouse: HWND,
    chk_mode_suppress: HWND,
    lbl_mode_timeout: HWND,
    edit_mode_timeout: HWND,
    lbl_mode_current: HWND,
    combo_mode_current: HWND,
    lbl_mode_chords: HWND,
    list_mode_chords: HWND,
    lbl_mode_chord_file: HWND,
    /// Slots-file combo for Set (parallel to `mode_chord_file_choices`).
    combo_mode_files: HWND,
    btn_mode_set_override: HWND,
    btn_mode_clear_override: HWND,
    lbl_mode_hint: HWND,
    lbl_mode_status: HWND,
    /// Parallel to list_mode_chords rows (`"0"`…`"9"`).
    mode_chord_code_choices: Vec<String>,
    /// Parallel to combo_mode_files entries (basenames; example excluded).
    mode_chord_file_choices: Vec<String>,
    /// Parallel to combo_mode_current (assigned books only).
    mode_current_file_choices: Vec<String>,
    suppress_mode_current_change: bool,
    // General page
    edit_timeout: HWND,
    edit_feedback_ms: HWND,
    combo_log: HWND,
    combo_locale: HWND,
    /// Parallel to combo_locale entries: `""` (system), `en`, then pack codes.
    locale_choices: Vec<String>,
    lbl_slots_file: HWND,
    combo_slots_file: HWND,
    btn_open_config_folder: HWND,
    lbl_backup_folder: HWND,
    edit_backup_folder: HWND,
    btn_backup_browse: HWND,
    btn_backup_config: HWND,
    btn_backup_restore: HWND,
    lbl_backup_status: HWND,
    /// Parallel to combo_slots_file: `""` = sample (read-only), else basename.
    slots_file_choices: Vec<String>,
    chk_autostart: HWND,
    edit_max_digits: HWND,
    // Footer — children of the main dialog (never parented under a page panel).
    btn_save: HWND,
    btn_cancel: HWND,
    btn_apply: HWND,
    /// Always visible — toggles Mode/Keys/Triggers/Search and extra General fields.
    chk_advanced: HWND,
    /// One panel per tab — show/hide the panel instead of each sibling control.
    panel_slots: HWND,
    panel_keys: HWND,
    panel_triggers: HWND,
    panel_search: HWND,
    panel_mode: HWND,
    panel_general: HWND,
    /// Group boxes (resized to panel width on layout).
    g_keys: HWND,
    g_triggers: HWND,
    g_search: HWND,
    g_mode: HWND,
    g_mode_switch: HWND,
    g_general: HWND,
    /// Keys / General labels that need width stretch with the panel.
    lbl_start_cap: HWND,
    lbl_confirm_cap: HWND,
    lbl_cancel_cap: HWND,
    lbl_search_cap: HWND,
    lbl_open_workdir_cap: HWND,
    lbl_digit_back_cap: HWND,
    lbl_mouse: HWND,
    lbl_hotkey: HWND,
    lbl_hotkey_hint: HWND,
    lbl_match_mode: HWND,
    lbl_timeout: HWND,
    lbl_max_digits: HWND,
    lbl_feedback: HWND,
    lbl_log: HWND,
    lbl_lang: HWND,
    /// Slots form column (labels stacked above fields — same X for both).
    slots_form_x: i32,
    /// Labels above each Slots field (resized with the form column).
    lbl_slot_id: HWND,
    lbl_slot_name: HWND,
    lbl_slot_path: HWND,
    lbl_slot_workdir: HWND,
    lbl_slot_desc: HWND,
    /// `settings.ui.locale` when the window was opened (detect Apply language change).
    locale_at_open: String,
    /// `settings.slotsFile` when the window was opened (detect Apply file switch).
    slots_file_at_open: String,
    capture: CaptureHook,
    capturing: Option<CaptureTarget>,
}

/// True if Settings should be reopened after the current close (locale Apply).
pub fn take_reopen_request() -> Option<u8> {
    if REOPEN_AFTER_APPLY.swap(false, Ordering::SeqCst) {
        Some(REOPEN_PAGE.load(Ordering::SeqCst))
    } else {
        None
    }
}

pub fn is_open() -> bool {
    !HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _)
        .0
        .is_null()
}

pub fn close_if_open() {
    let hwnd = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    if !hwnd.0.is_null() {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }
}

pub fn open(host: HWND, config: &AppConfig) -> anyhow::Result<()> {
    open_at_page(host, config, Page::Slots.as_u8())
}

/// `page`: `Page::as_u8` (0 = Slots). Used after locale Apply rebuild.
/// The tab strip starts with General; the first open still shows Slots.
pub fn open_at_page(host: HWND, config: &AppConfig, page: u8) -> anyhow::Result<()> {
    let existing = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    if !existing.0.is_null() {
        unsafe {
            super::focus_existing::bring_window_front(existing);
        }
        return Ok(());
    }

    unsafe {
        ensure_class()?;
        let instance = GetModuleHandleW(None)?;
        let mut initial_page = Page::from_u8(page);
        if !config.settings.ui.show_advanced && initial_page.is_advanced() {
            initial_page = Page::Slots;
        }

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // Per-monitor: preferred size, then shrink to work area (never enlarge).
        let (x, y, width, height) = settings_placement_for_cursor(pt);

        let title = to_wide(&tf(
            keys::SETTINGS_WINDOW_TITLE,
            &[("version", env!("CARGO_PKG_VERSION"))],
        ));
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            SETTINGS_CLASS,
            PCWSTR(title.as_ptr()),
            SETTINGS_WINDOW_STYLE | WS_CLIPCHILDREN | WS_VISIBLE,
            x,
            y,
            width,
            height,
            None,
            None,
            instance,
            None,
        )?;
        // CreateWindow can ignore size on some DPI/min-track paths — re-apply.
        {
            let (fx, fy, fw, fh) = settings_placement_for_cursor(pt);
            let _ = SetWindowPos(
                hwnd,
                HWND_TOP,
                fx,
                fy,
                fw,
                fh,
                SWP_NOACTIVATE | SWP_NOCOPYBITS,
            );
        }

        let state = Box::new(SettingsState {
            host,
            draft_settings: config.settings.clone(),
            draft_slots: config.slots.clone(),
            config_dir: config.dir.clone(),
            settings_writable: config.settings_writable,
            slots_writable: config.slots_writable,
            slots_path_at_open: config.slots_path.clone(),
            slots_baseline: config.slots.clone(),
            page: initial_page,
            tab_slots: HWND::default(),
            tab_keys: HWND::default(),
            tab_triggers: HWND::default(),
            tab_search: HWND::default(),
            tab_mode: HWND::default(),
            tab_general: HWND::default(),
            slot_list: HWND::default(),
            slot_ids: Vec::new(),
            edit_id: HWND::default(),
            edit_name: HWND::default(),
            edit_path: HWND::default(),
            edit_desc: HWND::default(),
            edit_workdir: HWND::default(),
            path_edit_prev: 0,
            workdir_edit_prev: 0,
            chk_open_workdir: HWND::default(),
            chk_focus_existing: HWND::default(),
            chk_instant_fire: HWND::default(),
            warn_label: HWND::default(),
            lbl_slots_current: HWND::default(),
            btn_add: HWND::default(),
            btn_delete: HWND::default(),
            btn_swap: HWND::default(),
            btn_slot_apply: HWND::default(),
            lbl_start: HWND::default(),
            edit_start_label: HWND::default(),
            edit_confirm_label: HWND::default(),
            edit_cancel_label: HWND::default(),
            edit_search_label: HWND::default(),
            edit_open_workdir_label: HWND::default(),
            edit_digit_back_label: HWND::default(),
            lbl_confirm: HWND::default(),
            lbl_cancel: HWND::default(),
            lbl_search: HWND::default(),
            lbl_open_workdir_key: HWND::default(),
            lbl_digit_back: HWND::default(),
            btn_cap_start: HWND::default(),
            btn_cap_confirm: HWND::default(),
            btn_cap_cancel: HWND::default(),
            btn_cap_search: HWND::default(),
            btn_cap_open_workdir: HWND::default(),
            btn_cap_digit_back: HWND::default(),
            chk_wake: HWND::default(),
            lbl_wake_cap: HWND::default(),
            lbl_wake: HWND::default(),
            btn_cap_wake: HWND::default(),
            lbl_key_set: HWND::default(),
            combo_key_set: HWND::default(),
            lbl_key_chord: HWND::default(),
            combo_key_chord: HWND::default(),
            lbl_key_chord_kb: HWND::default(),
            combo_key_chord_kb: HWND::default(),
            suppress_key_set_change: false,
            capture_status: HWND::default(),
            g_legend: HWND::default(),
            lbl_legend_idle: HWND::default(),
            lbl_legend_multi: HWND::default(),
            chk_legend_idle: [HWND::default(); LEGEND_ITEM_COUNT],
            chk_legend_multi: [HWND::default(); LEGEND_ITEM_COUNT],
            btn_legend_idle_up: HWND::default(),
            btn_legend_idle_down: HWND::default(),
            btn_legend_multi_up: HWND::default(),
            btn_legend_multi_down: HWND::default(),
            legend_idle_sel: 0,
            legend_multi_sel: 0,
            combo_mouse: HWND::default(),
            btn_cap_mouse: HWND::default(),
            chk_suppress: HWND::default(),
            edit_hotkey: HWND::default(),
            btn_reset_keys: HWND::default(),
            btn_reset_triggers: HWND::default(),
            lbl_triggers_status: HWND::default(),
            combo_search: HWND::default(),
            chk_case: HWND::default(),
            lbl_mode_mouse: HWND::default(),
            combo_mode_mouse: HWND::default(),
            chk_mode_suppress: HWND::default(),
            lbl_mode_timeout: HWND::default(),
            edit_mode_timeout: HWND::default(),
            lbl_mode_current: HWND::default(),
            combo_mode_current: HWND::default(),
            lbl_mode_chords: HWND::default(),
            list_mode_chords: HWND::default(),
            lbl_mode_chord_file: HWND::default(),
            combo_mode_files: HWND::default(),
            btn_mode_set_override: HWND::default(),
            btn_mode_clear_override: HWND::default(),
            lbl_mode_hint: HWND::default(),
            lbl_mode_status: HWND::default(),
            mode_chord_code_choices: Vec::new(),
            mode_chord_file_choices: Vec::new(),
            mode_current_file_choices: Vec::new(),
            suppress_mode_current_change: false,
            edit_timeout: HWND::default(),
            edit_feedback_ms: HWND::default(),
            combo_log: HWND::default(),
            combo_locale: HWND::default(),
            locale_choices: Vec::new(),
            lbl_slots_file: HWND::default(),
            combo_slots_file: HWND::default(),
            btn_open_config_folder: HWND::default(),
            lbl_backup_folder: HWND::default(),
            edit_backup_folder: HWND::default(),
            btn_backup_browse: HWND::default(),
            btn_backup_config: HWND::default(),
            btn_backup_restore: HWND::default(),
            lbl_backup_status: HWND::default(),
            slots_file_choices: Vec::new(),
            chk_autostart: HWND::default(),
            edit_max_digits: HWND::default(),
            btn_save: HWND::default(),
            btn_cancel: HWND::default(),
            btn_apply: HWND::default(),
            chk_advanced: HWND::default(),
            panel_slots: HWND::default(),
            panel_keys: HWND::default(),
            panel_triggers: HWND::default(),
            panel_search: HWND::default(),
            panel_mode: HWND::default(),
            panel_general: HWND::default(),
            g_keys: HWND::default(),
            g_triggers: HWND::default(),
            g_search: HWND::default(),
            g_mode: HWND::default(),
            g_mode_switch: HWND::default(),
            g_general: HWND::default(),
            lbl_start_cap: HWND::default(),
            lbl_confirm_cap: HWND::default(),
            lbl_cancel_cap: HWND::default(),
            lbl_search_cap: HWND::default(),
            lbl_open_workdir_cap: HWND::default(),
            lbl_digit_back_cap: HWND::default(),
            lbl_mouse: HWND::default(),
            lbl_hotkey: HWND::default(),
            lbl_hotkey_hint: HWND::default(),
            lbl_match_mode: HWND::default(),
            lbl_timeout: HWND::default(),
            lbl_max_digits: HWND::default(),
            lbl_feedback: HWND::default(),
            lbl_log: HWND::default(),
            lbl_lang: HWND::default(),
            slots_form_x: 292,
            lbl_slot_id: HWND::default(),
            lbl_slot_name: HWND::default(),
            lbl_slot_path: HWND::default(),
            lbl_slot_workdir: HWND::default(),
            lbl_slot_desc: HWND::default(),
            locale_at_open: config.settings.ui.locale.clone(),
            slots_file_at_open: config.settings.slots_file.clone(),
            capture: CaptureHook::none(),
            capturing: None,
        });
        let state_ptr = Box::into_raw(state);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);

        create_children(hwnd, state_ptr)?;
        populate_from_draft(state_ptr);
        apply_pending_backup_status(state_ptr);
        SETTINGS_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
        // Size to the tallest/widest tab, then clamp to the monitor work area.
        fit_settings_window_to_content(hwnd, &*state_ptr, pt);
        show_page(state_ptr, initial_page);
        set_triggers_disabled(true);

        let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
        super::focus_existing::bring_window_front(hwnd);

        info!("settings opened");
        Ok(())
    }
}

unsafe fn ensure_class() -> anyhow::Result<()> {
    if CLASS_REGISTERED.load(Ordering::SeqCst) {
        return Ok(());
    }
    let instance = GetModuleHandleW(None)?;
    let (app_icon, app_icon_sm) = load_class_icons();
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(settings_proc),
        hInstance: instance.into(),
        lpszClassName: SETTINGS_CLASS,
        hCursor: LoadCursorW(None, IDC_ARROW)?,
        hIcon: app_icon,
        hIconSm: app_icon_sm,
        hbrBackground: HBRUSH(GetSysColorBrush(COLOR_WINDOW).0),
        ..Default::default()
    };
    if RegisterClassExW(&wc) == 0 {
        let err = GetLastError();
        if err != ERROR_CLASS_ALREADY_EXISTS {
            anyhow::bail!("RegisterClassExW(settings) failed: {err:?}");
        }
    }
    CLASS_REGISTERED.store(true, Ordering::SeqCst);
    ensure_page_class()?;
    Ok(())
}

unsafe fn ensure_page_class() -> anyhow::Result<()> {
    if PAGE_CLASS_REGISTERED.load(Ordering::SeqCst) {
        return Ok(());
    }
    let instance = GetModuleHandleW(None)?;
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(settings_page_proc),
        hInstance: instance.into(),
        lpszClassName: SETTINGS_PAGE_CLASS,
        hCursor: LoadCursorW(None, IDC_ARROW)?,
        hbrBackground: HBRUSH(GetSysColorBrush(COLOR_WINDOW).0),
        ..Default::default()
    };
    if RegisterClassExW(&wc) == 0 {
        let err = GetLastError();
        if err != ERROR_CLASS_ALREADY_EXISTS {
            anyhow::bail!("RegisterClassExW(settings page) failed: {err:?}");
        }
    }
    PAGE_CLASS_REGISTERED.store(true, Ordering::SeqCst);
    Ok(())
}

/// Forward control notifications to the Settings dialog (controls live on the page panel).
unsafe extern "system" fn settings_page_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND | WM_NOTIFY => {
            if let Ok(parent) = GetParent(hwnd) {
                if !parent.0.is_null() {
                    return SendMessageW(parent, msg, wparam, lparam);
                }
            }
        }
        WM_CTLCOLORSTATIC => {
            // Slots warn label is handled by the dialog (returns non-zero).
            if let Ok(parent) = GetParent(hwnd) {
                if !parent.0.is_null() {
                    let handled = SendMessageW(parent, msg, wparam, lparam);
                    if handled.0 != 0 {
                        return handled;
                    }
                }
            }
            let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut _);
            let _ = SetBkColor(hdc, COLORREF(0x00FF_FFFF));
            let _ = SetTextColor(hdc, COLORREF(0x0000_0000));
            return LRESULT(GetSysColorBrush(COLOR_WINDOW).0 as isize);
        }
        // Do not forward: dialog DefWindowProc brushes blank out AUTOCHECKBOX.
        WM_CTLCOLOREDIT | WM_CTLCOLORBTN => {
            let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut _);
            let _ = SetBkColor(hdc, COLORREF(0x00FF_FFFF));
            let _ = SetTextColor(hdc, COLORREF(0x0000_0000));
            return LRESULT(GetSysColorBrush(COLOR_WINDOW).0 as isize);
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn create_page_panel(parent: HWND) -> anyhow::Result<HWND> {
    ensure_page_class()?;
    let instance = GetModuleHandleW(None)?;
    let mut rc = RECT::default();
    let _ = GetClientRect(parent, &mut rc);
    let (x, y, w, h) = page_panel_rect(&rc);
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        SETTINGS_PAGE_CLASS,
        w!(""),
        WINDOW_STYLE(WS_CHILD.0 | WS_CLIPCHILDREN.0),
        x,
        y,
        w,
        h,
        parent,
        None,
        instance,
        None,
    )
    .map_err(Into::into)
}

fn page_panel_rect(rc: &RECT) -> (i32, i32, i32, i32) {
    let w = rc.right.max(100);
    let h = (rc.bottom - TAB_BAND_H - FOOTER_BAND_H).max(100);
    (0, TAB_BAND_H, w, h)
}

fn scale_for_dpi(value: i32, dpi: u32) -> i32 {
    let dpi = dpi.max(96);
    ((value as i64 * dpi as i64) / 96) as i32
}

unsafe fn monitor_dpi_at(pt: POINT) -> u32 {
    let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
    let mut dpi_x = 0u32;
    let mut dpi_y = 0u32;
    if GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok() && dpi_x >= 96 {
        dpi_x
    } else {
        GetDpiForSystem().max(96)
    }
}

unsafe fn work_area_at_point(pt: POINT) -> Option<RECT> {
    let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(mon, &mut mi).as_bool() {
        Some(mi.rcWork)
    } else {
        None
    }
}

unsafe fn work_area_for_hwnd(hwnd: HWND) -> Option<RECT> {
    let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(mon, &mut mi).as_bool() {
        Some(mi.rcWork)
    } else {
        None
    }
}

unsafe fn outer_size_for_client(client_w: i32, client_h: i32, dpi: u32) -> (i32, i32) {
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: client_w.max(100),
        bottom: client_h.max(100),
    };
    if AdjustWindowRectExForDpi(
        &mut rc,
        SETTINGS_WINDOW_STYLE,
        false,
        WINDOW_EX_STYLE::default(),
        dpi.max(96),
    )
    .is_ok()
    {
        (
            (rc.right - rc.left).max(client_w),
            (rc.bottom - rc.top).max(client_h),
        )
    } else {
        (client_w + 16, client_h + 59)
    }
}

/// Provisional outer size for this monitor (before measuring tab content).
/// Position: cursor monitor work-area top-left + inset (§9 窓表示), not cursor-centered.
unsafe fn settings_placement_for_cursor(pt: POINT) -> (i32, i32, i32, i32) {
    let dpi = monitor_dpi_at(pt);
    let (prefer_w, prefer_h) = outer_size_for_client(
        scale_for_dpi(SETTINGS_PROVISIONAL_W, dpi),
        scale_for_dpi(SETTINGS_PROVISIONAL_H, dpi),
        dpi,
    );
    let (w, h) = clamp_size_to_work_area(prefer_w, prefer_h, pt);
    let (x, y) = super::placement::settings_window_pos(pt, w, h);
    (x, y, w, h)
}

/// Client size from the tallest / widest tab content, then clamp to the work area.
unsafe fn fit_settings_window_to_content(hwnd: HWND, s: &SettingsState, pt: POINT) {
    let _ = size_all_page_panels(hwnd, s);
    let slots_h = layout_slots_page(hwnd, s);
    let keys_h = layout_keys_page(hwnd, s);
    let triggers_h = layout_triggers_page(hwnd, s);
    let search_h = layout_search_page(hwnd, s);
    let mode_h = layout_mode_page(hwnd, s);
    let general_h = layout_general_page(hwnd, s);
    let mut panel_h = slots_h.max(general_h);
    if s.draft_settings.ui.show_advanced {
        panel_h = panel_h
            .max(keys_h)
            .max(triggers_h)
            .max(search_h)
            .max(mode_h);
    }
    panel_h += 8;

    let auto_w = measure_text_width(hwnd, &window_text(s.chk_autostart)) + 56;
    let client_w = SETTINGS_CONTENT_W.max(auto_w).max(SETTINGS_MIN_CLIENT_W);
    let client_h = (TAB_BAND_H + panel_h + FOOTER_BAND_H).max(SETTINGS_MIN_CLIENT_H);

    let dpi = GetDpiForWindow(hwnd).max(96);
    let (ow, oh) = outer_size_for_client(client_w, client_h, dpi);
    let (fw, fh) = clamp_size_to_work_area(ow, oh, pt);
    let (fx, fy) = super::placement::settings_window_pos(pt, fw, fh);
    let _ = SetWindowPos(
        hwnd,
        HWND_TOP,
        fx,
        fy,
        fw,
        fh,
        SWP_NOACTIVATE | SWP_NOCOPYBITS,
    );
    layout_settings(hwnd, s);
}

/// Shrink `(w, h)` to fit the work area at `pt` (never enlarge; §7 sizing).
unsafe fn clamp_size_to_work_area(w: i32, h: i32, pt: POINT) -> (i32, i32) {
    let Some(wa) = work_area_at_point(pt) else {
        return (w, h);
    };
    let margin = 8i32;
    let max_w = (wa.right - wa.left - margin).max(320);
    let max_h = (wa.bottom - wa.top - margin).max(240);
    (w.min(max_w), h.min(max_h))
}

/// Size every page panel to the page band (even when parked on HWND_MESSAGE).
unsafe fn size_all_page_panels(hwnd: HWND, s: &SettingsState) -> (i32, i32) {
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    if rc.right <= 0 || rc.bottom <= 0 {
        return (0, 0);
    }
    let (_, _, pw, ph) = page_panel_rect(&rc);
    for p in [
        s.panel_slots,
        s.panel_keys,
        s.panel_triggers,
        s.panel_search,
        s.panel_mode,
        s.panel_general,
    ] {
        if p.0.is_null() {
            continue;
        }
        let _ = SetWindowPos(
            p,
            HWND_TOP,
            0,
            0,
            pw,
            ph,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOCOPYBITS,
        );
    }
    (pw, ph)
}

fn active_page_panel(s: &SettingsState) -> HWND {
    match s.page {
        Page::Slots => s.panel_slots,
        Page::Keys => s.panel_keys,
        Page::Triggers => s.panel_triggers,
        Page::Search => s.panel_search,
        Page::Mode => s.panel_mode,
        Page::General => s.panel_general,
    }
}

/// Tabs + footer above the active page panel. Do not sink panels with
/// HWND_BOTTOM — that left a hidden full-size sibling above the active page
/// and produced Options→Keys ghosting (WS_CLIPCHILDREN holes / stale pixels).
unsafe fn pin_chrome(s: &SettingsState) {
    let top = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
    for h in [
        s.tab_general,
        s.tab_slots,
        s.tab_mode,
        s.tab_keys,
        s.tab_triggers,
        s.tab_search,
        s.chk_advanced,
        s.btn_save,
        s.btn_apply,
        s.btn_cancel,
        s.btn_add,
        s.btn_delete,
        s.btn_swap,
        s.btn_slot_apply,
    ] {
        if !h.0.is_null() {
            let _ = SetWindowPos(h, HWND_TOP, 0, 0, 0, 0, top);
        }
    }
}

/// Clear the tab content band (WS_CLIPCHILDREN otherwise leaves stale pixels).
unsafe fn erase_page_band(hwnd: HWND, rc: &RECT) {
    let (px, py, pw, ph) = page_panel_rect(rc);
    let band = RECT {
        left: px,
        top: py,
        right: px + pw,
        bottom: py + ph,
    };
    let hdc = GetDC(hwnd);
    if !hdc.0.is_null() {
        let _ = FillRect(hdc, &band, GetSysColorBrush(COLOR_WINDOW));
        let _ = ReleaseDC(hwnd, hdc);
    }
}

/// Show one page panel. Inactive panels are reparented to HWND_MESSAGE so they
/// cannot paint over the active tab (ShowWindow/zero-size was not enough when
/// navigating Options → Keys → Slots).
unsafe fn apply_page_panel_layout(hwnd: HWND, s: &SettingsState) {
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    if rc.right <= 0 || rc.bottom <= 0 {
        return;
    }
    let (px, py, pw, ph) = page_panel_rect(&rc);
    let active = active_page_panel(s);

    for p in [
        s.panel_slots,
        s.panel_keys,
        s.panel_triggers,
        s.panel_search,
        s.panel_mode,
        s.panel_general,
    ] {
        if p.0.is_null() || p == active {
            continue;
        }
        let _ = ShowWindow(p, SW_HIDE);
        let _ = SetParent(p, HWND_MESSAGE);
    }

    erase_page_band(hwnd, &rc);

    if !active.0.is_null() {
        let _ = SetParent(active, hwnd);
        let _ = SetWindowPos(
            active,
            HWND_TOP,
            px,
            py,
            pw,
            ph,
            SWP_NOACTIVATE | SWP_NOCOPYBITS | SWP_SHOWWINDOW,
        );
        let _ = ShowWindow(active, SW_SHOW);
    }
}

/// Move a child without copying old pixels (avoids leftover border lines).
unsafe fn move_child_clean(hwnd: HWND, x: i32, y: i32, w: i32, h: i32) {
    let _ = SetWindowPos(
        hwnd,
        HWND_TOP,
        x,
        y,
        w,
        h,
        SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOCOPYBITS,
    );
}

unsafe fn set_hwnd_visible(hwnd: HWND, on: bool) {
    if hwnd.0.is_null() {
        return;
    }
    let _ = ShowWindow(hwnd, if on { SW_SHOW } else { SW_HIDE });
}

/// General + Slots always; Mode/Keys/Triggers/Search only when Advanced is on.
/// Advanced checkbox sits on the right of the tab band. Open still starts on Slots.
unsafe fn place_tab_buttons(hwnd: HWND, s: &SettingsState) {
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    if rc.right <= 0 {
        return;
    }
    let adv = s.draft_settings.ui.show_advanced;
    let tab_y = 10i32;
    let tab_h = 26i32;
    let tab_gap = 8i32;
    let flags_show = SWP_NOACTIVATE | SWP_NOCOPYBITS | SWP_SHOWWINDOW;
    let flags_hide = SWP_NOACTIVATE | SWP_NOCOPYBITS | SWP_HIDEWINDOW;
    let mut tab_x = 12i32;
    let place = |h: HWND, x: i32, w: i32, show: bool| {
        if h.0.is_null() {
            return;
        }
        let _ = SetWindowPos(
            h,
            HWND_TOP,
            x,
            tab_y,
            w,
            tab_h,
            if show { flags_show } else { flags_hide },
        );
    };
    let w_gen = fitted_button_width(hwnd, &window_text(s.tab_general)).max(80);
    place(s.tab_general, tab_x, w_gen, true);
    tab_x += w_gen + tab_gap;
    let w_slots = fitted_button_width(hwnd, &window_text(s.tab_slots)).max(80);
    place(s.tab_slots, tab_x, w_slots, true);
    tab_x += w_slots + tab_gap;
    let w_mode = fitted_button_width(hwnd, &window_text(s.tab_mode)).max(80);
    place(s.tab_mode, tab_x, w_mode, adv);
    if adv {
        tab_x += w_mode + tab_gap;
    }
    let w_keys = fitted_button_width(hwnd, &window_text(s.tab_keys)).max(80);
    place(s.tab_keys, tab_x, w_keys, adv);
    if adv {
        tab_x += w_keys + tab_gap;
    }
    let w_tr = fitted_button_width(hwnd, &window_text(s.tab_triggers)).max(80);
    place(s.tab_triggers, tab_x, w_tr, adv);
    if adv {
        tab_x += w_tr + tab_gap;
    }
    let w_se = fitted_button_width(hwnd, &window_text(s.tab_search)).max(80);
    place(s.tab_search, tab_x, w_se, adv);
    if adv {
        tab_x += w_se + tab_gap;
    }
    let _ = EnableWindow(s.tab_mode, adv);
    let _ = EnableWindow(s.tab_keys, adv);
    let _ = EnableWindow(s.tab_triggers, adv);
    let _ = EnableWindow(s.tab_search, adv);
    sync_tab_checks(s);

    let adv_label_w = measure_text_width(hwnd, &window_text(s.chk_advanced)) + 28;
    let adv_x = (rc.right - FOOTER_RIGHT_PAD - adv_label_w).max(tab_x);
    let _ = SetWindowPos(
        s.chk_advanced,
        HWND_TOP,
        adv_x,
        tab_y,
        adv_label_w,
        tab_h,
        flags_show,
    );
}

/// Save/Apply/Cancel (+ Slots Add/Delete/Swap/Update when on Slots) in the footer band.
unsafe fn place_footer_buttons(hwnd: HWND, s: &SettingsState) {
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    if rc.right <= 0 || rc.bottom <= 0 {
        return;
    }
    let by = (rc.bottom - FOOTER_BOTTOM_PAD - FOOTER_BTN_H).max(TAB_BAND_H + 8);
    // NOCOPYBITS: without it, classic button borders leave vertical ghost lines in the gaps.
    let flags = SWP_NOACTIVATE | SWP_NOCOPYBITS | SWP_SHOWWINDOW;
    let save_w = fitted_button_width(hwnd, &window_text(s.btn_save));
    let apply_w = fitted_button_width(hwnd, &window_text(s.btn_apply));
    let cancel_w = fitted_button_width(hwnd, &window_text(s.btn_cancel));
    let add_w = fitted_button_width(hwnd, &window_text(s.btn_add));
    let del_w = fitted_button_width(hwnd, &window_text(s.btn_delete));
    let swap_w = fitted_button_width(hwnd, &window_text(s.btn_swap));
    let update_w = fitted_button_width(hwnd, &window_text(s.btn_slot_apply));
    let margin = 12i32;
    let slots_on = s.page == Page::Slots;
    let slot_flags = if slots_on {
        flags
    } else {
        SWP_NOACTIVATE | SWP_NOCOPYBITS | SWP_HIDEWINDOW
    };

    let cancel_x = (rc.right - FOOTER_RIGHT_PAD - cancel_w).max(8);
    let apply_x = (cancel_x - FOOTER_BTN_GAP - apply_w).max(8);
    let save_x = (apply_x - FOOTER_BTN_GAP - save_w).max(8);

    let place = |btn: HWND, x: i32, w: i32, show_flags| {
        let _ = SetWindowPos(btn, HWND_TOP, x, by, w, FOOTER_BTN_H, show_flags);
    };

    if slots_on {
        let left_w =
            add_w + FOOTER_BTN_GAP + del_w + FOOTER_BTN_GAP + swap_w + FOOTER_BTN_GAP + update_w;
        let update_right = margin + left_w;
        // Extra space lives between Update and Save. Never let that gap go below FOOTER_BTN_GAP.
        let pack_all = update_right + FOOTER_BTN_GAP > save_x;
        if pack_all {
            let mut sx = margin;
            place(s.btn_add, sx, add_w, flags);
            sx += add_w + FOOTER_BTN_GAP;
            place(s.btn_delete, sx, del_w, flags);
            sx += del_w + FOOTER_BTN_GAP;
            place(s.btn_swap, sx, swap_w, flags);
            sx += swap_w + FOOTER_BTN_GAP;
            place(s.btn_slot_apply, sx, update_w, flags);
            sx += update_w + FOOTER_BTN_GAP;
            place(s.btn_save, sx, save_w, flags);
            sx += save_w + FOOTER_BTN_GAP;
            place(s.btn_apply, sx, apply_w, flags);
            sx += apply_w + FOOTER_BTN_GAP;
            place(s.btn_cancel, sx, cancel_w, flags);
        } else {
            let mut sx = margin;
            place(s.btn_add, sx, add_w, flags);
            sx += add_w + FOOTER_BTN_GAP;
            place(s.btn_delete, sx, del_w, flags);
            sx += del_w + FOOTER_BTN_GAP;
            place(s.btn_swap, sx, swap_w, flags);
            sx += swap_w + FOOTER_BTN_GAP;
            place(s.btn_slot_apply, sx, update_w, flags);
            place(s.btn_save, save_x, save_w, flags);
            place(s.btn_apply, apply_x, apply_w, flags);
            place(s.btn_cancel, cancel_x, cancel_w, flags);
        }
    } else {
        place(s.btn_add, margin, add_w, slot_flags);
        place(s.btn_delete, margin, del_w, slot_flags);
        place(s.btn_swap, margin, swap_w, slot_flags);
        place(s.btn_slot_apply, margin, update_w, slot_flags);
        place(s.btn_save, save_x, save_w, flags);
        place(s.btn_apply, apply_x, apply_w, flags);
        place(s.btn_cancel, cancel_x, cancel_w, flags);
    }

    let footer = RECT {
        left: 0,
        top: (rc.bottom - FOOTER_BAND_H).max(0),
        right: rc.right,
        bottom: rc.bottom,
    };
    let _ = RedrawWindow(
        hwnd,
        Some(&footer),
        None,
        RDW_ERASE | RDW_INVALIDATE | RDW_FRAME | RDW_UPDATENOW,
    );
    for h in [
        s.btn_save,
        s.btn_apply,
        s.btn_cancel,
        s.btn_add,
        s.btn_delete,
        s.btn_swap,
        s.btn_slot_apply,
    ] {
        let _ = RedrawWindow(h, None, None, RDW_ERASE | RDW_INVALIDATE | RDW_UPDATENOW);
    }
}

/// Recompute page panels and reflow every tab so controls never sit under the footer.
unsafe fn layout_settings(hwnd: HWND, s: &SettingsState) {
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    if rc.right <= 0 || rc.bottom <= 0 {
        return;
    }

    // Size all panels first (including those parked on HWND_MESSAGE), then reflow.
    let _ = size_all_page_panels(hwnd, s);
    layout_slots_page(hwnd, s);
    layout_keys_page(hwnd, s);
    layout_triggers_page(hwnd, s);
    layout_search_page(hwnd, s);
    layout_mode_page(hwnd, s);
    layout_general_page(hwnd, s);
    place_tab_buttons(hwnd, s);
    apply_page_panel_layout(hwnd, s);
    place_footer_buttons(hwnd, s);
    pin_chrome(s);
    let active = active_page_panel(s);
    let _ = RedrawWindow(hwnd, None, None, RDW_ERASE | RDW_INVALIDATE | RDW_UPDATENOW);
    if !active.0.is_null() {
        let _ = RedrawWindow(
            active,
            None,
            None,
            RDW_ERASE | RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW,
        );
    }
}

/// Stacked Slots form height (ID/Name through Description + error), excluding left-list Y offset.
fn slots_form_stack_h(desc_h: i32, warn_h: i32) -> i32 {
    let labeled_edit = SLOTS_FORM_LABEL_H + LABEL_STACK_GAP + SLOTS_FORM_EDIT_H + FIELD_BLOCK_GAP;
    let chk = SLOTS_OPEN_WORKDIR_H + FIELD_BLOCK_GAP;
    labeled_edit * 3
        + chk * 3
        + SLOTS_FORM_LABEL_H
        + LABEL_STACK_GAP
        + desc_h
        + FIELD_BLOCK_GAP
        + warn_h
}

/// Visible height for `rows` listbox items (font must already be set).
unsafe fn listbox_rows_h(list: HWND, rows: i32) -> i32 {
    let item = SendMessageW(list, LB_GETITEMHEIGHT, WPARAM(0), LPARAM(0)).0 as i32;
    let row = if item > 4 { item } else { 16 };
    (row * rows + 4).max(MODE_CHORD_LIST_H_MIN)
}

/// Caption column for Action-key rows (widest label, capped so Trigger still fits).
unsafe fn action_key_caption_col_w(hwnd: HWND, inner_w: i32, labels: &[&str]) -> i32 {
    let mut w = 72;
    for label in labels {
        w = w.max(measure_text_width(hwnd, label));
    }
    (w + 16).clamp(80, (inner_w * 2 / 5).max(80))
}

/// Mode chord row: tab-separated code, slots basename, `meta.displayName`.
fn mode_chord_row_text(dir: &Path, code: &str, file: Option<&str>, _current: &str) -> String {
    let name = file
        .map(normalize_slots_file_name)
        .filter(|n| !n.is_empty());
    let Some(name) = name else {
        return code.to_string();
    };
    let path = resolve_slots_path(dir, &name);
    let dn = read_slots_display_name(&path).unwrap_or_default();
    format!("{code}\t{name}\t{dn}")
}

/// List-box tab stops are dialog units (¼ of the average character width).
unsafe fn set_listbox_tab_stops(list: HWND, stops_px: &[i32]) {
    if list.0.is_null() || stops_px.is_empty() {
        return;
    }
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let avg = (measure_text_width(list, alphabet) / 52).max(4);
    let dlus: Vec<i32> = stops_px.iter().map(|px| (px * 4) / avg).collect();
    let _ = SendMessageW(
        list,
        LB_SETTABSTOPS,
        WPARAM(dlus.len()),
        LPARAM(dlus.as_ptr() as isize),
    );
}

unsafe fn layout_slots_page(hwnd: HWND, s: &SettingsState) -> i32 {
    let mut prc = RECT::default();
    let _ = GetClientRect(s.panel_slots, &mut prc);
    if prc.right <= 0 || prc.bottom <= 0 {
        return 0;
    }
    let margin = 12i32;
    let list_w = SLOTS_LIST_W.min((prc.right / 3).max(200));
    let form_x = margin + list_w + SLOTS_LIST_FORM_GAP;
    let form_w = (prc.right - form_x - margin).max(200);
    let id_w = SLOTS_ID_COL_W.min(form_w / 3).max(72);
    let name_x = form_x + id_w + 12;
    let name_w = (form_w - id_w - 12).max(120);

    let list_top = margin.max(6);
    move_child_clean(
        s.lbl_slots_current,
        margin,
        list_top,
        (prc.right - margin * 2).max(120),
        SLOTS_FORM_LABEL_H,
    );
    let list_top = list_top + SLOTS_FORM_LABEL_H + LABEL_STACK_GAP;
    let mut fy = list_top;
    move_child_clean(s.lbl_slot_id, form_x, fy, id_w, SLOTS_FORM_LABEL_H);
    move_child_clean(s.lbl_slot_name, name_x, fy, name_w, SLOTS_FORM_LABEL_H);
    fy += SLOTS_FORM_LABEL_H + LABEL_STACK_GAP;
    move_child_clean(s.edit_id, form_x, fy, id_w.min(100), SLOTS_FORM_EDIT_H);
    move_child_clean(s.edit_name, name_x, fy, name_w, SLOTS_FORM_EDIT_H);
    fy += SLOTS_FORM_EDIT_H + FIELD_BLOCK_GAP;

    for (lbl, edit) in [
        (s.lbl_slot_path, s.edit_path),
        (s.lbl_slot_workdir, s.edit_workdir),
    ] {
        move_child_clean(lbl, form_x, fy, form_w, SLOTS_FORM_LABEL_H);
        fy += SLOTS_FORM_LABEL_H + LABEL_STACK_GAP;
        move_child_clean(edit, form_x, fy, form_w, SLOTS_FORM_EDIT_H);
        fy += SLOTS_FORM_EDIT_H + FIELD_BLOCK_GAP;
    }

    move_child_clean(s.chk_open_workdir, form_x, fy, form_w, SLOTS_OPEN_WORKDIR_H);
    fy += SLOTS_OPEN_WORKDIR_H + FIELD_BLOCK_GAP;
    move_child_clean(
        s.chk_focus_existing,
        form_x,
        fy,
        form_w,
        SLOTS_OPEN_WORKDIR_H,
    );
    fy += SLOTS_OPEN_WORKDIR_H + FIELD_BLOCK_GAP;
    move_child_clean(s.chk_instant_fire, form_x, fy, form_w, SLOTS_OPEN_WORKDIR_H);
    fy += SLOTS_OPEN_WORKDIR_H + FIELD_BLOCK_GAP;

    // Description grows into free space; warn sits under it. List bottom matches warn.
    let mut warn_h = SLOTS_WARN_H;
    let mut after = warn_h + margin;
    let mut budget = prc.bottom - fy - SLOTS_FORM_LABEL_H - LABEL_STACK_GAP - after;
    if budget < SLOTS_DESC_H_MIN {
        warn_h = 28;
        after = warn_h + margin;
        budget = prc.bottom - fy - SLOTS_FORM_LABEL_H - LABEL_STACK_GAP - after;
    }
    let desc_h = budget.clamp(SLOTS_DESC_H_MIN, SLOTS_DESC_H_MAX);

    move_child_clean(s.lbl_slot_desc, form_x, fy, form_w, SLOTS_FORM_LABEL_H);
    fy += SLOTS_FORM_LABEL_H + LABEL_STACK_GAP;
    move_child_clean(s.edit_desc, form_x, fy, form_w, desc_h);
    fy += desc_h + FIELD_BLOCK_GAP;

    move_child_clean(s.warn_label, form_x, fy, form_w, warn_h);
    fy += warn_h;

    let list_h = (fy - list_top).max(60);
    move_child_clean(s.slot_list, margin, list_top, list_w, list_h);

    let _ = hwnd;
    // Preferred window height uses the default Description, not a stretched panel.
    list_top + slots_form_stack_h(SLOTS_DESC_H_DEFAULT, SLOTS_WARN_H) + margin
}

unsafe fn layout_keys_page(hwnd: HWND, s: &SettingsState) -> i32 {
    let mut prc = RECT::default();
    let _ = GetClientRect(s.panel_keys, &mut prc);
    if prc.right <= 0 || prc.bottom <= 0 || s.g_keys.0.is_null() {
        return 0;
    }
    let content_w = (prc.right - 24).max(200);
    let stack_x = 28i32;
    let stack_inner_w = (content_w - 32).max(160);
    let keys_cap_w = fitted_button_width(hwnd, &t(keys::SETTINGS_CAPTURE))
        .max(fitted_button_width(hwnd, &t(keys::SETTINGS_CANCEL)));
    let label_h = 26i32;
    let row_ctl_h = 24i32;
    let disp_w = 56i32;
    let cap_texts = [
        window_text(s.lbl_start_cap),
        window_text(s.lbl_confirm_cap),
        window_text(s.lbl_cancel_cap),
        window_text(s.lbl_search_cap),
        window_text(s.lbl_open_workdir_cap),
        window_text(s.lbl_digit_back_cap),
        window_text(s.lbl_wake_cap),
    ];
    let cap_refs: Vec<&str> = cap_texts.iter().map(|t| t.as_str()).collect();
    let cap_w = action_key_caption_col_w(hwnd, stack_inner_w, &cap_refs);

    let keys_top = 6i32;
    let mut ky = keys_top + 28;
    let set_combo_w = 140i32.min(stack_inner_w / 3);
    let set_lbl_w = measure_text_width(hwnd, &window_text(s.lbl_key_set)).max(48) + KEYS_COL_GAP;
    move_child_clean(s.lbl_key_set, stack_x, ky, set_lbl_w, label_h);
    move_child_clean(
        s.combo_key_set,
        stack_x + set_lbl_w + KEYS_COL_GAP,
        ky,
        set_combo_w,
        200,
    );
    ky += label_h.max(row_ctl_h) + FIELD_BLOCK_GAP;
    let place_key_row = |ky: &mut i32, cap: HWND, value: HWND, edit: HWND, btn: HWND| {
        // Caption | Trigger (VK) | Display | Capture — one row.
        move_child_clean(cap, stack_x, *ky, cap_w, label_h);
        let rest_x = stack_x + cap_w + KEYS_COL_GAP;
        let rest_w = (stack_inner_w - cap_w - KEYS_COL_GAP).max(160);
        let value_w = (rest_w - keys_cap_w - disp_w - KEYS_COL_GAP * 2).max(80);
        move_child_clean(value, rest_x, *ky, value_w, label_h);
        move_child_clean(
            edit,
            rest_x + value_w + KEYS_COL_GAP,
            *ky,
            disp_w,
            row_ctl_h,
        );
        move_child_clean(
            btn,
            rest_x + value_w + KEYS_COL_GAP + disp_w + KEYS_COL_GAP,
            *ky,
            keys_cap_w,
            row_ctl_h,
        );
        *ky += label_h.max(row_ctl_h) + FIELD_BLOCK_GAP;
    };
    place_key_row(
        &mut ky,
        s.lbl_start_cap,
        s.lbl_start,
        s.edit_start_label,
        s.btn_cap_start,
    );
    place_key_row(
        &mut ky,
        s.lbl_confirm_cap,
        s.lbl_confirm,
        s.edit_confirm_label,
        s.btn_cap_confirm,
    );
    place_key_row(
        &mut ky,
        s.lbl_cancel_cap,
        s.lbl_cancel,
        s.edit_cancel_label,
        s.btn_cap_cancel,
    );
    place_key_row(
        &mut ky,
        s.lbl_search_cap,
        s.lbl_search,
        s.edit_search_label,
        s.btn_cap_search,
    );
    place_key_row(
        &mut ky,
        s.lbl_open_workdir_cap,
        s.lbl_open_workdir_key,
        s.edit_open_workdir_label,
        s.btn_cap_open_workdir,
    );
    place_key_row(
        &mut ky,
        s.lbl_digit_back_cap,
        s.lbl_digit_back,
        s.edit_digit_back_label,
        s.btn_cap_digit_back,
    );
    let wake_txt = window_text(s.chk_wake);
    let wake_h = if measure_text_width(hwnd, &wake_txt) + 28 > stack_inner_w {
        36
    } else {
        22
    };
    move_child_clean(s.chk_wake, stack_x, ky, stack_inner_w, wake_h);
    ky += wake_h.max(24) + FIELD_BLOCK_GAP;
    // ON: Caption | Trigger | (no Display) | Capture — Capture aligns with action rows.
    move_child_clean(s.lbl_wake_cap, stack_x, ky, cap_w, label_h);
    {
        let rest_x = stack_x + cap_w + KEYS_COL_GAP;
        let rest_w = (stack_inner_w - cap_w - KEYS_COL_GAP).max(160);
        let value_w = (rest_w - keys_cap_w - disp_w - KEYS_COL_GAP * 2).max(80);
        move_child_clean(s.lbl_wake, rest_x, ky, value_w, label_h);
        move_child_clean(
            s.btn_cap_wake,
            rest_x + value_w + KEYS_COL_GAP + disp_w + KEYS_COL_GAP,
            ky,
            keys_cap_w,
            row_ctl_h,
        );
    }
    ky += label_h.max(row_ctl_h) + FIELD_BLOCK_GAP;
    let reset_w = fitted_button_width(hwnd, &window_text(s.btn_reset_keys));
    move_child_clean(s.btn_reset_keys, stack_x, ky, reset_w, 26);
    ky += 26 + FIELD_BLOCK_GAP;
    let g_keys_h = (ky - keys_top).max(80);
    move_child_clean(s.g_keys, 12, keys_top, content_w, g_keys_h);
    let _ = SetWindowPos(
        s.btn_reset_keys,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    let _ = SetWindowPos(
        s.chk_wake,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    let _ = SetWindowPos(
        s.btn_cap_wake,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    let _ = SetWindowPos(
        s.g_keys,
        HWND_BOTTOM,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );

    ky = keys_top + g_keys_h + KEYS_GROUP_GAP;
    let legend_top = ky;
    ky += 28;
    let col_w = ((stack_inner_w - 16) / 2).max(160);
    let right_x = stack_x + col_w + 16;
    move_child_clean(s.lbl_legend_idle, stack_x, ky, col_w, label_h);
    move_child_clean(s.lbl_legend_multi, right_x, ky, col_w, label_h);
    ky += label_h + LABEL_STACK_GAP;
    let chk_h = 22i32;
    for i in 0..LEGEND_ITEM_COUNT {
        move_child_clean(s.chk_legend_idle[i], stack_x, ky, col_w, chk_h);
        move_child_clean(s.chk_legend_multi[i], right_x, ky, col_w, chk_h);
        ky += chk_h + FIELD_BLOCK_GAP;
    }
    ky += FIELD_BLOCK_GAP;
    let up_w = fitted_button_width(hwnd, &window_text(s.btn_legend_idle_up));
    let down_w = fitted_button_width(hwnd, &window_text(s.btn_legend_idle_down));
    move_child_clean(s.btn_legend_idle_up, stack_x, ky, up_w, 24);
    move_child_clean(s.btn_legend_idle_down, stack_x + up_w + 8, ky, down_w, 24);
    let m_up_w = fitted_button_width(hwnd, &window_text(s.btn_legend_multi_up));
    let m_down_w = fitted_button_width(hwnd, &window_text(s.btn_legend_multi_down));
    move_child_clean(s.btn_legend_multi_up, right_x, ky, m_up_w, 24);
    move_child_clean(
        s.btn_legend_multi_down,
        right_x + m_up_w + 8,
        ky,
        m_down_w,
        24,
    );
    ky += 24 + FIELD_BLOCK_GAP;
    move_child_clean(
        s.g_legend,
        12,
        legend_top,
        content_w,
        (ky - legend_top).max(80),
    );
    let _ = SetWindowPos(
        s.g_legend,
        HWND_BOTTOM,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );

    // Capture / error strip under Typing window (not between the two groups).
    ky += KEYS_GROUP_GAP;
    move_child_clean(
        s.capture_status,
        stack_x,
        ky,
        stack_inner_w,
        CAPTURE_STATUS_H,
    );
    ky += CAPTURE_STATUS_H;
    ky
}

/// Leader mouse / timeout / set letters — Switch triggers group on the Triggers tab.
unsafe fn layout_switch_triggers_group(
    hwnd: HWND,
    s: &SettingsState,
    content_w: i32,
    stack_x: i32,
    stack_inner_w: i32,
    label_h: i32,
    switch_top: i32,
) -> i32 {
    let mut oy = switch_top + 28;
    move_child_clean(s.lbl_mode_mouse, stack_x, oy, stack_inner_w, label_h);
    oy += label_h + LABEL_STACK_GAP;
    move_child_clean(s.combo_mode_mouse, stack_x, oy, 120.min(stack_inner_w), 200);
    oy += 28 + FIELD_BLOCK_GAP;
    let suppress_txt = window_text(s.chk_mode_suppress);
    let suppress_h = if measure_text_width(hwnd, &suppress_txt) + 28 > stack_inner_w {
        36
    } else {
        22
    };
    move_child_clean(s.chk_mode_suppress, stack_x, oy, stack_inner_w, suppress_h);
    oy += suppress_h + FIELD_BLOCK_GAP;
    move_child_clean(s.lbl_mode_timeout, stack_x, oy, stack_inner_w, label_h);
    oy += label_h + LABEL_STACK_GAP;
    move_child_clean(s.edit_mode_timeout, stack_x, oy, 80, 24);
    oy += 24 + FIELD_BLOCK_GAP;

    let num_lbl = window_text(s.lbl_key_chord);
    let kb_lbl = window_text(s.lbl_key_chord_kb);
    let num_w = measure_text_width(hwnd, &num_lbl).max(48) + KEYS_COL_GAP;
    let kb_w = measure_text_width(hwnd, &kb_lbl).max(48) + KEYS_COL_GAP;
    let letter_combo_w = 56i32;
    let pair_gap = 32i32;
    move_child_clean(s.lbl_key_chord, stack_x, oy, num_w, label_h);
    move_child_clean(
        s.combo_key_chord,
        stack_x + num_w + KEYS_COL_GAP,
        oy,
        letter_combo_w,
        200,
    );
    let kb_x = stack_x + num_w + KEYS_COL_GAP + letter_combo_w + pair_gap;
    move_child_clean(s.lbl_key_chord_kb, kb_x, oy, kb_w, label_h);
    move_child_clean(
        s.combo_key_chord_kb,
        kb_x + kb_w + KEYS_COL_GAP,
        oy,
        letter_combo_w,
        200,
    );
    oy += 28 + 14;
    move_child_clean(
        s.g_mode_switch,
        12,
        switch_top,
        content_w,
        (oy - switch_top).max(80),
    );
    let _ = SetWindowPos(
        s.chk_mode_suppress,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    let _ = SetWindowPos(
        s.g_mode_switch,
        HWND_BOTTOM,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    oy
}

unsafe fn layout_triggers_page(hwnd: HWND, s: &SettingsState) -> i32 {
    let mut prc = RECT::default();
    let _ = GetClientRect(s.panel_triggers, &mut prc);
    if prc.right <= 0 || prc.bottom <= 0 || s.g_triggers.0.is_null() || s.g_mode_switch.0.is_null()
    {
        return 0;
    }
    let content_w = (prc.right - 24).max(200);
    let stack_x = 28i32;
    let stack_inner_w = (content_w - 32).max(160);
    let keys_cap_w = fitted_button_width(hwnd, &window_text(s.btn_cap_mouse));
    let label_h = 26i32;
    let row_ctl_h = 24i32;

    let mut ky =
        layout_switch_triggers_group(hwnd, s, content_w, stack_x, stack_inner_w, label_h, 6);

    let trig_top = ky + KEYS_GROUP_GAP;
    ky = trig_top + 28;
    move_child_clean(s.lbl_mouse, stack_x, ky, stack_inner_w, label_h);
    ky += label_h + LABEL_STACK_GAP;
    let mouse_combo_w = 120i32.min(stack_inner_w / 2);
    move_child_clean(s.combo_mouse, stack_x, ky, mouse_combo_w, 200);
    move_child_clean(
        s.btn_cap_mouse,
        stack_x + mouse_combo_w + 8,
        ky,
        keys_cap_w,
        row_ctl_h,
    );
    ky += row_ctl_h + FIELD_BLOCK_GAP;
    let suppress_txt = window_text(s.chk_suppress);
    let suppress_h = if measure_text_width(hwnd, &suppress_txt) + 28 > stack_inner_w {
        36
    } else {
        22
    };
    move_child_clean(s.chk_suppress, stack_x, ky, stack_inner_w, suppress_h);
    ky += suppress_h + FIELD_BLOCK_GAP;
    move_child_clean(s.lbl_hotkey, stack_x, ky, stack_inner_w, label_h);
    ky += label_h + LABEL_STACK_GAP;
    move_child_clean(s.edit_hotkey, stack_x, ky, 220.min(stack_inner_w), 24);
    ky += 24 + LABEL_STACK_GAP;
    move_child_clean(s.lbl_hotkey_hint, stack_x, ky, stack_inner_w, label_h);
    ky += label_h + FIELD_BLOCK_GAP;
    let reset_w = fitted_button_width(hwnd, &window_text(s.btn_reset_triggers));
    move_child_clean(s.btn_reset_triggers, stack_x, ky, reset_w, 26);
    ky += 26 + LABEL_STACK_GAP;
    move_child_clean(s.lbl_triggers_status, stack_x, ky, stack_inner_w, label_h);
    ky += label_h + FIELD_BLOCK_GAP;
    let g_trig_h = (ky - trig_top).max(100);
    move_child_clean(s.g_triggers, 12, trig_top, content_w, g_trig_h);
    let _ = SetWindowPos(
        s.g_triggers,
        HWND_BOTTOM,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    let _ = SetWindowPos(
        s.chk_suppress,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    ky
}

unsafe fn layout_search_page(hwnd: HWND, s: &SettingsState) -> i32 {
    let mut prc = RECT::default();
    let _ = GetClientRect(s.panel_search, &mut prc);
    if prc.right <= 0 || prc.bottom <= 0 || s.g_search.0.is_null() {
        return 0;
    }
    let content_w = (prc.right - 24).max(200);
    let stack_x = 28i32;
    let stack_inner_w = (content_w - 32).max(160);
    let label_h = 26i32;

    let search_top = 6i32;
    let mut oy = search_top + 28;
    move_child_clean(s.lbl_match_mode, stack_x, oy, stack_inner_w, label_h);
    oy += label_h + LABEL_STACK_GAP;
    move_child_clean(s.combo_search, stack_x, oy, 160.min(stack_inner_w), 200);
    oy += 28 + FIELD_BLOCK_GAP;
    let case_txt = window_text(s.chk_case);
    let case_h = if measure_text_width(hwnd, &case_txt) + 28 > stack_inner_w {
        36
    } else {
        22
    };
    move_child_clean(s.chk_case, stack_x, oy, stack_inner_w, case_h);
    oy += case_h + 18;
    move_child_clean(
        s.g_search,
        12,
        search_top,
        content_w,
        (oy - search_top).max(80),
    );
    let _ = SetWindowPos(
        s.chk_case,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    let _ = SetWindowPos(
        s.g_search,
        HWND_BOTTOM,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    oy
}

/// Chord map (0–9) in one list; slots-file combo + Set/Clear on the header row.
unsafe fn layout_mode_page(hwnd: HWND, s: &SettingsState) -> i32 {
    let mut prc = RECT::default();
    let _ = GetClientRect(s.panel_mode, &mut prc);
    if prc.right <= 0 || prc.bottom <= 0 || s.g_mode.0.is_null() {
        return 0;
    }
    let content_w = (prc.right - 24).max(200);
    let stack_x = 28i32;
    let stack_inner_w = (content_w - 32).max(160);
    let label_h = 26i32;
    let margin = 12i32;

    let mode_top = 6i32;
    let mut oy = mode_top + 28;
    let cur_lbl_w = measure_text_width(hwnd, &window_text(s.lbl_mode_current)).max(72) + 8;
    move_child_clean(s.lbl_mode_current, stack_x, oy, cur_lbl_w, label_h);
    move_child_clean(
        s.combo_mode_current,
        stack_x + cur_lbl_w + 8,
        oy,
        (stack_inner_w - cur_lbl_w - 8).max(160),
        200,
    );
    oy += 28 + FIELD_BLOCK_GAP;

    let hint_h = 36i32;
    move_child_clean(s.lbl_mode_hint, stack_x, oy, stack_inner_w, hint_h);
    oy += hint_h + FIELD_BLOCK_GAP;

    let set_w = fitted_button_width(hwnd, &window_text(s.btn_mode_set_override));
    let clear_w = fitted_button_width(hwnd, &window_text(s.btn_mode_clear_override));
    let gap = 8i32;
    let file_label_w = measure_text_width(hwnd, &window_text(s.lbl_mode_chord_file)).max(64) + 8;
    let combo_w = 180i32.min(stack_inner_w / 3).max(120);
    let right_cluster = file_label_w + gap + combo_w + gap + set_w + gap + clear_w;
    let left_w = (stack_inner_w - right_cluster - gap).max(160);
    let right_x = stack_x + left_w + gap;

    move_child_clean(s.lbl_mode_chords, stack_x, oy, left_w, label_h);
    move_child_clean(s.lbl_mode_chord_file, right_x, oy, file_label_w, label_h);
    let combo_x = right_x + file_label_w + gap;
    move_child_clean(s.combo_mode_files, combo_x, oy, combo_w, 200);
    move_child_clean(
        s.btn_mode_set_override,
        combo_x + combo_w + gap,
        oy,
        set_w,
        24,
    );
    move_child_clean(
        s.btn_mode_clear_override,
        combo_x + combo_w + gap + set_w + gap,
        oy,
        clear_w,
        24,
    );
    oy += label_h + LABEL_STACK_GAP;

    let rows_h = listbox_rows_h(s.list_mode_chords, MODE_CHORD_ROWS);
    move_child_clean(s.list_mode_chords, stack_x, oy, stack_inner_w, rows_h);
    set_listbox_tab_stops(s.list_mode_chords, &[36, 160]);
    oy += rows_h + 4;
    move_child_clean(s.lbl_mode_status, stack_x, oy, stack_inner_w, label_h);
    oy += label_h + margin;

    move_child_clean(s.g_mode, 12, mode_top, content_w, (oy - mode_top).max(120));
    let _ = SetWindowPos(
        s.g_mode,
        HWND_BOTTOM,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    oy
}

unsafe fn layout_general_page(hwnd: HWND, s: &SettingsState) -> i32 {
    let mut prc = RECT::default();
    let _ = GetClientRect(s.panel_general, &mut prc);
    if prc.right <= 0 || prc.bottom <= 0 || s.g_general.0.is_null() {
        return 0;
    }
    let adv = s.draft_settings.ui.show_advanced;
    let content_w = (prc.right - 24).max(200);
    let stack_x = 28i32;
    let stack_inner_w = (content_w - 32).max(160);
    let label_h = 26i32;
    let misc_top = 6i32;
    let mut oy = misc_top + 28;

    // Essential: language, autostart, settings folder, backup.
    move_child_clean(s.lbl_lang, stack_x, oy, stack_inner_w, label_h);
    oy += label_h + LABEL_STACK_GAP;
    move_child_clean(s.combo_locale, stack_x, oy, 260.min(stack_inner_w), 200);
    oy += 28 + FIELD_BLOCK_GAP;

    set_hwnd_visible(s.chk_autostart, true);
    let auto_txt = window_text(s.chk_autostart);
    let autostart_h = if measure_text_width(hwnd, &auto_txt) + 28 > stack_inner_w {
        36
    } else {
        22
    };
    move_child_clean(s.chk_autostart, stack_x, oy, stack_inner_w, autostart_h);
    oy += autostart_h + FIELD_BLOCK_GAP;

    let open_folder_w = fitted_button_width(hwnd, &window_text(s.btn_open_config_folder));
    set_hwnd_visible(s.lbl_slots_file, adv);
    set_hwnd_visible(s.combo_slots_file, adv);
    if adv {
        move_child_clean(s.lbl_slots_file, stack_x, oy, stack_inner_w, label_h);
        oy += label_h + LABEL_STACK_GAP;
        let slots_combo_w = 300.min(stack_inner_w - open_folder_w - 8).max(160);
        move_child_clean(s.combo_slots_file, stack_x, oy, slots_combo_w, 28);
        move_child_clean(
            s.btn_open_config_folder,
            stack_x + stack_inner_w - open_folder_w,
            oy,
            open_folder_w,
            28,
        );
        oy += 28 + FIELD_BLOCK_GAP;
    } else {
        move_child_clean(s.btn_open_config_folder, stack_x, oy, open_folder_w, 28);
        oy += 28 + FIELD_BLOCK_GAP;
    }

    move_child_clean(s.lbl_backup_folder, stack_x, oy, stack_inner_w, label_h);
    oy += label_h + LABEL_STACK_GAP;
    let browse_w = fitted_button_width(hwnd, &window_text(s.btn_backup_browse));
    let backup_w = fitted_button_width(hwnd, &window_text(s.btn_backup_config));
    let restore_w = fitted_button_width(hwnd, &window_text(s.btn_backup_restore));
    let edit_w = (stack_inner_w - browse_w - backup_w - restore_w - 24).max(120);
    move_child_clean(s.edit_backup_folder, stack_x, oy, edit_w, 24);
    move_child_clean(s.btn_backup_browse, stack_x + edit_w + 8, oy, browse_w, 28);
    move_child_clean(
        s.btn_backup_config,
        stack_x + edit_w + 8 + browse_w + 8,
        oy,
        backup_w,
        28,
    );
    move_child_clean(
        s.btn_backup_restore,
        stack_x + edit_w + 8 + browse_w + 8 + backup_w + 8,
        oy,
        restore_w,
        28,
    );
    oy += 28;

    let adv_fields = [
        s.lbl_timeout,
        s.edit_timeout,
        s.lbl_max_digits,
        s.edit_max_digits,
        s.lbl_feedback,
        s.edit_feedback_ms,
        s.lbl_log,
        s.combo_log,
    ];
    for h in adv_fields {
        set_hwnd_visible(h, adv);
    }

    if adv {
        oy += FIELD_BLOCK_GAP;
        move_child_clean(s.lbl_timeout, stack_x, oy, stack_inner_w, label_h);
        oy += label_h + LABEL_STACK_GAP;
        move_child_clean(s.edit_timeout, stack_x, oy, 80, 24);
        oy += 24 + FIELD_BLOCK_GAP;
        move_child_clean(s.lbl_max_digits, stack_x, oy, stack_inner_w, label_h);
        oy += label_h + LABEL_STACK_GAP;
        move_child_clean(s.edit_max_digits, stack_x, oy, 80, 24);
        oy += 24 + FIELD_BLOCK_GAP;
        move_child_clean(s.lbl_feedback, stack_x, oy, stack_inner_w, label_h);
        oy += label_h + LABEL_STACK_GAP;
        move_child_clean(s.edit_feedback_ms, stack_x, oy, 80, 24);
        oy += 24 + FIELD_BLOCK_GAP;
        move_child_clean(s.lbl_log, stack_x, oy, stack_inner_w, label_h);
        oy += label_h + LABEL_STACK_GAP;
        move_child_clean(s.combo_log, stack_x, oy, 140.min(stack_inner_w), 200);
        oy += 28 + 18;
    } else {
        oy += 18;
    }

    move_child_clean(
        s.g_general,
        12,
        misc_top,
        content_w,
        (oy - misc_top).max(120),
    );
    let _ = SetWindowPos(
        s.chk_autostart,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    let _ = SetWindowPos(
        s.g_general,
        HWND_BOTTOM,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );

    // Status under the group — empty 40px is outside the box.
    oy += KEYS_GROUP_GAP;
    move_child_clean(
        s.lbl_backup_status,
        stack_x,
        oy,
        stack_inner_w,
        BACKUP_STATUS_H,
    );
    oy += BACKUP_STATUS_H;
    oy
}

unsafe fn create_btn(
    parent: HWND,
    id: isize,
    title: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    style: u32,
) -> windows::core::Result<HWND> {
    create_btn_ex(parent, id, title, x, y, w, h, style, true)
}

unsafe fn create_btn_ex(
    parent: HWND,
    id: isize,
    title: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    style: u32,
    visible: bool,
) -> windows::core::Result<HWND> {
    let instance = GetModuleHandleW(None)?;
    let title_w = to_wide(title);
    // Group boxes must not use WS_CLIPSIBLINGS — they paint an opaque body and would
    // erase sibling checkboxes (Instant fire looked empty).
    let clip = if style == BS_GROUPBOX {
        0
    } else {
        WS_CLIPSIBLINGS.0
    };
    let tab = if style == BS_GROUPBOX {
        0
    } else {
        WS_TABSTOP.0
    };
    let mut st = WS_CHILD.0 | tab | clip | style;
    if visible {
        st |= WS_VISIBLE.0;
    }
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        pcwstr(&title_w),
        WINDOW_STYLE(st),
        x,
        y,
        w,
        h,
        parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(id as _),
        instance,
        None,
    )
}

unsafe fn create_static(
    parent: HWND,
    title: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> windows::core::Result<HWND> {
    let instance = GetModuleHandleW(None)?;
    let title_w = to_wide(title);
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        pcwstr(&title_w),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0),
        x,
        y,
        w,
        h,
        parent,
        None,
        instance,
        None,
    )
}

unsafe fn create_edit(
    parent: HWND,
    id: isize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    extra: u32,
) -> windows::core::Result<HWND> {
    let instance = GetModuleHandleW(None)?;
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("EDIT"),
        w!(""),
        WINDOW_STYLE(
            WS_CHILD.0 | WS_VISIBLE.0 | WS_BORDER.0 | WS_TABSTOP.0 | ES_AUTOHSCROLL | extra,
        ),
        x,
        y,
        w,
        h,
        parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(id as _),
        instance,
        None,
    )
}

unsafe fn install_slot_drop_edits(s: &mut SettingsState) {
    use windows::Win32::UI::Shell::DragAcceptFiles;
    s.path_edit_prev = SetWindowLongPtrW(
        s.edit_path,
        GWLP_WNDPROC,
        slot_drop_edit_proc as *const () as usize as isize,
    );
    s.workdir_edit_prev = SetWindowLongPtrW(
        s.edit_workdir,
        GWLP_WNDPROC,
        slot_drop_edit_proc as *const () as usize as isize,
    );
    DragAcceptFiles(s.edit_path, true);
    DragAcceptFiles(s.edit_workdir, true);
    // Path / workdir: long UNC (default EDIT limit is 32k; set it explicitly).
    let _ = SendMessageW(s.edit_path, 0x00C5, WPARAM(32767), LPARAM(0));
    let _ = SendMessageW(s.edit_workdir, 0x00C5, WPARAM(32767), LPARAM(0));
}

unsafe extern "system" fn slot_drop_edit_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_DROPFILES {
        apply_slot_file_drop(hwnd, wparam);
        return LRESULT(0);
    }
    let orig = orig_slot_edit_proc(hwnd);
    CallWindowProcW(orig, hwnd, msg, wparam, lparam)
}

unsafe fn orig_slot_edit_proc(hwnd: HWND) -> windows::Win32::UI::WindowsAndMessaging::WNDPROC {
    let s = settings_state_from_child(hwnd);
    if s.is_null() {
        return None;
    }
    let prev = if hwnd == (*s).edit_path {
        (*s).path_edit_prev
    } else {
        (*s).workdir_edit_prev
    };
    if prev == 0 {
        None
    } else {
        Some(std::mem::transmute(prev))
    }
}

unsafe fn settings_state_from_child(hwnd: HWND) -> *mut SettingsState {
    let mut cur = hwnd;
    for _ in 0..5 {
        let Ok(parent) = GetParent(cur) else {
            break;
        };
        if parent.0.is_null() {
            break;
        }
        let ptr = GetWindowLongPtrW(parent, GWLP_USERDATA) as *mut SettingsState;
        if !ptr.is_null() && !(*ptr).edit_path.0.is_null() {
            return ptr;
        }
        cur = parent;
    }
    std::ptr::null_mut()
}

unsafe fn apply_slot_file_drop(edit: HWND, wparam: WPARAM) {
    use windows::Win32::UI::Shell::{DragFinish, DragQueryFileW, HDROP};
    let s = settings_state_from_child(edit);
    if s.is_null() {
        return;
    }
    let hdrop = HDROP(wparam.0 as *mut _);
    let field = if edit == (*s).edit_path {
        SlotDropField::Path
    } else if edit == (*s).edit_workdir {
        SlotDropField::Workdir
    } else {
        DragFinish(hdrop);
        return;
    };
    if !IsWindowEnabled(edit).as_bool() {
        DragFinish(hdrop);
        return;
    }
    let count = DragQueryFileW(hdrop, 0xFFFF_FFFF, None);
    if count == 0 {
        DragFinish(hdrop);
        return;
    }
    let needed = DragQueryFileW(hdrop, 0, None) as usize;
    if needed == 0 {
        DragFinish(hdrop);
        return;
    }
    let mut buf = vec![0u16; needed + 1];
    let n = DragQueryFileW(hdrop, 0, Some(&mut buf));
    DragFinish(hdrop);
    if n == 0 {
        return;
    }
    buf.truncate(n as usize);
    let raw = String::from_utf16_lossy(&buf);
    let path = std::path::PathBuf::from(&raw);
    let (text, is_dir) = match resolve_dropped_item(&path) {
        Some(v) => v,
        None => return,
    };
    let Some(value) = text_for_slot_drop(&text, is_dir, field) else {
        return;
    };
    set_text(edit, &value);
}

/// `(text, is_directory)` after resolving `.lnk` / `.url`. Date tokens stay literal.
/// Unresolved shortcuts leave the field unchanged (no fallback to the `.lnk` / `.url` path).
fn resolve_dropped_item(path: &std::path::Path) -> Option<(String, bool)> {
    let kind = dropped_item_kind(path);
    let resolved = match kind {
        DroppedItemKind::InternetShortcut => std::fs::read_to_string(path)
            .ok()
            .and_then(|body| url_from_internet_shortcut(&body))
            .map(|url| (url, false)),
        DroppedItemKind::ShortcutLnk => resolve_shortcut_target(path).map(|target| {
            if crate::core::launch::is_http_url(&target) {
                (target, false)
            } else {
                let p = std::path::PathBuf::from(&target);
                let is_dir = path_is_dir(&p);
                (target, is_dir)
            }
        }),
        DroppedItemKind::Other => None,
    };
    payload_for_dropped_item(kind, resolved, &path.to_string_lossy(), path_is_dir(path))
}

fn resolve_shortcut_target(path: &std::path::Path) -> Option<String> {
    use windows::core::Interface;
    use windows::Win32::System::Com::{
        CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let persist: IPersistFile = link.cast().ok()?;
        let wide = to_wide(&path.to_string_lossy());
        persist
            .Load(windows::core::PCWSTR(wide.as_ptr()), STGM_READ)
            .ok()?;
        let mut buf = [0u16; 1024];
        link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let s = String::from_utf16_lossy(&buf[..end]);
        let s = s.trim();
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    }
}

unsafe fn create_multiline_edit(
    parent: HWND,
    id: isize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> windows::core::Result<HWND> {
    let instance = GetModuleHandleW(None)?;
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("EDIT"),
        w!(""),
        WINDOW_STYLE(
            WS_CHILD.0
                | WS_VISIBLE.0
                | WS_BORDER.0
                | WS_TABSTOP.0
                | WS_VSCROLL.0
                | ES_MULTILINE
                | ES_AUTOVSCROLL
                | ES_WANTRETURN,
        ),
        x,
        y,
        w,
        h,
        parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(id as _),
        instance,
        None,
    )
}

/// Placeholder text shown when the edit is empty (does not become the value).
unsafe fn set_cue_banner(hwnd: HWND, text: &str) -> windows::core::Result<()> {
    const EM_SETCUEBANNER: u32 = WM_USER + 97; // 0x1501
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // wParam TRUE = show cue even when the control has focus
    SendMessageW(
        hwnd,
        EM_SETCUEBANNER,
        WPARAM(1),
        LPARAM(wide.as_ptr() as isize),
    );
    Ok(())
}

unsafe fn set_font(hwnd: HWND) {
    let font = HFONT(GetStockObject(DEFAULT_GUI_FONT).0);
    let _ = SendMessageW(hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
}

/// Pixel width of `text` in the dialog font (DEFAULT_GUI_FONT).
unsafe fn measure_text_width(hwnd: HWND, text: &str) -> i32 {
    if text.is_empty() {
        return 0;
    }
    let hdc = GetDC(hwnd);
    if hdc.0.is_null() {
        return (text.chars().count() as i32) * 8;
    }
    let font = GetStockObject(DEFAULT_GUI_FONT);
    let old = SelectObject(hdc, font);
    let wide: Vec<u16> = text.encode_utf16().collect();
    let mut size = SIZE::default();
    let ok = GetTextExtentPoint32W(hdc, &wide, &mut size);
    let _ = SelectObject(hdc, old);
    ReleaseDC(hwnd, hdc);
    if ok.as_bool() {
        size.cx
    } else {
        (text.chars().count() as i32) * 8
    }
}

unsafe fn fitted_button_width(hwnd: HWND, text: &str) -> i32 {
    (measure_text_width(hwnd, text) + FOOTER_BTN_PAD_X).max(FOOTER_BTN_W_MIN)
}

unsafe fn footer_row_min_client_w(hwnd: HWND, s: &SettingsState) -> i32 {
    let widths = [
        fitted_button_width(hwnd, &window_text(s.btn_add)),
        fitted_button_width(hwnd, &window_text(s.btn_delete)),
        fitted_button_width(hwnd, &window_text(s.btn_swap)),
        fitted_button_width(hwnd, &window_text(s.btn_slot_apply)),
        fitted_button_width(hwnd, &window_text(s.btn_save)),
        fitted_button_width(hwnd, &window_text(s.btn_apply)),
        fitted_button_width(hwnd, &window_text(s.btn_cancel)),
    ];
    let buttons: i32 = widths.iter().sum();
    12 + buttons + FOOTER_BTN_GAP * 6 + FOOTER_RIGHT_PAD
}

fn is_instant_digit_id(id: &str) -> bool {
    matches!(
        id,
        "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"
    )
}

unsafe fn create_children(parent: HWND, state_ptr: *mut SettingsState) -> anyhow::Result<()> {
    let s = &mut *state_ptr;
    let instance = GetModuleHandleW(None)?;

    const CONTENT_W: i32 = 860; // initial; layout_*_page stretches to panel width

    // Page panels — tab content is parented here. Footer buttons stay on `parent`.
    s.panel_slots = create_page_panel(parent)?;
    s.panel_keys = create_page_panel(parent)?;
    s.panel_triggers = create_page_panel(parent)?;
    s.panel_search = create_page_panel(parent)?;
    s.panel_mode = create_page_panel(parent)?;
    s.panel_general = create_page_panel(parent)?;
    let slots = s.panel_slots;
    let keys = s.panel_keys;
    let triggers = s.panel_triggers;
    let search = s.panel_search;
    let mode = s.panel_mode;
    // General controls were historically parented under `options`.
    let options = s.panel_general;
    // Dialog-absolute Y → panel-local (panel starts at TAB_BAND_H).
    let py = |y: i32| y - TAB_BAND_H;
    const LABEL_H: i32 = 26; // room for descenders (g / y) under DEFAULT_GUI_FONT

    // Resolve UI strings once — widths are measured against the active language.
    let txt_add = t(keys::SETTINGS_ADD);
    let txt_delete = t(keys::SETTINGS_DELETE);
    let txt_swap = t(keys::SETTINGS_SWAP_IDS);
    let txt_id = t(keys::SETTINGS_ID);
    let txt_name = t(keys::SETTINGS_NAME);
    let txt_path = t(keys::SETTINGS_PATH);
    let txt_working = t(keys::SETTINGS_WORKING);
    let txt_open_workdir = t(keys::SETTINGS_OPEN_WORKDIR);
    let txt_focus_existing = t(keys::SETTINGS_FOCUS_EXISTING);
    let txt_desc = t(keys::SETTINGS_DESCRIPTION);
    let txt_update_slot = t(keys::SETTINGS_UPDATE_SLOT);
    let txt_start = t(keys::SETTINGS_START);
    let txt_start_label = t(keys::SETTINGS_START_LABEL);
    let txt_confirm = t(keys::SETTINGS_CONFIRM);
    let txt_cancel_role = t(keys::SETTINGS_CANCEL);
    let txt_search = t(keys::SETTINGS_SEARCH);
    let txt_open_workdir_key = t(keys::SETTINGS_OPEN_WORKDIR_KEY);
    let txt_digit_back = t(keys::SETTINGS_DIGIT_BACK);
    let txt_wake = t(keys::SETTINGS_WAKE_KEY);
    let txt_wake_vk = t(keys::SETTINGS_WAKE_VK);
    let txt_key_set = t(keys::SETTINGS_KEY_SET);
    let txt_group_mode_switch = t(keys::SETTINGS_GROUP_MODE_SWITCH);
    let txt_key_set_numpad = t(keys::KEY_SET_NUMPAD);
    let txt_key_set_keyboard = t(keys::KEY_SET_KEYBOARD);
    let txt_capture = t(keys::SETTINGS_CAPTURE);
    let txt_mouse = t(keys::SETTINGS_MOUSE_BUTTON);
    let txt_suppress = t(keys::SETTINGS_SUPPRESS);
    let txt_hotkey = t(keys::SETTINGS_HOTKEY);
    let txt_hotkey_hint = t(keys::SETTINGS_HOTKEY_HINT);
    let txt_reset = t(keys::SETTINGS_RESET_DEFAULTS);
    let txt_reset_triggers = t(keys::SETTINGS_RESET_TRIGGERS);
    let txt_match = t(keys::SETTINGS_MATCH_MODE);
    let txt_case = t(keys::SETTINGS_CASE_SENSITIVE);
    let txt_timeout = t(keys::SETTINGS_TIMEOUT);
    let txt_max_digits = t(keys::SETTINGS_MAX_DIGITS);
    let txt_feedback_ms = t(keys::SETTINGS_FEEDBACK_MS);
    let txt_log = t(keys::SETTINGS_LOG_LEVEL);
    let txt_lang = t(keys::SETTINGS_UI_LANGUAGE);
    let txt_slots_file = t(keys::SETTINGS_SLOTS_FILE);
    let txt_open_config_folder = t(keys::SETTINGS_OPEN_CONFIG_FOLDER);
    let txt_backup_folder = t(keys::SETTINGS_BACKUP_FOLDER);
    let txt_backup_browse = t(keys::SETTINGS_BACKUP_BROWSE);
    let txt_backup_config = t(keys::SETTINGS_BACKUP_CONFIG);
    let txt_backup_restore = t(keys::SETTINGS_RESTORE_CONFIG);
    let txt_autostart = t(keys::SETTINGS_AUTOSTART);
    let txt_instant_fire = t(keys::SETTINGS_INSTANT_FIRE);
    let txt_tab_slots = t(keys::SETTINGS_TAB_SLOTS);
    let txt_tab_keys = t(keys::SETTINGS_TAB_KEYS);
    let txt_tab_triggers = t(keys::SETTINGS_TAB_TRIGGERS);
    let txt_tab_search = t(keys::SETTINGS_TAB_SEARCH);
    let txt_tab_mode = t(keys::SETTINGS_TAB_MODE);
    let txt_tab_general = t(keys::SETTINGS_TAB_GENERAL);
    let txt_advanced = t(keys::SETTINGS_ADVANCED);
    let txt_group_mode = t(keys::SETTINGS_GROUP_MODE);
    let txt_mode_mouse = t(keys::SETTINGS_MODE_MOUSE);
    let txt_mode_suppress = t(keys::SETTINGS_MODE_SUPPRESS);
    let txt_mode_timeout = t(keys::SETTINGS_MODE_TIMEOUT);
    let txt_mode_chords = t(keys::SETTINGS_MODE_CHORDS);
    let txt_mode_chord_file = t(keys::SETTINGS_MODE_CHORD_FILE);
    let txt_mode_set_override = t(keys::SETTINGS_MODE_SET_OVERRIDE);
    let txt_mode_clear_override = t(keys::SETTINGS_MODE_CLEAR_OVERRIDE);
    let txt_mode_hint = t(keys::SETTINGS_MODE_HINT);
    let txt_mode_current = t(keys::SETTINGS_MODE_CURRENT);
    let txt_save = t(keys::SETTINGS_SAVE);
    let txt_apply = t(keys::SETTINGS_APPLY);
    let txt_btn_cancel = t(keys::SETTINGS_BTN_CANCEL);

    // Slots: list left, form right — labels always above fields (no side clipping).
    let margin = 12i32;
    let slots_form_x = margin + SLOTS_LIST_W + SLOTS_LIST_FORM_GAP;
    s.slots_form_x = slots_form_x;
    let form_w = 520i32; // stretched later by layout_settings
    let id_w = SLOTS_ID_COL_W;
    let name_x = slots_form_x + id_w + 12;
    let name_w = (form_w - id_w - 12).max(160);

    // Keys / Options: label above each control; Capture / edits use the remaining width.
    let stack_x = 28i32;
    let stack_inner_w = CONTENT_W - 32;
    let keys_cap_w = fitted_button_width(parent, &txt_capture)
        .max(fitted_button_width(parent, &t(keys::SETTINGS_CANCEL)));

    let mut font_targets: Vec<HWND> = Vec::new();

    // ── Slots page ──
    let list_style = WINDOW_STYLE(
        WS_CHILD.0 | WS_VISIBLE.0 | WS_BORDER.0 | WS_VSCROLL.0 | WS_TABSTOP.0 | LBS_NOTIFY,
    );
    let list_top = py(48);
    let list_h = 280i32; // initial; layout_slots_page stretches to the panel
    s.lbl_slots_current =
        create_static(slots, "", margin, list_top, SLOTS_LIST_W + form_w, LABEL_H)?;
    s.slot_list = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("LISTBOX"),
        w!(""),
        list_style,
        margin,
        list_top,
        SLOTS_LIST_W,
        list_h,
        slots,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_SLOT_LIST as _),
        instance,
        None,
    )?;
    let add_w = fitted_button_width(parent, &txt_add);
    let del_w = fitted_button_width(parent, &txt_delete);
    // Slots actions live on the main dialog footer (same baseline as Save/Apply/Cancel).
    s.btn_add = create_btn(
        parent,
        IDC_SLOT_ADD,
        &txt_add,
        margin,
        0,
        add_w,
        FOOTER_BTN_H,
        BS_PUSHBUTTON,
    )?;
    s.btn_delete = create_btn(
        parent,
        IDC_SLOT_DELETE,
        &txt_delete,
        margin + add_w + 8,
        0,
        del_w,
        FOOTER_BTN_H,
        BS_PUSHBUTTON,
    )?;
    let swap_w = fitted_button_width(parent, &txt_swap);
    s.btn_swap = create_btn(
        parent,
        IDC_SLOT_SWAP,
        &txt_swap,
        margin + add_w + 8 + del_w + 8,
        0,
        swap_w,
        FOOTER_BTN_H,
        BS_PUSHBUTTON,
    )?;
    let update_w = fitted_button_width(parent, &txt_update_slot);
    s.btn_slot_apply = create_btn(
        parent,
        IDC_SLOT_APPLY,
        &txt_update_slot,
        margin + add_w + 8 + del_w + 8 + swap_w + 8,
        0,
        update_w,
        FOOTER_BTN_H,
        BS_PUSHBUTTON,
    )?;

    // Form: ID|Name on one row (labels above), then full-width stacked fields.
    let mut fy = list_top;
    let edit_h = 24i32;
    s.lbl_slot_id = create_static(slots, &txt_id, slots_form_x, fy, id_w, LABEL_H)?;
    s.lbl_slot_name = create_static(slots, &txt_name, name_x, fy, name_w, LABEL_H)?;
    fy += LABEL_H + LABEL_STACK_GAP;
    s.edit_id = create_edit(
        slots,
        IDC_SLOT_ID,
        slots_form_x,
        fy,
        id_w.min(100),
        edit_h,
        ES_NUMBER,
    )?;
    s.edit_name = create_edit(slots, IDC_SLOT_NAME, name_x, fy, name_w, edit_h, 0)?;
    fy += edit_h + FIELD_BLOCK_GAP;

    let place_full = |fy: &mut i32,
                      label_hwnd: &mut HWND,
                      label: &str,
                      edit: &mut HWND,
                      id: isize,
                      h: i32|
     -> windows::core::Result<()> {
        *label_hwnd = create_static(slots, label, slots_form_x, *fy, form_w, LABEL_H)?;
        *fy += LABEL_H + LABEL_STACK_GAP;
        *edit = if h > edit_h {
            create_multiline_edit(slots, id, slots_form_x, *fy, form_w, h)?
        } else {
            create_edit(slots, id, slots_form_x, *fy, form_w, h, 0)?
        };
        *fy += h + FIELD_BLOCK_GAP;
        Ok(())
    };
    place_full(
        &mut fy,
        &mut s.lbl_slot_path,
        &txt_path,
        &mut s.edit_path,
        IDC_SLOT_PATH,
        edit_h,
    )?;
    place_full(
        &mut fy,
        &mut s.lbl_slot_workdir,
        &txt_working,
        &mut s.edit_workdir,
        IDC_SLOT_WORKDIR,
        edit_h,
    )?;
    s.chk_open_workdir = create_btn(
        slots,
        IDC_SLOT_OPEN_WORKDIR,
        &txt_open_workdir,
        slots_form_x,
        fy,
        form_w,
        SLOTS_OPEN_WORKDIR_H,
        BS_AUTOCHECKBOX | BS_MULTILINE,
    )?;
    fy += SLOTS_OPEN_WORKDIR_H + FIELD_BLOCK_GAP;
    s.chk_focus_existing = create_btn(
        slots,
        IDC_SLOT_FOCUS_EXISTING,
        &txt_focus_existing,
        slots_form_x,
        fy,
        form_w,
        SLOTS_OPEN_WORKDIR_H,
        BS_AUTOCHECKBOX | BS_MULTILINE,
    )?;
    // Default on (matches omitted focusExisting in JSON).
    set_check(s.chk_focus_existing, true);
    fy += SLOTS_OPEN_WORKDIR_H + FIELD_BLOCK_GAP;
    s.chk_instant_fire = create_btn(
        slots,
        IDC_SLOT_INSTANT,
        &txt_instant_fire,
        slots_form_x,
        fy,
        form_w,
        SLOTS_OPEN_WORKDIR_H,
        BS_AUTOCHECKBOX | BS_MULTILINE,
    )?;
    fy += SLOTS_OPEN_WORKDIR_H + FIELD_BLOCK_GAP;
    place_full(
        &mut fy,
        &mut s.lbl_slot_desc,
        &txt_desc,
        &mut s.edit_desc,
        IDC_SLOT_DESC,
        SLOTS_DESC_H_DEFAULT,
    )?;
    s.warn_label = create_static(slots, "", slots_form_x, fy, form_w, SLOTS_WARN_H)?;

    set_cue_banner(s.edit_name, &t(keys::SETTINGS_CUE_NAME))?;
    set_cue_banner(s.edit_path, &t(keys::SETTINGS_CUE_PATH))?;
    set_cue_banner(s.edit_workdir, &t(keys::SETTINGS_CUE_WORKDIR))?;
    install_slot_drop_edits(s);

    font_targets.extend_from_slice(&[
        s.slot_list,
        s.lbl_slots_current,
        s.btn_add,
        s.btn_delete,
        s.btn_swap,
        s.lbl_slot_id,
        s.edit_id,
        s.lbl_slot_name,
        s.edit_name,
        s.lbl_slot_path,
        s.edit_path,
        s.lbl_slot_workdir,
        s.edit_workdir,
        s.chk_open_workdir,
        s.chk_focus_existing,
        s.chk_instant_fire,
        s.lbl_slot_desc,
        s.edit_desc,
        s.btn_slot_apply,
        s.warn_label,
    ]);

    // ── Keys page (caption | Trigger | Display | Capture on one line) ──
    let keys_top = py(48);
    // First control must clear the group-box caption strip (was clipping "Start (+)").
    let mut ky = keys_top + 28;
    let row_ctl_h = 24i32;
    s.lbl_key_set = create_static(keys, &txt_key_set, stack_x, ky, 90, LABEL_H)?;
    s.combo_key_set = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("COMBOBOX"),
        w!(""),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | CBS_DROPDOWNLIST as u32),
        stack_x + 98,
        ky,
        140.min(stack_inner_w / 3),
        200,
        keys,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_KEY_SET as _),
        instance,
        None,
    )?;
    ky += LABEL_H.max(row_ctl_h) + FIELD_BLOCK_GAP;
    let disp_w = 56i32;
    let cap_w = action_key_caption_col_w(
        parent,
        stack_inner_w,
        &[
            &txt_start,
            &txt_confirm,
            &txt_cancel_role,
            &txt_search,
            &txt_open_workdir_key,
            &txt_digit_back,
            &txt_wake_vk,
        ],
    );
    let place_key_row = |ky: &mut i32,
                         label_out: &mut HWND,
                         value_out: &mut HWND,
                         edit_out: &mut HWND,
                         btn_out: &mut HWND,
                         label: &str,
                         edit_id: isize,
                         btn_id: isize|
     -> windows::core::Result<()> {
        *label_out = create_static(keys, label, stack_x, *ky, cap_w, LABEL_H)?;
        let rest_x = stack_x + cap_w + KEYS_COL_GAP;
        let rest_w = (stack_inner_w - cap_w - KEYS_COL_GAP).max(160);
        let value_w = (rest_w - keys_cap_w - disp_w - KEYS_COL_GAP * 2).max(80);
        *value_out = create_static(keys, "", rest_x, *ky, value_w, LABEL_H)?;
        *edit_out = create_edit(
            keys,
            edit_id,
            rest_x + value_w + KEYS_COL_GAP,
            *ky,
            disp_w,
            row_ctl_h,
            0,
        )?;
        // Limit display name to 3 characters (EM_LIMITTEXT).
        let _ = SendMessageW(*edit_out, 0x00C5, WPARAM(3), LPARAM(0));
        set_cue_banner(*edit_out, &txt_start_label)?;
        *btn_out = create_btn(
            keys,
            btn_id,
            &txt_capture,
            rest_x + value_w + KEYS_COL_GAP + disp_w + KEYS_COL_GAP,
            *ky,
            keys_cap_w,
            row_ctl_h,
            BS_PUSHBUTTON,
        )?;
        *ky += LABEL_H.max(row_ctl_h) + FIELD_BLOCK_GAP;
        Ok(())
    };
    let mut lbl_start_cap = HWND::default();
    let mut lbl_confirm_cap = HWND::default();
    let mut lbl_cancel_cap = HWND::default();
    let mut lbl_search_cap = HWND::default();
    let mut lbl_open_workdir_cap = HWND::default();
    let mut lbl_digit_back_cap = HWND::default();
    place_key_row(
        &mut ky,
        &mut lbl_start_cap,
        &mut s.lbl_start,
        &mut s.edit_start_label,
        &mut s.btn_cap_start,
        &txt_start,
        IDC_START_LABEL,
        IDC_CAP_START,
    )?;
    place_key_row(
        &mut ky,
        &mut lbl_confirm_cap,
        &mut s.lbl_confirm,
        &mut s.edit_confirm_label,
        &mut s.btn_cap_confirm,
        &txt_confirm,
        IDC_CONFIRM_LABEL,
        IDC_CAP_CONFIRM,
    )?;
    place_key_row(
        &mut ky,
        &mut lbl_cancel_cap,
        &mut s.lbl_cancel,
        &mut s.edit_cancel_label,
        &mut s.btn_cap_cancel,
        &txt_cancel_role,
        IDC_CANCEL_LABEL,
        IDC_CAP_CANCEL,
    )?;
    place_key_row(
        &mut ky,
        &mut lbl_search_cap,
        &mut s.lbl_search,
        &mut s.edit_search_label,
        &mut s.btn_cap_search,
        &txt_search,
        IDC_SEARCH_LABEL,
        IDC_CAP_SEARCH,
    )?;
    place_key_row(
        &mut ky,
        &mut lbl_open_workdir_cap,
        &mut s.lbl_open_workdir_key,
        &mut s.edit_open_workdir_label,
        &mut s.btn_cap_open_workdir,
        &txt_open_workdir_key,
        IDC_OPEN_WORKDIR_LABEL,
        IDC_CAP_OPEN_WORKDIR,
    )?;
    place_key_row(
        &mut ky,
        &mut lbl_digit_back_cap,
        &mut s.lbl_digit_back,
        &mut s.edit_digit_back_label,
        &mut s.btn_cap_digit_back,
        &txt_digit_back,
        IDC_DIGIT_BACK_LABEL,
        IDC_CAP_DIGIT_BACK,
    )?;
    let wake_h = if measure_text_width(parent, &txt_wake) + 28 > stack_inner_w {
        36
    } else {
        22
    };
    s.chk_wake = create_btn(
        keys,
        IDC_WAKE,
        &txt_wake,
        stack_x,
        ky,
        stack_inner_w,
        wake_h,
        BS_AUTOCHECKBOX | BS_MULTILINE,
    )?;
    ky += wake_h.max(24) + FIELD_BLOCK_GAP;
    s.lbl_wake_cap = create_static(keys, &txt_wake_vk, stack_x, ky, cap_w, LABEL_H)?;
    {
        let rest_x = stack_x + cap_w + KEYS_COL_GAP;
        let rest_w = (stack_inner_w - cap_w - KEYS_COL_GAP).max(160);
        let value_w = (rest_w - keys_cap_w - disp_w - KEYS_COL_GAP * 2).max(80);
        s.lbl_wake = create_static(keys, "", rest_x, ky, value_w, LABEL_H)?;
        s.btn_cap_wake = create_btn(
            keys,
            IDC_CAP_WAKE,
            &txt_capture,
            rest_x + value_w + KEYS_COL_GAP + disp_w + KEYS_COL_GAP,
            ky,
            keys_cap_w,
            row_ctl_h,
            BS_PUSHBUTTON,
        )?;
    }
    ky += LABEL_H.max(row_ctl_h);
    s.lbl_start_cap = lbl_start_cap;
    s.lbl_confirm_cap = lbl_confirm_cap;
    s.lbl_cancel_cap = lbl_cancel_cap;
    s.lbl_search_cap = lbl_search_cap;
    s.lbl_open_workdir_cap = lbl_open_workdir_cap;
    s.lbl_digit_back_cap = lbl_digit_back_cap;
    ky += FIELD_BLOCK_GAP;
    let reset_keys_w = fitted_button_width(parent, &txt_reset);
    s.btn_reset_keys = create_btn(
        keys,
        IDC_RESET_KEYS,
        &txt_reset,
        stack_x,
        ky,
        reset_keys_w,
        26,
        BS_PUSHBUTTON,
    )?;
    ky += 26 + FIELD_BLOCK_GAP;
    let g_keys_h = (ky - keys_top).max(80);
    s.g_keys = create_btn(
        keys,
        0,
        &t(keys::SETTINGS_GROUP_ACTION_KEYS),
        12,
        keys_top,
        CONTENT_W,
        g_keys_h,
        BS_GROUPBOX,
    )?;

    let _ = SetWindowPos(
        s.chk_wake,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    let _ = SetWindowPos(
        s.btn_cap_wake,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    let _ = SetWindowPos(
        s.g_keys,
        HWND_BOTTOM,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    ky = keys_top + g_keys_h + KEYS_GROUP_GAP;
    let legend_top = ky;
    ky += 28;
    let col_w = ((stack_inner_w - 16) / 2).max(160);
    let right_x = stack_x + col_w + 16;
    let txt_legend_idle = t(keys::SETTINGS_LEGEND_IDLE);
    let txt_legend_multi = t(keys::SETTINGS_LEGEND_MULTI);
    let txt_legend_up = t(keys::SETTINGS_LEGEND_UP);
    let txt_legend_down = t(keys::SETTINGS_LEGEND_DOWN);
    s.lbl_legend_idle = create_static(keys, &txt_legend_idle, stack_x, ky, col_w, LABEL_H)?;
    s.lbl_legend_multi = create_static(keys, &txt_legend_multi, right_x, ky, col_w, LABEL_H)?;
    ky += LABEL_H + LABEL_STACK_GAP;
    let chk_h = 22i32;
    for i in 0..LEGEND_ITEM_COUNT {
        s.chk_legend_idle[i] = create_btn(
            keys,
            IDC_LEGEND_IDLE_0 + i as isize,
            "",
            stack_x,
            ky,
            col_w,
            chk_h,
            BS_AUTOCHECKBOX,
        )?;
        s.chk_legend_multi[i] = create_btn(
            keys,
            IDC_LEGEND_MULTI_0 + i as isize,
            "",
            right_x,
            ky,
            col_w,
            chk_h,
            BS_AUTOCHECKBOX,
        )?;
        ky += chk_h + FIELD_BLOCK_GAP;
    }
    ky += FIELD_BLOCK_GAP;
    let up_w = fitted_button_width(parent, &txt_legend_up);
    let down_w = fitted_button_width(parent, &txt_legend_down);
    s.btn_legend_idle_up = create_btn(
        keys,
        IDC_LEGEND_IDLE_UP,
        &txt_legend_up,
        stack_x,
        ky,
        up_w,
        24,
        BS_PUSHBUTTON,
    )?;
    s.btn_legend_idle_down = create_btn(
        keys,
        IDC_LEGEND_IDLE_DOWN,
        &txt_legend_down,
        stack_x + up_w + 8,
        ky,
        down_w,
        24,
        BS_PUSHBUTTON,
    )?;
    s.btn_legend_multi_up = create_btn(
        keys,
        IDC_LEGEND_MULTI_UP,
        &txt_legend_up,
        right_x,
        ky,
        up_w,
        24,
        BS_PUSHBUTTON,
    )?;
    s.btn_legend_multi_down = create_btn(
        keys,
        IDC_LEGEND_MULTI_DOWN,
        &txt_legend_down,
        right_x + up_w + 8,
        ky,
        down_w,
        24,
        BS_PUSHBUTTON,
    )?;
    ky += 24 + FIELD_BLOCK_GAP;
    s.g_legend = create_btn(
        keys,
        0,
        &t(keys::SETTINGS_GROUP_LEGEND),
        12,
        legend_top,
        CONTENT_W,
        (ky - legend_top).max(80),
        BS_GROUPBOX,
    )?;
    let _ = SetWindowPos(
        s.g_legend,
        HWND_BOTTOM,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    ky += KEYS_GROUP_GAP;
    s.capture_status = create_static(keys, "", stack_x, ky, stack_inner_w, CAPTURE_STATUS_H)?;

    font_targets.extend_from_slice(&[
        s.g_keys,
        s.lbl_start_cap,
        s.lbl_start,
        s.edit_start_label,
        s.btn_cap_start,
        s.lbl_confirm_cap,
        s.lbl_confirm,
        s.edit_confirm_label,
        s.btn_cap_confirm,
        s.lbl_cancel_cap,
        s.lbl_cancel,
        s.edit_cancel_label,
        s.btn_cap_cancel,
        s.lbl_search_cap,
        s.lbl_search,
        s.edit_search_label,
        s.btn_cap_search,
        s.lbl_open_workdir_cap,
        s.lbl_open_workdir_key,
        s.edit_open_workdir_label,
        s.btn_cap_open_workdir,
        s.lbl_digit_back_cap,
        s.lbl_digit_back,
        s.edit_digit_back_label,
        s.btn_cap_digit_back,
        s.lbl_key_set,
        s.combo_key_set,
        s.chk_wake,
        s.lbl_wake_cap,
        s.lbl_wake,
        s.btn_cap_wake,
        s.btn_reset_keys,
        s.capture_status,
        s.g_legend,
        s.lbl_legend_idle,
        s.lbl_legend_multi,
        s.btn_legend_idle_up,
        s.btn_legend_idle_down,
        s.btn_legend_multi_up,
        s.btn_legend_multi_down,
    ]);
    font_targets.extend_from_slice(&s.chk_legend_idle);
    font_targets.extend_from_slice(&s.chk_legend_multi);

    // ── Triggers page ──
    let switch_top = py(48);
    let mut sy = switch_top + 28;
    s.lbl_mode_mouse = create_static(
        triggers,
        &txt_mode_mouse,
        stack_x,
        sy,
        stack_inner_w,
        LABEL_H,
    )?;
    sy += LABEL_H + LABEL_STACK_GAP;
    let mode_mouse_combo_w = 120i32;
    s.combo_mode_mouse = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("COMBOBOX"),
        w!(""),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | CBS_DROPDOWNLIST as u32),
        stack_x,
        sy,
        mode_mouse_combo_w,
        200,
        triggers,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_MODE_MOUSE as _),
        instance,
        None,
    )?;
    for label in ["x1", "x2", "middle"] {
        let w = to_wide(label);
        let _ = SendMessageW(
            s.combo_mode_mouse,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w.as_ptr() as isize),
        );
    }
    sy += 28 + FIELD_BLOCK_GAP;
    let mode_suppress_h = if measure_text_width(parent, &txt_mode_suppress) + 28 > stack_inner_w {
        36
    } else {
        22
    };
    s.chk_mode_suppress = create_btn(
        triggers,
        IDC_MODE_SUPPRESS,
        &txt_mode_suppress,
        stack_x,
        sy,
        stack_inner_w,
        mode_suppress_h,
        BS_AUTOCHECKBOX | BS_MULTILINE,
    )?;
    sy += mode_suppress_h + FIELD_BLOCK_GAP;
    s.lbl_mode_timeout = create_static(
        triggers,
        &txt_mode_timeout,
        stack_x,
        sy,
        stack_inner_w,
        LABEL_H,
    )?;
    sy += LABEL_H + LABEL_STACK_GAP;
    s.edit_mode_timeout = create_edit(triggers, IDC_MODE_TIMEOUT, stack_x, sy, 80, 24, ES_NUMBER)?;
    sy += 24 + FIELD_BLOCK_GAP;

    s.lbl_key_chord = create_static(triggers, &txt_key_set_numpad, stack_x, sy, 90, LABEL_H)?;
    s.combo_key_chord = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("COMBOBOX"),
        w!(""),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | CBS_DROPDOWNLIST as u32),
        stack_x + 98,
        sy,
        56,
        200,
        triggers,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_KEY_CHORD as _),
        instance,
        None,
    )?;
    s.lbl_key_chord_kb = create_static(
        triggers,
        &txt_key_set_keyboard,
        stack_x + 170,
        sy,
        90,
        LABEL_H,
    )?;
    s.combo_key_chord_kb = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("COMBOBOX"),
        w!(""),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | CBS_DROPDOWNLIST as u32),
        stack_x + 268,
        sy,
        56,
        200,
        triggers,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_MODE_LETTER_KEYBOARD as _),
        instance,
        None,
    )?;
    sy += 28 + 14;
    s.g_mode_switch = create_btn(
        triggers,
        0,
        &txt_group_mode_switch,
        12,
        switch_top,
        CONTENT_W,
        (sy - switch_top).max(80),
        BS_GROUPBOX,
    )?;
    let _ = SetWindowPos(
        s.chk_mode_suppress,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    let _ = SetWindowPos(
        s.g_mode_switch,
        HWND_BOTTOM,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );

    let trig_top = py(48);
    let mut ky = trig_top + 28; // clear Launch-triggers caption
    s.lbl_mouse = create_static(triggers, &txt_mouse, stack_x, ky, stack_inner_w, LABEL_H)?;
    ky += LABEL_H + LABEL_STACK_GAP;
    let mouse_combo_w = 120i32;
    s.combo_mouse = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("COMBOBOX"),
        w!(""),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | CBS_DROPDOWNLIST as u32),
        stack_x,
        ky,
        mouse_combo_w,
        200,
        triggers,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_MOUSE_BTN as _),
        instance,
        None,
    )?;
    for label in ["x2", "x1", "middle"] {
        let w = to_wide(label);
        let _ = SendMessageW(
            s.combo_mouse,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w.as_ptr() as isize),
        );
    }
    s.btn_cap_mouse = create_btn(
        triggers,
        IDC_CAP_MOUSE,
        &txt_capture,
        stack_x + mouse_combo_w + 8,
        ky,
        keys_cap_w,
        row_ctl_h,
        BS_PUSHBUTTON,
    )?;
    ky += row_ctl_h + FIELD_BLOCK_GAP;
    let suppress_h = if measure_text_width(parent, &txt_suppress) + 28 > stack_inner_w {
        36
    } else {
        22
    };
    s.chk_suppress = create_btn(
        triggers,
        IDC_MOUSE_SUPPRESS,
        &txt_suppress,
        stack_x,
        ky,
        stack_inner_w,
        suppress_h,
        BS_AUTOCHECKBOX | BS_MULTILINE,
    )?;
    ky += suppress_h + FIELD_BLOCK_GAP;
    s.lbl_hotkey = create_static(triggers, &txt_hotkey, stack_x, ky, stack_inner_w, LABEL_H)?;
    ky += LABEL_H + LABEL_STACK_GAP;
    s.edit_hotkey = create_edit(
        triggers,
        IDC_HOTKEY,
        stack_x,
        ky,
        220.min(stack_inner_w),
        24,
        0,
    )?;
    ky += 24 + LABEL_STACK_GAP;
    s.lbl_hotkey_hint = create_static(
        triggers,
        &txt_hotkey_hint,
        stack_x,
        ky,
        stack_inner_w,
        LABEL_H,
    )?;
    ky += LABEL_H + FIELD_BLOCK_GAP;
    let reset_w = fitted_button_width(parent, &txt_reset_triggers);
    s.btn_reset_triggers = create_btn(
        triggers,
        IDC_RESET_TRIGGERS,
        &txt_reset_triggers,
        stack_x,
        ky,
        reset_w,
        26,
        BS_PUSHBUTTON,
    )?;
    ky += 26 + LABEL_STACK_GAP;
    s.lbl_triggers_status = create_static(triggers, "", stack_x, ky, stack_inner_w, LABEL_H)?;
    ky += LABEL_H + FIELD_BLOCK_GAP;
    let g_trig_h = (ky - trig_top).max(100);
    s.g_triggers = create_btn(
        triggers,
        0,
        &t(keys::SETTINGS_GROUP_TRIGGERS),
        12,
        trig_top,
        CONTENT_W,
        g_trig_h,
        BS_GROUPBOX,
    )?;
    let _ = SetWindowPos(
        s.g_triggers,
        HWND_BOTTOM,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    font_targets.extend_from_slice(&[
        s.g_mode_switch,
        s.lbl_mode_mouse,
        s.combo_mode_mouse,
        s.chk_mode_suppress,
        s.lbl_mode_timeout,
        s.edit_mode_timeout,
        s.lbl_key_chord,
        s.combo_key_chord,
        s.lbl_key_chord_kb,
        s.combo_key_chord_kb,
        s.g_triggers,
        s.lbl_mouse,
        s.combo_mouse,
        s.btn_cap_mouse,
        s.chk_suppress,
        s.lbl_hotkey,
        s.edit_hotkey,
        s.lbl_hotkey_hint,
        s.btn_reset_triggers,
        s.lbl_triggers_status,
    ]);

    // ── Search page ──
    let search_top = py(48);
    let mut oy = search_top + 28;
    s.lbl_match_mode = create_static(search, &txt_match, stack_x, oy, stack_inner_w, LABEL_H)?;
    oy += LABEL_H + LABEL_STACK_GAP;
    s.combo_search = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("COMBOBOX"),
        w!(""),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | CBS_DROPDOWNLIST as u32),
        stack_x,
        oy,
        160.min(stack_inner_w),
        200,
        search,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_SEARCH_MODE as _),
        instance,
        None,
    )?;
    for label in ["substring", "prefix", "exact", "fuzzy"] {
        let w = to_wide(label);
        let _ = SendMessageW(
            s.combo_search,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w.as_ptr() as isize),
        );
    }
    oy += 28 + FIELD_BLOCK_GAP;
    let case_h = if measure_text_width(parent, &txt_case) + 28 > stack_inner_w {
        36
    } else {
        22
    };
    s.chk_case = create_btn(
        search,
        IDC_CASE_SENSITIVE,
        &txt_case,
        stack_x,
        oy,
        stack_inner_w,
        case_h,
        BS_AUTOCHECKBOX | BS_MULTILINE,
    )?;
    oy += case_h + 16;
    s.g_search = create_btn(
        search,
        0,
        &t(keys::SETTINGS_GROUP_SEARCH),
        12,
        search_top,
        CONTENT_W,
        (oy - search_top).max(80),
        BS_GROUPBOX,
    )?;
    let _ = SetWindowPos(
        s.chk_case,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );

    // ── Mode page (§4 / §6 / §7 — book combo + chord list) ──
    let mode_top = oy + 4;
    oy = mode_top + 28;
    s.lbl_mode_current = create_static(
        mode,
        &txt_mode_current,
        stack_x,
        oy,
        120.min(stack_inner_w),
        LABEL_H,
    )?;
    s.combo_mode_current = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("COMBOBOX"),
        w!(""),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | CBS_DROPDOWNLIST as u32),
        stack_x + 128,
        oy,
        (stack_inner_w - 128).max(160),
        200,
        mode,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_MODE_CURRENT as _),
        instance,
        None,
    )?;
    oy += 28 + FIELD_BLOCK_GAP;
    let mode_hint_h = 36i32;
    s.lbl_mode_hint = create_static(
        mode,
        &txt_mode_hint,
        stack_x,
        oy,
        stack_inner_w,
        mode_hint_h,
    )?;
    oy += mode_hint_h + FIELD_BLOCK_GAP;

    s.mode_chord_code_choices.clear();
    for i in 0..10usize {
        if let Some(code) = chord_code_at(i) {
            s.mode_chord_code_choices.push(code);
        }
    }

    let mode_set_w = fitted_button_width(parent, &txt_mode_set_override);
    let mode_clear_w = fitted_button_width(parent, &txt_mode_clear_override);
    let gap = 8i32;
    let file_label_w = measure_text_width(parent, &txt_mode_chord_file).max(64) + 8;
    let combo_w = 180i32.min(stack_inner_w / 3).max(120);
    let right_cluster = file_label_w + gap + combo_w + gap + mode_set_w + gap + mode_clear_w;
    let left_w = (stack_inner_w - right_cluster - gap).max(160);
    let right_x = stack_x + left_w + gap;
    let combo_x = right_x + file_label_w + gap;

    s.lbl_mode_chords = create_static(mode, &txt_mode_chords, stack_x, oy, left_w, LABEL_H)?;
    s.lbl_mode_chord_file = create_static(
        mode,
        &txt_mode_chord_file,
        right_x,
        oy,
        file_label_w,
        LABEL_H,
    )?;
    s.combo_mode_files = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("COMBOBOX"),
        w!(""),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | CBS_DROPDOWNLIST as u32),
        combo_x,
        oy,
        combo_w,
        200,
        mode,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_MODE_CHORD_FILE as _),
        instance,
        None,
    )?;
    s.btn_mode_set_override = create_btn(
        mode,
        IDC_MODE_SET_OVERRIDE,
        &txt_mode_set_override,
        combo_x + combo_w + gap,
        oy,
        mode_set_w,
        row_ctl_h,
        BS_PUSHBUTTON,
    )?;
    s.btn_mode_clear_override = create_btn(
        mode,
        IDC_MODE_CLEAR_OVERRIDE,
        &txt_mode_clear_override,
        combo_x + combo_w + gap + mode_set_w + gap,
        oy,
        mode_clear_w,
        row_ctl_h,
        BS_PUSHBUTTON,
    )?;
    oy += LABEL_H + LABEL_STACK_GAP;

    let mode_list_style = WINDOW_STYLE(
        WS_CHILD.0
            | WS_VISIBLE.0
            | WS_BORDER.0
            | WS_VSCROLL.0
            | WS_TABSTOP.0
            | LBS_NOTIFY
            | LBS_USETABSTOPS,
    );
    s.list_mode_chords = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("LISTBOX"),
        w!(""),
        mode_list_style,
        stack_x,
        oy,
        stack_inner_w,
        MODE_CHORD_LIST_H_FALLBACK,
        mode,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_MODE_CHORD_LIST as _),
        instance,
        None,
    )?;
    s.mode_chord_file_choices.clear();
    for name in list_slots_file_candidates(&s.config_dir) {
        let label = slots_book_label(&s.config_dir, &name);
        let w = to_wide(&label);
        let _ = SendMessageW(
            s.combo_mode_files,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w.as_ptr() as isize),
        );
        s.mode_chord_file_choices.push(name);
    }
    s.lbl_mode_status = create_static(mode, "", stack_x, oy, stack_inner_w, LABEL_H)?;
    oy += MODE_CHORD_LIST_H_FALLBACK + LABEL_H + 16;

    s.g_mode = create_btn(
        mode,
        0,
        &txt_group_mode,
        12,
        mode_top,
        CONTENT_W,
        (oy - mode_top).max(120),
        BS_GROUPBOX,
    )?;
    let _ = SetWindowPos(
        s.g_mode,
        HWND_BOTTOM,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    font_targets.extend_from_slice(&[
        s.g_mode,
        s.lbl_mode_current,
        s.combo_mode_current,
        s.lbl_mode_chords,
        s.list_mode_chords,
        s.lbl_mode_chord_file,
        s.combo_mode_files,
        s.btn_mode_set_override,
        s.btn_mode_clear_override,
        s.lbl_mode_hint,
        s.lbl_mode_status,
    ]);

    // ── General page (controls parented under `options` = panel_general) ──
    let misc_top = oy + 4;
    oy = misc_top + 28;
    s.lbl_timeout = create_static(options, &txt_timeout, stack_x, oy, stack_inner_w, LABEL_H)?;
    oy += LABEL_H + LABEL_STACK_GAP;
    s.edit_timeout = create_edit(options, IDC_TIMEOUT, stack_x, oy, 80, 24, ES_NUMBER)?;
    oy += 24 + FIELD_BLOCK_GAP;
    s.lbl_max_digits = create_static(
        options,
        &txt_max_digits,
        stack_x,
        oy,
        stack_inner_w,
        LABEL_H,
    )?;
    oy += LABEL_H + LABEL_STACK_GAP;
    s.edit_max_digits = create_edit(options, IDC_MAX_DIGITS, stack_x, oy, 80, 24, ES_NUMBER)?;
    oy += 24 + FIELD_BLOCK_GAP;
    s.lbl_feedback = create_static(
        options,
        &txt_feedback_ms,
        stack_x,
        oy,
        stack_inner_w,
        LABEL_H,
    )?;
    oy += LABEL_H + LABEL_STACK_GAP;
    s.edit_feedback_ms = create_edit(options, IDC_FEEDBACK_MS, stack_x, oy, 80, 24, ES_NUMBER)?;
    oy += 24 + FIELD_BLOCK_GAP;
    s.lbl_log = create_static(options, &txt_log, stack_x, oy, stack_inner_w, LABEL_H)?;
    oy += LABEL_H + LABEL_STACK_GAP;
    s.combo_log = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("COMBOBOX"),
        w!(""),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | CBS_DROPDOWNLIST as u32),
        stack_x,
        oy,
        140.min(stack_inner_w),
        200,
        options,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_LOG_LEVEL as _),
        instance,
        None,
    )?;
    for label in ["warn", "info", "debug"] {
        let w = to_wide(label);
        let _ = SendMessageW(
            s.combo_log,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w.as_ptr() as isize),
        );
    }
    oy += 28 + FIELD_BLOCK_GAP;
    s.lbl_lang = create_static(options, &txt_lang, stack_x, oy, stack_inner_w, LABEL_H)?;
    oy += LABEL_H + LABEL_STACK_GAP;
    s.combo_locale = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("COMBOBOX"),
        w!(""),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | CBS_DROPDOWNLIST as u32),
        stack_x,
        oy,
        260.min(stack_inner_w),
        200,
        options,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_UI_LOCALE as _),
        instance,
        None,
    )?;
    s.locale_choices.clear();
    s.locale_choices.push(String::new());
    let sys = to_wide(&t(keys::SETTINGS_LANG_SYSTEM));
    let _ = SendMessageW(
        s.combo_locale,
        CB_ADDSTRING,
        WPARAM(0),
        LPARAM(sys.as_ptr() as isize),
    );
    s.locale_choices.push("en".into());
    let en = to_wide(&t(keys::SETTINGS_LANG_ENGLISH));
    let _ = SendMessageW(
        s.combo_locale,
        CB_ADDSTRING,
        WPARAM(0),
        LPARAM(en.as_ptr() as isize),
    );
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    for pack in i18n::list_available_packs(&i18n::lang_dir_next_to_exe(&exe_dir)) {
        s.locale_choices.push(pack.language.clone());
        let w = to_wide(&pack.name);
        let _ = SendMessageW(
            s.combo_locale,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w.as_ptr() as isize),
        );
    }
    oy += 28 + FIELD_BLOCK_GAP;
    let autostart_h = if measure_text_width(parent, &txt_autostart) + 28 > stack_inner_w {
        36
    } else {
        22
    };
    s.chk_autostart = create_btn(
        options,
        IDC_AUTOSTART,
        &txt_autostart,
        stack_x,
        oy,
        stack_inner_w,
        autostart_h,
        BS_AUTOCHECKBOX | BS_MULTILINE,
    )?;
    oy += autostart_h + FIELD_BLOCK_GAP;
    s.lbl_slots_file = create_static(
        options,
        &txt_slots_file,
        stack_x,
        oy,
        stack_inner_w,
        LABEL_H,
    )?;
    oy += LABEL_H + LABEL_STACK_GAP;
    let open_folder_w = fitted_button_width(parent, &txt_open_config_folder);
    let slots_combo_w = 300.min(stack_inner_w - open_folder_w - 8).max(160);
    s.combo_slots_file = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("COMBOBOX"),
        w!(""),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | CBS_DROPDOWNLIST as u32),
        stack_x,
        oy,
        slots_combo_w,
        200,
        options,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_SLOTS_FILE as _),
        instance,
        None,
    )?;
    s.slots_file_choices.clear();
    s.slots_file_choices.push(String::new());
    let example_label = to_wide(&t(keys::SETTINGS_SLOTS_FILE_EXAMPLE));
    let _ = SendMessageW(
        s.combo_slots_file,
        CB_ADDSTRING,
        WPARAM(0),
        LPARAM(example_label.as_ptr() as isize),
    );
    let mut candidates = list_slots_file_candidates(&s.config_dir);
    let current = normalize_slots_file_name(&s.draft_settings.slots_file);
    if !current.is_empty() && !candidates.iter().any(|n| n.eq_ignore_ascii_case(&current)) {
        candidates.push(current.clone());
        candidates.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
    }
    for name in candidates {
        s.slots_file_choices.push(name.clone());
        let label = slots_book_label(&s.config_dir, &name);
        let w = to_wide(&label);
        let _ = SendMessageW(
            s.combo_slots_file,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(w.as_ptr() as isize),
        );
    }
    s.btn_open_config_folder = create_btn(
        options,
        IDC_OPEN_CONFIG_FOLDER,
        &txt_open_config_folder,
        stack_x + stack_inner_w - open_folder_w,
        oy,
        open_folder_w,
        28,
        BS_PUSHBUTTON,
    )?;
    oy += 28 + FIELD_BLOCK_GAP;
    s.lbl_backup_folder = create_static(
        options,
        &txt_backup_folder,
        stack_x,
        oy,
        stack_inner_w,
        LABEL_H,
    )?;
    oy += LABEL_H + LABEL_STACK_GAP;
    let browse_w = fitted_button_width(parent, &txt_backup_browse);
    let backup_w = fitted_button_width(parent, &txt_backup_config);
    let restore_w = fitted_button_width(parent, &txt_backup_restore);
    let edit_w = (stack_inner_w - browse_w - backup_w - restore_w - 24).max(120);
    s.edit_backup_folder = create_edit(
        options,
        IDC_BACKUP_FOLDER,
        stack_x,
        oy,
        edit_w,
        24,
        ES_READONLY,
    )?;
    s.btn_backup_browse = create_btn(
        options,
        IDC_BACKUP_BROWSE,
        &txt_backup_browse,
        stack_x + edit_w + 8,
        oy,
        browse_w,
        28,
        BS_PUSHBUTTON,
    )?;
    s.btn_backup_config = create_btn(
        options,
        IDC_BACKUP_CONFIG,
        &txt_backup_config,
        stack_x + edit_w + 8 + browse_w + 8,
        oy,
        backup_w,
        28,
        BS_PUSHBUTTON,
    )?;
    s.btn_backup_restore = create_btn(
        options,
        IDC_BACKUP_RESTORE,
        &txt_backup_restore,
        stack_x + edit_w + 8 + browse_w + 8 + backup_w + 8,
        oy,
        restore_w,
        28,
        BS_PUSHBUTTON,
    )?;
    oy += 28 + 18; // bottom padding inside General group
    s.g_general = create_btn(
        options,
        0,
        &t(keys::SETTINGS_GROUP_GENERAL),
        12,
        misc_top,
        CONTENT_W,
        (oy - misc_top).max(120),
        BS_GROUPBOX,
    )?;
    let _ = SetWindowPos(
        s.chk_autostart,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    // Status strip under the group (Keys Capture strip). Empty 40px stays outside the box.
    oy += KEYS_GROUP_GAP;
    s.lbl_backup_status = {
        let instance = GetModuleHandleW(None)?;
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!(""),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | SS_NOPREFIX),
            stack_x,
            oy,
            stack_inner_w,
            BACKUP_STATUS_H,
            options,
            None,
            instance,
            None,
        )?
    };
    // Group boxes stay under their sibling controls.
    for gb in [s.g_search, s.g_general] {
        let _ = SetWindowPos(
            gb,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
    font_targets.extend_from_slice(&[
        s.g_search,
        s.lbl_match_mode,
        s.combo_search,
        s.chk_case,
        s.g_general,
        s.lbl_timeout,
        s.edit_timeout,
        s.lbl_max_digits,
        s.edit_max_digits,
        s.lbl_feedback,
        s.edit_feedback_ms,
        s.lbl_log,
        s.combo_log,
        s.lbl_lang,
        s.combo_locale,
        s.lbl_slots_file,
        s.combo_slots_file,
        s.btn_open_config_folder,
        s.lbl_backup_folder,
        s.edit_backup_folder,
        s.btn_backup_browse,
        s.btn_backup_config,
        s.btn_backup_restore,
        s.lbl_backup_status,
        s.chk_autostart,
    ]);

    // Tabs + footer on the dialog (created last so they stay above the panels).
    // Strip order: General, Slots, then Advanced tabs. First open still shows Slots.
    let tab_y = 10i32;
    let tab_h = 26i32;
    let tab_gap = 8i32;
    let mut tab_x = 12i32;
    let tw_general = fitted_button_width(parent, &txt_tab_general).max(80);
    s.tab_general = create_btn(
        parent,
        IDC_TAB_GENERAL,
        &txt_tab_general,
        tab_x,
        tab_y,
        tw_general,
        tab_h,
        TAB_STICKY | WS_GROUP,
    )?;
    tab_x += tw_general + tab_gap;
    let tw_slots = fitted_button_width(parent, &txt_tab_slots).max(80);
    s.tab_slots = create_btn(
        parent,
        IDC_TAB_SLOTS,
        &txt_tab_slots,
        tab_x,
        tab_y,
        tw_slots,
        tab_h,
        TAB_STICKY,
    )?;
    tab_x += tw_slots + tab_gap;
    let tw_mode = fitted_button_width(parent, &txt_tab_mode).max(80);
    s.tab_mode = create_btn(
        parent,
        IDC_TAB_MODE,
        &txt_tab_mode,
        tab_x,
        tab_y,
        tw_mode,
        tab_h,
        TAB_STICKY,
    )?;
    tab_x += tw_mode + tab_gap;
    let tw_keys = fitted_button_width(parent, &txt_tab_keys).max(80);
    s.tab_keys = create_btn(
        parent,
        IDC_TAB_KEYS,
        &txt_tab_keys,
        tab_x,
        tab_y,
        tw_keys,
        tab_h,
        TAB_STICKY,
    )?;
    tab_x += tw_keys + tab_gap;
    let tw_triggers = fitted_button_width(parent, &txt_tab_triggers).max(80);
    s.tab_triggers = create_btn(
        parent,
        IDC_TAB_TRIGGERS,
        &txt_tab_triggers,
        tab_x,
        tab_y,
        tw_triggers,
        tab_h,
        TAB_STICKY,
    )?;
    tab_x += tw_triggers + tab_gap;
    let tw_search = fitted_button_width(parent, &txt_tab_search).max(80);
    s.tab_search = create_btn(
        parent,
        IDC_TAB_SEARCH,
        &txt_tab_search,
        tab_x,
        tab_y,
        tw_search,
        tab_h,
        TAB_STICKY,
    )?;
    // Footer on the main dialog — create hidden, then place_footer_buttons shows them
    // (avoids a first paint at dummy coords that leaves border ghosts).
    let save_w = fitted_button_width(parent, &txt_save);
    let apply_w = fitted_button_width(parent, &txt_apply);
    let cancel_w = fitted_button_width(parent, &txt_btn_cancel);
    s.btn_save = create_btn_ex(
        parent,
        IDC_SAVE,
        &txt_save,
        0,
        0,
        save_w,
        FOOTER_BTN_H,
        BS_PUSHBUTTON,
        false,
    )?;
    s.btn_apply = create_btn_ex(
        parent,
        IDC_APPLY,
        &txt_apply,
        0,
        0,
        apply_w,
        FOOTER_BTN_H,
        BS_PUSHBUTTON,
        false,
    )?;
    s.btn_cancel = create_btn_ex(
        parent,
        IDC_CANCEL,
        &txt_btn_cancel,
        0,
        0,
        cancel_w,
        FOOTER_BTN_H,
        BS_PUSHBUTTON,
        false,
    )?;
    s.chk_advanced = create_btn(
        parent,
        IDC_ADVANCED,
        &txt_advanced,
        0,
        0,
        100,
        26,
        BS_AUTOCHECKBOX,
    )?;

    font_targets.extend_from_slice(&[
        s.tab_general,
        s.tab_slots,
        s.tab_mode,
        s.tab_keys,
        s.tab_triggers,
        s.tab_search,
        s.btn_save,
        s.btn_apply,
        s.btn_cancel,
        s.chk_advanced,
    ]);
    for h in font_targets {
        set_font(h);
    }
    layout_settings(parent, s);

    Ok(())
}

unsafe fn sync_tab_checks(s: &SettingsState) {
    set_check(s.tab_general, s.page == Page::General);
    set_check(s.tab_slots, s.page == Page::Slots);
    set_check(s.tab_mode, s.page == Page::Mode);
    set_check(s.tab_keys, s.page == Page::Keys);
    set_check(s.tab_triggers, s.page == Page::Triggers);
    set_check(s.tab_search, s.page == Page::Search);
}

unsafe fn show_page(state_ptr: *mut SettingsState, page: Page) {
    let s = &mut *state_ptr;
    let same = s.page == page;
    s.page = page;
    sync_tab_checks(s);
    let parent = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    if parent.0.is_null() {
        return;
    }
    if same {
        let _ = SetFocus(active_page_panel(s));
        return;
    }
    layout_settings(parent, s);
    let _ = SetFocus(active_page_panel(s));
}

unsafe fn set_text(hwnd: HWND, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = SetWindowTextW(hwnd, PCWSTR(wide.as_ptr()));
}

unsafe fn window_text(hwnd: HWND) -> String {
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    let n = GetWindowTextW(hwnd, &mut buf);
    if n <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..n as usize])
}

unsafe fn set_check(hwnd: HWND, on: bool) {
    let _ = SendMessageW(
        hwnd,
        BM_SETCHECK,
        WPARAM(if on {
            BST_CHECKED as usize
        } else {
            BST_UNCHECKED as usize
        }),
        LPARAM(0),
    );
}

unsafe fn get_check(hwnd: HWND) -> bool {
    SendMessageW(hwnd, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == BST_CHECKED
}

unsafe fn populate_from_draft(state_ptr: *mut SettingsState) {
    let s = &mut *state_ptr;
    refresh_slot_list(state_ptr);
    update_key_labels(state_ptr);

    let mouse = mouse_trigger_from_settings(&s.draft_settings);
    let mouse_idx = match mouse.button.to_ascii_lowercase().as_str() {
        "x1" => 1,
        "middle" | "mbutton" => 2,
        _ => 0,
    };
    let _ = SendMessageW(s.combo_mouse, CB_SETCURSEL, WPARAM(mouse_idx), LPARAM(0));
    set_check(s.chk_suppress, mouse.suppress);
    set_text(s.edit_hotkey, &hotkey_from_settings(&s.draft_settings));

    // Instant fire checkbox is synced from draft via update_slots_edit_enabled /
    // load_slot_into_fields (single-digit slot IDs only).

    let mode_idx = match s.draft_settings.search.mode {
        SearchMode::Substring => 0,
        SearchMode::Prefix => 1,
        SearchMode::Exact => 2,
        SearchMode::Fuzzy => 3,
    };
    let _ = SendMessageW(s.combo_search, CB_SETCURSEL, WPARAM(mode_idx), LPARAM(0));
    set_check(s.chk_case, s.draft_settings.search.case_sensitive);
    set_text(s.edit_timeout, &s.draft_settings.timeout_sec.to_string());
    set_text(s.edit_max_digits, &s.draft_settings.max_digits.to_string());
    set_text(
        s.edit_feedback_ms,
        &s.draft_settings.feedback_ms.to_string(),
    );
    let log_idx = match s.draft_settings.log_level.to_ascii_lowercase().as_str() {
        "info" => 1,
        "debug" => 2,
        _ => 0,
    };
    let _ = SendMessageW(s.combo_log, CB_SETCURSEL, WPARAM(log_idx), LPARAM(0));
    let want = s.draft_settings.ui.locale.trim().to_ascii_lowercase();
    let loc_idx = s
        .locale_choices
        .iter()
        .position(|c| c == &want)
        .unwrap_or(0);
    let _ = SendMessageW(s.combo_locale, CB_SETCURSEL, WPARAM(loc_idx), LPARAM(0));
    let want_slots = normalize_slots_file_name(&s.draft_settings.slots_file);
    let slots_idx = s
        .slots_file_choices
        .iter()
        .position(|c| c.eq_ignore_ascii_case(&want_slots))
        .unwrap_or(0);
    let _ = SendMessageW(
        s.combo_slots_file,
        CB_SETCURSEL,
        WPARAM(slots_idx),
        LPARAM(0),
    );
    set_text(
        s.edit_backup_folder,
        &backup_folder_display(&s.draft_settings.backup_folder),
    );
    set_text(s.lbl_backup_status, "");
    set_check(s.chk_autostart, s.draft_settings.autostart);
    set_check(s.chk_wake, s.draft_settings.keys.wake_enabled);
    fill_key_set_controls(state_ptr);
    set_check(s.chk_advanced, s.draft_settings.ui.show_advanced);

    let mode_mouse_idx = match s
        .draft_settings
        .mode_switch
        .mouse_button
        .to_ascii_lowercase()
        .as_str()
    {
        "x2" => 1,
        "middle" | "mbutton" => 2,
        _ => 0,
    };
    let _ = SendMessageW(
        s.combo_mode_mouse,
        CB_SETCURSEL,
        WPARAM(mode_mouse_idx),
        LPARAM(0),
    );
    set_check(s.chk_mode_suppress, s.draft_settings.mode_switch.suppress);
    set_text(
        s.edit_mode_timeout,
        &s.draft_settings.mode_switch.chord_timeout_ms.to_string(),
    );
    let _ = SendMessageW(s.combo_mode_files, CB_SETCURSEL, WPARAM(0), LPARAM(0));
    refresh_mode_chord_list(state_ptr, None);
    sync_mode_file_combo_to_list_sel(state_ptr);
    refresh_mode_current_combo(state_ptr);
    set_text((*state_ptr).lbl_mode_status, "");

    {
        let s = &mut *state_ptr;
        s.draft_settings.legend.normalize();
    }
    refresh_legend_checks(state_ptr);

    update_slots_edit_enabled(state_ptr);
    update_conflict_warning(state_ptr);
}

fn legend_item_label(id: LegendId) -> String {
    match id {
        LegendId::Start => t(keys::APP_TYPING_LEGEND_MULTI),
        LegendId::OpenWorkdir => t(keys::APP_TYPING_LEGEND_OPEN_WORKDIR),
        LegendId::Search => t(keys::APP_TYPING_LEGEND_SEARCH),
        LegendId::DigitBack => t(keys::APP_TYPING_LEGEND_DIGIT_BACK),
        LegendId::Cancel => t(keys::APP_TYPING_LEGEND_ESC),
    }
}

unsafe fn refresh_legend_checks(state_ptr: *mut SettingsState) {
    let s = &*state_ptr;
    for (idle, chks) in [(true, s.chk_legend_idle), (false, s.chk_legend_multi)] {
        let page = s.draft_settings.legend.page(if idle {
            crate::core::sequence::Mode::Idle
        } else {
            crate::core::sequence::Mode::MultiDigit
        });
        for i in 0..LEGEND_ITEM_COUNT {
            let id = page
                .order
                .get(i)
                .and_then(|x| LegendId::parse(x))
                .unwrap_or(LegendId::ALL[i]);
            set_text(chks[i], &legend_item_label(id));
            let shown = !page.hidden.iter().any(|h| LegendId::parse(h) == Some(id));
            set_check(chks[i], shown);
        }
    }
}

unsafe fn legend_sync_hidden_from_checks(state_ptr: *mut SettingsState, idle: bool) {
    let s = &mut *state_ptr;
    let chks = if idle {
        s.chk_legend_idle
    } else {
        s.chk_legend_multi
    };
    let page = s.draft_settings.legend.page_mut(idle);
    let mut hidden = Vec::new();
    for i in 0..LEGEND_ITEM_COUNT {
        let Some(id) = page.order.get(i).and_then(|x| LegendId::parse(x)) else {
            continue;
        };
        if !get_check(chks[i]) {
            hidden.push(id.as_str().to_string());
        }
    }
    page.hidden = hidden;
}

unsafe fn legend_move_sel(state_ptr: *mut SettingsState, idle: bool, delta: i32) {
    legend_sync_hidden_from_checks(state_ptr, idle);
    let s = &mut *state_ptr;
    let sel = if idle {
        s.legend_idle_sel
    } else {
        s.legend_multi_sel
    };
    let page = s.draft_settings.legend.page_mut(idle);
    if page.order.len() < 2 {
        return;
    }
    let n = page.order.len();
    let j = sel as i32 + delta;
    if j < 0 || j >= n as i32 {
        return;
    }
    page.order.swap(sel, j as usize);
    if idle {
        s.legend_idle_sel = j as usize;
    } else {
        s.legend_multi_sel = j as usize;
    }
    refresh_legend_checks(state_ptr);
}

unsafe fn refresh_slot_list(state_ptr: *mut SettingsState) {
    let s = &mut *state_ptr;
    let sorted = s.draft_slots.sorted();
    s.slot_ids = sorted.iter().map(|slot| slot.id.clone()).collect();
    let _ = SendMessageW(s.slot_list, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    for slot in sorted {
        let line = format!("{}  {}", slot.id, slot.name);
        let wide: Vec<u16> = line.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = SendMessageW(
            s.slot_list,
            LB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
}

unsafe fn combo_slots_file_selection(state_ptr: *mut SettingsState) -> String {
    let s = &*state_ptr;
    let idx = SendMessageW(s.combo_slots_file, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    s.slots_file_choices
        .get(idx as usize)
        .cloned()
        .unwrap_or_default()
}

unsafe fn slots_edit_block_message(state_ptr: *mut SettingsState) -> String {
    let s = &*state_ptr;
    let name = combo_slots_file_selection(state_ptr);
    if !slots_file_is_writable(&name) {
        t(keys::SETTINGS_SLOTS_READONLY)
    } else if !s.slots_writable && resolve_slots_path(&s.config_dir, &name) == s.slots_path_at_open
    {
        t(keys::SETTINGS_SLOTS_UNWRITABLE)
    } else {
        t(keys::SETTINGS_SLOTS_READONLY)
    }
}

/// Whether the Slots tab may mutate draft slots for the combo selection.
unsafe fn slots_edit_allowed(state_ptr: *mut SettingsState) -> bool {
    let s = &*state_ptr;
    let name = combo_slots_file_selection(state_ptr);
    if !slots_file_is_writable(&name) {
        return false;
    }
    let path = resolve_slots_path(&s.config_dir, &name);
    // Broken personal file still selected — never stage overwrites.
    if !s.slots_writable && path == s.slots_path_at_open {
        return false;
    }
    true
}

unsafe fn update_slots_edit_enabled(state_ptr: *mut SettingsState) {
    let s = &*state_ptr;
    let allow = slots_edit_allowed(state_ptr);
    let _ = EnableWindow(s.btn_add, allow);
    let _ = EnableWindow(s.btn_delete, allow);
    let _ = EnableWindow(s.btn_swap, allow);
    let _ = EnableWindow(s.btn_slot_apply, allow);
    let _ = EnableWindow(s.edit_id, allow);
    let _ = EnableWindow(s.edit_name, allow);
    let _ = EnableWindow(s.edit_path, allow);
    let _ = EnableWindow(s.edit_desc, allow);
    let _ = EnableWindow(s.edit_workdir, allow);
    let _ = EnableWindow(s.chk_open_workdir, allow);
    let path = window_text(s.edit_path).trim().to_string();
    let focus_ok = allow && !is_http_url(&path) && !is_chain_path(&path);
    let _ = EnableWindow(s.chk_focus_existing, focus_ok);
    if !focus_ok {
        set_check(s.chk_focus_existing, true);
    }
    let id = window_text(s.edit_id).trim().to_string();
    let instant_ok = allow && is_instant_digit_id(&id);
    let _ = EnableWindow(s.chk_instant_fire, instant_ok);
    if instant_ok {
        let on = s.draft_settings.instant.get(&id).copied().unwrap_or(false);
        set_check(s.chk_instant_fire, on);
    } else {
        set_check(s.chk_instant_fire, false);
    }
    if !allow {
        set_text(s.warn_label, &slots_edit_block_message(state_ptr));
    } else {
        update_conflict_warning(state_ptr);
    }
}

unsafe fn update_conflict_warning(state_ptr: *mut SettingsState) {
    if !slots_edit_allowed(state_ptr) {
        set_text(
            (*state_ptr).warn_label,
            &slots_edit_block_message(state_ptr),
        );
        return;
    }
    let s = &*state_ptr;
    let mut parts = String::new();

    let conflicts = leading_zero_conflicts(s.draft_slots.all());
    if !conflicts.is_empty() {
        let mut pairs = String::new();
        for (a, b) in conflicts.iter().take(4) {
            pairs.push_str(&tf(
                keys::SETTINGS_WARN_LOOKALIKE_PAIR,
                &[("a", a.as_str()), ("b", b.as_str())],
            ));
        }
        parts.push_str(&tf(keys::SETTINGS_WARN_LOOKALIKE, &[("pairs", &pairs)]));
    }

    match validate_chains(s.draft_slots.all()) {
        Ok(warnings) if !warnings.is_empty() => {
            let mut items = String::new();
            for w in warnings.iter().take(4) {
                items.push_str(&tf(
                    keys::SETTINGS_WARN_CHAIN_MISSING_ITEM,
                    &[("slot", w.slot_id.as_str()), ("ids", &w.missing.join(", "))],
                ));
            }
            if !parts.is_empty() {
                parts.push('\n');
            }
            parts.push_str(&tf(keys::SETTINGS_WARN_CHAIN_MISSING, &[("items", &items)]));
        }
        Ok(_) => {}
        Err(ChainBlockError::Invalid { slot_id, message }) => {
            if !parts.is_empty() {
                parts.push('\n');
            }
            parts.push_str(&tf(
                keys::SETTINGS_CHAIN_INVALID,
                &[("slot", &slot_id), ("message", &message)],
            ));
        }
        Err(ChainBlockError::Cycle { path }) => {
            if !parts.is_empty() {
                parts.push('\n');
            }
            parts.push_str(&tf(
                keys::SETTINGS_CHAIN_CYCLE,
                &[("path", &path.join(" → "))],
            ));
        }
    }

    set_text(s.warn_label, &parts);
}

/// Open the config folder (not the selected slots file's parent) in Explorer.
unsafe fn open_config_folder(dir: &Path) {
    let wide = to_wide(&dir.to_string_lossy());
    ShellExecuteW(
        None,
        w!("open"),
        PCWSTR(wide.as_ptr()),
        None,
        None,
        SW_SHOWNORMAL,
    );
}

unsafe fn backup_config_folder(state_ptr: *mut SettingsState) {
    let config_dir = (*state_ptr).config_dir.clone();
    let dest_parent = match resolve_backup_parent_from_ui(&*state_ptr) {
        Ok(p) => p,
        Err(key) => {
            set_text((*state_ptr).lbl_backup_status, &t(key));
            return;
        }
    };
    (*state_ptr).draft_settings.backup_folder = backup_folder_store(&dest_parent.to_string_lossy());
    if backup_dest_forbidden(&dest_parent, &config_dir) {
        set_text(
            (*state_ptr).lbl_backup_status,
            &t(keys::SETTINGS_BACKUP_INTO_APP),
        );
        return;
    }
    match backup_config_json(&config_dir, &dest_parent) {
        Ok(dest) => {
            let path = dest.display().to_string();
            set_text(
                (*state_ptr).lbl_backup_status,
                &tf(keys::SETTINGS_BACKUP_OK, &[("path", &path)]),
            );
        }
        Err(ConfigError::BackupIntoConfig) => {
            set_text(
                (*state_ptr).lbl_backup_status,
                &t(keys::SETTINGS_BACKUP_INTO_APP),
            );
        }
        Err(e) => {
            let err = e.to_string();
            set_text(
                (*state_ptr).lbl_backup_status,
                &tf(keys::SETTINGS_BACKUP_FAIL, &[("error", &err)]),
            );
        }
    }
}

fn pending_backup_status() -> &'static Mutex<String> {
    static SLOT: OnceLock<Mutex<String>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(String::new()))
}

unsafe fn apply_pending_backup_status(state_ptr: *mut SettingsState) {
    let Ok(mut pending) = pending_backup_status().lock() else {
        return;
    };
    if pending.is_empty() {
        return;
    }
    set_text((*state_ptr).lbl_backup_status, pending.as_str());
    pending.clear();
}

unsafe fn restore_config_folder(state_ptr: *mut SettingsState) {
    if !confirm_leave_slots_book(state_ptr) {
        return;
    }
    let hwnd = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    let config_dir = (*state_ptr).config_dir.clone();
    let start = match resolve_backup_parent_from_ui(&*state_ptr) {
        Ok(p) => p,
        Err(key) => {
            set_text((*state_ptr).lbl_backup_status, &t(key));
            return;
        }
    };
    let Some(stamp) = pick_folder(hwnd, &start, &t(keys::SETTINGS_RESTORE_PICK)) else {
        return;
    };
    if path_is_inside(&stamp, &config_dir) || path_is_inside(&config_dir, &stamp) {
        set_text(
            (*state_ptr).lbl_backup_status,
            &t(keys::SETTINGS_RESTORE_FROM_LIVE),
        );
        return;
    }
    let current_writable = slots_file_is_writable(&(*state_ptr).draft_settings.slots_file);
    let Some(choice) = restore_choice_dialog(hwnd, current_writable) else {
        return;
    };
    let current = (*state_ptr).draft_settings.slots_file.clone();
    match restore_config_json(
        &config_dir,
        &stamp,
        choice.scope,
        choice.include_settings,
        &current,
    ) {
        Ok(report) => {
            let files = report.copied.join(", ");
            if let Ok(mut pending) = pending_backup_status().lock() {
                *pending = tf(keys::SETTINGS_RESTORE_OK, &[("files", &files)]);
            }
            info!(files = %files, "settings restore copied");
            REOPEN_PAGE.store(Page::General.as_u8(), Ordering::SeqCst);
            REOPEN_AFTER_APPLY.store(true, Ordering::SeqCst);
            let host = (*state_ptr).host;
            let _ = PostMessageW(host, WM_DK_SETTINGS_APPLIED, WPARAM(0), LPARAM(0));
            let _ = DestroyWindow(hwnd);
        }
        Err(ConfigError::RestoreFromLive) => {
            set_text(
                (*state_ptr).lbl_backup_status,
                &t(keys::SETTINGS_RESTORE_FROM_LIVE),
            );
        }
        Err(ConfigError::RestoreNothing) => {
            set_text(
                (*state_ptr).lbl_backup_status,
                &t(keys::SETTINGS_RESTORE_NOTHING),
            );
        }
        Err(ConfigError::RestoreCurrentReadOnly) => {
            set_text(
                (*state_ptr).lbl_backup_status,
                &t(keys::SETTINGS_RESTORE_READONLY),
            );
        }
        Err(ConfigError::RestoreCurrentMissing) => {
            set_text(
                (*state_ptr).lbl_backup_status,
                &t(keys::SETTINGS_RESTORE_MISSING),
            );
        }
        Err(e) => {
            let err = e.to_string();
            set_text(
                (*state_ptr).lbl_backup_status,
                &tf(keys::SETTINGS_RESTORE_FAIL, &[("error", &err)]),
            );
        }
    }
}

struct RestoreChoice {
    scope: RestoreSlotsScope,
    include_settings: bool,
}

struct RestoreDlgState {
    radio_all: HWND,
    radio_current: HWND,
    chk_settings: HWND,
    choice: Option<RestoreChoice>,
    done: bool,
}

const RESTORE_CLASS: PCWSTR = w!("DialKeyRestoreChoice");
const IDC_RESTORE_ALL: isize = 1;
const IDC_RESTORE_CURRENT: isize = 2;
const IDC_RESTORE_SETTINGS: isize = 3;
const IDC_RESTORE_OK: isize = 4;
const IDC_RESTORE_CANCEL: isize = 5;
const RESTORE_DLG_STYLE: WINDOW_STYLE = WINDOW_STYLE(WS_POPUP.0 | WS_CAPTION.0 | WS_SYSMENU.0);

unsafe fn restore_outer_size(client_w: i32, client_h: i32, dpi: u32) -> (i32, i32) {
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: client_w.max(100),
        bottom: client_h.max(80),
    };
    if AdjustWindowRectExForDpi(
        &mut rc,
        RESTORE_DLG_STYLE,
        false,
        WINDOW_EX_STYLE::default(),
        dpi.max(96),
    )
    .is_ok()
    {
        (
            (rc.right - rc.left).max(client_w),
            (rc.bottom - rc.top).max(client_h),
        )
    } else {
        (client_w + 16, client_h + 39)
    }
}

unsafe fn ensure_restore_class() -> anyhow::Result<()> {
    static REGISTERED: AtomicBool = AtomicBool::new(false);
    if REGISTERED.load(Ordering::SeqCst) {
        return Ok(());
    }
    let instance = GetModuleHandleW(None)?;
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: windows::Win32::UI::WindowsAndMessaging::CS_HREDRAW
            | windows::Win32::UI::WindowsAndMessaging::CS_VREDRAW,
        lpfnWndProc: Some(restore_dlg_proc),
        hInstance: instance.into(),
        hCursor: LoadCursorW(None, IDC_ARROW)?,
        hbrBackground: HBRUSH(GetSysColorBrush(COLOR_WINDOW).0),
        lpszClassName: RESTORE_CLASS,
        ..Default::default()
    };
    if RegisterClassExW(&wc) == 0 {
        let err = GetLastError();
        if err != ERROR_CLASS_ALREADY_EXISTS {
            anyhow::bail!("RegisterClassExW restore dialog failed: {err:?}");
        }
    }
    REGISTERED.store(true, Ordering::SeqCst);
    Ok(())
}

unsafe fn restore_choice_dialog(owner: HWND, current_writable: bool) -> Option<RestoreChoice> {
    if ensure_restore_class().is_err() {
        error!("restore dialog class failed");
        return None;
    }
    let instance = GetModuleHandleW(None).ok()?;
    let title = to_wide(&t(keys::SETTINGS_RESTORE_TITLE));
    let dlg = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        RESTORE_CLASS,
        PCWSTR(title.as_ptr()),
        RESTORE_DLG_STYLE,
        0,
        0,
        200,
        120,
        owner,
        None,
        instance,
        None,
    )
    .ok()?;

    let txt_all = t(keys::SETTINGS_RESTORE_ALL_BOOKS);
    let txt_current = t(keys::SETTINGS_RESTORE_CURRENT_BOOK);
    let txt_settings = t(keys::SETTINGS_RESTORE_INCLUDE_SETTINGS);
    let txt_hint = t(keys::SETTINGS_RESTORE_HINT);
    let txt_ok = t(keys::SETTINGS_RESTORE_TITLE);
    let txt_cancel = t(keys::SETTINGS_BTN_CANCEL);

    let margin = 16i32;
    let row_h = 22i32;
    let row_gap = 4i32;
    let hint_h = 22i32;
    let hint_gap = 8i32;
    let btn_h = 28i32;
    let btn_gap = 12i32;
    let chk_pad = 28i32;
    let ok_w = fitted_button_width(dlg, &txt_ok).max(80);
    let cancel_w = fitted_button_width(dlg, &txt_cancel).max(80);
    let option_w = [
        measure_text_width(dlg, &txt_all),
        measure_text_width(dlg, &txt_current),
        measure_text_width(dlg, &txt_settings),
    ]
    .into_iter()
    .max()
    .unwrap_or(160)
        + chk_pad;
    let hint_w = measure_text_width(dlg, &txt_hint).max(option_w);
    let btns_w = ok_w + 8 + cancel_w;
    let inner_w = option_w.max(hint_w).max(btns_w);
    let client_w = margin + inner_w + margin;
    let mut oy = margin;
    let radio_all = create_btn(
        dlg,
        IDC_RESTORE_ALL,
        &txt_all,
        margin,
        oy,
        option_w,
        row_h,
        BS_AUTORADIOBUTTON | WS_GROUP,
    )
    .ok()?;
    oy += row_h + row_gap;
    let radio_current = create_btn(
        dlg,
        IDC_RESTORE_CURRENT,
        &txt_current,
        margin,
        oy,
        option_w,
        row_h,
        BS_AUTORADIOBUTTON,
    )
    .ok()?;
    oy += row_h + row_gap;
    let chk_settings = create_btn(
        dlg,
        IDC_RESTORE_SETTINGS,
        &txt_settings,
        margin,
        oy,
        option_w,
        row_h,
        BS_AUTOCHECKBOX,
    )
    .ok()?;
    oy += row_h + hint_gap;
    let hint = create_static(dlg, &txt_hint, margin, oy, inner_w, hint_h).ok()?;
    oy += hint_h + btn_gap;
    let btn_x = margin + inner_w - btns_w;
    let btn_ok = create_btn(
        dlg,
        IDC_RESTORE_OK,
        &txt_ok,
        btn_x,
        oy,
        ok_w,
        btn_h,
        BS_PUSHBUTTON,
    )
    .ok()?;
    let btn_cancel = create_btn(
        dlg,
        IDC_RESTORE_CANCEL,
        &txt_cancel,
        btn_x + ok_w + 8,
        oy,
        cancel_w,
        btn_h,
        BS_PUSHBUTTON,
    )
    .ok()?;
    oy += btn_h + margin;
    for h in [
        radio_all,
        radio_current,
        chk_settings,
        hint,
        btn_ok,
        btn_cancel,
    ] {
        set_font(h);
    }

    let dpi = GetDpiForWindow(dlg);
    let (ow, oh) = restore_outer_size(client_w, oy, dpi);
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let _ = SetWindowPos(
        dlg,
        HWND_TOP,
        pt.x.saturating_sub(80),
        pt.y.saturating_sub(40),
        ow,
        oh,
        SWP_SHOWWINDOW,
    );
    set_check(radio_all, true);
    if !current_writable {
        let _ = EnableWindow(radio_current, false);
    }

    let state = Box::into_raw(Box::new(RestoreDlgState {
        radio_all,
        radio_current,
        chk_settings,
        choice: None,
        done: false,
    }));
    SetWindowLongPtrW(dlg, GWLP_USERDATA, state as isize);
    let _ = EnableWindow(owner, false);
    let _ = SetForegroundWindow(dlg);

    let mut msg = MSG::default();
    while !(*state).done {
        let ok = GetMessageW(&mut msg, None, 0, 0);
        if !ok.as_bool() {
            break;
        }
        if IsDialogMessageW(dlg, &msg).as_bool() {
            continue;
        }
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    let _ = EnableWindow(owner, true);
    let _ = SetForegroundWindow(owner);
    SetWindowLongPtrW(dlg, GWLP_USERDATA, 0);
    let _ = DestroyWindow(dlg);
    let boxed = Box::from_raw(state);
    boxed.choice
}

unsafe extern "system" fn restore_dlg_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam.0 as u16) as isize;
            let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RestoreDlgState;
            if state.is_null() {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            match id {
                IDC_RESTORE_OK => {
                    let all = get_check((*state).radio_all);
                    let include = get_check((*state).chk_settings);
                    (*state).choice = Some(RestoreChoice {
                        scope: if all {
                            RestoreSlotsScope::AllBooks
                        } else {
                            RestoreSlotsScope::CurrentBook
                        },
                        include_settings: include,
                    });
                    (*state).done = true;
                }
                IDC_RESTORE_CANCEL => {
                    (*state).done = true;
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RestoreDlgState;
            if !state.is_null() {
                (*state).done = true;
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn browse_backup_folder(state_ptr: *mut SettingsState) {
    let hwnd = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    let current = PathBuf::from(window_text((*state_ptr).edit_backup_folder).trim());
    let Some(picked) = pick_folder(hwnd, &current, &t(keys::SETTINGS_BACKUP_FOLDER)) else {
        return;
    };
    set_text(
        (*state_ptr).edit_backup_folder,
        &picked.display().to_string(),
    );
    (*state_ptr).draft_settings.backup_folder = backup_folder_store(&picked.to_string_lossy());
}

unsafe fn resolve_backup_parent_from_ui(s: &SettingsState) -> Result<PathBuf, &'static str> {
    let shown = window_text(s.edit_backup_folder);
    let shown = shown.trim();
    if shown.is_empty() {
        return desktop_dir().ok_or(keys::SETTINGS_BACKUP_NO_FOLDER);
    }
    Ok(PathBuf::from(shown))
}

fn backup_dest_forbidden(dest_parent: &Path, config_dir: &Path) -> bool {
    if path_is_inside(dest_parent, config_dir) {
        return true;
    }
    path_is_inside(dest_parent, &exe_dir())
}

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn backup_folder_display(stored: &str) -> String {
    let stored = stored.trim();
    if stored.is_empty() {
        desktop_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    } else {
        stored.to_string()
    }
}

fn backup_folder_store(shown: &str) -> String {
    let shown = shown.trim();
    if shown.is_empty() {
        return String::new();
    }
    if let Some(desk) = desktop_dir() {
        if path_is_inside(Path::new(shown), &desk) && path_is_inside(&desk, Path::new(shown)) {
            return String::new();
        }
    }
    shown.to_string()
}

fn desktop_dir() -> Option<PathBuf> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Desktop, SHGetKnownFolderPath, KF_FLAG_DEFAULT};

    unsafe {
        match SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, HANDLE::default()) {
            Ok(pwstr) => {
                let s = pwstr.to_string().ok();
                CoTaskMemFree(Some(pwstr.0 as *const _));
                if let Some(s) = s.filter(|p| !p.is_empty()) {
                    return Some(PathBuf::from(s));
                }
            }
            Err(e) => {
                error!(error = %e, "SHGetKnownFolderPath(Desktop) failed");
            }
        }
    }
    std::env::var_os("USERPROFILE").map(|u| PathBuf::from(u).join("Desktop"))
}

unsafe fn pick_folder(owner: HWND, current: &Path, title: &str) -> Option<PathBuf> {
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, IShellItem, SHCreateItemFromParsingName,
        FILEOPENDIALOGOPTIONS, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
    };

    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    let dialog: IFileOpenDialog =
        match CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) {
            Ok(d) => d,
            Err(e) => {
                error!(error = %e, "IFileOpenDialog create failed");
                return None;
            }
        };
    let opts = FILEOPENDIALOGOPTIONS(FOS_PICKFOLDERS.0 | FOS_FORCEFILESYSTEM.0);
    let _ = dialog.SetOptions(opts);
    let title = to_wide(title);
    let _ = dialog.SetTitle(PCWSTR(title.as_ptr()));
    if current.is_dir() {
        let cur = to_wide(&current.to_string_lossy());
        if let Ok(item) =
            SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(cur.as_ptr()), None)
        {
            let _ = dialog.SetFolder(&item);
        }
    }
    match dialog.Show(owner) {
        Ok(()) => {}
        Err(e) if e.code().0 == 0x8007_04C7u32 as i32 => return None,
        Err(e) => {
            error!(error = %e, "backup folder picker failed");
            return None;
        }
    }
    let item = match dialog.GetResult() {
        Ok(i) => i,
        Err(e) => {
            error!(error = %e, "IFileOpenDialog GetResult failed");
            return None;
        }
    };
    let pw = match item.GetDisplayName(SIGDN_FILESYSPATH) {
        Ok(p) => p,
        Err(e) => {
            error!(error = %e, "IShellItem GetDisplayName failed");
            return None;
        }
    };
    let s = pw.to_string().ok();
    CoTaskMemFree(Some(pw.0 as *const _));
    s.filter(|p| !p.is_empty()).map(PathBuf::from)
}

/// Rebuild the chord list (always 0–9 rows: code, slots file, display name).
/// `restore_sel`: keep that row after Set/Clear; `None` selects the current book.
unsafe fn refresh_mode_chord_list(state_ptr: *mut SettingsState, restore_sel: Option<i32>) {
    let s = &*state_ptr;
    let candidates = list_slots_file_candidates(&s.config_dir);
    let map = build_mode_chord_map(&candidates, &s.draft_settings.mode_switch.keys);
    let current = normalize_slots_file_name(&s.draft_settings.slots_file);
    let _ = SendMessageW(s.list_mode_chords, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    let mut current_idx = 0i32;
    for i in 0..10usize {
        let Some(code) = chord_code_at(i) else {
            break;
        };
        let file = map.get(&code).map(|f| f.as_str());
        if let Some(f) = file {
            let name = normalize_slots_file_name(f);
            if !current.is_empty() && name.eq_ignore_ascii_case(&current) {
                current_idx = i as i32;
            }
        }
        let line = mode_chord_row_text(&s.config_dir, &code, file, &current);
        let wide: Vec<u16> = line.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = SendMessageW(
            s.list_mode_chords,
            LB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
    let sel = restore_sel.filter(|v| *v >= 0).unwrap_or(current_idx);
    let _ = SendMessageW(
        s.list_mode_chords,
        LB_SETCURSEL,
        WPARAM(sel as usize),
        LPARAM(0),
    );
}

/// Code for the selected row in the Mode chord list (0–9).
unsafe fn selected_mode_chord_code(s: &SettingsState) -> Option<String> {
    let sel = SendMessageW(s.list_mode_chords, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if sel < 0 {
        return None;
    }
    s.mode_chord_code_choices.get(sel as usize).cloned()
}

/// Sync the display-name list to the effective map for the selected code row.
unsafe fn sync_mode_file_combo_to_list_sel(state_ptr: *mut SettingsState) {
    let s = &*state_ptr;
    let Some(code) = selected_mode_chord_code(s) else {
        return;
    };
    let candidates = list_slots_file_candidates(&s.config_dir);
    let map = build_mode_chord_map(&candidates, &s.draft_settings.mode_switch.keys);
    if let Some(file) = map.get(&code) {
        if let Some(fi) = s.mode_chord_file_choices.iter().position(|f| f == file) {
            let _ = SendMessageW(s.combo_mode_files, CB_SETCURSEL, WPARAM(fi), LPARAM(0));
        }
    }
}

/// Pin selected list-row code to selected slots file; swap if that file is on another code.
unsafe fn set_mode_override(state_ptr: *mut SettingsState) {
    let hwnd = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    let s = &mut *state_ptr;
    let file_idx = SendMessageW(s.combo_mode_files, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    let (Some(code), Some(file)) = (
        selected_mode_chord_code(s),
        s.mode_chord_file_choices.get(file_idx as usize).cloned(),
    ) else {
        msgbox(
            hwnd,
            &t(keys::SETTINGS_MODE_SET_NEED_BOTH),
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONWARNING,
        );
        return;
    };

    let candidates = list_slots_file_candidates(&s.config_dir);
    let map = build_mode_chord_map(&candidates, &s.draft_settings.mode_switch.keys);
    let previous = map.get(&code).cloned();
    let other_code = map
        .iter()
        .find(|(c, f)| *f == &file && *c != &code)
        .map(|(c, _)| c.clone());
    let code_s = code.clone();
    let file_s = file.clone();
    let swapped_other = other_code.clone();

    let chord_keys = &mut s.draft_settings.mode_switch.keys;
    if let Some(other) = other_code {
        // Swap: code ↔ other for this file / previous file.
        chord_keys.remove(&code);
        chord_keys.remove(&other);
        chord_keys.insert(code.clone(), file.clone());
        if let Some(prev) = previous {
            if prev != file {
                chord_keys.insert(other, prev);
            }
        }
    } else {
        chord_keys.retain(|k, v| k != &code && v != &file);
        chord_keys.insert(code, file);
    }
    let sel = SendMessageW(s.list_mode_chords, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    refresh_mode_chord_list(state_ptr, Some(sel as i32));
    sync_mode_file_combo_to_list_sel(state_ptr);
    let msg = if let Some(other) = swapped_other {
        tf(
            keys::SETTINGS_MODE_SET_SWAPPED,
            &[("code", &code_s), ("other", &other), ("file", &file_s)],
        )
    } else {
        tf(
            keys::SETTINGS_MODE_SET_OK,
            &[("code", &code_s), ("file", &file_s)],
        )
    };
    set_text((*state_ptr).lbl_mode_status, &msg);
    refresh_mode_current_combo(state_ptr);
}

/// Remove the override for the selected list-row code, reverting it to auto-assignment.
unsafe fn clear_mode_override(state_ptr: *mut SettingsState) {
    let s = &mut *state_ptr;
    let cleared = selected_mode_chord_code(s);
    if let Some(code) = &cleared {
        s.draft_settings.mode_switch.keys.remove(code);
    }
    let sel = SendMessageW(s.list_mode_chords, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    refresh_mode_chord_list(state_ptr, Some(sel as i32));
    sync_mode_file_combo_to_list_sel(state_ptr);
    if let Some(code) = cleared {
        set_text(
            (*state_ptr).lbl_mode_status,
            &tf(keys::SETTINGS_MODE_CLEAR_OK, &[("code", &code)]),
        );
    }
    refresh_mode_current_combo(state_ptr);
}

unsafe fn refresh_slots_current_label(state_ptr: *mut SettingsState) {
    let s = &*state_ptr;
    let name = normalize_slots_file_name(&s.draft_settings.slots_file);
    let book = if name.is_empty() {
        t(keys::SETTINGS_SLOTS_FILE_EXAMPLE)
    } else {
        let candidates = list_slots_file_candidates(&s.config_dir);
        let map = build_mode_chord_map(&candidates, &s.draft_settings.mode_switch.keys);
        let code = map
            .iter()
            .find(|(_, f)| normalize_slots_file_name(f).eq_ignore_ascii_case(&name));
        let dn = slots_book_label(&s.config_dir, &name);
        match code {
            Some((c, _)) => format!("{c} · {dn}"),
            None => dn,
        }
    };
    set_text(
        s.lbl_slots_current,
        &tf(keys::SETTINGS_SLOTS_CURRENT, &[("book", &book)]),
    );
}

unsafe fn refresh_mode_current_combo(state_ptr: *mut SettingsState) {
    {
        let s = &mut *state_ptr;
        s.suppress_mode_current_change = true;
        let candidates = list_slots_file_candidates(&s.config_dir);
        let map = build_mode_chord_map(&candidates, &s.draft_settings.mode_switch.keys);
        let current = normalize_slots_file_name(&s.draft_settings.slots_file);
        let _ = SendMessageW(s.combo_mode_current, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        s.mode_current_file_choices.clear();
        let mut sel = -1i32;
        for i in 0..10usize {
            let Some(code) = chord_code_at(i) else {
                break;
            };
            let Some(file) = map.get(&code).map(|f| normalize_slots_file_name(f)) else {
                continue;
            };
            if file.is_empty() {
                continue;
            }
            let dn = slots_book_label(&s.config_dir, &file);
            let line = format!("{code} · {dn}");
            let w = to_wide(&line);
            let _ = SendMessageW(
                s.combo_mode_current,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(w.as_ptr() as isize),
            );
            if !current.is_empty() && file.eq_ignore_ascii_case(&current) {
                sel = s.mode_current_file_choices.len() as i32;
            }
            s.mode_current_file_choices.push(file);
        }
        if sel >= 0 {
            let _ = SendMessageW(
                s.combo_mode_current,
                CB_SETCURSEL,
                WPARAM(sel as usize),
                LPARAM(0),
            );
        }
        s.suppress_mode_current_change = false;
    }
    refresh_slots_current_label(state_ptr);
}

unsafe fn confirm_leave_slots_book(state_ptr: *mut SettingsState) -> bool {
    let s = &*state_ptr;
    let dirty = slot_form_dirty(state_ptr) || s.draft_slots != s.slots_baseline;
    if !dirty {
        return true;
    }
    let hwnd = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    let title: Vec<u16> = t(keys::SETTINGS_DIALOG_TITLE)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let body: Vec<u16> = t(keys::SETTINGS_MODE_SWITCH_DIRTY)
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let r = MessageBoxW(
        hwnd,
        PCWSTR(body.as_ptr()),
        PCWSTR(title.as_ptr()),
        MB_YESNOCANCEL | MB_ICONWARNING,
    );
    if r == IDYES {
        if slot_form_dirty(state_ptr) && !apply_slot_fields(state_ptr) {
            return false;
        }
        let s = &*state_ptr;
        let target_name = normalize_slots_file_name(&s.draft_settings.slots_file);
        if !slots_file_is_writable(&target_name) {
            return true;
        }
        if let Err(e) = save_slots(&s.config_dir, &target_name, &s.draft_slots) {
            msgbox(
                hwnd,
                &tf(
                    keys::SETTINGS_SAVE_SLOTS_FAILED,
                    &[("error", &e.to_string())],
                ),
                &t(keys::SETTINGS_DIALOG_TITLE),
                MB_OK | MB_ICONWARNING,
            );
            return false;
        }
        let baseline = s.draft_slots.clone();
        (*state_ptr).slots_baseline = baseline;
        true
    } else {
        r == IDNO
    }
}

unsafe fn switch_editing_book(state_ptr: *mut SettingsState, file: &str) {
    let want = normalize_slots_file_name(file);
    let cur = normalize_slots_file_name(&(*state_ptr).draft_settings.slots_file);
    if want.eq_ignore_ascii_case(&cur) {
        return;
    }
    if !confirm_leave_slots_book(state_ptr) {
        refresh_mode_current_combo(state_ptr);
        return;
    }
    {
        let s = &mut *state_ptr;
        s.draft_settings.slots_file = want.clone();
        match load_slots_for_settings(&s.config_dir, &want) {
            Ok((path, slots, writable)) => {
                s.draft_slots = slots.clone();
                s.slots_baseline = slots;
                s.slots_path_at_open = path;
                s.slots_writable = writable;
            }
            Err(e) => {
                error!("switch book failed: {e}");
                s.draft_slots = SlotRegistry::default();
                s.slots_baseline = SlotRegistry::default();
                s.slots_writable = false;
            }
        }
    }
    refresh_slot_list(state_ptr);
    let first = (*state_ptr).slot_ids.first().cloned();
    if let Some(id) = first {
        let _ = SendMessageW((*state_ptr).slot_list, LB_SETCURSEL, WPARAM(0), LPARAM(0));
        load_slot_into_fields(state_ptr, &id);
    } else {
        clear_slot_fields(state_ptr);
    }
    {
        let s = &*state_ptr;
        let want = normalize_slots_file_name(&s.draft_settings.slots_file);
        let idx = s
            .slots_file_choices
            .iter()
            .position(|c| c.eq_ignore_ascii_case(&want))
            .unwrap_or(0);
        let _ = SendMessageW(s.combo_slots_file, CB_SETCURSEL, WPARAM(idx), LPARAM(0));
    }
    update_slots_edit_enabled(state_ptr);
    refresh_mode_chord_list(state_ptr, None);
    sync_mode_file_combo_to_list_sel(state_ptr);
    refresh_mode_current_combo(state_ptr);
}

unsafe fn on_mode_current_sel(state_ptr: *mut SettingsState) {
    if (*state_ptr).suppress_mode_current_change {
        return;
    }
    let s = &*state_ptr;
    let idx = SendMessageW(s.combo_mode_current, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if idx < 0 {
        return;
    }
    let Some(file) = s.mode_current_file_choices.get(idx as usize).cloned() else {
        return;
    };
    switch_editing_book(state_ptr, &file);
}

unsafe fn on_mode_list_dblclk(state_ptr: *mut SettingsState) {
    let s = &*state_ptr;
    let Some(code) = selected_mode_chord_code(s) else {
        return;
    };
    let candidates = list_slots_file_candidates(&s.config_dir);
    let map = build_mode_chord_map(&candidates, &s.draft_settings.mode_switch.keys);
    let Some(file) = map.get(&code) else {
        return;
    };
    switch_editing_book(state_ptr, file);
}

fn key_set_label(id: &str) -> String {
    match id {
        KEY_SET_NUMPAD => t(keys::KEY_SET_NUMPAD),
        KEY_SET_KEYBOARD => t(keys::KEY_SET_KEYBOARD),
        other => other.to_string(),
    }
}

unsafe fn fill_az_combo(combo: HWND, selected: char) {
    const CB_RESETCONTENT: u32 = 0x014B;
    let _ = SendMessageW(combo, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
    let mut idx = 0i32;
    for (i, c) in ('a'..='z').enumerate() {
        let label = to_wide(&c.to_string());
        let _ = SendMessageW(
            combo,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(label.as_ptr() as isize),
        );
        if c == selected {
            idx = i as i32;
        }
    }
    let _ = SendMessageW(combo, CB_SETCURSEL, WPARAM(idx as usize), LPARAM(0));
}

unsafe fn fill_key_set_controls(state_ptr: *mut SettingsState) {
    let s = &mut *state_ptr;
    s.draft_settings.keys.normalize();
    s.suppress_key_set_change = true;
    const CB_RESETCONTENT: u32 = 0x014B;
    let _ = SendMessageW(s.combo_key_set, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
    let mut set_idx = 0i32;
    for (i, set) in s.draft_settings.keys.sets.iter().enumerate() {
        let label = to_wide(&key_set_label(&set.id));
        let _ = SendMessageW(
            s.combo_key_set,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(label.as_ptr() as isize),
        );
        if set.id == s.draft_settings.keys.active {
            set_idx = i as i32;
        }
    }
    let _ = SendMessageW(
        s.combo_key_set,
        CB_SETCURSEL,
        WPARAM(set_idx as usize),
        LPARAM(0),
    );

    fill_az_combo(
        s.combo_key_chord,
        s.draft_settings.keys.chord_for_id(KEY_SET_NUMPAD),
    );
    fill_az_combo(
        s.combo_key_chord_kb,
        s.draft_settings.keys.chord_for_id(KEY_SET_KEYBOARD),
    );

    set_text(s.lbl_wake, &s.draft_settings.keys.wake);
    s.suppress_key_set_change = false;
}

unsafe fn on_key_set_sel(state_ptr: *mut SettingsState) {
    let s = &mut *state_ptr;
    if s.suppress_key_set_change {
        return;
    }
    collect_live_key_fields(s);
    s.draft_settings.keys.sync_live_into_sets();
    let idx = SendMessageW(s.combo_key_set, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if idx < 0 {
        return;
    }
    let id = s
        .draft_settings
        .keys
        .sets
        .get(idx as usize)
        .map(|set| set.id.clone());
    let Some(id) = id else {
        return;
    };
    if !s.draft_settings.keys.activate(&id) {
        return;
    }
    update_key_labels(state_ptr);
    fill_key_set_controls(state_ptr);
}

unsafe fn on_mode_letter_sel(state_ptr: *mut SettingsState, set_id: &str, combo: HWND) {
    let s = &mut *state_ptr;
    if s.suppress_key_set_change {
        return;
    }
    let idx = SendMessageW(combo, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if idx < 0 {
        return;
    }
    let letter = (b'a' + idx as u8) as char;
    if let Err(e) = s.draft_settings.keys.set_chord_for_id(set_id, letter) {
        let hwnd = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
        msgbox(
            hwnd,
            &format_key_error(&e),
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONWARNING,
        );
        fill_key_set_controls(state_ptr);
    }
}

unsafe fn collect_live_key_fields(s: &mut SettingsState) {
    let start_l = window_text(s.edit_start_label);
    let confirm_l = window_text(s.edit_confirm_label);
    let cancel_l = window_text(s.edit_cancel_label);
    let search_l = window_text(s.edit_search_label);
    let open_l = window_text(s.edit_open_workdir_label);
    let back_l = window_text(s.edit_digit_back_label);
    let wake_on = get_check(s.chk_wake);
    let k = &mut s.draft_settings.keys;
    k.set_label(KeyRole::Start, &start_l);
    k.set_label(KeyRole::Confirm, &confirm_l);
    k.set_label(KeyRole::Cancel, &cancel_l);
    k.set_label(KeyRole::Search, &search_l);
    k.set_label(KeyRole::OpenWorkdir, &open_l);
    k.set_label(KeyRole::DigitBack, &back_l);
    k.wake_enabled = wake_on;
}

unsafe fn update_key_labels(state_ptr: *mut SettingsState) {
    let s = &*state_ptr;
    let k = &s.draft_settings.keys;
    set_text(s.lbl_start, &k.start);
    set_text(s.edit_start_label, &k.start_label);
    set_text(s.lbl_confirm, &k.confirm);
    set_text(s.edit_confirm_label, &k.confirm_label);
    set_text(s.lbl_cancel, &k.cancel);
    set_text(s.edit_cancel_label, &k.cancel_label);
    set_text(s.lbl_search, &k.search);
    set_text(s.edit_search_label, &k.search_label);
    set_text(s.lbl_open_workdir_key, &k.open_workdir);
    set_text(s.edit_open_workdir_label, &k.open_workdir_label);
    set_text(s.lbl_digit_back, &k.digit_back);
    set_text(s.edit_digit_back_label, &k.digit_back_label);
    set_text(s.lbl_wake, &k.wake);
}

unsafe fn load_slot_into_fields(state_ptr: *mut SettingsState, id: &str) {
    let s = &*state_ptr;
    let Some(slot) = s.draft_slots.get(id) else {
        return;
    };
    set_text(s.edit_id, &slot.id);
    set_text(s.edit_name, &slot.name);
    set_text(s.edit_path, &slot.path);
    set_text(s.edit_desc, &slot.description);
    set_text(s.edit_workdir, &slot.workdir);
    set_check(s.chk_open_workdir, slot.open_workdir);
    let focus_ok = !is_http_url(&slot.path) && !is_chain_path(&slot.path);
    let _ = EnableWindow(
        s.chk_focus_existing,
        slots_edit_allowed(state_ptr) && focus_ok,
    );
    set_check(
        s.chk_focus_existing,
        if focus_ok { slot.focus_existing } else { true },
    );
    let instant_ok = slots_edit_allowed(state_ptr) && is_instant_digit_id(id);
    let _ = EnableWindow(s.chk_instant_fire, instant_ok);
    if instant_ok {
        let on = s.draft_settings.instant.get(id).copied().unwrap_or(false);
        set_check(s.chk_instant_fire, on);
    } else {
        set_check(s.chk_instant_fire, false);
    }
}

unsafe fn clear_slot_fields(state_ptr: *mut SettingsState) {
    let s = &*state_ptr;
    set_text(s.edit_id, "");
    set_text(s.edit_name, "");
    set_text(s.edit_path, "");
    set_text(s.edit_desc, "");
    set_text(s.edit_workdir, "");
    set_check(s.chk_open_workdir, false);
    set_check(s.chk_focus_existing, true);
    set_check(s.chk_instant_fire, false);
    let _ = EnableWindow(s.chk_instant_fire, false);
}

fn msgbox(
    hwnd: HWND,
    text: &str,
    title: &str,
    flags: windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE,
) {
    let t: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let b: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = MessageBoxW(hwnd, PCWSTR(b.as_ptr()), PCWSTR(t.as_ptr()), flags);
    }
}

/// True when the right-pane form differs from the matching draft slot (or is a new draft).
unsafe fn slot_form_dirty(state_ptr: *mut SettingsState) -> bool {
    let s = &*state_ptr;
    let id = window_text(s.edit_id).trim().to_string();
    let name = window_text(s.edit_name).trim().to_string();
    let path = window_text(s.edit_path).trim().to_string();
    let description = window_text(s.edit_desc).trim().to_string();
    let workdir = window_text(s.edit_workdir).trim().to_string();
    let open_workdir = get_check(s.chk_open_workdir);
    let focus_existing = if is_http_url(&path) || is_chain_path(&path) {
        true
    } else {
        get_check(s.chk_focus_existing)
    };

    if id.is_empty()
        && name.is_empty()
        && path.is_empty()
        && description.is_empty()
        && workdir.is_empty()
        && !open_workdir
        && focus_existing
    {
        return false;
    }

    match s.draft_slots.get(&id) {
        Some(slot) => {
            slot.name != name
                || slot.path != path
                || slot.description != description
                || slot.workdir != workdir
                || slot.open_workdir != open_workdir
                || slot.focus_existing != focus_existing
        }
        None => true,
    }
}

unsafe fn apply_slot_fields(state_ptr: *mut SettingsState) -> bool {
    let hwnd = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    if !slots_edit_allowed(state_ptr) {
        msgbox(
            hwnd,
            &slots_edit_block_message(state_ptr),
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONWARNING,
        );
        return false;
    }
    let s = &mut *state_ptr;
    let id = window_text(s.edit_id).trim().to_string();
    let name = window_text(s.edit_name).trim().to_string();
    let path = window_text(s.edit_path).trim().to_string();
    let description = window_text(s.edit_desc).trim().to_string();
    let workdir = window_text(s.edit_workdir).trim().to_string();
    // chain: / URL slots ignore workdir open (spec). Keep the checkbox value only
    // when it applies so a later path edit does not surprise the user — still save
    // false for chain/URL.
    let open_workdir =
        get_check(s.chk_open_workdir) && !is_http_url(&path) && !is_chain_path(&path);
    // URL / chain never run focus-existing; force default true so JSON omits the field.
    let focus_existing = if is_http_url(&path) || is_chain_path(&path) {
        true
    } else {
        get_check(s.chk_focus_existing)
    };

    if !is_valid_slot_id(&id) {
        msgbox(
            hwnd,
            &t(keys::SETTINGS_SLOT_ID_DIGITS),
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONWARNING,
        );
        return false;
    }
    if name.is_empty() || path.is_empty() {
        msgbox(
            hwnd,
            &t(keys::SETTINGS_NAME_PATH_REQUIRED),
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONWARNING,
        );
        return false;
    }

    // Path existence warning for filesystem paths only (URLs / chain are not files).
    if !is_http_url(&path) && !is_chain_path(&path) {
        let path_obj = Path::new(&path);
        if path_obj.is_absolute() && !path_exists(path_obj) {
            msgbox(
                hwnd,
                &tf(keys::SETTINGS_PATH_MISSING, &[("path", &path)]),
                &t(keys::SETTINGS_DIALOG_TITLE),
                MB_OK | MB_ICONWARNING,
            );
        }
    }

    let proposed = Slot {
        id: id.clone(),
        name,
        path,
        description,
        workdir,
        open_workdir,
        focus_existing,
    };

    // Validate chains against the registry as if this upsert were committed.
    let mut probe = s.draft_slots.clone();
    if !probe.upsert(proposed.clone()) {
        return false;
    }
    if let Err(err) = validate_chains(probe.all()) {
        let text = match err {
            ChainBlockError::Invalid { slot_id, message } => tf(
                keys::SETTINGS_CHAIN_INVALID,
                &[("slot", &slot_id), ("message", &message)],
            ),
            ChainBlockError::Cycle { path } => {
                tf(keys::SETTINGS_CHAIN_CYCLE, &[("path", &path.join(" → "))])
            }
        };
        msgbox(
            hwnd,
            &text,
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONWARNING,
        );
        return false;
    }

    if !s.draft_slots.upsert(proposed) {
        return false;
    }
    if is_instant_digit_id(&id) {
        s.draft_settings
            .instant
            .insert(id.clone(), get_check(s.chk_instant_fire));
    }
    refresh_slot_list(state_ptr);
    update_conflict_warning(state_ptr);
    // Reselect and scroll into view so success is obvious.
    let s = &*state_ptr;
    if let Some(idx) = s.slot_ids.iter().position(|x| x == &id) {
        let _ = SendMessageW(s.slot_list, LB_SETCURSEL, WPARAM(idx), LPARAM(0));
        let _ = SendMessageW(s.slot_list, LB_SETTOPINDEX, WPARAM(idx), LPARAM(0));
    }
    true
}

unsafe fn swap_selected_slot_ids(state_ptr: *mut SettingsState) {
    let s = &mut *state_ptr;
    let hwnd = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    let sel = SendMessageW(s.slot_list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if sel < 0 {
        msgbox(
            hwnd,
            &t(keys::SETTINGS_SWAP_SELECT_FIRST),
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONWARNING,
        );
        return;
    }
    let Some(from) = s.slot_ids.get(sel as usize).cloned() else {
        return;
    };
    let to = window_text(s.edit_id).trim().to_string();
    if !is_valid_slot_id(&to) {
        msgbox(
            hwnd,
            &t(keys::SETTINGS_SLOT_ID_DIGITS),
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONWARNING,
        );
        return;
    }
    if from == to {
        msgbox(
            hwnd,
            &t(keys::SETTINGS_SWAP_NEED_OTHER),
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONWARNING,
        );
        return;
    }
    if s.draft_slots.get(&to).is_none() {
        msgbox(
            hwnd,
            &tf(keys::SETTINGS_SWAP_TARGET_MISSING, &[("id", &to)]),
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONWARNING,
        );
        return;
    }
    if !s.draft_slots.swap_ids(&from, &to) {
        return;
    }
    refresh_slot_list(state_ptr);
    update_conflict_warning(state_ptr);
    // After swap, the selected row's content moved to `to`.
    load_slot_into_fields(state_ptr, &to);
    let s = &*state_ptr;
    if let Some(idx) = s.slot_ids.iter().position(|x| x == &to) {
        let _ = SendMessageW(s.slot_list, LB_SETCURSEL, WPARAM(idx), LPARAM(0));
        let _ = SendMessageW(s.slot_list, LB_SETTOPINDEX, WPARAM(idx), LPARAM(0));
    }
}

unsafe fn collect_ui_into_draft(state_ptr: *mut SettingsState) -> Result<(), String> {
    {
        let s = &mut *state_ptr;
        collect_live_key_fields(s);
        s.draft_settings.keys.sync_live_into_sets();
    }
    legend_sync_hidden_from_checks(state_ptr, true);
    legend_sync_hidden_from_checks(state_ptr, false);
    let s = &mut *state_ptr;
    let mouse_idx = SendMessageW(s.combo_mouse, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    let button = match mouse_idx {
        1 => "x1",
        2 => "middle",
        _ => "x2",
    };
    set_mouse_trigger(&mut s.draft_settings, button, get_check(s.chk_suppress));
    set_hotkey_trigger(&mut s.draft_settings, window_text(s.edit_hotkey).trim());

    if let Err(e) = validate_key_bindings(&s.draft_settings.keys) {
        return Err(format_key_error(&e));
    }
    if let Err(e) = validate_display_labels(&s.draft_settings.keys) {
        return Err(format_key_error(&e));
    }
    if let Err(e) = validate_key_set_chords(&s.draft_settings.keys) {
        return Err(format_key_error(&e));
    }

    // Instant map is updated when chk_instant_fire / slot ID changes (apply / click).

    let mode_idx = SendMessageW(s.combo_search, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    s.draft_settings.search = SearchSettings {
        mode: match mode_idx {
            1 => SearchMode::Prefix,
            2 => SearchMode::Exact,
            3 => SearchMode::Fuzzy,
            _ => SearchMode::Substring,
        },
        case_sensitive: get_check(s.chk_case),
    };

    let timeout: u32 = window_text(s.edit_timeout)
        .trim()
        .parse()
        .map_err(|_| t(keys::SETTINGS_TIMEOUT_NUMBER))?;
    if timeout == 0 {
        return Err(t(keys::SETTINGS_TIMEOUT_MIN));
    }
    s.draft_settings.timeout_sec = timeout;

    let max_digits: usize = window_text(s.edit_max_digits)
        .trim()
        .parse()
        .map_err(|_| t(keys::SETTINGS_MAX_DIGITS_NUMBER))?;
    if max_digits == 0 || max_digits > 32 {
        return Err(t(keys::SETTINGS_MAX_DIGITS_RANGE));
    }
    s.draft_settings.max_digits = max_digits;

    let feedback_ms: u32 = window_text(s.edit_feedback_ms)
        .trim()
        .parse()
        .map_err(|_| t(keys::SETTINGS_FEEDBACK_MS_NUMBER))?;
    s.draft_settings.feedback_ms = feedback_ms;

    let log_idx = SendMessageW(s.combo_log, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    s.draft_settings.log_level = match log_idx {
        1 => "info".into(),
        2 => "debug".into(),
        _ => "warn".into(),
    };
    let loc_idx = SendMessageW(s.combo_locale, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    s.draft_settings.ui.locale = s
        .locale_choices
        .get(loc_idx as usize)
        .cloned()
        .unwrap_or_default();
    let sf_idx = SendMessageW(s.combo_slots_file, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    s.draft_settings.slots_file = s
        .slots_file_choices
        .get(sf_idx as usize)
        .cloned()
        .unwrap_or_default();
    s.draft_settings.backup_folder = backup_folder_store(&window_text(s.edit_backup_folder));
    s.draft_settings.autostart = get_check(s.chk_autostart);
    s.draft_settings.ui.show_advanced = get_check(s.chk_advanced);

    let mode_mouse_idx = SendMessageW(s.combo_mode_mouse, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    s.draft_settings.mode_switch.mouse_button = match mode_mouse_idx {
        1 => "x2".into(),
        2 => "middle".into(),
        _ => "x1".into(),
    };
    s.draft_settings.mode_switch.suppress = get_check(s.chk_mode_suppress);
    s.draft_settings.mode_switch.chord_timeout_ms = window_text(s.edit_mode_timeout)
        .trim()
        .parse()
        .unwrap_or(s.draft_settings.mode_switch.chord_timeout_ms);

    s.draft_settings.schema_version = 1;
    Ok(())
}

fn role_i18n(role: &str) -> String {
    match role {
        "start" => t(keys::ROLE_START),
        "confirm" => t(keys::ROLE_CONFIRM),
        "cancel" => t(keys::ROLE_CANCEL),
        "search" => t(keys::ROLE_SEARCH),
        "openWorkdir" => t(keys::ROLE_OPEN_WORKDIR),
        "digitBack" => t(keys::ROLE_DIGIT_BACK),
        "wake" => t(keys::ROLE_WAKE),
        other => other.to_string(),
    }
}

fn format_key_error(e: &KeyBindingError) -> String {
    match e {
        KeyBindingError::ReservedDigit { role } => {
            let role = role_i18n(role);
            tf(keys::SETTINGS_KEY_RESERVED_DIGIT, &[("role", &role)])
        }
        KeyBindingError::Duplicate {
            role_a,
            role_b,
            name,
        } => {
            let role_a = role_i18n(role_a);
            let role_b = role_i18n(role_b);
            tf(
                keys::SETTINGS_KEY_DUPLICATE,
                &[
                    ("role_a", &role_a),
                    ("role_b", &role_b),
                    ("name", name.as_str()),
                ],
            )
        }
        KeyBindingError::DuplicateDisplay {
            role_a,
            role_b,
            label,
        } => {
            let role_a = role_i18n(role_a);
            let role_b = role_i18n(role_b);
            tf(
                keys::SETTINGS_KEY_DISPLAY_DUPLICATE,
                &[
                    ("role_a", &role_a),
                    ("role_b", &role_b),
                    ("label", label.as_str()),
                ],
            )
        }
        KeyBindingError::UnknownName { role, name } => {
            let role = role_i18n(role);
            tf(
                keys::SETTINGS_KEY_UNKNOWN,
                &[("role", &role), ("name", name.as_str())],
            )
        }
        KeyBindingError::DuplicateChord {
            set_a,
            set_b,
            letter,
        } => tf(
            keys::SETTINGS_KEY_DUPLICATE_CHORD,
            &[
                ("set_a", &key_set_label(set_a)),
                ("set_b", &key_set_label(set_b)),
                ("letter", &letter.to_string()),
            ],
        ),
    }
}

unsafe fn save_draft(state_ptr: *mut SettingsState, close_after: bool) {
    let hwnd = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    // Phase 8 / A-4: commit an uncommitted slot form before writing disk.
    // Failure keeps the existing validation dialogs and aborts Save/Apply.
    // Read-only sample selection: skip slot commit (settings-only save).
    if slots_edit_allowed(state_ptr) && slot_form_dirty(state_ptr) && !apply_slot_fields(state_ptr)
    {
        return;
    }
    if let Err(e) = collect_ui_into_draft(state_ptr) {
        msgbox(
            hwnd,
            &e,
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONWARNING,
        );
        return;
    }
    let s = &*state_ptr;
    // Spec invariant: never overwrite a broken config file — user repairs by hand.
    if !s.settings_writable {
        msgbox(
            hwnd,
            &t(keys::SETTINGS_UNWRITABLE),
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONWARNING,
        );
        return;
    }
    if let Err(err) = validate_chains(s.draft_slots.all()) {
        let text = match err {
            ChainBlockError::Invalid { slot_id, message } => tf(
                keys::SETTINGS_CHAIN_INVALID,
                &[("slot", &slot_id), ("message", &message)],
            ),
            ChainBlockError::Cycle { path } => {
                tf(keys::SETTINGS_CHAIN_CYCLE, &[("path", &path.join(" → "))])
            }
        };
        msgbox(
            hwnd,
            &text,
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONWARNING,
        );
        return;
    }
    if let Err(e) = save_settings(&s.config_dir, &s.draft_settings) {
        error!("save settings failed: {e}");
        msgbox(
            hwnd,
            &tf(
                keys::SETTINGS_SAVE_SETTINGS_FAILED,
                &[("error", &e.to_string())],
            ),
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONWARNING,
        );
        return;
    }
    // Example / empty selection is read-only. Broken personal file: never overwrite.
    let target_name = normalize_slots_file_name(&s.draft_settings.slots_file);
    let mut slots_saved = false;
    if slots_file_is_writable(&target_name) {
        let target_path = resolve_slots_path(&s.config_dir, &target_name);
        if !s.slots_writable && target_path == s.slots_path_at_open {
            msgbox(
                hwnd,
                &t(keys::SETTINGS_SLOTS_UNWRITABLE),
                &t(keys::SETTINGS_DIALOG_TITLE),
                MB_OK | MB_ICONWARNING,
            );
            return;
        }
        if let Err(e) = save_slots(&s.config_dir, &target_name, &s.draft_slots) {
            error!("save slots failed: {e}");
            msgbox(
                hwnd,
                &tf(
                    keys::SETTINGS_SAVE_SLOTS_FAILED,
                    &[("error", &e.to_string())],
                ),
                &t(keys::SETTINGS_DIALOG_TITLE),
                MB_OK | MB_ICONWARNING,
            );
            return;
        }
        slots_saved = true;
    } else {
        info!("skipping slots save (sample / read-only selection)");
    }
    // Refresh active catalog so tray/menus pick up the new language without restart.
    let locale_changed = s.draft_settings.ui.locale != s.locale_at_open;
    let slots_file_changed = normalize_slots_file_name(&s.draft_settings.slots_file)
        != normalize_slots_file_name(&s.slots_file_at_open);
    let host = s.host;
    let page = s.page.as_u8();
    let new_baseline = if slots_saved {
        Some(s.draft_slots.clone())
    } else {
        None
    };
    {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        let os = super::locale::os_ui_language();
        let cat = i18n::resolve(
            &s.draft_settings.ui.locale,
            &i18n::lang_dir_next_to_exe(&exe_dir),
            os.as_deref(),
        );
        i18n::install(cat);
    }
    if let Some(baseline) = new_baseline {
        (*state_ptr).slots_baseline = baseline;
    }
    let _ = PostMessageW(host, WM_DK_SETTINGS_APPLIED, WPARAM(0), LPARAM(0));
    info!("settings saved");
    if close_after {
        // OK/Save: window closes; next open (and tray) already use the new catalog.
        let _ = DestroyWindow(hwnd);
    } else if locale_changed || slots_file_changed {
        // Apply + language / slots-file change: rebuild Settings from reloaded config.
        REOPEN_PAGE.store(page, Ordering::SeqCst);
        REOPEN_AFTER_APPLY.store(true, Ordering::SeqCst);
        let _ = DestroyWindow(hwnd);
    } else {
        msgbox(
            hwnd,
            &t(keys::SETTINGS_SAVED),
            &t(keys::SETTINGS_DIALOG_TITLE),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

unsafe fn capture_button_hwnd(s: &SettingsState, target: CaptureTarget) -> HWND {
    match target {
        CaptureTarget::Key(role) => key_role_controls(s, role).2,
        CaptureTarget::Wake => s.btn_cap_wake,
        CaptureTarget::Mouse => s.btn_cap_mouse,
    }
}

unsafe fn key_role_controls(s: &SettingsState, role: KeyRole) -> (HWND, HWND, HWND) {
    match role {
        KeyRole::Start => (s.lbl_start, s.edit_start_label, s.btn_cap_start),
        KeyRole::Confirm => (s.lbl_confirm, s.edit_confirm_label, s.btn_cap_confirm),
        KeyRole::Cancel => (s.lbl_cancel, s.edit_cancel_label, s.btn_cap_cancel),
        KeyRole::Search => (s.lbl_search, s.edit_search_label, s.btn_cap_search),
        KeyRole::OpenWorkdir => (
            s.lbl_open_workdir_key,
            s.edit_open_workdir_label,
            s.btn_cap_open_workdir,
        ),
        KeyRole::DigitBack => (
            s.lbl_digit_back,
            s.edit_digit_back_label,
            s.btn_cap_digit_back,
        ),
    }
}

unsafe fn sync_key_role_fields(s: &SettingsState, role: KeyRole) {
    let k = &s.draft_settings.keys;
    let (lbl, edit, _) = key_role_controls(s, role);
    set_text(lbl, k.vk_name(role));
    set_text(edit, k.label_raw(role));
}

unsafe fn refresh_capture_buttons(state_ptr: *mut SettingsState) {
    let s = &*state_ptr;
    let cap = t(keys::SETTINGS_CAPTURE);
    let cancel = t(keys::SETTINGS_CANCEL);
    for role in KeyRole::ALL {
        let (_, _, btn) = key_role_controls(s, role);
        let on = matches!(s.capturing, Some(CaptureTarget::Key(r)) if r == role);
        set_text(btn, if on { &cancel } else { &cap });
    }
    let wake_on = matches!(s.capturing, Some(CaptureTarget::Wake));
    set_text(s.btn_cap_wake, if wake_on { &cancel } else { &cap });
    let mouse_on = matches!(s.capturing, Some(CaptureTarget::Mouse));
    set_text(s.btn_cap_mouse, if mouse_on { &cancel } else { &cap });
}

unsafe fn on_capture_button(state_ptr: *mut SettingsState, target: CaptureTarget) {
    if (*state_ptr).capturing == Some(target) {
        stop_capture(state_ptr);
        return;
    }
    begin_capture(state_ptr, target);
}

unsafe fn begin_capture(state_ptr: *mut SettingsState, target: CaptureTarget) {
    stop_capture(state_ptr);
    let s = &mut *state_ptr;
    let hwnd = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
    CAPTURE_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
    s.capturing = Some(target);
    let _ = SetFocus(capture_button_hwnd(s, target));

    let module = match GetModuleHandleW(None) {
        Ok(m) => m,
        Err(e) => {
            error!("capture: GetModuleHandleW failed: {e}");
            s.capturing = None;
            return;
        }
    };

    match target {
        CaptureTarget::Key(_) | CaptureTarget::Wake => {
            set_text(s.capture_status, &t(keys::SETTINGS_CAPTURING_KEY));
            match SetWindowsHookExW(WH_KEYBOARD_LL, Some(capture_keyboard_proc), module, 0) {
                Ok(h) => s.capture.kb = Some(h),
                Err(e) => {
                    error!("capture keyboard hook failed: {e}");
                    s.capturing = None;
                    set_text(s.capture_status, &t(keys::SETTINGS_CAPTURE_FAILED));
                }
            }
        }
        CaptureTarget::Mouse => {
            set_text(s.capture_status, &t(keys::SETTINGS_CAPTURING_MOUSE));
            match SetWindowsHookExW(WH_MOUSE_LL, Some(capture_mouse_proc), module, 0) {
                Ok(h) => s.capture.mouse = Some(h),
                Err(e) => {
                    error!("capture mouse hook failed: {e}");
                    s.capturing = None;
                    set_text(s.capture_status, &t(keys::SETTINGS_CAPTURE_FAILED));
                }
            }
        }
    }
    refresh_capture_buttons(state_ptr);
}

unsafe fn stop_capture(state_ptr: *mut SettingsState) {
    let s = &mut *state_ptr;
    s.capture = CaptureHook::none();
    s.capturing = None;
    CAPTURE_HWND.store(0, Ordering::SeqCst);
    set_text(s.capture_status, "");
    refresh_capture_buttons(state_ptr);
}

unsafe extern "system" fn capture_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code == 0 && wparam.0 == WM_KEYDOWN as usize {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        if !info.flags.contains(LLKHF_INJECTED) {
            // Display edits keep their keys; do not assign them as bindings.
            if focused_is_edit() {
                return CallNextHookEx(None, code, wparam, lparam);
            }
            let hwnd = HWND(CAPTURE_HWND.load(Ordering::SeqCst) as *mut _);
            if !hwnd.0.is_null() {
                let _ = PostMessageW(
                    hwnd,
                    WM_DK_CAPTURE_KEY,
                    WPARAM(info.vkCode as usize),
                    LPARAM(0),
                );
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn capture_mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == 0 {
        let msg = wparam.0 as u32;
        // Never capture L/R — settings would become unusable (spec).
        if msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN {
            return CallNextHookEx(None, code, wparam, lparam);
        }
        let button: Option<u16> = if msg == WM_MBUTTONDOWN {
            Some(0) // middle sentinel
        } else if msg == WM_XBUTTONDOWN {
            let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
            let xbtn = ((info.mouseData >> 16) & 0xFFFF) as u16;
            if xbtn == XBUTTON1 {
                Some(1)
            } else if xbtn == XBUTTON2 {
                Some(2)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(b) = button {
            let hwnd = HWND(CAPTURE_HWND.load(Ordering::SeqCst) as *mut _);
            if !hwnd.0.is_null() {
                let _ = PostMessageW(hwnd, WM_DK_CAPTURE_MOUSE, WPARAM(b as usize), LPARAM(0));
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// True when keyboard focus is an Edit (Display labels, timeout, etc.).
/// Capture must not swallow those keystrokes — they are display/text, not bindings.
unsafe fn focused_is_edit() -> bool {
    let hwnd = GetFocus();
    if hwnd.0.is_null() {
        return false;
    }
    let mut buf = [0u16; 16];
    let n = GetClassNameW(hwnd, &mut buf);
    if n <= 0 {
        return false;
    }
    let class = String::from_utf16_lossy(&buf[..n as usize]);
    class.eq_ignore_ascii_case("Edit")
}

unsafe fn on_capture_key(state_ptr: *mut SettingsState, vk: u16) {
    let s = &mut *state_ptr;
    s.draft_settings.keys.wake_enabled = get_check(s.chk_wake);
    match s.capturing {
        Some(CaptureTarget::Wake) => {
            let mut trial = s.draft_settings.keys.clone();
            match apply_captured_wake(&mut trial, vk) {
                Ok(()) => {
                    s.draft_settings.keys = trial;
                    set_text(s.lbl_wake, &s.draft_settings.keys.wake);
                    stop_capture(state_ptr);
                }
                Err(e) => {
                    stop_capture(state_ptr);
                    let hwnd = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
                    msgbox(
                        hwnd,
                        &format_key_error(&e),
                        &t(keys::SETTINGS_DIALOG_TITLE),
                        MB_OK | MB_ICONWARNING,
                    );
                }
            }
        }
        Some(CaptureTarget::Key(role)) => {
            let mut trial = s.draft_settings.keys.clone();
            match apply_captured_vk(&mut trial, role, vk) {
                Ok(swapped) => {
                    s.draft_settings.keys = trial;
                    sync_key_role_fields(s, role);
                    if let Some(other) = swapped {
                        sync_key_role_fields(s, other);
                    }
                    stop_capture(state_ptr);
                }
                Err(e) => {
                    stop_capture(state_ptr);
                    let hwnd = HWND(SETTINGS_HWND.load(Ordering::SeqCst) as *mut _);
                    msgbox(
                        hwnd,
                        &format_key_error(&e),
                        &t(keys::SETTINGS_DIALOG_TITLE),
                        MB_OK | MB_ICONWARNING,
                    );
                }
            }
        }
        _ => stop_capture(state_ptr),
    }
}

unsafe fn on_capture_mouse(state_ptr: *mut SettingsState, code: usize) {
    let s = &mut *state_ptr;
    let button = match code {
        0 => "middle",
        1 => "x1",
        _ => "x2",
    };
    let suppress = get_check(s.chk_suppress);
    set_mouse_trigger(&mut s.draft_settings, button, suppress);
    let idx = match button {
        "x1" => 1,
        "middle" => 2,
        _ => 0,
    };
    let _ = SendMessageW(s.combo_mouse, CB_SETCURSEL, WPARAM(idx), LPARAM(0));
    stop_capture(state_ptr);
}

unsafe extern "system" fn settings_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SettingsState;

    match msg {
        WM_DK_CAPTURE_KEY => {
            if !state_ptr.is_null() {
                on_capture_key(state_ptr, wparam.0 as u16);
            }
            LRESULT(0)
        }
        WM_DK_CAPTURE_MOUSE => {
            if !state_ptr.is_null() {
                on_capture_mouse(state_ptr, wparam.0);
            }
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            // Warning strip under "Update slot" — white, not dialog gray.
            // Return 0 for everything else so the page proc can supply COLOR_WINDOW.
            if !state_ptr.is_null() {
                let ctl = HWND(lparam.0 as *mut _);
                if ctl == (*state_ptr).warn_label {
                    let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut _);
                    let _ = SetBkColor(hdc, COLORREF(0x00FF_FFFF));
                    let _ = SetTextColor(hdc, COLORREF(0x0000_0000));
                    return LRESULT(GetStockObject(WHITE_BRUSH).0 as isize);
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            if state_ptr.is_null() {
                return LRESULT(0);
            }
            let notify = ((wparam.0 >> 16) & 0xFFFF) as u16;
            let id = (wparam.0 & 0xFFFF) as isize;

            // Typing a Display label (or any edit) must end Capture so digits
            // are not treated as Search/Start bindings.
            if notify == EN_SETFOCUS {
                stop_capture(state_ptr);
            }

            if notify == BN_CLICKED || notify == 0 {
                match id {
                    IDC_TAB_SLOTS => show_page(state_ptr, Page::Slots),
                    IDC_TAB_KEYS => show_page(state_ptr, Page::Keys),
                    IDC_TAB_TRIGGERS => show_page(state_ptr, Page::Triggers),
                    IDC_TAB_SEARCH => show_page(state_ptr, Page::Search),
                    IDC_TAB_MODE => show_page(state_ptr, Page::Mode),
                    IDC_TAB_GENERAL => show_page(state_ptr, Page::General),
                    IDC_ADVANCED => {
                        let s = &mut *state_ptr;
                        s.draft_settings.ui.show_advanced = get_check(s.chk_advanced);
                        if !s.draft_settings.ui.show_advanced && s.page.is_advanced() {
                            s.page = Page::Slots;
                        }
                        let mut pt = POINT::default();
                        let _ = GetCursorPos(&mut pt);
                        fit_settings_window_to_content(hwnd, s, pt);
                    }
                    IDC_SLOT_ADD => {
                        if !slots_edit_allowed(state_ptr) {
                            msgbox(
                                hwnd,
                                &slots_edit_block_message(state_ptr),
                                &t(keys::SETTINGS_DIALOG_TITLE),
                                MB_OK | MB_ICONWARNING,
                            );
                        } else {
                            clear_slot_fields(state_ptr);
                            let _ = SetFocus((*state_ptr).edit_id);
                        }
                    }
                    IDC_SLOT_DELETE => {
                        if !slots_edit_allowed(state_ptr) {
                            msgbox(
                                hwnd,
                                &slots_edit_block_message(state_ptr),
                                &t(keys::SETTINGS_DIALOG_TITLE),
                                MB_OK | MB_ICONWARNING,
                            );
                        } else {
                            let s = &mut *state_ptr;
                            let sel =
                                SendMessageW(s.slot_list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
                            if sel >= 0 {
                                if let Some(id) = s.slot_ids.get(sel as usize).cloned() {
                                    s.draft_slots.remove(&id);
                                    refresh_slot_list(state_ptr);
                                    update_conflict_warning(state_ptr);
                                    clear_slot_fields(state_ptr);
                                }
                            }
                        }
                    }
                    IDC_SLOT_SWAP => {
                        if !slots_edit_allowed(state_ptr) {
                            msgbox(
                                hwnd,
                                &slots_edit_block_message(state_ptr),
                                &t(keys::SETTINGS_DIALOG_TITLE),
                                MB_OK | MB_ICONWARNING,
                            );
                        } else {
                            swap_selected_slot_ids(state_ptr);
                        }
                    }
                    IDC_SLOT_INSTANT => {
                        let s = &mut *state_ptr;
                        let sid = window_text(s.edit_id).trim().to_string();
                        if is_instant_digit_id(&sid) {
                            s.draft_settings
                                .instant
                                .insert(sid, get_check(s.chk_instant_fire));
                        }
                    }
                    IDC_SLOT_APPLY => {
                        let _ = apply_slot_fields(state_ptr);
                    }
                    IDC_CAP_START => {
                        on_capture_button(state_ptr, CaptureTarget::Key(KeyRole::Start))
                    }
                    IDC_CAP_CONFIRM => {
                        on_capture_button(state_ptr, CaptureTarget::Key(KeyRole::Confirm))
                    }
                    IDC_CAP_CANCEL => {
                        on_capture_button(state_ptr, CaptureTarget::Key(KeyRole::Cancel))
                    }
                    IDC_CAP_SEARCH => {
                        on_capture_button(state_ptr, CaptureTarget::Key(KeyRole::Search))
                    }
                    IDC_CAP_OPEN_WORKDIR => {
                        on_capture_button(state_ptr, CaptureTarget::Key(KeyRole::OpenWorkdir))
                    }
                    IDC_CAP_DIGIT_BACK => {
                        on_capture_button(state_ptr, CaptureTarget::Key(KeyRole::DigitBack))
                    }
                    IDC_CAP_WAKE => on_capture_button(state_ptr, CaptureTarget::Wake),
                    IDC_CAP_MOUSE => on_capture_button(state_ptr, CaptureTarget::Mouse),
                    IDC_WAKE => {
                        let s = &mut *state_ptr;
                        s.draft_settings.keys.wake_enabled = get_check(s.chk_wake);
                    }
                    IDC_RESET_KEYS => {
                        (*state_ptr).draft_settings.keys.reset_current_set();
                        populate_from_draft(state_ptr);
                        set_text((*state_ptr).capture_status, &t(keys::SETTINGS_RESET_STATUS));
                    }
                    IDC_RESET_TRIGGERS => {
                        let shipped = default_shipped_settings();
                        let s = &mut *state_ptr;
                        s.draft_settings.triggers = shipped.triggers;
                        let books = s.draft_settings.mode_switch.keys.clone();
                        s.draft_settings.mode_switch = shipped.mode_switch;
                        s.draft_settings.mode_switch.keys = books;
                        s.draft_settings.keys.reset_switch_chords();
                        populate_from_draft(state_ptr);
                        set_text(
                            (*state_ptr).lbl_triggers_status,
                            &t(keys::SETTINGS_RESET_TRIGGERS_STATUS),
                        );
                    }
                    IDC_MODE_SET_OVERRIDE => set_mode_override(state_ptr),
                    IDC_MODE_CLEAR_OVERRIDE => clear_mode_override(state_ptr),
                    IDC_LEGEND_IDLE_UP => legend_move_sel(state_ptr, true, -1),
                    IDC_LEGEND_IDLE_DOWN => legend_move_sel(state_ptr, true, 1),
                    IDC_LEGEND_MULTI_UP => legend_move_sel(state_ptr, false, -1),
                    IDC_LEGEND_MULTI_DOWN => legend_move_sel(state_ptr, false, 1),
                    IDC_OPEN_CONFIG_FOLDER => open_config_folder(&(*state_ptr).config_dir),
                    IDC_BACKUP_BROWSE => browse_backup_folder(state_ptr),
                    IDC_BACKUP_CONFIG => backup_config_folder(state_ptr),
                    IDC_BACKUP_RESTORE => restore_config_folder(state_ptr),
                    IDC_SAVE => save_draft(state_ptr, true),
                    IDC_APPLY => save_draft(state_ptr, false),
                    IDC_CANCEL => {
                        let _ = DestroyWindow(hwnd);
                    }
                    _ => {
                        if id >= IDC_LEGEND_IDLE_0
                            && id < IDC_LEGEND_IDLE_0 + LEGEND_ITEM_COUNT as isize
                        {
                            (*state_ptr).legend_idle_sel = (id - IDC_LEGEND_IDLE_0) as usize;
                            legend_sync_hidden_from_checks(state_ptr, true);
                        } else if id >= IDC_LEGEND_MULTI_0
                            && id < IDC_LEGEND_MULTI_0 + LEGEND_ITEM_COUNT as isize
                        {
                            (*state_ptr).legend_multi_sel = (id - IDC_LEGEND_MULTI_0) as usize;
                            legend_sync_hidden_from_checks(state_ptr, false);
                        }
                    }
                }
            }
            if id == IDC_SLOT_LIST && notify == LBN_SELCHANGE {
                let s = &*state_ptr;
                let sel = SendMessageW(s.slot_list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
                if sel >= 0 {
                    if let Some(id) = s.slot_ids.get(sel as usize) {
                        load_slot_into_fields(state_ptr, id);
                    }
                }
            }
            if id == IDC_MODE_CHORD_LIST && notify == LBN_SELCHANGE {
                sync_mode_file_combo_to_list_sel(state_ptr);
            }
            if id == IDC_MODE_CHORD_LIST && notify == LBN_DBLCLK {
                on_mode_list_dblclk(state_ptr);
            }
            if id == IDC_MODE_CURRENT && notify == CBN_SELCHANGE {
                on_mode_current_sel(state_ptr);
            }
            if id == IDC_SLOTS_FILE && notify == CBN_SELCHANGE {
                update_slots_edit_enabled(state_ptr);
            }
            if id == IDC_KEY_SET && notify == CBN_SELCHANGE {
                on_key_set_sel(state_ptr);
            }
            if id == IDC_KEY_CHORD && notify == CBN_SELCHANGE {
                let combo = (*state_ptr).combo_key_chord;
                on_mode_letter_sel(state_ptr, KEY_SET_NUMPAD, combo);
            }
            if id == IDC_MODE_LETTER_KEYBOARD && notify == CBN_SELCHANGE {
                let combo = (*state_ptr).combo_key_chord_kb;
                on_mode_letter_sel(state_ptr, KEY_SET_KEYBOARD, combo);
            }
            if id == IDC_SLOT_ID && notify == EN_CHANGE {
                update_slots_edit_enabled(state_ptr);
            }
            if id == IDC_SLOT_PATH && notify == EN_CHANGE {
                update_slots_edit_enabled(state_ptr);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            if !state_ptr.is_null() {
                stop_capture(state_ptr);
            }
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            // Never force a minimum larger than the monitor work area — that was
            // pushing Settings below the taskbar on 1280×720 (~672px work height).
            let dpi = GetDpiForWindow(hwnd).max(96);
            let mut min_client_w = SETTINGS_MIN_CLIENT_W;
            if !state_ptr.is_null()
                && !(*state_ptr).btn_save.0.is_null()
                && !(*state_ptr).btn_slot_apply.0.is_null()
            {
                min_client_w = min_client_w.max(footer_row_min_client_w(hwnd, &*state_ptr));
            }
            let (mut min_w, mut min_h) = outer_size_for_client(
                scale_for_dpi(min_client_w, dpi),
                scale_for_dpi(SETTINGS_MIN_CLIENT_H, dpi),
                dpi,
            );
            let info = lparam.0 as *mut MINMAXINFO;
            if !info.is_null() {
                if let Some(wa) = work_area_for_hwnd(hwnd) {
                    let max_w = (wa.right - wa.left - 8).max(320);
                    let max_h = (wa.bottom - wa.top - 8).max(240);
                    min_w = min_w.min(max_w);
                    min_h = min_h.min(max_h);
                    (*info).ptMinTrackSize = POINT { x: min_w, y: min_h };
                    (*info).ptMaxTrackSize = POINT { x: max_w, y: max_h };
                } else {
                    (*info).ptMinTrackSize = POINT { x: min_w, y: min_h };
                }
            }
            LRESULT(0)
        }
        WM_DPICHANGED => {
            // Suggested size/position from the system for the new DPI.
            let suggested = lparam.0 as *const RECT;
            if !suggested.is_null() {
                let r = *suggested;
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOP,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            if !state_ptr.is_null() {
                layout_settings(hwnd, &*state_ptr);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            if !state_ptr.is_null() {
                layout_settings(hwnd, &*state_ptr);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            if !state_ptr.is_null() {
                stop_capture(state_ptr);
                let state = Box::from_raw(state_ptr);
                // Panels parked on HWND_MESSAGE are not destroyed with this dialog.
                for p in [
                    state.panel_slots,
                    state.panel_keys,
                    state.panel_triggers,
                    state.panel_search,
                    state.panel_mode,
                    state.panel_general,
                ] {
                    if p.0.is_null() {
                        continue;
                    }
                    if let Ok(parent) = GetParent(p) {
                        if parent == HWND_MESSAGE {
                            let _ = DestroyWindow(p);
                        }
                    }
                }
                SETTINGS_HWND.store(0, Ordering::SeqCst);
                set_triggers_disabled(false);
                let _ = PostMessageW(state.host, WM_DK_SETTINGS_CLOSED, WPARAM(0), LPARAM(0));
                info!("settings closed");
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
