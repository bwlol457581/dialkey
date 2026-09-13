//! Mode chord map: auto-assign slot files to 0–9, with optional overrides.

use std::collections::{HashMap, HashSet};

use tracing::warn;

/// Chord codes in assignment order: `0`…`9` only.
pub fn chord_code_at(index: usize) -> Option<String> {
    match index {
        0..=9 => Some(index.to_string()),
        _ => None,
    }
}

pub fn is_valid_chord_code(code: &str) -> bool {
    let mut chars = code.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => c.is_ascii_digit(),
        _ => false,
    }
}

fn basename_only(name: &str) -> String {
    name.replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(name)
        .trim()
        .to_string()
}

fn is_default_slots_file(name: &str) -> bool {
    name.eq_ignore_ascii_case("slots.json")
}

/// Build chord → basename map.
///
/// 1. Auto-assign: code `0` is reserved for the shipped book `slots.json`.
///    Other candidates fill `1`…`9` in the given order. They never take `0`.
/// 2. Apply `overrides` (chord → basename). Invalid / duplicate overrides are
///    skipped with warnings. Legacy `a`–`z` overrides are ignored.
pub fn build_mode_chord_map(
    candidates: &[String],
    overrides: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    let mut rest: Vec<&String> = Vec::new();
    let mut home: Option<&String> = None;
    for file in candidates {
        if home.is_none() && is_default_slots_file(file) {
            home = Some(file);
        } else {
            rest.push(file);
        }
    }
    if let Some(file) = home {
        map.insert("0".into(), file.clone());
    }
    let mut next = 1usize;
    for file in rest {
        let Some(code) = chord_code_at(next) else {
            break;
        };
        map.insert(code, file.clone());
        next += 1;
    }

    if overrides.is_empty() {
        return map;
    }

    let candidate_set: HashSet<&str> = candidates.iter().map(|s| s.as_str()).collect();
    let mut accepted: Vec<(String, String)> = Vec::new();
    let mut override_codes: HashSet<String> = HashSet::new();
    let mut override_files: HashSet<String> = HashSet::new();

    for (raw_code, raw_file) in overrides {
        let code = raw_code.trim().to_ascii_lowercase();
        let file = basename_only(raw_file);
        if !is_valid_chord_code(&code) {
            warn!(code = %raw_code, "modeSwitch.keys: invalid chord (use 0-9); ignored");
            continue;
        }
        if file.is_empty()
            || file.eq_ignore_ascii_case("slots.example.json")
            || !file.to_ascii_lowercase().ends_with(".json")
        {
            warn!(code = %code, file = %raw_file, "modeSwitch.keys: invalid slots file; ignored");
            continue;
        }
        if !candidate_set.contains(file.as_str()) {
            warn!(
                code = %code,
                file = %file,
                "modeSwitch.keys: file not in config folder candidates; ignored"
            );
            continue;
        }
        if !override_codes.insert(code.clone()) {
            warn!(code = %code, "modeSwitch.keys: duplicate chord in overrides; ignored");
            continue;
        }
        if !override_files.insert(file.clone()) {
            warn!(
                code = %code,
                file = %file,
                "modeSwitch.keys: duplicate file in overrides; ignored"
            );
            continue;
        }
        accepted.push((code, file));
    }

    for (code, file) in &accepted {
        map.retain(|c, f| c != code && f != file);
    }
    for (code, file) in accepted {
        map.insert(code, file);
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_assigns_other_books_from_code_one() {
        let files: Vec<String> = (0..12).map(|i| format!("slots.{i}.json")).collect();
        let map = build_mode_chord_map(&files, &HashMap::new());
        assert!(map.get("0").is_none());
        assert_eq!(map.get("1").map(String::as_str), Some("slots.0.json"));
        assert_eq!(map.get("9").map(String::as_str), Some("slots.8.json"));
        assert!(map.get("a").is_none());
        assert_eq!(map.len(), 9);
    }

    #[test]
    fn override_replaces_and_dedupes_file() {
        let files = vec![
            "slots.json".into(),
            "slots.work.json".into(),
            "slots.hobby.json".into(),
        ];
        let mut ov = HashMap::new();
        ov.insert("1".into(), "slots.work.json".into());
        let map = build_mode_chord_map(&files, &ov);
        assert_eq!(map.get("1").map(String::as_str), Some("slots.work.json"));
        let work_count = map.values().filter(|f| *f == "slots.work.json").count();
        assert_eq!(work_count, 1);
    }

    #[test]
    fn chord_must_be_single_digit() {
        assert!(is_valid_chord_code("0"));
        assert!(is_valid_chord_code("9"));
        assert!(!is_valid_chord_code("a"));
        assert!(!is_valid_chord_code("z"));
        assert!(!is_valid_chord_code(""));
        assert!(!is_valid_chord_code("10"));
        assert!(!is_valid_chord_code("+"));

        let files = vec!["slots.json".into(), "slots.work.json".into()];
        let mut ov = HashMap::new();
        ov.insert("a".into(), "slots.work.json".into());
        ov.insert("10".into(), "slots.work.json".into());
        let map = build_mode_chord_map(&files, &ov);
        assert_eq!(map.get("0").map(String::as_str), Some("slots.json"));
        assert_eq!(map.get("1").map(String::as_str), Some("slots.work.json"));
        assert!(map.get("a").is_none());
        assert!(map.get("10").is_none());
    }

    #[test]
    fn shipped_slots_json_is_always_code_zero() {
        let files = vec![
            "slots.hobby.json".into(),
            "slots.json".into(),
            "slots.work.json".into(),
        ];
        let map = build_mode_chord_map(&files, &HashMap::new());
        assert_eq!(map.get("0").map(String::as_str), Some("slots.json"));
        assert_eq!(map.get("1").map(String::as_str), Some("slots.hobby.json"));
        assert_eq!(map.get("2").map(String::as_str), Some("slots.work.json"));
    }

    #[test]
    fn other_books_never_take_code_zero() {
        let files = vec!["slots.hobby.json".into(), "slots.work.json".into()];
        let map = build_mode_chord_map(&files, &HashMap::new());
        assert!(map.get("0").is_none());
        assert_eq!(map.get("1").map(String::as_str), Some("slots.hobby.json"));
        assert_eq!(map.get("2").map(String::as_str), Some("slots.work.json"));
    }
}
