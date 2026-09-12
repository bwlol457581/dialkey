//! `chain:` special command — parse, validate, and walk one level.

use std::collections::{HashMap, HashSet};

use super::slots::{is_valid_slot_id, Slot};

const CHAIN_PREFIX: &str = "chain:";

/// True when `path` is a `chain:` special command (scheme-style, like URLs).
pub fn is_chain_path(path: &str) -> bool {
    path.trim()
        .get(..CHAIN_PREFIX.len())
        .map(|s| s.eq_ignore_ascii_case(CHAIN_PREFIX))
        .unwrap_or(false)
}

/// Parse `chain:11,20,31` into ordered target ids. Whitespace around ids is stripped.
///
/// Returns `Err` with a short English reason when the prefix is wrong, empty, or
/// an id is not digits-only.
pub fn parse_chain_ids(path: &str) -> Result<Vec<String>, String> {
    let trimmed = path.trim();
    if !is_chain_path(trimmed) {
        return Err("path is not a chain: command".into());
    }
    let rest = trimmed[CHAIN_PREFIX.len()..].trim();
    if rest.is_empty() {
        return Err("chain: has no target ids".into());
    }
    let mut ids = Vec::new();
    for part in rest.split(',') {
        let id = part.trim();
        if id.is_empty() {
            continue;
        }
        if !is_valid_slot_id(id) {
            return Err(format!("chain target id must be digits only (got {id:?})"));
        }
        ids.push(id.to_string());
    }
    if ids.is_empty() {
        return Err("chain: has no target ids".into());
    }
    Ok(ids)
}

/// Blocking validation errors for chain slots (must not save).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainBlockError {
    /// Invalid `chain:` syntax or non-digit id.
    Invalid { slot_id: String, message: String },
    /// Self-reference or multi-hop cycle among chain edges.
    Cycle { path: Vec<String> },
}

impl std::fmt::Display for ChainBlockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid { slot_id, message } => {
                write!(f, "slot {slot_id}: {message}")
            }
            Self::Cycle { path } => {
                write!(f, "chain cycle: {}", path.join(" → "))
            }
        }
    }
}

/// Non-blocking warnings (missing targets) shown in the settings error strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainMissingWarning {
    pub slot_id: String,
    pub missing: Vec<String>,
}

/// Collect chain edges from every `chain:` slot. Invalid syntax → `Err`.
fn chain_edges(slots: &[Slot]) -> Result<HashMap<String, Vec<String>>, ChainBlockError> {
    let mut edges = HashMap::new();
    for slot in slots {
        if !is_chain_path(&slot.path) {
            continue;
        }
        match parse_chain_ids(&slot.path) {
            Ok(targets) => {
                edges.insert(slot.id.clone(), targets);
            }
            Err(message) => {
                return Err(ChainBlockError::Invalid {
                    slot_id: slot.id.clone(),
                    message,
                });
            }
        }
    }
    Ok(edges)
}

/// Validate all chain slots: syntax + cycles. Missing targets are warnings only.
pub fn validate_chains(slots: &[Slot]) -> Result<Vec<ChainMissingWarning>, ChainBlockError> {
    let edges = chain_edges(slots)?;
    if let Some(cycle) = find_cycle(&edges) {
        return Err(ChainBlockError::Cycle { path: cycle });
    }

    let known: HashSet<&str> = slots.iter().map(|s| s.id.as_str()).collect();
    let mut warnings = Vec::new();
    for (from, targets) in &edges {
        let missing: Vec<String> = targets
            .iter()
            .filter(|id| !known.contains(id.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            warnings.push(ChainMissingWarning {
                slot_id: from.clone(),
                missing,
            });
        }
    }
    warnings.sort_by(|a, b| a.slot_id.cmp(&b.slot_id));
    Ok(warnings)
}

/// DFS cycle detection over the chain edge graph (1-hop edges only; still
/// catches A→B→A when B is also a chain).
fn find_cycle(edges: &HashMap<String, Vec<String>>) -> Option<Vec<String>> {
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    let mut stack = Vec::new();

    fn dfs(
        node: &str,
        edges: &HashMap<String, Vec<String>>,
        visiting: &mut HashSet<String>,
        visited: &mut HashSet<String>,
        stack: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        if visited.contains(node) {
            return None;
        }
        if visiting.contains(node) {
            let start = stack.iter().position(|id| id == node).unwrap_or(0);
            let mut path: Vec<String> = stack[start..].to_vec();
            path.push(node.to_string());
            return Some(path);
        }
        visiting.insert(node.to_string());
        stack.push(node.to_string());
        if let Some(targets) = edges.get(node) {
            for t in targets {
                // Only follow edges that are themselves chain nodes, or self-ref.
                // Self-reference: edge A→A even when A is the only chain.
                if t == node {
                    let mut path = stack.clone();
                    path.push(t.clone());
                    return Some(path);
                }
                if edges.contains_key(t.as_str()) {
                    if let Some(c) = dfs(t, edges, visiting, visited, stack) {
                        return Some(c);
                    }
                }
            }
        }
        stack.pop();
        visiting.remove(node);
        visited.insert(node.to_string());
        None
    }

    let mut nodes: Vec<&String> = edges.keys().collect();
    nodes.sort();
    for n in nodes {
        if let Some(c) = dfs(n, edges, &mut visiting, &mut visited, &mut stack) {
            return Some(c);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: &str, path: &str) -> Slot {
        Slot {
            id: id.into(),
            name: id.into(),
            path: path.into(),
            description: String::new(),
            workdir: String::new(),
            open_workdir: false,
            focus_existing: true,
        }
    }

    #[test]
    fn detects_chain_prefix() {
        assert!(is_chain_path("chain:1,2"));
        assert!(is_chain_path("  CHAIN:11  "));
        assert!(!is_chain_path("https://example.com"));
        assert!(!is_chain_path("notepad.exe"));
    }

    #[test]
    fn parses_ids_and_whitespace() {
        assert_eq!(
            parse_chain_ids("chain:11, 20,31").unwrap(),
            vec!["11", "20", "31"]
        );
    }

    #[test]
    fn rejects_bad_ids() {
        assert!(parse_chain_ids("chain:11,ab").is_err());
        assert!(parse_chain_ids("chain:").is_err());
    }

    #[test]
    fn self_ref_is_cycle() {
        let slots = vec![slot("5", "chain:5")];
        let err = validate_chains(&slots).unwrap_err();
        assert!(matches!(err, ChainBlockError::Cycle { .. }));
    }

    #[test]
    fn mutual_chain_is_cycle() {
        let slots = vec![slot("1", "chain:2"), slot("2", "chain:1")];
        assert!(matches!(
            validate_chains(&slots),
            Err(ChainBlockError::Cycle { .. })
        ));
    }

    #[test]
    fn missing_target_is_warning_not_block() {
        let slots = vec![slot("5", "chain:11,99"), slot("11", "calc.exe")];
        let warnings = validate_chains(&slots).unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].slot_id, "5");
        assert_eq!(warnings[0].missing, vec!["99".to_string()]);
    }

    #[test]
    fn valid_chain_ok() {
        let slots = vec![
            slot("5", "chain:11,20"),
            slot("11", "calc.exe"),
            slot("20", "notepad.exe"),
        ];
        assert!(validate_chains(&slots).unwrap().is_empty());
    }
}
