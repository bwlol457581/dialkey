//! Windows extended-length paths (`\\?\` / `\\?\UNC\`).
//!
//! Pure string transform — no Win32. JSON keeps the typed path; callers apply
//! this at exists-check / spawn / shell-open time only.

use std::path::{Path, PathBuf};

/// Prefix a Windows absolute local or UNC path so APIs can exceed MAX_PATH (260).
///
/// Relative names, bare filenames (`notepad.exe`), device paths (`\\.\`), and
/// strings that already start with `\\?\` are left as-is (aside from `/` → `\`).
pub fn to_extended_length(path: &str) -> String {
    let s = path.trim();
    if s.is_empty() {
        return String::new();
    }
    let s = s.replace('/', "\\");
    if s.starts_with(r"\\?\") {
        return strip_trailing_slash_keep_drive_root(&s);
    }
    if s.starts_with(r"\\.\") {
        return s;
    }
    if let Some(rest) = s.strip_prefix(r"\\") {
        return strip_trailing_slash_keep_drive_root(&format!(r"\\?\UNC\{rest}"));
    }
    let b = s.as_bytes();
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' {
        return strip_trailing_slash_keep_drive_root(&format!(r"\\?\{s}"));
    }
    s
}

fn strip_trailing_slash_keep_drive_root(s: &str) -> String {
    if !s.ends_with('\\') {
        return s.to_string();
    }
    let trimmed = s.trim_end_matches('\\');
    if trimmed.ends_with(':') {
        return format!("{trimmed}\\");
    }
    trimmed.to_string()
}

/// `exists` that can see long UNC / local paths on Windows.
pub fn path_exists(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let raw = path.to_string_lossy();
        Path::new(&to_extended_length(&raw)).exists() || path.exists()
    }
    #[cfg(not(windows))]
    {
        path.exists()
    }
}

/// `is_dir` that can see long UNC / local paths on Windows.
pub fn path_is_dir(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let raw = path.to_string_lossy();
        Path::new(&to_extended_length(&raw)).is_dir() || path.is_dir()
    }
    #[cfg(not(windows))]
    {
        path.is_dir()
    }
}

/// Extended-length [`PathBuf`] for spawn / `current_dir` / shell. Relative unchanged.
pub fn path_for_win32(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(to_extended_length(&path.to_string_lossy()))
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_drive_and_unc() {
        assert_eq!(to_extended_length(r"C:\work\a.xlsx"), r"\\?\C:\work\a.xlsx");
        assert_eq!(to_extended_length(r"C:/work/a.xlsx"), r"\\?\C:\work\a.xlsx");
        assert_eq!(
            to_extended_length(r"\\192.168.10.254\File-Server\3. 品質保証ĐBCL\2026\202608"),
            r"\\?\UNC\192.168.10.254\File-Server\3. 品質保証ĐBCL\2026\202608"
        );
    }

    #[test]
    fn leaves_relative_and_already_prefixed() {
        assert_eq!(to_extended_length("notepad.exe"), "notepad.exe");
        assert_eq!(to_extended_length(r"tools\foo.bat"), r"tools\foo.bat");
        assert_eq!(to_extended_length(r"\\?\C:\already"), r"\\?\C:\already");
        assert_eq!(
            to_extended_length(r"\\?\UNC\server\share\x"),
            r"\\?\UNC\server\share\x"
        );
        assert_eq!(to_extended_length(r"\\.\pipe\foo"), r"\\.\pipe\foo");
        assert_eq!(to_extended_length(""), "");
    }

    #[test]
    fn keeps_drive_root_slash() {
        assert_eq!(to_extended_length(r"C:\"), r"\\?\C:\");
        assert_eq!(to_extended_length(r"C:\work\"), r"\\?\C:\work");
    }
}
