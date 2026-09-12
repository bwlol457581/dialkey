//! Slot registry — string IDs with leading zeros preserved.

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Slot {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub workdir: String,
    /// When true, also open the working directory (or Path's parent) in Explorer.
    /// Ignored for URL / `chain:` slots. Default false; omitted from JSON when false.
    #[serde(
        default,
        rename = "openWorkdir",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub open_workdir: bool,
    /// When true (default), try best-effort "already open → focus" before spawn.
    /// Set `false` to always launch a new instance for this slot. Omitted when true.
    #[serde(
        default = "default_true",
        rename = "focusExisting",
        skip_serializing_if = "is_true"
    )]
    pub focus_existing: bool,
}

fn default_true() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SlotRegistry {
    /// Insertion order preserved for "first wins" on duplicate ids.
    slots: Vec<Slot>,
    by_id: HashMap<String, usize>,
}

impl SlotRegistry {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(slots: Vec<Slot>) -> Self {
        let mut reg = Self::default();
        for slot in slots {
            reg.insert(slot);
        }
        reg
    }

    pub fn insert(&mut self, slot: Slot) {
        if !is_valid_slot_id(&slot.id) {
            warn!("skipping slot with invalid id (digits only): {:?}", slot.id);
            return;
        }
        if self.by_id.contains_key(&slot.id) {
            warn!("duplicate slot id {:?}; keeping first", slot.id);
            return;
        }
        let idx = self.slots.len();
        self.by_id.insert(slot.id.clone(), idx);
        self.slots.push(slot);
    }

    pub fn get(&self, id: &str) -> Option<&Slot> {
        self.by_id.get(id).map(|&i| &self.slots[i])
    }

    pub fn all(&self) -> &[Slot] {
        &self.slots
    }

    /// Natural-sorted view for settings / Search lists.
    pub fn sorted(&self) -> Vec<&Slot> {
        let mut out: Vec<&Slot> = self.slots.iter().collect();
        out.sort_by(|a, b| natural_cmp_id(&a.id, &b.id));
        out
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let Some(&idx) = self.by_id.get(id) else {
            return false;
        };
        self.slots.remove(idx);
        self.rebuild_index();
        true
    }

    /// Insert or replace by id. Returns false if the id is invalid.
    pub fn upsert(&mut self, slot: Slot) -> bool {
        if !is_valid_slot_id(&slot.id) {
            warn!("skipping slot with invalid id (digits only): {:?}", slot.id);
            return false;
        }
        if let Some(&idx) = self.by_id.get(&slot.id) {
            self.slots[idx] = slot;
            true
        } else {
            let idx = self.slots.len();
            self.by_id.insert(slot.id.clone(), idx);
            self.slots.push(slot);
            true
        }
    }

    /// Exchange the ids of two existing slots. Instant flags are not moved
    /// (they stay keyed by digit). Returns false if either id is missing or equal.
    pub fn swap_ids(&mut self, a: &str, b: &str) -> bool {
        if a == b {
            return false;
        }
        let Some(&ia) = self.by_id.get(a) else {
            return false;
        };
        let Some(&ib) = self.by_id.get(b) else {
            return false;
        };
        self.slots[ia].id = b.to_string();
        self.slots[ib].id = a.to_string();
        self.rebuild_index();
        true
    }

    fn rebuild_index(&mut self) {
        self.by_id.clear();
        for (i, slot) in self.slots.iter().enumerate() {
            self.by_id.insert(slot.id.clone(), i);
        }
    }

    /// Instant slots for the idle view, natural-sorted by id.
    /// (Idle dial currently paints 0–9 fixed rows; kept for callers/tests.)
    #[allow(dead_code)]
    pub fn instant_slots(&self, instant: &HashMap<String, bool>) -> Vec<&Slot> {
        let mut out: Vec<&Slot> = self
            .slots
            .iter()
            .filter(|s| instant.get(&s.id).copied().unwrap_or(false))
            .collect();
        out.sort_by(|a, b| natural_cmp_id(&a.id, &b.id));
        out
    }

    /// Prefix match over all slots (kept for tests / Search-style callers).
    #[allow(dead_code)]
    pub fn prefix_matches(&self, prefix: &str) -> Vec<&Slot> {
        let mut out: Vec<&Slot> = self
            .slots
            .iter()
            .filter(|s| s.id.starts_with(prefix))
            .collect();
        out.sort_by(|a, b| natural_cmp_id(&a.id, &b.id));
        out
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }
}

pub fn is_valid_slot_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_digit())
}

/// Pairs of slot ids that look alike because of leading zeros (e.g. `"1"` / `"01"`).
pub fn leading_zero_conflicts(slots: &[Slot]) -> Vec<(String, String)> {
    let mut by_value: HashMap<u128, Vec<String>> = HashMap::new();
    for slot in slots {
        if let Some(n) = parse_u128(&slot.id) {
            by_value.entry(n).or_default().push(slot.id.clone());
        }
    }
    let mut out = Vec::new();
    for ids in by_value.values() {
        if ids.len() < 2 {
            continue;
        }
        let mut sorted = ids.clone();
        sorted.sort_by(|a, b| natural_cmp_id(a, b));
        for i in 0..sorted.len() {
            for j in (i + 1)..sorted.len() {
                out.push((sorted[i].clone(), sorted[j].clone()));
            }
        }
    }
    out.sort_by(|a, b| natural_cmp_id(&a.0, &b.0).then_with(|| natural_cmp_id(&a.1, &b.1)));
    out
}

/// Natural order: `"2"` before `"10"`. Leading zeros make distinct ids;
/// compare as integer values when both are pure digits, then by length/string
/// so `"01"` and `"1"` stay distinct but sort near each other.
pub fn natural_cmp_id(a: &str, b: &str) -> Ordering {
    match (parse_u128(a), parse_u128(b)) {
        (Some(na), Some(nb)) => match na.cmp(&nb) {
            Ordering::Equal => a.len().cmp(&b.len()).then_with(|| a.cmp(b)),
            other => other,
        },
        _ => a.cmp(b),
    }
}

fn parse_u128(s: &str) -> Option<u128> {
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Deserialize a slot id that may be a JSON string or integer.
pub fn coerce_slot_id(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_ids_preserve_leading_zeros() {
        let mut reg = SlotRegistry::default();
        reg.insert(Slot {
            id: "01".into(),
            name: "A".into(),
            path: "a".into(),
            description: String::new(),
            workdir: String::new(),
            open_workdir: false,
            focus_existing: true,
        });
        reg.insert(Slot {
            id: "1".into(),
            name: "B".into(),
            path: "b".into(),
            description: String::new(),
            workdir: String::new(),
            open_workdir: false,
            focus_existing: true,
        });
        assert!(reg.get("01").is_some());
        assert!(reg.get("1").is_some());
        assert_ne!(reg.get("01").unwrap().name, reg.get("1").unwrap().name);
    }

    #[test]
    fn natural_sort_order() {
        let mut ids = vec!["10", "2", "01", "1"];
        ids.sort_by(|a, b| natural_cmp_id(a, b));
        // Same numeric value: shorter id first, so "1" before "01"
        assert_eq!(ids, vec!["1", "01", "2", "10"]);
    }

    #[test]
    fn duplicate_first_wins() {
        let mut reg = SlotRegistry::default();
        reg.insert(Slot {
            id: "1".into(),
            name: "first".into(),
            path: "a".into(),
            description: String::new(),
            workdir: String::new(),
            open_workdir: false,
            focus_existing: true,
        });
        reg.insert(Slot {
            id: "1".into(),
            name: "second".into(),
            path: "b".into(),
            description: String::new(),
            workdir: String::new(),
            open_workdir: false,
            focus_existing: true,
        });
        assert_eq!(reg.get("1").unwrap().name, "first");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn detects_leading_zero_conflicts() {
        let slots = vec![
            Slot {
                id: "1".into(),
                name: "A".into(),
                path: "a".into(),
                description: String::new(),
                workdir: String::new(),
                open_workdir: false,
                focus_existing: true,
            },
            Slot {
                id: "01".into(),
                name: "B".into(),
                path: "b".into(),
                description: String::new(),
                workdir: String::new(),
                open_workdir: false,
                focus_existing: true,
            },
            Slot {
                id: "2".into(),
                name: "C".into(),
                path: "c".into(),
                description: String::new(),
                workdir: String::new(),
                open_workdir: false,
                focus_existing: true,
            },
        ];
        let c = leading_zero_conflicts(&slots);
        assert_eq!(c, vec![("1".into(), "01".into())]);
    }

    #[test]
    fn swap_ids_exchanges_content() {
        let mut reg = SlotRegistry::default();
        reg.insert(Slot {
            id: "1".into(),
            name: "A".into(),
            path: "a".into(),
            description: String::new(),
            workdir: String::new(),
            open_workdir: false,
            focus_existing: true,
        });
        reg.insert(Slot {
            id: "3".into(),
            name: "B".into(),
            path: "b".into(),
            description: String::new(),
            workdir: String::new(),
            open_workdir: true,
            focus_existing: true,
        });
        assert!(reg.swap_ids("1", "3"));
        assert_eq!(reg.get("1").unwrap().name, "B");
        assert_eq!(reg.get("3").unwrap().name, "A");
        assert!(reg.get("1").unwrap().open_workdir);
        assert!(!reg.get("3").unwrap().open_workdir);
    }

    #[test]
    fn prefix_match() {
        let reg = SlotRegistry::new(vec![
            Slot {
                id: "1".into(),
                name: "One".into(),
                path: "a".into(),
                description: String::new(),
                workdir: String::new(),
                open_workdir: false,
                focus_existing: true,
            },
            Slot {
                id: "11".into(),
                name: "Eleven".into(),
                path: "b".into(),
                description: String::new(),
                workdir: String::new(),
                open_workdir: false,
                focus_existing: true,
            },
            Slot {
                id: "2".into(),
                name: "Two".into(),
                path: "c".into(),
                description: String::new(),
                workdir: String::new(),
                open_workdir: false,
                focus_existing: true,
            },
        ]);
        let m = reg.prefix_matches("1");
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].id, "1");
        assert_eq!(m[1].id, "11");
    }

    #[test]
    fn focus_existing_defaults_true_and_omits_when_true() {
        let raw = r#"{"id":"1","name":"A","path":"a.exe"}"#;
        let slot: Slot = serde_json::from_str(raw).unwrap();
        assert!(slot.focus_existing);
        let out = serde_json::to_value(&slot).unwrap();
        assert!(out.get("focusExisting").is_none());

        let off = Slot {
            id: "2".into(),
            name: "B".into(),
            path: "b.exe".into(),
            description: String::new(),
            workdir: String::new(),
            open_workdir: false,
            focus_existing: false,
        };
        let v = serde_json::to_value(&off).unwrap();
        assert_eq!(
            v.get("focusExisting"),
            Some(&serde_json::Value::Bool(false))
        );
    }
}
