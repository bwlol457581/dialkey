//! Load `settings.json` / `slots.json` with BOM tolerance and safe failure.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tracing::{error, info, warn};

use super::keys::KeyBindings;
use super::legend::LegendSettings;
use super::search::SearchSettings;
use super::slots::{coerce_slot_id, is_valid_slot_id, Slot, SlotRegistry};

const CURRENT_SCHEMA: u32 = 1;
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("settings schemaVersion {found} is newer than supported {CURRENT_SCHEMA}")]
    SettingsTooNew { found: u32 },
    #[error("slots schemaVersion {found} is newer than supported {CURRENT_SCHEMA}")]
    SlotsTooNew { found: u32 },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialize error: {0}")]
    Serialize(String),
    #[error("backup destination cannot be inside the config folder")]
    BackupIntoConfig,
    #[error("restore source cannot be the live config folder")]
    RestoreFromLive,
    #[error("nothing to restore")]
    RestoreNothing,
    #[error("current slots file is read-only")]
    RestoreCurrentReadOnly,
    #[error("current slots file is not in the backup folder")]
    RestoreCurrentMissing,
}

/// Which personal slot books to copy back from a backup stamp folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreSlotsScope {
    AllBooks,
    CurrentBook,
}

/// Basenames written by a successful restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreReport {
    pub copied: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_triggers")]
    pub triggers: Vec<Value>,
    #[serde(default)]
    pub keys: KeyBindings,
    #[serde(default = "default_instant")]
    pub instant: HashMap<String, bool>,
    #[serde(default)]
    pub search: SearchSettings,
    #[serde(default)]
    pub ui: UiSettings,
    #[serde(default = "default_timeout")]
    pub timeout_sec: u32,
    #[serde(default = "default_max_digits")]
    pub max_digits: usize,
    /// Acceptance feedback duration in ms (typing window stays open this long).
    /// `0` disables. Default 500.
    #[serde(default = "default_feedback_ms")]
    pub feedback_ms: u32,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub docs_url: String,
    /// Enable HKCU Run-key autostart (self-healing on each launch).
    #[serde(default)]
    pub autostart: bool,
    /// Basename of the slots JSON in the config dir.
    /// Default `"slots.json"`. Empty string → load `slots.example.json` (read-only).
    #[serde(default = "default_slots_file")]
    pub slots_file: String,
    /// Manual mode switch (§4 / §6). Omitted keys use defaults.
    #[serde(default)]
    pub mode_switch: ModeSwitchSettings,
    /// Typing-window subcommand legend (Idle vs Narrow down). Omitted → shipped defaults.
    #[serde(default)]
    pub legend: LegendSettings,
    /// Absolute folder for JSON backups. Empty → Desktop at backup time.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub backup_folder: String,
}

/// Leader mouse + optional chord overrides for swapping `slotsFile`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeSwitchSettings {
    #[serde(default = "default_mode_mouse")]
    pub mouse_button: String,
    #[serde(default = "default_true")]
    pub suppress: bool,
    /// Arm timeout after leader click. `0` disables mouse mode-switch.
    #[serde(default = "default_chord_timeout_ms")]
    pub chord_timeout_ms: u32,
    /// Optional chord → slots basename overrides (`"1"` → `"slots.work.json"`).
    #[serde(default)]
    pub keys: HashMap<String, String>,
}

impl Default for ModeSwitchSettings {
    fn default() -> Self {
        Self {
            mouse_button: default_mode_mouse(),
            suppress: true,
            chord_timeout_ms: default_chord_timeout_ms(),
            keys: HashMap::new(),
        }
    }
}

fn default_mode_mouse() -> String {
    "x1".into()
}

fn default_true() -> bool {
    true
}

fn default_chord_timeout_ms() -> u32 {
    2000
}

fn default_instant() -> HashMap<String, bool> {
    (0..10).map(|d| (d.to_string(), false)).collect()
}

fn default_timeout() -> u32 {
    30
}

fn default_max_digits() -> usize {
    10
}

fn default_feedback_ms() -> u32 {
    500
}

fn default_log_level() -> String {
    "warn".into()
}

fn default_slots_file() -> String {
    "slots.json".into()
}

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA
}

/// Shipped launch triggers (mouse X2 + empty hotkey + tray).
fn default_triggers() -> Vec<Value> {
    vec![
        serde_json::json!({ "type": "mouse", "button": "x2", "suppress": true }),
        serde_json::json!({ "type": "hotkey", "key": "" }),
        serde_json::json!({ "type": "tray" }),
    ]
}

/// Shipped sample; never overwritten by DialKey.
pub const SLOTS_EXAMPLE_FILE: &str = "slots.example.json";

/// Portable JSON folder next to the exe (`settings.json`, `slots*.json`).
pub const SETTINGS_DIR_NAME: &str = "settings";

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA,
            triggers: default_triggers(),
            keys: {
                let mut k = KeyBindings::default();
                k.normalize();
                k
            },
            instant: default_instant(),
            search: SearchSettings::default(),
            ui: UiSettings::default(),
            timeout_sec: default_timeout(),
            max_digits: default_max_digits(),
            feedback_ms: default_feedback_ms(),
            log_level: default_log_level(),
            docs_url: String::new(),
            autostart: false,
            slots_file: default_slots_file(),
            mode_switch: ModeSwitchSettings::default(),
            legend: LegendSettings::default(),
            backup_folder: String::new(),
        }
    }
}

/// UI preferences (language pack selection).
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UiSettings {
    /// `""` = follow OS UI language; `en` = embedded English; otherwise `lang/<code>.toml`.
    #[serde(default)]
    pub locale: String,
    /// Show Mode / Keys / Triggers / Search and extra General fields.
    /// Omitted / `false` = Slots + essential General only.
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_advanced: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub dir: PathBuf,
    pub settings: Settings,
    pub slots: SlotRegistry,
    /// Absolute path of the slots file that was loaded (may be the example).
    pub slots_path: PathBuf,
    /// Set when `slots.json` was created on this launch (open Settings once).
    pub open_settings_on_start: bool,
    /// `false` if the on-disk file was unreadable — must not overwrite it.
    pub settings_writable: bool,
    /// `false` if the slots file is the example, missing (fell back), or unreadable.
    pub slots_writable: bool,
}

/// Resolve the config directory.
///
/// Order: `--config` arg → `{exe}/settings/` (after migrating exe-adjacent JSON) →
/// `{exe}/config` → `{CARGO_MANIFEST_DIR}/config` (debug) → create `{exe}/settings`.
pub fn resolve_config_dir(
    exe_dir: &Path,
    cli_config: Option<&Path>,
) -> Result<PathBuf, ConfigError> {
    if let Some(dir) = cli_config {
        return Ok(dir.to_path_buf());
    }
    let nested = exe_dir.join(SETTINGS_DIR_NAME);
    if nested.join("settings.json").is_file() {
        return Ok(nested);
    }
    if exe_dir.join("settings.json").is_file() {
        match migrate_json_into_settings_dir(exe_dir) {
            Ok(dir) if dir.join("settings.json").is_file() => return Ok(dir),
            Err(e) => {
                warn!(error = %e, "settings-folder migrate failed; using exe directory");
                return Ok(exe_dir.to_path_buf());
            }
            _ => return Ok(exe_dir.to_path_buf()),
        }
    }
    if exe_dir.join("config").join("settings.json").is_file() {
        return Ok(exe_dir.join("config"));
    }
    if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
        let candidate = PathBuf::from(manifest).join("config");
        if candidate.join("settings.json").is_file() {
            return Ok(candidate);
        }
    }
    fs::create_dir_all(&nested)?;
    Ok(nested)
}

/// Move `settings.json` / `slots*.json` from `exe_dir` into `{exe}/settings/`
/// when the destination file does not already exist (never overwrite).
fn migrate_json_into_settings_dir(exe_dir: &Path) -> Result<PathBuf, ConfigError> {
    let dest = exe_dir.join(SETTINGS_DIR_NAME);
    fs::create_dir_all(&dest)?;
    for name in json_files_in_dir(exe_dir) {
        let src = exe_dir.join(&name);
        let dst = dest.join(&name);
        if !src.is_file() || dst.is_file() {
            continue;
        }
        match fs::rename(&src, &dst) {
            Ok(()) => info!(file = %name, "moved JSON into settings/"),
            Err(_) => {
                fs::copy(&src, &dst)?;
                fs::remove_file(&src)?;
                info!(file = %name, "copied JSON into settings/ (rename failed)");
            }
        }
    }
    Ok(dest)
}

/// `settings.json` and `slots*.json` basenames in `dir` (not recursive, not `backup/`).
fn json_files_in_dir(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return names;
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".json.tmp") {
            continue;
        }
        let keep =
            lower == "settings.json" || (lower.starts_with("slots") && lower.ends_with(".json"));
        if keep {
            names.push(name.to_string());
        }
    }
    names
}

pub fn load_app_config(
    exe_dir: &Path,
    cli_config: Option<&Path>,
) -> Result<AppConfig, ConfigError> {
    let dir = resolve_config_dir(exe_dir, cli_config)?;

    let (settings, settings_writable) = match load_settings(&dir.join("settings.json")) {
        Ok(s) => (s, true),
        Err(ConfigError::SettingsTooNew { found }) => {
            return Err(ConfigError::SettingsTooNew { found });
        }
        Err(e) => {
            error!(
                "failed to load settings.json: {e}; using defaults (file will not be overwritten)"
            );
            (Settings::default(), false)
        }
    };

    let open_settings_on_start = ensure_default_slots_file(&dir, &settings.slots_file)?;
    let (slots_path, slots, slots_writable) = load_slots_for_settings(&dir, &settings.slots_file)?;

    info!(
        config_dir = %dir.display(),
        slots_file = %slots_path.display(),
        slots = slots.len(),
        open_settings_on_start,
        settings_writable,
        slots_writable,
        "config loaded"
    );

    Ok(AppConfig {
        dir,
        settings,
        slots,
        slots_path,
        open_settings_on_start,
        settings_writable,
        slots_writable,
    })
}

/// Sanitize a `slotsFile` setting to a basename (no directories).
/// Empty → example (read-only). Rejects `..` and path separators.
pub fn normalize_slots_file_name(slots_file: &str) -> String {
    let trimmed = slots_file.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let base = Path::new(trimmed)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .trim();
    if base.is_empty() || base == "." || base == ".." {
        return String::new();
    }
    if base.contains('/') || base.contains('\\') {
        return String::new();
    }
    base.to_string()
}

/// Whether DialKey may write this slots selection (`""` / example → no).
pub fn slots_file_is_writable(slots_file: &str) -> bool {
    let name = normalize_slots_file_name(slots_file);
    if name.is_empty() {
        return false;
    }
    !name.eq_ignore_ascii_case(SLOTS_EXAMPLE_FILE)
}

/// Resolve the on-disk path for a `slotsFile` value (basename only).
pub fn resolve_slots_path(dir: &Path, slots_file: &str) -> PathBuf {
    let name = normalize_slots_file_name(slots_file);
    if name.is_empty() {
        dir.join(SLOTS_EXAMPLE_FILE)
    } else {
        dir.join(name)
    }
}

/// Personal `slots*.json` files that actually exist (excludes the shipped example).
fn scan_existing_slots_files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return names;
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        if !lower.starts_with("slots") || !lower.ends_with(".json") {
            continue;
        }
        if lower == SLOTS_EXAMPLE_FILE {
            continue;
        }
        if lower.ends_with(".json.tmp") {
            continue;
        }
        names.push(name.to_string());
    }
    names
}

/// List personal `slots*.json` basenames in `dir` (excludes the shipped example).
/// Sorted ASCII case-insensitive. If none exist, includes `slots.json` so first-run
/// can create it. Does **not** invent `slots.json` when other books are present.
pub fn list_slots_file_candidates(dir: &Path) -> Vec<String> {
    let mut names = scan_existing_slots_files(dir);
    if names.is_empty() {
        names.push(default_slots_file());
    }
    names.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
    names
}

pub fn load_slots_for_settings(
    dir: &Path,
    slots_file: &str,
) -> Result<(PathBuf, SlotRegistry, bool), ConfigError> {
    let preferred = resolve_slots_path(dir, slots_file);
    let want_writable = slots_file_is_writable(slots_file);

    if preferred.is_file() {
        match load_slots(&preferred) {
            Ok(s) => {
                return Ok((preferred, s, want_writable));
            }
            Err(e @ ConfigError::SlotsTooNew { .. }) => return Err(e),
            Err(e) => {
                error!(
                    "failed to load {}: {e}; continuing with empty slots (file will not be overwritten)",
                    preferred.display()
                );
                return Ok((preferred, SlotRegistry::default(), false));
            }
        }
    }

    // Missing personal file (or missing explicit path) → example fallback, read-only.
    let example = dir.join(SLOTS_EXAMPLE_FILE);
    if example.is_file() && preferred != example {
        warn!(
            missing = %preferred.display(),
            "slots file missing; falling back to {}",
            SLOTS_EXAMPLE_FILE
        );
        match load_slots(&example) {
            Ok(s) => return Ok((example, s, false)),
            Err(e @ ConfigError::SlotsTooNew { .. }) => return Err(e),
            Err(e) => {
                error!(
                    "failed to load {}: {e}; continuing with empty slots",
                    example.display()
                );
                return Ok((example, SlotRegistry::default(), false));
            }
        }
    }

    if example.is_file() {
        match load_slots(&example) {
            Ok(s) => return Ok((example, s, false)),
            Err(e @ ConfigError::SlotsTooNew { .. }) => return Err(e),
            Err(e) => {
                error!(
                    "failed to load {}: {e}; continuing with empty slots",
                    example.display()
                );
                return Ok((example, SlotRegistry::default(), false));
            }
        }
    }

    warn!(
        "no slots file at {} and no {}; continuing with empty slots",
        preferred.display(),
        SLOTS_EXAMPLE_FILE
    );
    Ok((preferred, SlotRegistry::default(), false))
}

/// Create `slots.json` from the example only when **no** personal slots file exists.
/// Returns `true` when the file was created on this call (first run).
fn ensure_default_slots_file(dir: &Path, _slots_file: &str) -> Result<bool, ConfigError> {
    if !scan_existing_slots_files(dir).is_empty() {
        return Ok(false);
    }
    let slots_path = dir.join("slots.json");
    if slots_path.is_file() {
        return Ok(false);
    }
    let example = dir.join(SLOTS_EXAMPLE_FILE);
    if example.is_file() {
        fs::copy(&example, &slots_path)?;
        info!("created slots.json from {}", SLOTS_EXAMPLE_FILE);
    } else {
        let body = format!(
            "{{\n  \"schemaVersion\": {CURRENT_SCHEMA},\n  \"dialkeyVersion\": \"{APP_VERSION}\",\n  \"slots\": []\n}}\n"
        );
        atomic_write(&slots_path, body.as_bytes())?;
        info!("created empty slots.json");
    }
    Ok(true)
}

/// Copy `settings.json` and `slots*.json` into `{dest_parent}/yyyyMMddHHmm/`.
/// If that stamp folder already exists, it is replaced (overwrite).
/// Refuses a destination inside `src_dir` (the live config folder).
pub fn backup_config_json(src_dir: &Path, dest_parent: &Path) -> Result<PathBuf, ConfigError> {
    if dest_parent.as_os_str().is_empty() {
        return Err(ConfigError::Serialize("backup folder is empty".into()));
    }
    if path_is_inside(dest_parent, src_dir) {
        return Err(ConfigError::BackupIntoConfig);
    }
    fs::create_dir_all(dest_parent)?;
    if path_is_inside(dest_parent, src_dir) {
        return Err(ConfigError::BackupIntoConfig);
    }
    let stamp = chrono::Local::now().format("%Y%m%d%H%M").to_string();
    let dest = dest_parent.join(&stamp);
    if dest.exists() {
        fs::remove_dir_all(&dest)?;
    }
    fs::create_dir_all(&dest)?;
    let mut copied = 0usize;
    for name in json_files_in_dir(src_dir) {
        let src = src_dir.join(&name);
        if !src.is_file() {
            continue;
        }
        fs::copy(&src, dest.join(&name))?;
        copied += 1;
    }
    info!(path = %dest.display(), copied, "config backup written");
    Ok(dest)
}

/// Copy selected JSON from a backup stamp folder into the live config folder.
///
/// Never writes `slots.example.json`. Scope is chosen at call time (not stored).
/// Each file is written atomically (temp → flush → rename).
pub fn restore_config_json(
    live_dir: &Path,
    stamp_dir: &Path,
    scope: RestoreSlotsScope,
    include_settings: bool,
    current_slots_file: &str,
) -> Result<RestoreReport, ConfigError> {
    if stamp_dir.as_os_str().is_empty() || !stamp_dir.is_dir() {
        return Err(ConfigError::RestoreNothing);
    }
    if path_is_inside(stamp_dir, live_dir) || path_is_inside(live_dir, stamp_dir) {
        return Err(ConfigError::RestoreFromLive);
    }

    let mut jobs: Vec<(String, PathBuf)> = Vec::new();
    if include_settings {
        if let Some(name) = find_basename_ci(stamp_dir, "settings.json") {
            jobs.push((name.clone(), stamp_dir.join(&name)));
        }
    }
    match scope {
        RestoreSlotsScope::AllBooks => {
            for name in scan_existing_slots_files(stamp_dir) {
                jobs.push((name.clone(), stamp_dir.join(&name)));
            }
        }
        RestoreSlotsScope::CurrentBook => {
            if !slots_file_is_writable(current_slots_file) {
                return Err(ConfigError::RestoreCurrentReadOnly);
            }
            let want = normalize_slots_file_name(current_slots_file);
            let Some(name) = find_basename_ci(stamp_dir, &want) else {
                return Err(ConfigError::RestoreCurrentMissing);
            };
            jobs.push((want, stamp_dir.join(name)));
        }
    }

    if jobs.is_empty() {
        return Err(ConfigError::RestoreNothing);
    }

    let mut copied = Vec::new();
    for (dest_name, src) in jobs {
        if dest_name.eq_ignore_ascii_case(SLOTS_EXAMPLE_FILE) {
            continue;
        }
        let dest = live_dir.join(&dest_name);
        let bytes = fs::read(&src)?;
        atomic_write(&dest, &bytes)?;
        copied.push(dest_name);
    }
    if copied.is_empty() {
        return Err(ConfigError::RestoreNothing);
    }
    info!(count = copied.len(), "config restore written");
    Ok(RestoreReport { copied })
}

fn find_basename_ci(dir: &Path, want: &str) -> Option<String> {
    let want = want.to_ascii_lowercase();
    let Ok(entries) = fs::read_dir(dir) else {
        return None;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.to_ascii_lowercase() == want && entry.path().is_file() {
            return Some(name.to_string());
        }
    }
    None
}

/// True when `inner` is `outer` or a descendant (slash-insensitive, ASCII case-insensitive).
pub fn path_is_inside(inner: &Path, outer: &Path) -> bool {
    if outer.as_os_str().is_empty() {
        return false;
    }
    let inner = path_compare_key(inner);
    let outer = path_compare_key(outer);
    if outer.is_empty() {
        return false;
    }
    inner == outer || inner.starts_with(&format!("{outer}\\"))
}

fn path_compare_key(p: &Path) -> String {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(p))
            .unwrap_or_else(|_| p.to_path_buf())
    };
    let abs = fs::canonicalize(&abs).unwrap_or(abs);
    let mut s = abs
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase();
    if let Some(rest) = s.strip_prefix("\\\\?\\") {
        s = rest.to_string();
    }
    while s.ends_with('\\') && s.len() > 3 {
        s.pop();
    }
    s
}

/// Atomically write `settings.json` (UTF-8, no BOM).
pub fn save_settings(dir: &Path, settings: &Settings) -> Result<(), ConfigError> {
    let mut settings = settings.clone();
    settings.keys.normalize();
    let mut value =
        serde_json::to_value(&settings).map_err(|e| ConfigError::Serialize(e.to_string()))?;
    // Keep instant keys ordered 0..9 for readable diffs.
    if let Some(obj) = value.as_object_mut() {
        if let Some(Value::Object(instant)) = obj.remove("instant") {
            let mut ordered = serde_json::Map::new();
            for d in 0..10 {
                let k = d.to_string();
                let v = instant.get(&k).cloned().unwrap_or(Value::Bool(false));
                ordered.insert(k, v);
            }
            obj.insert("instant".into(), Value::Object(ordered));
        }
        // Normalize slotsFile to a basename (or empty = example).
        if let Some(Value::String(sf)) = obj.get_mut("slotsFile") {
            *sf = normalize_slots_file_name(sf);
        }
        stamp_dialkey_version(obj);
    }
    let body =
        serde_json::to_string_pretty(&value).map_err(|e| ConfigError::Serialize(e.to_string()))?;
    let path = dir.join("settings.json");
    atomic_write(&path, format!("{body}\n").as_bytes())?;
    info!(path = %path.display(), "settings.json saved");
    Ok(())
}

/// Atomically write the selected slots file (UTF-8, no BOM).
/// Never call this for the example file or when the load path was unreadable.
/// Preserves existing `meta` (including `displayName`) so Settings saves do not wipe it.
pub fn save_slots(dir: &Path, slots_file: &str, slots: &SlotRegistry) -> Result<(), ConfigError> {
    if !slots_file_is_writable(slots_file) {
        return Err(ConfigError::Serialize(
            "refusing to write the sample slots file (read-only)".into(),
        ));
    }
    let path = resolve_slots_path(dir, slots_file);
    let meta = read_slots_meta(&path);
    let mut sorted: Vec<&Slot> = slots.all().iter().collect();
    sorted.sort_by(|a, b| super::slots::natural_cmp_id(&a.id, &b.id));
    let file = SyncedSlotsFile {
        schema_version: CURRENT_SCHEMA,
        dialkey_version: APP_VERSION.to_string(),
        meta,
        slots: sorted.into_iter().cloned().collect(),
    };
    let body =
        serde_json::to_string_pretty(&file).map_err(|e| ConfigError::Serialize(e.to_string()))?;
    atomic_write(&path, format!("{body}\n").as_bytes())?;
    info!(path = %path.display(), count = slots.len(), "slots file saved");
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncedSlotsFile {
    schema_version: u32,
    dialkey_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<SlotsMeta>,
    slots: Vec<Slot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SlotsMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
}

/// Read `meta.displayName` from a slots JSON path (empty → `None`).
pub fn read_slots_display_name(path: &Path) -> Option<String> {
    read_slots_meta(path).and_then(|m| {
        m.display_name
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    })
}

fn read_slots_meta(path: &Path) -> Option<SlotsMeta> {
    let text = read_utf8_bom(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let meta = value.get("meta")?;
    serde_json::from_value(meta.clone()).ok()
}

/// Label for a slots book: `meta.displayName` if set, otherwise basename.
pub fn slots_book_label(dir: &Path, basename: &str) -> String {
    let name = normalize_slots_file_name(basename);
    if name.is_empty() {
        return SLOTS_EXAMPLE_FILE.to_string();
    }
    let path = resolve_slots_path(dir, &name);
    read_slots_display_name(&path).unwrap_or(name)
}

/// Shipped defaults (Reset keys / missing fields). Same as `Settings::default()`.
pub fn default_shipped_settings() -> Settings {
    Settings::default()
}

/// Update the mouse / hotkey entries inside `settings.triggers` in place.
pub fn set_mouse_trigger(settings: &mut Settings, button: &str, suppress: bool) {
    let mut found = false;
    for t in &mut settings.triggers {
        if t.get("type").and_then(|v| v.as_str()) == Some("mouse") {
            *t = serde_json::json!({
                "type": "mouse",
                "button": button,
                "suppress": suppress
            });
            found = true;
            break;
        }
    }
    if !found {
        settings.triggers.push(serde_json::json!({
            "type": "mouse",
            "button": button,
            "suppress": suppress
        }));
    }
}

pub fn set_hotkey_trigger(settings: &mut Settings, key: &str) {
    let mut found = false;
    for t in &mut settings.triggers {
        if t.get("type").and_then(|v| v.as_str()) == Some("hotkey") {
            *t = serde_json::json!({ "type": "hotkey", "key": key });
            found = true;
            break;
        }
    }
    if !found {
        settings
            .triggers
            .push(serde_json::json!({ "type": "hotkey", "key": key }));
    }
}

pub fn hotkey_from_settings(settings: &Settings) -> String {
    for t in &settings.triggers {
        if t.get("type").and_then(|v| v.as_str()) == Some("hotkey") {
            return t
                .get("key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
        }
    }
    String::new()
}

fn read_schema_version(value: &Value, path: &Path) -> u32 {
    match value.get("schemaVersion") {
        None => {
            warn!(
                path = %path.display(),
                "schemaVersion missing; treating as 1"
            );
            1
        }
        Some(v) => match v.as_u64() {
            Some(n) => n as u32,
            None => {
                warn!(
                    path = %path.display(),
                    "schemaVersion is not a number; treating as 1"
                );
                1
            }
        },
    }
}

fn log_json_stamp(path: &Path, value: &Value, schema: u32) {
    let written_by = value
        .get("dialkeyVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if written_by.is_empty() {
        info!(
            path = %path.display(),
            schema_version = schema,
            "JSON loaded (no dialkeyVersion)"
        );
        return;
    }
    if written_by != APP_VERSION {
        info!(
            path = %path.display(),
            schema_version = schema,
            dialkey_version = written_by,
            this_build = APP_VERSION,
            "JSON written by another DialKey version (schema check only; not rejected)"
        );
        return;
    }
    info!(
        path = %path.display(),
        schema_version = schema,
        dialkey_version = written_by,
        "JSON loaded"
    );
}

fn stamp_dialkey_version(obj: &mut serde_json::Map<String, Value>) {
    obj.remove("dialkeyVersion");
    let mut out = serde_json::Map::new();
    if let Some(v) = obj.remove("schemaVersion") {
        out.insert("schemaVersion".into(), v);
    }
    out.insert(
        "dialkeyVersion".into(),
        Value::String(APP_VERSION.to_string()),
    );
    for (k, v) in obj.iter() {
        out.insert(k.clone(), v.clone());
    }
    *obj = out;
}

fn load_settings(path: &Path) -> Result<Settings, ConfigError> {
    let text = read_utf8_bom(path)?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| ConfigError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
    let ver = read_schema_version(&value, path);
    if ver > CURRENT_SCHEMA {
        return Err(ConfigError::SettingsTooNew { found: ver });
    }
    log_json_stamp(path, &value, ver);
    let mut settings: Settings = serde_json::from_value(value)
        .map_err(|e| ConfigError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
    settings.legend.normalize();
    settings.keys.normalize();
    Ok(settings)
}

fn load_slots(path: &Path) -> Result<SlotRegistry, ConfigError> {
    let text = read_utf8_bom(path)?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| ConfigError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
    let ver = read_schema_version(&value, path);
    if ver > CURRENT_SCHEMA {
        return Err(ConfigError::SlotsTooNew { found: ver });
    }
    log_json_stamp(path, &value, ver);

    let mut reg = SlotRegistry::default();
    let Some(arr) = value.get("slots").and_then(|v| v.as_array()) else {
        return Ok(reg);
    };

    for (i, item) in arr.iter().enumerate() {
        let Some(id) = item.get("id").and_then(coerce_slot_id) else {
            warn!("slots[{i}]: missing id; skipped");
            continue;
        };
        if !is_valid_slot_id(&id) {
            warn!("slots[{i}]: invalid id {id:?}; skipped");
            continue;
        }
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let path = item
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() || path.is_empty() {
            warn!("slots[{i}] id={id}: name/path required; skipped");
            continue;
        }
        let description = item
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let workdir = item
            .get("workdir")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let open_workdir = item
            .get("openWorkdir")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let focus_existing = item
            .get("focusExisting")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        reg.insert(Slot {
            id,
            name,
            path,
            description,
            workdir,
            open_workdir,
            focus_existing,
        });
    }
    Ok(reg)
}

/// Read a file as UTF-8, accepting an optional BOM.
pub fn read_utf8_bom(path: &Path) -> Result<String, ConfigError> {
    let bytes = fs::read(path)?;
    let text = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&bytes[3..]).into_owned()
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };
    Ok(text)
}

/// Atomic write: temp → flush → rename. Always UTF-8 without BOM.
pub fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), ConfigError> {
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(contents)?;
        f.flush()?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Mouse trigger config extracted from settings (Phase 1 uses mouse only).
#[derive(Debug, Clone)]
pub struct MouseTrigger {
    pub button: String,
    pub suppress: bool,
}

pub fn mouse_trigger_from_settings(settings: &Settings) -> MouseTrigger {
    for t in &settings.triggers {
        if t.get("type").and_then(|v| v.as_str()) == Some("mouse") {
            return MouseTrigger {
                button: t
                    .get("button")
                    .and_then(|v| v.as_str())
                    .unwrap_or("x2")
                    .to_string(),
                suppress: t.get("suppress").and_then(|v| v.as_bool()).unwrap_or(true),
            };
        }
    }
    MouseTrigger {
        button: "x2".into(),
        suppress: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn reads_bom_prefixed_json() {
        let dir = tempfile_dir();
        let path = dir.join("t.json");
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(&[0xEF, 0xBB, 0xBF]).unwrap();
        f.write_all(br#"{"schemaVersion":1,"slots":[{"id":1,"name":"N","path":"n.exe"}]}"#)
            .unwrap();
        drop(f);
        let reg = load_slots(&path).unwrap();
        assert!(reg.get("1").is_some());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn broken_slots_do_not_panic() {
        let dir = tempfile_dir();
        let path = dir.join("slots.json");
        fs::write(&path, b"{ not json").unwrap();
        let result = load_slots(&path);
        assert!(result.is_err());
        // File still present and untouched
        assert_eq!(fs::read(&path).unwrap(), b"{ not json");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn empty_slots_file_resolves_to_example_readonly() {
        assert_eq!(normalize_slots_file_name(""), "");
        assert_eq!(
            resolve_slots_path(Path::new("C:\\cfg"), ""),
            PathBuf::from("C:\\cfg").join(SLOTS_EXAMPLE_FILE)
        );
        assert!(!slots_file_is_writable(""));
        assert!(!slots_file_is_writable(SLOTS_EXAMPLE_FILE));
        assert!(slots_file_is_writable("slots.json"));
        assert!(slots_file_is_writable("slots.work.json"));
        // Path components are stripped to a basename (no directory escape).
        assert_eq!(normalize_slots_file_name("../slots.json"), "slots.json");
        assert_eq!(
            normalize_slots_file_name("subdir/slots.work.json"),
            "slots.work.json"
        );
    }

    #[test]
    fn missing_personal_file_falls_back_to_example() {
        let dir = tempfile_dir();
        fs::write(
            dir.join(SLOTS_EXAMPLE_FILE),
            br#"{"schemaVersion":1,"slots":[{"id":"9","name":"Ex","path":"e.exe"}]}"#,
        )
        .unwrap();
        let (path, reg, writable) = load_slots_for_settings(&dir, "slots.missing.json").unwrap();
        assert_eq!(path, dir.join(SLOTS_EXAMPLE_FILE));
        assert!(!writable);
        assert!(reg.get("9").is_some());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn list_candidates_excludes_example_includes_existing_only() {
        let dir = tempfile_dir();
        fs::write(dir.join(SLOTS_EXAMPLE_FILE), b"{}").unwrap();
        fs::write(dir.join("slots.work.json"), b"{}").unwrap();
        let names = list_slots_file_candidates(&dir);
        assert!(!names.iter().any(|n| n == "slots.json"));
        assert!(names.iter().any(|n| n == "slots.work.json"));
        assert!(!names
            .iter()
            .any(|n| n.eq_ignore_ascii_case(SLOTS_EXAMPLE_FILE)));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn list_candidates_empty_dir_offers_slots_json() {
        let dir = tempfile_dir();
        let names = list_slots_file_candidates(&dir);
        assert_eq!(names, vec!["slots.json".to_string()]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn backup_config_json_copies_and_overwrites() {
        let dir = tempfile_dir();
        let dest_parent = tempfile_dir();
        fs::write(dir.join("settings.json"), b"{\"schemaVersion\":1}").unwrap();
        fs::write(
            dir.join("slots.ndv.json"),
            b"{\"schemaVersion\":1,\"slots\":[]}",
        )
        .unwrap();
        let dest = backup_config_json(&dir, &dest_parent).unwrap();
        assert!(dest.join("settings.json").is_file());
        assert!(dest.join("slots.ndv.json").is_file());
        assert_eq!(dest.parent().unwrap(), dest_parent.as_path());
        fs::write(
            dir.join("settings.json"),
            b"{\"schemaVersion\":1,\"logLevel\":\"debug\"}",
        )
        .unwrap();
        let dest2 = backup_config_json(&dir, &dest_parent).unwrap();
        assert_eq!(dest, dest2);
        let body = fs::read_to_string(dest2.join("settings.json")).unwrap();
        assert!(body.contains("debug"));
        let _ = fs::remove_dir_all(dir);
        let _ = fs::remove_dir_all(dest_parent);
    }

    #[test]
    fn backup_config_json_refuses_dest_inside_src() {
        let dir = tempfile_dir();
        fs::write(dir.join("settings.json"), b"{\"schemaVersion\":1}").unwrap();
        let err = backup_config_json(&dir, &dir).unwrap_err();
        assert!(matches!(err, ConfigError::BackupIntoConfig));
        let nested = dir.join("inside");
        fs::create_dir_all(&nested).unwrap();
        let err = backup_config_json(&dir, &nested).unwrap_err();
        assert!(matches!(err, ConfigError::BackupIntoConfig));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn restore_all_books_skips_example_and_settings_unless_asked() {
        let live = tempfile_dir();
        let stamp = tempfile_dir();
        fs::write(live.join("slots.json"), b"old").unwrap();
        fs::write(live.join("settings.json"), b"live-settings").unwrap();
        fs::write(stamp.join("slots.json"), b"new-slots").unwrap();
        fs::write(stamp.join("slots.work.json"), b"work").unwrap();
        fs::write(stamp.join(SLOTS_EXAMPLE_FILE), b"example").unwrap();
        fs::write(stamp.join("settings.json"), b"stamp-settings").unwrap();
        let report = restore_config_json(
            &live,
            &stamp,
            RestoreSlotsScope::AllBooks,
            false,
            "slots.json",
        )
        .unwrap();
        assert!(report.copied.iter().any(|n| n == "slots.json"));
        assert!(report.copied.iter().any(|n| n == "slots.work.json"));
        assert!(!report
            .copied
            .iter()
            .any(|n| n.eq_ignore_ascii_case(SLOTS_EXAMPLE_FILE)));
        assert!(!report
            .copied
            .iter()
            .any(|n| n.eq_ignore_ascii_case("settings.json")));
        assert_eq!(
            fs::read_to_string(live.join("slots.json")).unwrap(),
            "new-slots"
        );
        assert_eq!(
            fs::read_to_string(live.join("settings.json")).unwrap(),
            "live-settings"
        );
        assert!(!live.join(SLOTS_EXAMPLE_FILE).is_file());
        let _ = fs::remove_dir_all(live);
        let _ = fs::remove_dir_all(stamp);
    }

    #[test]
    fn restore_current_book_only_and_optional_settings() {
        let live = tempfile_dir();
        let stamp = tempfile_dir();
        fs::write(live.join("slots.json"), b"old-home").unwrap();
        fs::write(live.join("slots.work.json"), b"old-work").unwrap();
        fs::write(live.join("settings.json"), b"old-set").unwrap();
        fs::write(stamp.join("slots.json"), b"new-home").unwrap();
        fs::write(stamp.join("slots.work.json"), b"new-work").unwrap();
        fs::write(stamp.join("settings.json"), b"new-set").unwrap();
        restore_config_json(
            &live,
            &stamp,
            RestoreSlotsScope::CurrentBook,
            true,
            "slots.work.json",
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(live.join("slots.json")).unwrap(),
            "old-home"
        );
        assert_eq!(
            fs::read_to_string(live.join("slots.work.json")).unwrap(),
            "new-work"
        );
        assert_eq!(
            fs::read_to_string(live.join("settings.json")).unwrap(),
            "new-set"
        );
        let _ = fs::remove_dir_all(live);
        let _ = fs::remove_dir_all(stamp);
    }

    #[test]
    fn restore_refuses_live_folder_and_readonly_current() {
        let live = tempfile_dir();
        fs::write(live.join("slots.json"), b"x").unwrap();
        let err = restore_config_json(
            &live,
            &live,
            RestoreSlotsScope::AllBooks,
            false,
            "slots.json",
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::RestoreFromLive));
        let stamp = tempfile_dir();
        fs::write(stamp.join("slots.json"), b"n").unwrap();
        let err = restore_config_json(&live, &stamp, RestoreSlotsScope::CurrentBook, false, "")
            .unwrap_err();
        assert!(matches!(err, ConfigError::RestoreCurrentReadOnly));
        let err = restore_config_json(
            &live,
            &stamp,
            RestoreSlotsScope::CurrentBook,
            false,
            "slots.missing.json",
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::RestoreCurrentMissing));
        let _ = fs::remove_dir_all(live);
        let _ = fs::remove_dir_all(stamp);
    }

    #[test]
    fn path_is_inside_self_and_child() {
        let dir = tempfile_dir();
        let child = dir.join("child");
        fs::create_dir_all(&child).unwrap();
        assert!(path_is_inside(&dir, &dir));
        assert!(path_is_inside(&child, &dir));
        assert!(!path_is_inside(&dir, &child));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn migrate_json_into_settings_dir_moves_without_overwrite() {
        let exe = tempfile_dir();
        fs::write(exe.join("settings.json"), b"{\"schemaVersion\":1}").unwrap();
        fs::write(
            exe.join("slots.json"),
            b"{\"schemaVersion\":1,\"slots\":[]}",
        )
        .unwrap();
        let dest = migrate_json_into_settings_dir(&exe).unwrap();
        assert!(dest.join("settings.json").is_file());
        assert!(dest.join("slots.json").is_file());
        assert!(!exe.join("settings.json").is_file());
        fs::write(
            exe.join("settings.json"),
            b"{\"schemaVersion\":1,\"logLevel\":\"info\"}",
        )
        .unwrap();
        fs::write(dest.join("keep.json"), b"x").unwrap();
        let dest2 = migrate_json_into_settings_dir(&exe).unwrap();
        assert_eq!(dest, dest2);
        // Destination settings.json was not overwritten.
        let body = fs::read_to_string(dest.join("settings.json")).unwrap();
        assert!(!body.contains("logLevel"));
        let _ = fs::remove_dir_all(exe);
    }

    #[test]
    fn save_slots_refuses_example() {
        let dir = tempfile_dir();
        let err = save_slots(&dir, "", &SlotRegistry::default()).unwrap_err();
        assert!(err.to_string().contains("read-only"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn save_slots_preserves_meta_display_name() {
        let dir = tempfile_dir();
        fs::write(
            dir.join("slots.json"),
            br#"{
  "schemaVersion": 1,
  "meta": { "displayName": "Home" },
  "slots": [{ "id": "1", "name": "A", "path": "a.exe" }]
}"#,
        )
        .unwrap();
        assert_eq!(slots_book_label(&dir, "slots.json"), "Home");
        let reg = load_slots(&dir.join("slots.json")).unwrap();
        save_slots(&dir, "slots.json", &reg).unwrap();
        let text = fs::read_to_string(dir.join("slots.json")).unwrap();
        assert!(text.contains("displayName"));
        assert!(text.contains("Home"));
        assert!(text.contains("dialkeyVersion"));
        assert!(text.contains(APP_VERSION));
        assert_eq!(slots_book_label(&dir, "slots.json"), "Home");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn omitted_triggers_use_shipped_mouse_x2() {
        let s: Settings = serde_json::from_str(r#"{"schemaVersion":1}"#).unwrap();
        assert_eq!(s.triggers, default_triggers());
        assert_eq!(Settings::default().triggers, default_triggers());
        assert_eq!(default_shipped_settings().triggers, default_triggers());
        let mouse = s
            .triggers
            .iter()
            .find(|t| t.get("type").and_then(|v| v.as_str()) == Some("mouse"));
        assert_eq!(
            mouse.and_then(|t| t.get("button").and_then(|v| v.as_str())),
            Some("x2")
        );
    }

    #[test]
    fn omitted_show_advanced_defaults_false_and_is_omitted_on_save() {
        let s: Settings = serde_json::from_str(r#"{"schemaVersion":1}"#).unwrap();
        assert!(!s.ui.show_advanced);
        let v = serde_json::to_value(&s).unwrap();
        assert!(v["ui"].get("showAdvanced").is_none());
        let on = Settings {
            ui: UiSettings {
                locale: String::new(),
                show_advanced: true,
            },
            ..Settings::default()
        };
        let v_on = serde_json::to_value(&on).unwrap();
        assert_eq!(v_on["ui"]["showAdvanced"], true);
    }

    #[test]
    fn missing_schema_version_is_treated_as_one() {
        let dir = tempfile_dir();
        fs::write(dir.join("settings.json"), br#"{"logLevel":"info"}"#).unwrap();
        fs::write(
            dir.join("slots.json"),
            br#"{"slots":[{"id":"1","name":"A","path":"a.exe"}]}"#,
        )
        .unwrap();
        let cfg = load_app_config(&dir, Some(&dir)).unwrap();
        assert_eq!(cfg.settings.log_level, "info");
        assert!(cfg.slots.get("1").is_some());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn settings_schema_too_new_is_fatal() {
        let dir = tempfile_dir();
        fs::write(
            dir.join("settings.json"),
            br#"{"schemaVersion":99,"logLevel":"info"}"#,
        )
        .unwrap();
        let err = load_app_config(&dir, Some(&dir)).unwrap_err();
        assert!(matches!(err, ConfigError::SettingsTooNew { found: 99 }));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn slots_schema_too_new_is_fatal() {
        let dir = tempfile_dir();
        fs::write(dir.join("settings.json"), br#"{"schemaVersion":1}"#).unwrap();
        fs::write(
            dir.join("slots.json"),
            br#"{"schemaVersion":99,"slots":[{"id":"1","name":"A","path":"a.exe"}]}"#,
        )
        .unwrap();
        let err = load_app_config(&dir, Some(&dir)).unwrap_err();
        assert!(matches!(err, ConfigError::SlotsTooNew { found: 99 }));
        let body = fs::read_to_string(dir.join("slots.json")).unwrap();
        assert!(body.contains("\"schemaVersion\":99"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn save_settings_writes_dialkey_version() {
        let dir = tempfile_dir();
        save_settings(&dir, &Settings::default()).unwrap();
        let text = fs::read_to_string(dir.join("settings.json")).unwrap();
        assert!(text.contains(&format!("\"dialkeyVersion\": \"{APP_VERSION}\"")));
        let loaded = load_settings(&dir.join("settings.json")).unwrap();
        assert_eq!(loaded.schema_version, CURRENT_SCHEMA);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn foreign_dialkey_version_still_loads() {
        let dir = tempfile_dir();
        fs::write(
            dir.join("settings.json"),
            br#"{"schemaVersion":1,"dialkeyVersion":"0.0.1","logLevel":"debug"}"#,
        )
        .unwrap();
        let s = load_settings(&dir.join("settings.json")).unwrap();
        assert_eq!(s.log_level, "debug");
        let _ = fs::remove_dir_all(dir);
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "dialkey-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
