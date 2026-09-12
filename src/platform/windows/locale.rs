//! OS UI language detection for language-pack resolution.

use crate::core::i18n::extract_language_code;

/// Short language code from the Windows user UI locale (`ja`, `vi`, `en`, …).
pub fn os_ui_language() -> Option<String> {
    unsafe {
        let mut buf = [0u16; 85];
        let len = windows::Win32::Globalization::GetUserDefaultLocaleName(&mut buf);
        if len <= 1 {
            return None;
        }
        let s = String::from_utf16_lossy(&buf[..(len as usize - 1)]);
        let code = extract_language_code(&s);
        if code.is_empty() {
            None
        } else {
            Some(code)
        }
    }
}
