//! Fill Settings Path / workdir from a dropped item (spec Appendix A).
//!
//! Date tokens are not expanded or inserted — the OS string is used as-is.

use std::path::Path;

use super::launch::is_http_url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotDropField {
    Path,
    Workdir,
}

/// Kind of dropped filesystem item (from extension, case-insensitive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DroppedItemKind {
    ShortcutLnk,
    InternetShortcut,
    Other,
}

pub fn dropped_item_kind(path: &Path) -> DroppedItemKind {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("lnk") => DroppedItemKind::ShortcutLnk,
        Some("url") => DroppedItemKind::InternetShortcut,
        _ => DroppedItemKind::Other,
    }
}

/// Combine OS resolution with the dropped path.
///
/// Unresolved `.lnk` / `.url` must not fall back to the shortcut file path.
/// `resolved` is the `.lnk` target or `.url` address when resolution succeeded.
pub fn payload_for_dropped_item(
    kind: DroppedItemKind,
    resolved: Option<(String, bool)>,
    raw_path: &str,
    raw_is_dir: bool,
) -> Option<(String, bool)> {
    match kind {
        DroppedItemKind::ShortcutLnk | DroppedItemKind::InternetShortcut => resolved,
        DroppedItemKind::Other => {
            let raw = raw_path.trim();
            if raw.is_empty() {
                None
            } else {
                Some((raw.to_string(), raw_is_dir))
            }
        }
    }
}

/// Value to write into the field that received the drop.
///
/// `dropped` is already resolved (`.lnk` target / `.url` address). `is_directory`
/// is the filesystem kind of that resolved path (ignored for `http(s)`).
pub fn text_for_slot_drop(
    dropped: &str,
    is_directory: bool,
    field: SlotDropField,
) -> Option<String> {
    let dropped = dropped.trim();
    if dropped.is_empty() {
        return None;
    }
    if is_http_url(dropped) {
        return match field {
            SlotDropField::Path => Some(dropped.to_string()),
            SlotDropField::Workdir => None,
        };
    }
    match field {
        SlotDropField::Path => Some(dropped.to_string()),
        SlotDropField::Workdir => {
            if is_directory {
                Some(dropped.to_string())
            } else {
                let p = Path::new(dropped);
                let parent = p.parent()?;
                if parent.as_os_str().is_empty() {
                    None
                } else {
                    Some(parent.to_string_lossy().into_owned())
                }
            }
        }
    }
}

/// `[InternetShortcut] URL=` from a `.url` file body.
pub fn url_from_internet_shortcut(text: &str) -> Option<String> {
    for raw in text.lines() {
        let line = raw.trim();
        let Some(rest) = line.strip_prefix("URL=") else {
            continue;
        };
        let url = rest.trim();
        if is_http_url(url) {
            return Some(url.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_keeps_file_and_folder() {
        assert_eq!(
            text_for_slot_drop(r"C:\tools\app.exe", false, SlotDropField::Path).as_deref(),
            Some(r"C:\tools\app.exe")
        );
        assert_eq!(
            text_for_slot_drop(r"C:\tools", true, SlotDropField::Path).as_deref(),
            Some(r"C:\tools")
        );
    }

    #[test]
    fn workdir_uses_parent_of_file() {
        assert_eq!(
            text_for_slot_drop(r"C:\tools\app.exe", false, SlotDropField::Workdir).as_deref(),
            Some(r"C:\tools")
        );
        assert_eq!(
            text_for_slot_drop(r"C:\tools", true, SlotDropField::Workdir).as_deref(),
            Some(r"C:\tools")
        );
    }

    #[test]
    fn url_only_fills_path() {
        assert_eq!(
            text_for_slot_drop("https://example.com/docs", false, SlotDropField::Path).as_deref(),
            Some("https://example.com/docs")
        );
        assert!(
            text_for_slot_drop("https://example.com/docs", false, SlotDropField::Workdir).is_none()
        );
    }

    #[test]
    fn does_not_rewrite_yyyy_tokens() {
        let raw = r"C:\work\{yyyy}\file.txt";
        assert_eq!(
            text_for_slot_drop(raw, false, SlotDropField::Path).as_deref(),
            Some(raw)
        );
    }

    #[test]
    fn parses_internet_shortcut() {
        let body = "[InternetShortcut]\r\nURL=https://example.com/a\r\n";
        assert_eq!(
            url_from_internet_shortcut(body).as_deref(),
            Some("https://example.com/a")
        );
        assert!(url_from_internet_shortcut("URL=ftp://x").is_none());
    }

    #[test]
    fn unresolved_shortcut_does_not_keep_lnk_path() {
        assert_eq!(
            payload_for_dropped_item(
                DroppedItemKind::ShortcutLnk,
                None,
                r"C:\Users\me\Desktop\broken.lnk",
                false,
            ),
            None
        );
        assert_eq!(
            payload_for_dropped_item(
                DroppedItemKind::InternetShortcut,
                None,
                r"C:\Users\me\Desktop\page.url",
                false,
            ),
            None
        );
    }

    #[test]
    fn resolved_shortcut_uses_target() {
        assert_eq!(
            payload_for_dropped_item(
                DroppedItemKind::ShortcutLnk,
                Some((r"C:\tools\app.exe".into(), false)),
                r"C:\Users\me\Desktop\app.lnk",
                false,
            ),
            Some((r"C:\tools\app.exe".into(), false))
        );
    }

    #[test]
    fn ordinary_file_keeps_raw_path() {
        assert_eq!(
            payload_for_dropped_item(DroppedItemKind::Other, None, r"C:\tools\app.exe", false,),
            Some((r"C:\tools\app.exe".into(), false))
        );
    }

    #[test]
    fn kind_from_extension() {
        assert_eq!(
            dropped_item_kind(Path::new(r"C:\Desktop\App.LNK")),
            DroppedItemKind::ShortcutLnk
        );
        assert_eq!(
            dropped_item_kind(Path::new(r"C:\Desktop\docs.URL")),
            DroppedItemKind::InternetShortcut
        );
        assert_eq!(
            dropped_item_kind(Path::new(r"C:\tools\app.exe")),
            DroppedItemKind::Other
        );
    }
}
