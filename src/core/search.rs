//! Phone-book search (OS-independent).

use serde::{Deserialize, Serialize};

use super::slots::{natural_cmp_id, Slot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SearchMode {
    Exact,
    Prefix,
    #[default]
    Substring,
    Fuzzy,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSettings {
    #[serde(default)]
    pub mode: SearchMode,
    #[serde(default)]
    pub case_sensitive: bool,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            mode: SearchMode::Substring,
            case_sensitive: false,
        }
    }
}

/// Filter and natural-sort slots for Search.
pub fn search_slots<'a>(
    slots: &'a [Slot],
    query: &str,
    settings: &SearchSettings,
) -> Vec<&'a Slot> {
    let q = query.trim();
    if q.is_empty() {
        let mut all: Vec<&Slot> = slots.iter().collect();
        all.sort_by(|a, b| natural_cmp_id(&a.id, &b.id));
        return all;
    }

    let terms: Vec<String> = q
        .split_whitespace()
        .map(|t| normalize(t, settings.case_sensitive))
        .filter(|t| !t.is_empty())
        .collect();
    if terms.is_empty() {
        let mut all: Vec<&Slot> = slots.iter().collect();
        all.sort_by(|a, b| natural_cmp_id(&a.id, &b.id));
        return all;
    }

    let mut out: Vec<&Slot> = slots
        .iter()
        .filter(|s| {
            let hay = slot_haystack(s, settings.case_sensitive);
            terms
                .iter()
                .all(|term| match_term(&hay, term, settings.mode))
        })
        .collect();
    out.sort_by(|a, b| natural_cmp_id(&a.id, &b.id));
    out
}

fn slot_haystack(slot: &Slot, case_sensitive: bool) -> String {
    // Spec search targets: slot id / name / program name / description.
    let program = std::path::Path::new(&slot.path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let joined = format!("{} {} {} {}", slot.id, slot.name, program, slot.description);
    normalize(&joined, case_sensitive)
}

fn normalize(s: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        s.to_string()
    } else {
        s.to_lowercase()
    }
}

fn match_term(haystack: &str, term: &str, mode: SearchMode) -> bool {
    match mode {
        SearchMode::Exact => haystack.split_whitespace().any(|tok| tok == term) || haystack == term,
        SearchMode::Prefix => {
            haystack.split_whitespace().any(|tok| tok.starts_with(term))
                || haystack.starts_with(term)
        }
        SearchMode::Substring => haystack.contains(term),
        SearchMode::Fuzzy => is_subsequence(term, haystack),
    }
}

/// Every character of `needle` appears in order in `haystack`.
fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut it = haystack.chars();
    for ch in needle.chars() {
        loop {
            match it.next() {
                Some(h) if h == ch => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: &str, name: &str, path: &str, desc: &str) -> Slot {
        Slot {
            id: id.into(),
            name: name.into(),
            path: path.into(),
            description: desc.into(),
            workdir: String::new(),
            open_workdir: false,
            focus_existing: true,
        }
    }

    #[test]
    fn substring_and_terms() {
        let slots = vec![
            slot("31", "Patrol Split", "C:/tools/split_patrol.bat", "monthly"),
            slot("1", "Notepad", "notepad.exe", "Text editor"),
        ];
        let settings = SearchSettings {
            mode: SearchMode::Substring,
            case_sensitive: false,
        };
        let hits = search_slots(&slots, "patrol split", &settings);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "31");
    }

    #[test]
    fn fuzzy_ptrl() {
        let slots = vec![slot("31", "Patrol", "patrol.exe", "")];
        let settings = SearchSettings {
            mode: SearchMode::Fuzzy,
            case_sensitive: false,
        };
        let hits = search_slots(&slots, "ptrl", &settings);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn empty_query_lists_all_natural_order() {
        let slots = vec![
            slot("10", "Ten", "t.exe", ""),
            slot("2", "Two", "t.exe", ""),
        ];
        let hits = search_slots(&slots, "", &SearchSettings::default());
        assert_eq!(hits[0].id, "2");
        assert_eq!(hits[1].id, "10");
    }

    #[test]
    fn prefix_mode() {
        let slots = vec![
            slot("1", "Cursor", "Cursor.exe", ""),
            slot("2", "Calc", "calc.exe", ""),
        ];
        let settings = SearchSettings {
            mode: SearchMode::Prefix,
            case_sensitive: false,
        };
        let hits = search_slots(&slots, "cur", &settings);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "Cursor");
    }

    #[test]
    fn id_is_searchable() {
        let slots = vec![
            slot("31", "Other", "tool.exe", "desc"),
            slot("1", "Calc", "calc.exe", ""),
        ];
        let hits = search_slots(&slots, "31", &SearchSettings::default());
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "31");

        let by_one = search_slots(&slots, "1", &SearchSettings::default());
        assert!(by_one.iter().any(|s| s.id == "1"));
    }
}
