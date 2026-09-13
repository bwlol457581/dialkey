//! Best-effort match helpers for "already open → focus" (OS-independent).
//!
//! Tabbed apps (Excel, browsers) remain fuzzy — we only tighten false positives
//! (browser titles, `a.xlsx.bak`, etc.) without claiming tab-accurate focus.

use std::path::Path;

/// True when `title` likely refers to the document/folder at `path`.
///
/// Prefer a **bounded** full file-name token (not a raw substring). Stem-only
/// match is also bounded and requires stem length ≥ 3.
pub fn window_title_matches(title: &str, path: &Path) -> bool {
    let title_l = title.to_lowercase();
    if title_l.trim().is_empty() {
        return false;
    }

    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
        let name_l = name.to_lowercase();
        if !name_l.is_empty() && contains_bounded_token(&title_l, &name_l) {
            return true;
        }
    }

    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        let stem_l = stem.to_lowercase();
        // Avoid matching tiny stems ("a", "1") that appear everywhere.
        if stem_l.chars().count() >= 3 && contains_bounded_token(&title_l, &stem_l) {
            return true;
        }
    }

    false
}

/// True when `needle` appears in `haystack` as its own token.
///
/// Boundaries: start/end of string, or a character that is not part of a
/// typical path/file token (`A–Z`, `0–9`, `_`, `.`). So `a.xlsx` matches
/// `a.xlsx - Excel` and `Download a.xlsx`, but not `a.xlsx.bak` or `b_a.xlsx`.
pub fn contains_bounded_token(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() || haystack.is_empty() {
        return false;
    }
    let h: Vec<char> = haystack.chars().collect();
    let n: Vec<char> = needle.chars().collect();
    if n.len() > h.len() {
        return false;
    }
    let last_start = h.len() - n.len();
    for start in 0..=last_start {
        if h[start..start + n.len()] != n[..] {
            continue;
        }
        let before_ok = start == 0 || !is_token_char(h[start - 1]);
        let after = start + n.len();
        let after_ok = after >= h.len() || !is_token_char(h[after]);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.'
}

/// True when a process image path refers to the same `.exe` as `path`.
pub fn process_image_matches(image: &str, path: &Path) -> bool {
    let Some(target) = path.to_str() else {
        return false;
    };
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .eq_ignore_ascii_case("exe");
    if !ext {
        return false;
    }
    let a = normalize_path_key(image);
    let b = normalize_path_key(target);
    !a.is_empty() && a == b
}

/// Known browser / web-view top-level classes — skip for document title matches.
///
/// Not an allow-list of Office apps (that would bit-rot). Denying a few web
/// hosts cuts the common false-positive of focusing Chrome/Edge on a download
/// title. Explorer / editors are left alone.
pub fn is_document_focus_denied_class(class_name: &str) -> bool {
    matches!(
        class_name,
        "Chrome_WidgetWin_1"
            | "Chrome_WidgetWin_0"
            | "MozillaWindowClass"
            | "OperaWindowClass"
            | "ApplicationFrameWindow" // Edge / Store hosts (broad but avoids UWP Edge tabs)
    )
}

fn normalize_path_key(s: &str) -> String {
    s.trim().trim_matches('"').replace('/', "\\").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn title_matches_filename_token_and_stem_token() {
        let p = PathBuf::from(r"C:\docs\aaa.xlsx");
        assert!(window_title_matches("aaa.xlsx - Excel", &p));
        assert!(window_title_matches("AAA - Excel", &p));
        assert!(window_title_matches("Download aaa.xlsx now", &p));
        assert!(!window_title_matches("Book1 - Excel", &p));
        assert!(!window_title_matches("", &p));
    }

    #[test]
    fn rejects_embedded_filename_false_positives() {
        let p = PathBuf::from(r"C:\docs\a.xlsx");
        assert!(window_title_matches("a.xlsx - Excel", &p));
        assert!(!window_title_matches("a.xlsx.bak - Notepad", &p));
        assert!(!window_title_matches("b_a.xlsx - Excel", &p));
        // Spaced title still matches by token — browser hosts are filtered by class deny.
        assert!(window_title_matches(
            "How to download a.xlsx from the web - Chrome",
            &p
        ));
    }

    #[test]
    fn short_stem_does_not_false_positive_easily() {
        let p = PathBuf::from(r"C:\docs\ab.txt");
        // stem "ab" is < 3 — only full name matches
        assert!(window_title_matches("ab.txt - Editor", &p));
        assert!(!window_title_matches("about - Editor", &p));
    }

    #[test]
    fn bounded_token_helper() {
        assert!(contains_bounded_token("foo bar baz", "bar"));
        assert!(!contains_bounded_token("foobarbaz", "bar"));
        assert!(contains_bounded_token("report.docx — Word", "report.docx"));
        assert!(!contains_bounded_token("report.docx.bak", "report.docx"));
    }

    #[test]
    fn deny_browser_classes_only() {
        assert!(is_document_focus_denied_class("Chrome_WidgetWin_1"));
        assert!(is_document_focus_denied_class("Chrome_WidgetWin_0"));
        assert!(is_document_focus_denied_class("MozillaWindowClass"));
        assert!(is_document_focus_denied_class("OperaWindowClass"));
        assert!(is_document_focus_denied_class("ApplicationFrameWindow"));
        assert!(!is_document_focus_denied_class("XLMAIN"));
        assert!(!is_document_focus_denied_class("CabinetWClass"));
        assert!(!is_document_focus_denied_class("Notepad"));
    }

    /// Mirrors document path in `focus_existing`: title match ∧ ¬denied class.
    fn document_would_focus(title: &str, class: &str, path: &Path) -> bool {
        window_title_matches(title, path) && !is_document_focus_denied_class(class)
    }

    #[test]
    fn false_focus_browser_download_title_is_denied() {
        let p = PathBuf::from(r"C:\docs\report.xlsx");
        let title = "report.xlsx - Google Chrome";
        // Title alone would match; class deny must block focus theft.
        assert!(window_title_matches(title, &p));
        assert!(!document_would_focus(title, "Chrome_WidgetWin_1", &p));
        assert!(!document_would_focus(title, "MozillaWindowClass", &p));
        assert!(!document_would_focus(title, "ApplicationFrameWindow", &p));
        // Real Office host still focuses.
        assert!(document_would_focus("report.xlsx - Excel", "XLMAIN", &p));
    }

    #[test]
    fn false_focus_backup_and_embedded_names_do_not_match() {
        let p = PathBuf::from(r"C:\docs\a.xlsx");
        assert!(!document_would_focus("a.xlsx.bak - Notepad", "Notepad", &p));
        assert!(!document_would_focus("b_a.xlsx - Excel", "XLMAIN", &p));
        assert!(!document_would_focus("xa.xlsx - Excel", "XLMAIN", &p));
        assert!(document_would_focus("a.xlsx - Excel", "XLMAIN", &p));
    }

    #[test]
    fn exe_image_match_is_case_and_slash_insensitive() {
        let p = PathBuf::from(r"C:\Tools\Foo.exe");
        assert!(process_image_matches(r"c:/tools/foo.exe", &p));
        assert!(!process_image_matches(r"c:\tools\bar.exe", &p));
        assert!(!process_image_matches(
            r"c:\tools\foo.exe",
            Path::new(r"C:\docs\a.xlsx")
        ));
    }
}
