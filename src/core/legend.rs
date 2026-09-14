//! Typing-window subcommand legend: order and visibility per page.
//!
//! OS-independent. Idle vs MultiDigit (narrow-down) lists are independent.

use serde::{Deserialize, Serialize};

use crate::core::keys::KeyRole;
use crate::core::sequence::Mode;

/// All legend items (fixed set). Window always reserves this many option rows.
pub const LEGEND_ITEM_COUNT: usize = 6;

/// Next-digit decade (`{prefix}0`…`{prefix}9`).
pub const DIAL_ROWS: usize = 10;

/// Current typed id, Start cue when Multi is armed, or empty (Idle), plus the decade.
pub const DIAL_AREA_ROWS: usize = 1 + DIAL_ROWS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegendId {
    Start,
    OpenWorkdir,
    Search,
    DigitBack,
    Cancel,
    Settings,
}

impl LegendId {
    pub const ALL: [LegendId; LEGEND_ITEM_COUNT] = [
        LegendId::Start,
        LegendId::OpenWorkdir,
        LegendId::Search,
        LegendId::DigitBack,
        LegendId::Cancel,
        LegendId::Settings,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            LegendId::Start => "start",
            LegendId::OpenWorkdir => "openWorkdir",
            LegendId::Search => "search",
            LegendId::DigitBack => "digitBack",
            LegendId::Cancel => "cancel",
            LegendId::Settings => "settings",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "start" => Some(LegendId::Start),
            "openWorkdir" | "open_workdir" => Some(LegendId::OpenWorkdir),
            "search" => Some(LegendId::Search),
            "digitBack" | "digit_back" => Some(LegendId::DigitBack),
            "cancel" => Some(LegendId::Cancel),
            "settings" => Some(LegendId::Settings),
            _ => None,
        }
    }

    pub fn key_role(self) -> KeyRole {
        match self {
            LegendId::Start => KeyRole::Start,
            LegendId::OpenWorkdir => KeyRole::OpenWorkdir,
            LegendId::Search => KeyRole::Search,
            LegendId::DigitBack => KeyRole::DigitBack,
            LegendId::Cancel => KeyRole::Cancel,
            LegendId::Settings => KeyRole::Settings,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegendPage {
    /// Display order (unknown ids ignored; missing ids appended).
    #[serde(default)]
    pub order: Vec<String>,
    /// Ids not shown on this page (key still works).
    #[serde(default)]
    pub hidden: Vec<String>,
}

impl Default for LegendPage {
    fn default() -> Self {
        Self {
            order: Vec::new(),
            hidden: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegendSettings {
    #[serde(default)]
    pub idle: LegendPage,
    #[serde(default)]
    pub multi: LegendPage,
}

impl Default for LegendSettings {
    fn default() -> Self {
        Self {
            idle: LegendPage {
                order: default_idle_order(),
                hidden: default_idle_hidden(),
            },
            multi: LegendPage {
                order: default_multi_order(),
                hidden: default_multi_hidden(),
            },
        }
    }
}

fn default_idle_order() -> Vec<String> {
    vec![
        "start".into(),
        "search".into(),
        "cancel".into(),
        "settings".into(),
        "openWorkdir".into(),
        "digitBack".into(),
    ]
}

fn default_idle_hidden() -> Vec<String> {
    vec!["openWorkdir".into(), "digitBack".into()]
}

fn default_multi_order() -> Vec<String> {
    vec![
        "openWorkdir".into(),
        "search".into(),
        "digitBack".into(),
        "cancel".into(),
        "settings".into(),
        "start".into(),
    ]
}

fn default_multi_hidden() -> Vec<String> {
    vec!["start".into(), "settings".into()]
}

impl LegendSettings {
    /// Fill missing ids, drop unknowns, keep first occurrence.
    /// Empty pages (omitted in JSON) get shipped defaults including hidden items.
    pub fn normalize(&mut self) {
        if self.idle.order.is_empty() && self.idle.hidden.is_empty() {
            self.idle.order = default_idle_order();
            self.idle.hidden = default_idle_hidden();
        } else {
            normalize_order_and_hidden(
                &mut self.idle,
                &default_idle_order(),
                &default_idle_hidden(),
            );
        }
        if self.multi.order.is_empty() && self.multi.hidden.is_empty() {
            self.multi.order = default_multi_order();
            self.multi.hidden = default_multi_hidden();
        } else {
            normalize_order_and_hidden(
                &mut self.multi,
                &default_multi_order(),
                &default_multi_hidden(),
            );
        }
    }

    pub fn page(&self, mode: Mode) -> &LegendPage {
        match mode {
            Mode::Idle => &self.idle,
            Mode::MultiDigit => &self.multi,
        }
    }

    pub fn page_mut(&mut self, idle: bool) -> &mut LegendPage {
        if idle {
            &mut self.idle
        } else {
            &mut self.multi
        }
    }

    /// Visible items in order for this page (not padded).
    pub fn visible(&self, mode: Mode) -> Vec<LegendId> {
        let page = self.page(mode);
        page.order
            .iter()
            .filter_map(|s| LegendId::parse(s))
            .filter(|id| !page.hidden.iter().any(|h| LegendId::parse(h) == Some(*id)))
            .collect()
    }
}

fn normalize_order_and_hidden(
    page: &mut LegendPage,
    default_order: &[String],
    default_hidden: &[String],
) {
    let original: std::collections::HashSet<String> = page
        .order
        .iter()
        .filter_map(|s| LegendId::parse(s).map(|id| id.as_str().to_string()))
        .collect();
    let mut seen = std::collections::HashSet::new();
    let mut order = Vec::new();
    for s in page.order.iter().chain(default_order.iter()) {
        let Some(id) = LegendId::parse(s) else {
            continue;
        };
        let key = id.as_str();
        if seen.insert(key) {
            order.push(key.to_string());
        }
    }
    page.order = order;

    let mut hidden = Vec::new();
    let mut hseen = std::collections::HashSet::new();
    for s in &page.hidden {
        let Some(id) = LegendId::parse(s) else {
            continue;
        };
        let key = id.as_str();
        if hseen.insert(key) {
            hidden.push(key.to_string());
        }
    }
    for s in default_hidden {
        let Some(id) = LegendId::parse(s) else {
            continue;
        };
        let key = id.as_str();
        if original.contains(key) {
            continue;
        }
        if hseen.insert(key) {
            hidden.push(key.to_string());
        }
    }
    page.hidden = hidden;
}

/// Consecutive ids `{prefix}0` … `{prefix}9` (empty prefix → `"0"`…`"9"`).
pub fn decade_ids(prefix: &str) -> Vec<String> {
    (0u8..=9).map(|d| format!("{prefix}{d}")).collect()
}

/// First dial-row label.
///
/// Idle stays blank. Multi keeps the Start display so later screens still
/// show `+` (`+`, then `+1`, `+10`). The decade stays `0`–`9` / `{prefix}0`–`9`.
pub fn dial_head_display(idle: bool, prefix: &str, start_display: &str) -> String {
    if idle {
        return String::new();
    }
    let cue = start_display.trim();
    let cue: String = if cue.is_empty() {
        "+".into()
    } else {
        cue.chars().take(3).collect()
    };
    if prefix.is_empty() {
        cue
    } else {
        format!("{cue}{prefix}")
    }
}

/// Dial block: current buffer id (or empty) + next-digit decade.
///
/// At `max_digits` there is no next decade — those 10 rows stay empty.
pub fn dial_area_ids(prefix: &str, max_digits: usize) -> Vec<String> {
    let mut rows = Vec::with_capacity(DIAL_AREA_ROWS);
    if prefix.is_empty() {
        rows.push(String::new());
    } else {
        rows.push(prefix.to_string());
    }
    if max_digits == 0 || prefix.len() >= max_digits {
        rows.extend(std::iter::repeat_with(String::new).take(DIAL_ROWS));
    } else {
        rows.extend(decade_ids(prefix));
    }
    debug_assert_eq!(rows.len(), DIAL_AREA_ROWS);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_idle_hides_open_and_back() {
        let s = LegendSettings::default();
        let v = s.visible(Mode::Idle);
        assert_eq!(
            v,
            vec![
                LegendId::Start,
                LegendId::Search,
                LegendId::Cancel,
                LegendId::Settings
            ]
        );
    }

    #[test]
    fn default_multi_is_open_search_back_cancel() {
        let s = LegendSettings::default();
        let v = s.visible(Mode::MultiDigit);
        assert_eq!(
            v,
            vec![
                LegendId::OpenWorkdir,
                LegendId::Search,
                LegendId::DigitBack,
                LegendId::Cancel
            ]
        );
    }

    #[test]
    fn normalize_appends_missing_and_drops_junk() {
        let mut s = LegendSettings {
            idle: LegendPage {
                order: vec!["search".into(), "nope".into(), "start".into()],
                hidden: vec!["nope".into(), "cancel".into()],
            },
            multi: LegendPage::default(),
        };
        s.normalize();
        assert_eq!(s.idle.order[0], "search");
        assert_eq!(s.idle.order[1], "start");
        assert!(s.idle.order.contains(&"cancel".to_string()));
        assert_eq!(s.idle.order.len(), LEGEND_ITEM_COUNT);
        assert!(s.idle.order.contains(&"settings".to_string()));
        assert!(!s.idle.hidden.contains(&"settings".to_string()));
        assert_eq!(
            s.idle.hidden,
            vec![
                "cancel".to_string(),
                "openWorkdir".to_string(),
                "digitBack".to_string()
            ]
        );
        assert_eq!(s.multi.order.len(), LEGEND_ITEM_COUNT);
    }

    #[test]
    fn decade_empty_and_prefix() {
        assert_eq!(
            decade_ids(""),
            vec!["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]
        );
        assert_eq!(
            decade_ids("6"),
            vec!["60", "61", "62", "63", "64", "65", "66", "67", "68", "69"]
        );
    }

    #[test]
    fn dial_head_shows_start_cue_when_multi_armed() {
        assert_eq!(dial_head_display(true, "", "+"), "");
        assert_eq!(dial_head_display(false, "", "+"), "+");
        assert_eq!(dial_head_display(false, "", "  "), "+");
        assert_eq!(dial_head_display(false, "", "ADD"), "ADD");
        assert_eq!(dial_head_display(false, "1", "+"), "+1");
        assert_eq!(dial_head_display(false, "10", "+"), "+10");
        assert_eq!(dial_head_display(false, "1", "ADD"), "ADD1");
    }

    #[test]
    fn dial_area_keeps_current_then_next_decade() {
        let idle = dial_area_ids("", 10);
        assert_eq!(idle.len(), DIAL_AREA_ROWS);
        assert_eq!(idle[0], "");
        assert_eq!(
            &idle[1..],
            &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]
        );

        let one = dial_area_ids("1", 10);
        assert_eq!(one[0], "1");
        assert_eq!(
            &one[1..],
            &["10", "11", "12", "13", "14", "15", "16", "17", "18", "19"]
        );

        let ten = dial_area_ids("10", 10);
        assert_eq!(ten[0], "10");
        assert_eq!(ten[1], "100");
        assert_eq!(ten[10], "109");

        let full = dial_area_ids("1234567890", 10);
        assert_eq!(full[0], "1234567890");
        assert!(full[1..].iter().all(|s| s.is_empty()));
    }
}
