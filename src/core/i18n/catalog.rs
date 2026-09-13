//! English embedded + optional external pack overlay.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use tracing::warn;

use super::english::lookup_english;
use super::simple_toml;
use super::LANGUAGE_KEY_TARGET_MAJOR;

#[derive(Debug, Clone)]
pub struct LanguagePackMeta {
    pub language: String,
    pub name: String,
    pub pack_version: String,
    pub target: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct LanguageCatalog {
    overlay: HashMap<String, String>,
    resolved_language: String,
    pack: Option<LanguagePackMeta>,
    used_fallback: bool,
}

impl LanguageCatalog {
    pub fn english() -> Self {
        Self {
            overlay: HashMap::new(),
            resolved_language: "en".into(),
            pack: None,
            used_fallback: false,
        }
    }

    pub fn resolved_language(&self) -> &str {
        &self.resolved_language
    }

    pub fn pack(&self) -> Option<&LanguagePackMeta> {
        self.pack.as_ref()
    }

    pub fn used_fallback(&self) -> bool {
        self.used_fallback
    }

    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        if let Some(v) = self.overlay.get(key) {
            if !v.is_empty() {
                return v.as_str();
            }
        }
        lookup_english(key).unwrap_or(key)
    }

    pub fn format(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut s = self.get(key).to_string();
        for (name, value) in args {
            s = s.replace(&format!("{{{name}}}"), value);
        }
        s
    }

    pub fn version_line(&self) -> Option<String> {
        self.pack.as_ref().map(|p| {
            format!(
                "Language pack: {} {} (target {})",
                p.language, p.pack_version, p.target
            )
        })
    }
}

/// Resolve catalog per spec §13-5.
///
/// `requested_locale`: `settings.ui.locale` (`""` = follow OS, `en` = embedded).
/// `os_lang`: short language code from the OS (`ja`, `vi`, `en`, …), if known.
pub fn resolve(requested_locale: &str, lang_dir: &Path, os_lang: Option<&str>) -> LanguageCatalog {
    let explicit = normalize_code(requested_locale);
    if let Some(code) = explicit {
        if is_english(&code) {
            return LanguageCatalog::english();
        }
        return resolve_from_code(&code, lang_dir, true);
    }

    let Some(detected) = os_lang.map(str::trim).filter(|s| !s.is_empty()) else {
        return LanguageCatalog::english();
    };
    let code = extract_language_code(detected);
    if is_english(&code) {
        return LanguageCatalog::english();
    }
    resolve_from_code(&code, lang_dir, false)
}

fn resolve_from_code(code: &str, lang_dir: &Path, warn_if_missing: bool) -> LanguageCatalog {
    match try_load_pack(lang_dir, code) {
        Ok(pack) => {
            if !is_target_compatible(&pack.meta.target) {
                warn!(
                    language = %code,
                    target = %pack.meta.target,
                    app_target = LANGUAGE_KEY_TARGET_MAJOR,
                    "language pack target incompatible; using English"
                );
                return LanguageCatalog {
                    overlay: HashMap::new(),
                    resolved_language: "en".into(),
                    pack: None,
                    used_fallback: true,
                };
            }
            LanguageCatalog {
                overlay: pack.ui,
                resolved_language: pack.meta.language.clone(),
                pack: Some(pack.meta),
                used_fallback: false,
            }
        }
        Err(e) => {
            if warn_if_missing {
                warn!(language = %code, error = %e, "language pack unavailable; using English");
            }
            LanguageCatalog {
                overlay: HashMap::new(),
                resolved_language: "en".into(),
                pack: None,
                used_fallback: true,
            }
        }
    }
}

struct LoadedPack {
    meta: LanguagePackMeta,
    ui: HashMap<String, String>,
}

fn try_load_pack(lang_dir: &Path, code: &str) -> Result<LoadedPack, String> {
    let path = lang_dir.join(format!("{code}.toml"));
    if !path.is_file() {
        return Err(format!("not found: {}", path.display()));
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let sections = simple_toml::parse(&text).map_err(|e| e.to_string())?;
    let meta = sections
        .get("meta")
        .ok_or_else(|| "missing [meta]".to_string())?;
    let language = require(meta, "language")?;
    let pack_version = require(meta, "pack_version")?;
    let target = require(meta, "target")?;
    let name = meta
        .get("name")
        .cloned()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| language.clone());
    let ui = sections.get("ui").cloned().unwrap_or_default();
    Ok(LoadedPack {
        meta: LanguagePackMeta {
            language,
            name,
            pack_version,
            target,
            path,
        },
        ui,
    })
}

fn require(map: &HashMap<String, String>, key: &str) -> Result<String, String> {
    map.get(key)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("missing meta.{key}"))
}

pub fn is_target_compatible(target: &str) -> bool {
    major(target) == major(LANGUAGE_KEY_TARGET_MAJOR)
}

fn major(version: &str) -> Option<u32> {
    let digits: String = version
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

pub fn normalize_code(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    Some(extract_language_code(t))
}

pub fn extract_language_code(tag: &str) -> String {
    let primary = tag
        .split(['-', '_'])
        .next()
        .unwrap_or(tag)
        .trim()
        .to_ascii_lowercase();
    primary
}

fn is_english(code: &str) -> bool {
    code.eq_ignore_ascii_case("en")
}

/// Lightweight listing for the Settings language combo (meta only).
#[derive(Debug, Clone)]
pub struct AvailablePack {
    pub language: String,
    pub name: String,
}

pub fn list_available_packs(lang_dir: &Path) -> Vec<AvailablePack> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(lang_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if is_english(stem) {
            continue;
        }
        match try_load_pack(lang_dir, stem) {
            Ok(pack) if is_target_compatible(&pack.meta.target) => {
                out.push(AvailablePack {
                    language: pack.meta.language,
                    name: pack.meta.name,
                });
            }
            Ok(pack) => {
                warn!(
                    path = %pack.meta.path.display(),
                    target = %pack.meta.target,
                    "skipping incompatible language pack"
                );
            }
            Err(e) => {
                warn!(path = %path.display(), error = %e, "skipping broken language pack");
            }
        }
    }
    out.sort_by(|a, b| a.language.cmp(&b.language));
    out
}

// ── Process-wide active catalog ─────────────────────────────────────────

static ACTIVE: OnceLock<RwLock<Arc<LanguageCatalog>>> = OnceLock::new();

fn active_lock() -> &'static RwLock<Arc<LanguageCatalog>> {
    ACTIVE.get_or_init(|| RwLock::new(Arc::new(LanguageCatalog::english())))
}

pub fn install(catalog: LanguageCatalog) {
    *active_lock().write().unwrap() = Arc::new(catalog);
}

pub fn active() -> Arc<LanguageCatalog> {
    active_lock().read().unwrap().clone()
}

pub fn t(key: &str) -> String {
    active().get(key).to_string()
}

pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    active().format(key, args)
}

pub fn lang_dir_next_to_exe(exe_dir: &Path) -> PathBuf {
    let next = exe_dir.join("lang");
    if next.is_dir() {
        return next;
    }
    // Debug convenience: `cargo run` keeps packs in the repo `lang/` folder.
    if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
        let candidate = PathBuf::from(manifest).join("lang");
        if candidate.is_dir() {
            return candidate;
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn english_has_all_keys() {
        for key in super::super::keys::ALL_KEYS {
            assert!(
                lookup_english(key).is_some(),
                "missing english default for {key}"
            );
        }
        assert_eq!(
            super::super::english::english_defaults().len(),
            super::super::keys::ALL_KEYS.len()
        );
    }

    #[test]
    fn overlay_falls_back_per_key() {
        let mut overlay = HashMap::new();
        overlay.insert("tray_menu_quit".into(), "終了".into());
        let cat = LanguageCatalog {
            overlay,
            resolved_language: "ja".into(),
            pack: None,
            used_fallback: false,
        };
        assert_eq!(cat.get("tray_menu_quit"), "終了");
        assert_eq!(cat.get("tray_menu_settings"), "Settings");
    }

    #[test]
    fn target_major_check() {
        assert!(is_target_compatible("1.0"));
        assert!(is_target_compatible("1.2.3"));
        assert!(!is_target_compatible("2.0"));
    }

    #[test]
    fn loads_pack_from_temp() {
        let dir = tempfile_dir();
        let path = dir.join("ja.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(
            f,
            r#"[meta]
language = "ja"
name = "日本語"
pack_version = "1.0.0"
target = "1.0"

[ui]
tray_menu_quit = "終了"
"#
        )
        .unwrap();
        let cat = resolve("ja", &dir, None);
        assert_eq!(cat.resolved_language(), "ja");
        assert_eq!(cat.get("tray_menu_quit"), "終了");
        assert_eq!(cat.get("tray_menu_settings"), "Settings");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn tempfile_dir() -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("dialkey-i18n-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
