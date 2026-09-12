//! Shared window messages and class names.

use windows::core::{w, PCWSTR};
use windows::Win32::UI::WindowsAndMessaging::WM_APP;

/// Posted / sent to open or toggle the typing window.
pub const WM_DK_TRIGGER: u32 = WM_APP + 1;
/// Digit / bound key from the keyboard hook.
pub const WM_DK_KEY: u32 = WM_APP + 2;
/// Timeout or cancel close request.
pub const WM_DK_TIMEOUT: u32 = WM_APP + 3;
/// Tray icon callback from Shell (often SendMessage'd into WndProc).
pub const WM_DK_TRAY: u32 = WM_APP + 10;
/// Reload configuration.
pub const WM_DK_RELOAD: u32 = WM_APP + 11;
/// Quit the application.
pub const WM_DK_QUIT: u32 = WM_APP + 12;
/// Queued tray event for the main loop (posted from host_proc).
pub const WM_DK_TRAY_DISPATCH: u32 = WM_APP + 14;
/// Open the Search window.
pub const WM_DK_SEARCH: u32 = WM_APP + 15;
/// Search window closed.
pub const WM_DK_SEARCH_CLOSED: u32 = WM_APP + 16;
/// Open the settings window.
pub const WM_DK_SETTINGS: u32 = WM_APP + 17;
/// Settings window closed.
pub const WM_DK_SETTINGS_CLOSED: u32 = WM_APP + 18;
/// Settings were saved — host should reload config from disk.
pub const WM_DK_SETTINGS_APPLIED: u32 = WM_APP + 19;
/// Mode-switch leader pressed — arm chord listen.
pub const WM_DK_MODE_ARM: u32 = WM_APP + 20;
/// Chord key (0-9 / a-z) while mode-armed. wparam = vk.
pub const WM_DK_MODE_CHORD: u32 = WM_APP + 21;
/// Tray picked a mode file. lparam unused; basename in MODE_PICK_NAME.
pub const WM_DK_MODE_PICK: u32 = WM_APP + 22;
/// Periodic mouse-hook health check.
pub const WM_DK_HOOK_HEALTH: u32 = WM_APP + 23;
/// Launch failed (toast). Posted from typing / Search.
pub const WM_DK_LAUNCH_FAILED: u32 = WM_APP + 24;
/// X1 letter chord for a key set. wparam = vk.
pub const WM_DK_KEYSET_CHORD: u32 = WM_APP + 25;
/// Tray picked a key set. wparam = index into the key-set list.
pub const WM_DK_KEYSET_PICK: u32 = WM_APP + 26;

/// `COPYDATASTRUCT.dwData` for `--launch <id>` (second process → resident host).
/// Payload is UTF-8 slot id bytes. Handled synchronously in `host_proc` via `WM_COPYDATA`.
pub const COPYDATA_LAUNCH: usize = 0x0044_4B4C; // 'DKL\0'

pub const HOST_CLASS: PCWSTR = w!("DialKeyHost");
pub const TYPING_CLASS: PCWSTR = w!("DialKeyTypingWindow");
pub const SEARCH_CLASS: PCWSTR = w!("DialKeySearch");
pub const SETTINGS_CLASS: PCWSTR = w!("DialKeySettings");

pub const TRAY_ICON_ID: u32 = 1;
pub const HOTKEY_ID: i32 = 1;
