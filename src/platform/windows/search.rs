//! Focused Search window (IME-capable). No keyboard hook.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use tracing::{error, info, warn};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{
    GetLastError, ERROR_CLASS_ALREADY_EXISTS, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    GetStockObject, GetSysColorBrush, ScreenToClient, COLOR_WINDOW, DEFAULT_GUI_FONT, HBRUSH, HFONT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetCursorPos,
    GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, LoadCursorW, MoveWindow, PostMessageW,
    RegisterClassExW, SendMessageW, SetWindowLongPtrW, SetWindowTextW, ShowWindow, GWLP_USERDATA,
    GWLP_WNDPROC, IDC_ARROW, SW_SHOWNORMAL, WINDOW_EX_STYLE, WINDOW_STYLE, WM_COMMAND,
    WM_DESTROY, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONUP, WM_SETFONT, WM_SIZE, WNDCLASSEXW, WNDPROC,
    WS_BORDER, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
    WS_TABSTOP, WS_THICKFRAME, WS_VISIBLE, WS_VSCROLL,
};

use crate::core::config::AppConfig;
use crate::core::i18n::{keys as i18n_keys, tf};
use crate::core::search::search_slots;
use crate::core::slots::Slot;

use super::icon::load_class_icons;
use super::launch_queue;
use super::messages::{SEARCH_CLASS, WM_DK_LAUNCH_FAILED, WM_DK_SEARCH_CLOSED};

const IDC_EDIT: isize = 100;
const IDC_LIST: isize = 101;
const IDC_HINT: isize = 102;
const EN_CHANGE: u16 = 0x0300;
const LBN_DBLCLK: u16 = 2;
const ES_AUTOHSCROLL: u32 = 0x0080;
const LBS_NOTIFY: u32 = 0x0001;
const VK_UP: u32 = 0x26;
const VK_DOWN: u32 = 0x28;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;

// LISTBOX messages
const LB_ADDSTRING: u32 = 0x0180;
const LB_RESETCONTENT: u32 = 0x0184;
const LB_SETCURSEL: u32 = 0x0186;
const LB_GETCURSEL: u32 = 0x0188;
const LB_GETCOUNT: u32 = 0x018B;
const LB_ITEMFROMPOINT: u32 = 0x01A9;

static SEARCH_HWND: AtomicIsize = AtomicIsize::new(0);
static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);
static SEARCH_LAUNCHING: AtomicBool = AtomicBool::new(false);

struct SearchState {
    host: HWND,
    config: AppConfig,
    exe_dir: PathBuf,
    edit: HWND,
    list: HWND,
    hint: HWND,
    edit_prev: Option<WNDPROC>,
    list_prev: Option<WNDPROC>,
    /// Slot ids currently shown in the listbox, in order.
    visible_ids: Vec<String>,
    /// Resolved `keys.openWorkdir` VK (same role as the typing window).
    owd_vk: u16,
    /// True from that VK's keydown until either another key interrupts it
    /// (used as a modifier — e.g. Ctrl+A) or it fires on keyup (tap-alone).
    owd_pending: bool,
}

pub fn is_open() -> bool {
    !HWND(SEARCH_HWND.load(Ordering::SeqCst) as *mut _)
        .0
        .is_null()
}

/// Close the Search if it is open (e.g. before opening typing mode / reload).
pub fn close_if_open() {
    let hwnd = HWND(SEARCH_HWND.load(Ordering::SeqCst) as *mut _);
    if !hwnd.0.is_null() {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }
}

pub fn open(host: HWND, config: &AppConfig, exe_dir: &std::path::Path) -> anyhow::Result<()> {
    let existing = HWND(SEARCH_HWND.load(Ordering::SeqCst) as *mut _);
    if !existing.0.is_null() {
        unsafe {
            let _ = super::focus_existing::bring_window_front(existing);
        }
        return Ok(());
    }

    SEARCH_LAUNCHING.store(false, Ordering::SeqCst);

    unsafe {
        ensure_class()?;
        let instance = GetModuleHandleW(None)?;

        let width = 480i32;
        let height = 420i32;
        // Spec §9: primary bottom-right, window bottom-right anchored.
        let (x, y) = super::placement::search_window_pos(width, height);

        let title: Vec<u16> = crate::core::i18n::tf(
            crate::core::i18n::keys::SEARCH_WINDOW_TITLE,
            &[("version", env!("CARGO_PKG_VERSION"))],
        )
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            SEARCH_CLASS,
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPED
                | WS_CAPTION
                | WS_SYSMENU
                | WS_THICKFRAME
                | WS_MINIMIZEBOX
                | WS_CLIPCHILDREN
                | WS_VISIBLE,
            x,
            y,
            width,
            height,
            None,
            None,
            instance,
            None,
        )?;

        let owd_vk = config.settings.keys.resolved().open_workdir;
        let state = Box::new(SearchState {
            host,
            config: config.clone(),
            exe_dir: exe_dir.to_path_buf(),
            edit: HWND::default(),
            list: HWND::default(),
            hint: HWND::default(),
            edit_prev: None,
            list_prev: None,
            visible_ids: Vec::new(),
            owd_vk,
            owd_pending: false,
        });
        let state_ptr = Box::into_raw(state);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);

        create_children(hwnd, state_ptr)?;
        refresh_list(state_ptr);

        SEARCH_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
        let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
        super::focus_existing::bring_window_front(hwnd);
        let _ = SetFocus((*state_ptr).edit);

        info!("Search opened");
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
        lpfnWndProc: Some(search_proc),
        hInstance: instance.into(),
        lpszClassName: SEARCH_CLASS,
        hCursor: LoadCursorW(None, IDC_ARROW)?,
        hIcon: app_icon,
        hIconSm: app_icon_sm,
        hbrBackground: HBRUSH(GetSysColorBrush(COLOR_WINDOW).0),
        ..Default::default()
    };
    if RegisterClassExW(&wc) == 0 {
        let err = GetLastError();
        if err != ERROR_CLASS_ALREADY_EXISTS {
            anyhow::bail!("RegisterClassExW(search) failed: {err:?}");
        }
    }
    CLASS_REGISTERED.store(true, Ordering::SeqCst);
    Ok(())
}

unsafe fn create_children(parent: HWND, state_ptr: *mut SearchState) -> anyhow::Result<()> {
    let instance = GetModuleHandleW(None)?;
    let mut rc = RECT::default();
    let _ = GetClientRect(parent, &mut rc);

    let edit_style =
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_BORDER.0 | WS_TABSTOP.0 | ES_AUTOHSCROLL);
    let pad = 12i32;
    let edit_h = 26i32;
    let edit = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("EDIT"),
        w!(""),
        edit_style,
        pad,
        pad,
        (rc.right - pad * 2).max(40),
        edit_h,
        parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_EDIT as _),
        instance,
        None,
    )?;

    let hint_h = 18i32;
    let list_style = WINDOW_STYLE(
        WS_CHILD.0 | WS_VISIBLE.0 | WS_BORDER.0 | WS_VSCROLL.0 | WS_TABSTOP.0 | LBS_NOTIFY,
    );
    let list_top = pad + edit_h + 10;
    let list_h = (rc.bottom - list_top - pad - hint_h - 6).max(40);
    let list = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("LISTBOX"),
        w!(""),
        list_style,
        pad,
        list_top,
        (rc.right - pad * 2).max(40),
        list_h,
        parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_LIST as _),
        instance,
        None,
    )?;

    let hint_style = WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0);
    let hint_y = list_top + list_h + 6;
    let hint = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!(""),
        hint_style,
        pad,
        hint_y,
        (rc.right - pad * 2).max(40),
        hint_h,
        parent,
        windows::Win32::UI::WindowsAndMessaging::HMENU(IDC_HINT as _),
        instance,
        None,
    )?;

    let font = HFONT(GetStockObject(DEFAULT_GUI_FONT).0);
    let _ = SendMessageW(edit, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    let _ = SendMessageW(list, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    let _ = SendMessageW(hint, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));

    let state = &mut *state_ptr;
    state.edit = edit;
    state.list = list;
    state.hint = hint;
    set_hint_text(hint, state.owd_vk, &state.config);

    let prev_edit = SetWindowLongPtrW(
        edit,
        GWLP_WNDPROC,
        edit_subclass as *const () as usize as isize,
    );
    state.edit_prev = Some(std::mem::transmute::<isize, WNDPROC>(prev_edit));
    SetWindowLongPtrW(edit, GWLP_USERDATA, state_ptr as isize);

    let prev_list = SetWindowLongPtrW(
        list,
        GWLP_WNDPROC,
        list_subclass as *const () as usize as isize,
    );
    state.list_prev = Some(std::mem::transmute::<isize, WNDPROC>(prev_list));
    SetWindowLongPtrW(list, GWLP_USERDATA, state_ptr as isize);
    Ok(())
}

/// "{key}: open folder" using the same display label as the typing window's
/// Open-workdir legend (`keys.openWorkdirLabel`, default short VK name).
unsafe fn set_hint_text(hint: HWND, _owd_vk: u16, config: &AppConfig) {
    let label = config
        .settings
        .keys
        .display_label(crate::core::keys::KeyRole::OpenWorkdir);
    let text = tf(i18n_keys::SEARCH_HINT_OPEN_WORKDIR, &[("key", &label)]);
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = SetWindowTextW(hint, PCWSTR(wide.as_ptr()));
}

unsafe fn refresh_list(state_ptr: *mut SearchState) {
    let state = &mut *state_ptr;
    let query = window_text(state.edit);
    let hits: Vec<&Slot> = search_slots(
        state.config.slots.all(),
        &query,
        &state.config.settings.search,
    );
    state.visible_ids = hits.iter().map(|s| s.id.clone()).collect();

    let _ = SendMessageW(state.list, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
    for slot in &hits {
        let line = if slot.description.is_empty() {
            format!("{}  {}", slot.id, slot.name)
        } else {
            format!("{}  {} — {}", slot.id, slot.name, slot.description)
        };
        let wide: Vec<u16> = line.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = SendMessageW(
            state.list,
            LB_ADDSTRING,
            WPARAM(0),
            LPARAM(wide.as_ptr() as isize),
        );
    }
    if !hits.is_empty() {
        let _ = SendMessageW(state.list, LB_SETCURSEL, WPARAM(0), LPARAM(0));
    }
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

/// Resolve the current target: an exact id match in the query box first,
/// otherwise the highlighted listbox row.
unsafe fn resolve_selection(state_ptr: *mut SearchState) -> Option<Slot> {
    let state = &*state_ptr;
    let query = window_text(state.edit);
    let query = query.trim();

    if let Some(slot) = state.config.slots.get(query) {
        return Some(slot.clone());
    }

    let sel = SendMessageW(state.list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if sel < 0 {
        return None;
    }
    let id = state.visible_ids.get(sel as usize)?;
    state.config.slots.get(id).cloned()
}

unsafe fn launch_selection(state_ptr: *mut SearchState) {
    match resolve_selection(state_ptr) {
        Some(slot) => finish_launch(state_ptr, &slot, false),
        None => warn!("Search: nothing selected"),
    }
}

/// Open the target's working folder only (registered `workdir`, else the
/// Path's parent). Same role/behaviour as the typing window's Open-workdir
/// key (`keys.openWorkdir`) — URL / `chain:` have no folder and just fail.
unsafe fn open_selection_workdir(state_ptr: *mut SearchState) {
    match resolve_selection(state_ptr) {
        Some(slot) => finish_launch(state_ptr, &slot, true),
        None => warn!("Search: nothing selected (open folder)"),
    }
}

unsafe fn finish_launch(state_ptr: *mut SearchState, slot: &Slot, folder_only: bool) {
    if SEARCH_LAUNCHING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    let state = &*state_ptr;
    let slot = slot.clone();
    let exe_dir = state.exe_dir.clone();
    let registry = state.config.slots.clone();
    let host = state.host;
    let hwnd = HWND(SEARCH_HWND.load(Ordering::SeqCst) as *mut _);
    if !hwnd.0.is_null() {
        let _ = DestroyWindow(hwnd);
    }
    if let Err(e) = launch_queue::queue(slot.clone(), exe_dir, registry, folder_only, host) {
        error!(slot_id = %slot.id, folder_only, "Search launch queue failed: {e}");
        let _ = PostMessageW(host, WM_DK_LAUNCH_FAILED, WPARAM(0), LPARAM(0));
    } else {
        info!(slot_id = %slot.id, folder_only, "Search launch queued");
    }
}

/// Hit-test list item under the cursor (client coords via ScreenToClient).
unsafe fn list_item_under_cursor(list: HWND) -> Option<usize> {
    let mut pt = POINT::default();
    if GetCursorPos(&mut pt).is_err() {
        return None;
    }
    if !ScreenToClient(list, &mut pt).as_bool() {
        return None;
    }
    let packed = ((pt.y as u16 as u32) << 16) | (pt.x as u16 as u32);
    let result = SendMessageW(list, LB_ITEMFROMPOINT, WPARAM(0), LPARAM(packed as isize)).0 as u32;
    let outside = (result >> 16) & 0xFFFF;
    if outside != 0 {
        return None;
    }
    Some((result & 0xFFFF) as usize)
}

unsafe extern "system" fn list_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SearchState;
    let prev = if state_ptr.is_null() {
        None
    } else {
        (*state_ptr).list_prev
    };

    // Spec: click launches. Let the listbox update selection first, then run.
    if !state_ptr.is_null() && msg == WM_LBUTTONUP {
        let result = match prev {
            Some(p) => CallWindowProcW(p, hwnd, msg, wparam, lparam),
            None => DefWindowProcW(hwnd, msg, wparam, lparam),
        };
        if let Some(idx) = list_item_under_cursor(hwnd) {
            let _ = SendMessageW(hwnd, LB_SETCURSEL, WPARAM(idx), LPARAM(0));
            launch_selection(state_ptr);
        }
        return result;
    }

    match prev {
        Some(p) => CallWindowProcW(p, hwnd, msg, wparam, lparam),
        None => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn edit_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SearchState;
    if !state_ptr.is_null() && msg == WM_KEYDOWN {
        let vk = wparam.0 as u32;
        let owd_vk = (*state_ptr).owd_vk;
        // Tap-alone tracking for the Open-workdir role key (same role as the
        // typing window's Ctrl). If any other key arrives first, this VK was
        // held as a modifier (Ctrl+A / Ctrl+C / …) — do not fire on its keyup.
        (*state_ptr).owd_pending = vk as u16 == owd_vk;
        let list = (*state_ptr).list;
        match vk {
            VK_UP => {
                let sel = SendMessageW(list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
                if sel > 0 {
                    let _ = SendMessageW(list, LB_SETCURSEL, WPARAM((sel - 1) as usize), LPARAM(0));
                }
                return LRESULT(0);
            }
            VK_DOWN => {
                let sel = SendMessageW(list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
                let count = SendMessageW(list, LB_GETCOUNT, WPARAM(0), LPARAM(0)).0;
                if sel + 1 < count {
                    let _ = SendMessageW(list, LB_SETCURSEL, WPARAM((sel + 1) as usize), LPARAM(0));
                }
                return LRESULT(0);
            }
            VK_RETURN => {
                launch_selection(state_ptr);
                return LRESULT(0);
            }
            VK_ESCAPE => {
                let pb = HWND(SEARCH_HWND.load(Ordering::SeqCst) as *mut _);
                if !pb.0.is_null() {
                    let _ = DestroyWindow(pb);
                }
                return LRESULT(0);
            }
            _ => {}
        }
    } else if !state_ptr.is_null() && msg == WM_KEYUP {
        let vk = wparam.0 as u32;
        let state = &mut *state_ptr;
        if vk as u16 == state.owd_vk && state.owd_pending {
            state.owd_pending = false;
            open_selection_workdir(state_ptr);
            return LRESULT(0);
        }
    }

    let prev = if state_ptr.is_null() {
        None
    } else {
        (*state_ptr).edit_prev
    };
    match prev {
        Some(p) => CallWindowProcW(p, hwnd, msg, wparam, lparam),
        None => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn search_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SearchState;

    match msg {
        WM_COMMAND => {
            if state_ptr.is_null() {
                return LRESULT(0);
            }
            let notify = ((wparam.0 >> 16) & 0xFFFF) as u16;
            let id = (wparam.0 & 0xFFFF) as isize;
            if id == IDC_EDIT && notify == EN_CHANGE {
                refresh_list(state_ptr);
            } else if id == IDC_LIST && notify == LBN_DBLCLK {
                launch_selection(state_ptr);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            if !state_ptr.is_null() {
                let state = &*state_ptr;
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                let pad = 12i32;
                let edit_h = 26i32;
                let hint_h = 18i32;
                let _ = MoveWindow(
                    state.edit,
                    pad,
                    pad,
                    (rc.right - pad * 2).max(40),
                    edit_h,
                    true,
                );
                let list_top = pad + edit_h + 10;
                let list_h = (rc.bottom - list_top - pad - hint_h - 6).max(40);
                let _ = MoveWindow(
                    state.list,
                    pad,
                    list_top,
                    (rc.right - pad * 2).max(40),
                    list_h,
                    true,
                );
                let _ = MoveWindow(
                    state.hint,
                    pad,
                    list_top + list_h + 6,
                    (rc.right - pad * 2).max(40),
                    hint_h,
                    true,
                );
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            if !state_ptr.is_null() {
                let state = Box::from_raw(state_ptr);
                SEARCH_HWND.store(0, Ordering::SeqCst);
                let _ = PostMessageW(state.host, WM_DK_SEARCH_CLOSED, WPARAM(0), LPARAM(0));
                info!("Search closed");
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
