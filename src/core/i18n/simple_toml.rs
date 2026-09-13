//! Minimal TOML subset for language packs: `[section]`, `key = "value"`, `#` comments.

use std::collections::HashMap;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TomlError {
    #[error("invalid table header at line {0}")]
    BadHeader(usize),
    #[error("empty table name at line {0}")]
    EmptyTable(usize),
    #[error("invalid key/value at line {0}")]
    BadKeyValue(usize),
    #[error("empty key at line {0}")]
    EmptyKey(usize),
    #[error("invalid escape at line {0}")]
    BadEscape(usize),
}

pub type Sections = HashMap<String, HashMap<String, String>>;

pub fn parse(text: &str) -> Result<Sections, TomlError> {
    let mut sections: Sections = HashMap::new();
    let mut current_name = String::new();
    ensure_section(&mut sections, &current_name);

    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = strip_comment(line).trim().to_string();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('[') {
            if !trimmed.ends_with(']') {
                return Err(TomlError::BadHeader(line_no));
            }
            let name = trimmed[1..trimmed.len() - 1].trim();
            if name.is_empty() {
                return Err(TomlError::EmptyTable(line_no));
            }
            current_name = name.to_string();
            ensure_section(&mut sections, &current_name);
            continue;
        }

        let Some(eq) = trimmed.find('=') else {
            return Err(TomlError::BadKeyValue(line_no));
        };
        if eq == 0 {
            return Err(TomlError::BadKeyValue(line_no));
        }
        let key = trimmed[..eq].trim();
        if key.is_empty() {
            return Err(TomlError::EmptyKey(line_no));
        }
        let raw_value = trimmed[eq + 1..].trim();
        let value = parse_string_value(raw_value, line_no)?;
        sections
            .get_mut(&current_name)
            .expect("section")
            .insert(key.to_string(), value);
    }

    Ok(sections)
}

fn ensure_section(sections: &mut Sections, name: &str) {
    sections
        .entry(name.to_string())
        .or_insert_with(HashMap::new);
}

fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '"' && (i == 0 || bytes[i - 1] != b'\\') {
            in_string = !in_string;
        } else if c == '#' && !in_string {
            return &line[..i];
        }
        i += 1;
    }
    line
}

fn parse_string_value(raw: &str, line_no: usize) -> Result<String, TomlError> {
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        return unescape(&raw[1..raw.len() - 1], line_no);
    }
    Ok(raw.to_string())
}

fn unescape(value: &str, line_no: usize) -> Result<String, TomlError> {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(next) = chars.next() else {
            return Err(TomlError::BadEscape(line_no));
        };
        match next {
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            _ => return Err(TomlError::BadEscape(line_no)),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meta_and_ui() {
        let text = r#"
# comment
[meta]
language = "ja"
name = "日本語"

[ui]
tray_quit = "終了"
note = "line\n tw o"
"#;
        let s = parse(text).unwrap();
        assert_eq!(s["meta"]["language"], "ja");
        assert_eq!(s["meta"]["name"], "日本語");
        assert_eq!(s["ui"]["tray_quit"], "終了");
        assert_eq!(s["ui"]["note"], "line\n tw o");
    }
}
