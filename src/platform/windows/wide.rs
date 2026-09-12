//! UTF-16 helpers for Win32 APIs.

use windows::core::PCWSTR;

pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn pcwstr(wide: &[u16]) -> PCWSTR {
    PCWSTR(wide.as_ptr())
}
