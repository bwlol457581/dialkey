//! Phone-book search (OS-independent).

use serde::{Deserialize, Serialize};

use super::keys::ResolvedKeys;
use super::slots::{natural_cmp_id, Slot};

/// Search-window keydown: follow Capture roles. Esc is not always Cancel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchWindowAction {
    Close,
    Launch,
    NavUp,
    NavDown,
    Pass,
}

/// Classify a Search keydown. Cancel closes. Confirm or Start launches (fire).
/// Open-workdir is tap-alone on keyup — Pass here so the caller can track it.
pub fn search_window_keydown(vk: u16, keys: &ResolvedKeys) -> SearchWindowAction {
    if keys.is_cancel(vk) {
        return SearchWindowAction::Close;
    }
    if keys.is_open_workdir(vk) {
        return SearchWindowAction::Pass;
    }
    if vk == keys.confirm || keys.is_start(vk) {
        return SearchWindowAction::Launch;
    }
    match vk {
        0x26 => SearchWindowAction::NavUp,   // VK_UP
        0x28 => SearchWindowAction::NavDown, // VK_DOWN
        _ => SearchWindowAction::Pass,
    }
}

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

    fn resolved() -> crate::core::keys::ResolvedKeys {
        crate::core::keys::KeyBindings::default().resolved()
    }

    #[test]
    fn search_window_follows_cancel_not_hardcoded_esc() {
        let k = resolved();
        assert_eq!(
            search_window_keydown(crate::core::keys::VK_SUBTRACT, &k),
            SearchWindowAction::Close
        );
        assert_eq!(search_window_keydown(0x0D, &k), SearchWindowAction::Launch);
        assert_eq!(
            search_window_keydown(crate::core::keys::VK_ADD, &k),
            SearchWindowAction::Launch
        );
        assert_eq!(search_window_keydown(0x1B, &k), SearchWindowAction::Pass);

        let mut rebound = resolved();
        rebound.cancel = 0x1B;
        assert_eq!(search_window_keydown(0x1B, &rebound), SearchWindowAction::Close);
        assert_eq!(
            search_window_keydown(crate::core::keys::VK_SUBTRACT, &rebound),
            SearchWindowAction::Pass
        );
    }

    #[test]
    fn search_window_esc_as_start_or_confirm_launches() {
        let mut k = resolved();
        k.cancel = crate::core::keys::VK_SUBTRACT;
        k.start = 0x1B;
        assert_eq!(search_window_keydown(0x1B, &k), SearchWindowAction::Launch);

        let mut k2 = resolved();
        k2.cancel = crate::core::keys::VK_SUBTRACT;
        k2.confirm = 0x1B;
        assert_eq!(search_window_keydown(0x1B, &k2), SearchWindowAction::Launch);
    }

    #[test]
    fn search_window_open_workdir_is_pass_on_keydown() {
        let k = resolved();
        assert_eq!(
            search_window_keydown(crate::core::keys::VK_CONTROL, &k),
            SearchWindowAction::Pass
        );
    }
}
