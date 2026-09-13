//! Load the embedded application icon (winres resource ID 1).

use std::sync::OnceLock;

use windows::core::PCWSTR;
use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{LoadImageW, HICON, IMAGE_ICON, LR_DEFAULTCOLOR};

/// Resource ID assigned by `winres` when `set_icon` is used.
pub const IDI_APP: usize = 1;

static CLASS_ICONS: OnceLock<(isize, isize)> = OnceLock::new();

fn module_instance() -> HINSTANCE {
    unsafe {
        GetModuleHandleW(None)
            .map(|m| HINSTANCE(m.0))
            .unwrap_or_default()
    }
}

/// Load a sized copy of the app icon. Caller owns the `HICON` and must
/// `DestroyIcon` it when finished (unless it is null).
pub unsafe fn load_app_icon(cx: i32, cy: i32) -> HICON {
    let instance = module_instance();
    if instance.0.is_null() {
        return HICON::default();
    }
    match LoadImageW(
        instance,
        PCWSTR(IDI_APP as *const u16),
        IMAGE_ICON,
        cx,
        cy,
        LR_DEFAULTCOLOR,
    ) {
        Ok(handle) => HICON(handle.0),
        Err(_) => HICON::default(),
    }
}

/// Default large / small icons for `WNDCLASSEXW`.
///
/// Cached for the process lifetime (Windows does not copy class icons).
pub unsafe fn load_class_icons() -> (HICON, HICON) {
    let (large, small) = *CLASS_ICONS.get_or_init(|| {
        let large = load_app_icon(32, 32);
        let small = load_app_icon(16, 16);
        (large.0 as isize, small.0 as isize)
    });
    (HICON(large as _), HICON(small as _))
}
