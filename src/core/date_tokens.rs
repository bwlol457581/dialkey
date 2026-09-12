//! Date token expansion for slot `path` / `workdir` (Phase 10 / B-6).
//!
//! Only brace-wrapped tokens are expanded. Expansion uses a fixed instant
//! (normally "now") so multi-token paths stay consistent within one launch.

use std::path::{Path, PathBuf};

use chrono::{Datelike, Local, Timelike};

use super::winpath::path_exists;

/// Calendar parts used for token expansion (local time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateParts {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

impl DateParts {
    /// Local wall-clock time at the call site.
    pub fn now() -> Self {
        let local = Local::now();
        Self {
            year: local.year(),
            month: local.month(),
            day: local.day(),
            hour: local.hour(),
            minute: local.minute(),
        }
    }
}

const KNOWN_TOKENS: &[&str] = &["yyyy", "yy", "MM", "dd", "HH", "min", "yyyyMM", "yyyyMMdd"];

/// True when `s` contains at least one known date token (`{yyyy}`, …).
pub fn contains_date_token(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '}') {
                let name: String = chars[i + 1..i + 1 + end].iter().collect();
                if KNOWN_TOKENS.contains(&name.as_str()) {
                    return true;
                }
                i += end + 2;
                continue;
            }
        }
        i += 1;
    }
    false
}

/// Expand known `{…}` date tokens; leave unknown brace groups unchanged.
pub fn expand_date_tokens(path: &str, when: &DateParts) -> String {
    let mut out = String::with_capacity(path.len());
    let chars: Vec<char> = path.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '}') {
                let name: String = chars[i + 1..i + 1 + end].iter().collect();
                if let Some(val) = token_value(&name, when) {
                    out.push_str(&val);
                } else {
                    out.push('{');
                    out.push_str(&name);
                    out.push('}');
                }
                i += end + 2;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn token_value(name: &str, when: &DateParts) -> Option<String> {
    match name {
        "yyyy" => Some(format!("{:04}", when.year)),
        "yy" => Some(format!("{:02}", when.year.rem_euclid(100))),
        "MM" => Some(format!("{:02}", when.month)),
        "dd" => Some(format!("{:02}", when.day)),
        "HH" => Some(format!("{:02}", when.hour)),
        "min" => Some(format!("{:02}", when.minute)),
        "yyyyMM" => Some(format!("{:04}{:02}", when.year, when.month)),
        "yyyyMMdd" => Some(format!("{:04}{:02}{:02}", when.year, when.month, when.day)),
        _ => None,
    }
}

/// If `path` is missing, return the deepest existing ancestor.
/// Never creates directories. Returns `path` unchanged when nothing exists.
pub fn deepest_existing_ancestor(path: &Path) -> PathBuf {
    if path_exists(path) {
        return path.to_path_buf();
    }
    let mut current = path.to_path_buf();
    while let Some(parent) = current.parent() {
        if parent.as_os_str().is_empty() {
            break;
        }
        if path_exists(parent) {
            return parent.to_path_buf();
        }
        // Avoid infinite loop on odd roots (e.g. `\\?\` edge cases).
        if parent == current {
            break;
        }
        current = parent.to_path_buf();
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> DateParts {
        DateParts {
            year: 2026,
            month: 8,
            day: 10,
            hour: 14,
            minute: 5,
        }
    }

    #[test]
    fn expands_all_known_tokens() {
        let w = sample();
        assert_eq!(expand_date_tokens("{yyyy}", &w), "2026");
        assert_eq!(expand_date_tokens("{yy}", &w), "26");
        assert_eq!(expand_date_tokens("{MM}", &w), "08");
        assert_eq!(expand_date_tokens("{dd}", &w), "10");
        assert_eq!(expand_date_tokens("{HH}", &w), "14");
        assert_eq!(expand_date_tokens("{min}", &w), "05");
        assert_eq!(expand_date_tokens("{yyyyMM}", &w), "202608");
        assert_eq!(expand_date_tokens("{yyyyMMdd}", &w), "20260810");
    }

    #[test]
    fn expands_composite_path() {
        let w = sample();
        let s = expand_date_tokens(r"D:\work\{yyyy}\{yyyyMM}\report_{yyyyMM}.xlsx", &w);
        assert_eq!(s, r"D:\work\2026\202608\report_202608.xlsx");
    }

    #[test]
    fn leaves_bare_yyyy_alone() {
        let w = sample();
        assert_eq!(
            expand_date_tokens(r"D:\yyyy_backup\file.txt", &w),
            r"D:\yyyy_backup\file.txt"
        );
    }

    #[test]
    fn leaves_unknown_and_mm_alone() {
        let w = sample();
        // `{mm}` is intentionally not minutes — left unchanged.
        assert_eq!(expand_date_tokens("a_{mm}_{foo}_b", &w), "a_{mm}_{foo}_b");
    }

    #[test]
    fn detects_known_tokens() {
        assert!(contains_date_token(r"D:\work\{yyyyMM}\a.xlsx"));
        assert!(contains_date_token("{min}"));
        assert!(!contains_date_token(r"D:\yyyy_backup\a.xlsx"));
        assert!(!contains_date_token("{mm}"));
        assert!(!contains_date_token("chain:1,2"));
    }

    #[test]
    fn deepest_existing_walks_up() {
        let root =
            std::env::temp_dir().join(format!("dialkey_date_token_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("2026")).unwrap();
        let missing = root.join("2026").join("202608").join("report.xlsx");
        let got = deepest_existing_ancestor(&missing);
        assert_eq!(got, root.join("2026"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn deepest_existing_keeps_present_path() {
        let root =
            std::env::temp_dir().join(format!("dialkey_date_token_present_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("ok.txt");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(deepest_existing_ancestor(&file), file);
        let _ = std::fs::remove_dir_all(&root);
    }
}
