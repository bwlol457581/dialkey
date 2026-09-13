//! Virtual-key name parsing and digit mapping (OS-independent numeric codes).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Key bindings loaded from `settings.json` (`keys` object).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyBindings {
    pub start: String,
    pub confirm: String,
    pub cancel: String,
    /// Opens Search (formerly `phonebook` in older configs).
    #[serde(alias = "phonebook")]
    pub search: String,
    /// Multi-digit: launch slot and open its working folder (default Ctrl).
    #[serde(default = "default_open_workdir_key")]
    pub open_workdir: String,
    /// Delete last digit; empty multi → Idle; Idle → close (default →).
    #[serde(default = "default_digit_back_key")]
    pub digit_back: String,
    /// Optional typing-window legend labels (max 3 chars). Empty = VK short label.
    #[serde(
        default = "default_start_label",
        skip_serializing_if = "String::is_empty"
    )]
    pub start_label: String,
    #[serde(
        default = "default_confirm_label",
        skip_serializing_if = "String::is_empty"
    )]
    pub confirm_label: String,
    #[serde(
        default = "default_cancel_label",
        skip_serializing_if = "String::is_empty"
    )]
    pub cancel_label: String,
    #[serde(
        default = "default_search_label",
        skip_serializing_if = "String::is_empty"
    )]
    pub search_label: String,
    #[serde(
        default = "default_open_workdir_label",
        skip_serializing_if = "String::is_empty"
    )]
    pub open_workdir_label: String,
    #[serde(
        default = "default_digit_back_label",
        skip_serializing_if = "String::is_empty"
    )]
    pub digit_back_label: String,
    /// Swallow this VK while the typing window is open (wireless pad ON).
    /// Off by default — wired pads do not need it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wake_enabled: bool,
    /// ON-key VK name. Default numpad `-` (unused while off). Omitted when default.
    #[serde(
        default = "default_wake_key",
        skip_serializing_if = "is_default_wake_key"
    )]
    pub wake: String,
    /// Active key set id (`numpad` / `keyboard`). Omitted when numpad.
    #[serde(
        default = "default_key_set_numpad",
        skip_serializing_if = "is_numpad_id"
    )]
    pub active: String,
    /// All key sets (shipped two + room to grow). Empty on old files until normalize.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sets: Vec<KeySetStored>,
}

/// Shipped set ids. UI shows two; JSON is a list so more can be added later.
pub const KEY_SET_NUMPAD: &str = "numpad";
pub const KEY_SET_KEYBOARD: &str = "keyboard";

fn default_key_set_numpad() -> String {
    KEY_SET_NUMPAD.into()
}

fn is_numpad_id(s: &str) -> bool {
    s.is_empty() || s == KEY_SET_NUMPAD
}

fn default_wake_key() -> String {
    "VK_SUBTRACT".into()
}

fn is_default_wake_key(s: &str) -> bool {
    s.is_empty() || s.eq_ignore_ascii_case("VK_SUBTRACT")
}

fn default_numpad_chord() -> String {
    "t".into()
}

fn default_keyboard_chord() -> String {
    "k".into()
}

/// One named key set (bindings + X1 letter). Flattened beside live fields in JSON.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeySetStored {
    pub id: String,
    #[serde(default)]
    pub chord: String,
    pub start: String,
    pub confirm: String,
    pub cancel: String,
    #[serde(alias = "phonebook")]
    pub search: String,
    #[serde(default = "default_open_workdir_key")]
    pub open_workdir: String,
    #[serde(default = "default_digit_back_key")]
    pub digit_back: String,
    #[serde(
        default = "default_start_label",
        skip_serializing_if = "String::is_empty"
    )]
    pub start_label: String,
    #[serde(
        default = "default_confirm_label",
        skip_serializing_if = "String::is_empty"
    )]
    pub confirm_label: String,
    #[serde(
        default = "default_cancel_label",
        skip_serializing_if = "String::is_empty"
    )]
    pub cancel_label: String,
    #[serde(
        default = "default_search_label",
        skip_serializing_if = "String::is_empty"
    )]
    pub search_label: String,
    #[serde(
        default = "default_open_workdir_label",
        skip_serializing_if = "String::is_empty"
    )]
    pub open_workdir_label: String,
    #[serde(
        default = "default_digit_back_label",
        skip_serializing_if = "String::is_empty"
    )]
    pub digit_back_label: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wake_enabled: bool,
    #[serde(default = "default_wake_key")]
    pub wake: String,
}

/// Normalize a key-set chord to a single `a`–`z` letter.
pub fn normalize_key_chord(raw: &str) -> Option<char> {
    let mut chars = raw.trim().chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => Some(c.to_ascii_lowercase()),
        _ => None,
    }
}

fn default_open_workdir_key() -> String {
    "VK_CONTROL".into()
}

fn default_digit_back_key() -> String {
    "VK_RIGHT".into()
}

fn default_start_label() -> String {
    "+".into()
}
fn default_confirm_label() -> String {
    "Ent".into()
}
fn default_cancel_label() -> String {
    "ESC".into()
}
fn default_search_label() -> String {
    "\\".into()
}
fn default_open_workdir_label() -> String {
    "Ctr".into()
}
fn default_digit_back_label() -> String {
    "→".into()
}

/// Trim / clamp a key display label to at most 3 Unicode scalars.
pub fn clamp_key_display(raw: &str) -> String {
    raw.trim().chars().take(3).collect()
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            start: "VK_ADD".into(),
            confirm: "VK_RETURN".into(),
            cancel: "VK_ESCAPE".into(),
            search: "VK_OEM_5".into(),
            open_workdir: default_open_workdir_key(),
            digit_back: default_digit_back_key(),
            start_label: default_start_label(),
            confirm_label: default_confirm_label(),
            cancel_label: default_cancel_label(),
            search_label: default_search_label(),
            open_workdir_label: default_open_workdir_label(),
            digit_back_label: default_digit_back_label(),
            wake_enabled: false,
            wake: default_wake_key(),
            active: default_key_set_numpad(),
            sets: Vec::new(),
        }
    }
}

impl KeyBindings {
    pub fn resolved(&self) -> ResolvedKeys {
        ResolvedKeys {
            start: parse_vk_name(&self.start).unwrap_or(0x6B),
            confirm: parse_vk_name(&self.confirm).unwrap_or(0x0D),
            cancel: parse_vk_name(&self.cancel).unwrap_or(0x1B),
            search: parse_vk_name(&self.search).unwrap_or(VK_OEM_5),
            open_workdir: parse_vk_name(&self.open_workdir).unwrap_or(VK_CONTROL),
            digit_back: parse_vk_name(&self.digit_back).unwrap_or(0x27),
            wake: parse_vk_name(&self.wake).unwrap_or(VK_SUBTRACT),
            wake_enabled: self.wake_enabled,
        }
    }

    /// Raw label field for a role (may be empty).
    pub fn label_raw(&self, role: KeyRole) -> &str {
        match role {
            KeyRole::Start => self.start_label.as_str(),
            KeyRole::Confirm => self.confirm_label.as_str(),
            KeyRole::Cancel => self.cancel_label.as_str(),
            KeyRole::Search => self.search_label.as_str(),
            KeyRole::OpenWorkdir => self.open_workdir_label.as_str(),
            KeyRole::DigitBack => self.digit_back_label.as_str(),
        }
    }

    pub fn vk_name(&self, role: KeyRole) -> &str {
        match role {
            KeyRole::Start => self.start.as_str(),
            KeyRole::Confirm => self.confirm.as_str(),
            KeyRole::Cancel => self.cancel.as_str(),
            KeyRole::Search => self.search.as_str(),
            KeyRole::OpenWorkdir => self.open_workdir.as_str(),
            KeyRole::DigitBack => self.digit_back.as_str(),
        }
    }

    pub fn set_vk(&mut self, role: KeyRole, name: String) {
        match role {
            KeyRole::Start => self.start = name,
            KeyRole::Confirm => self.confirm = name,
            KeyRole::Cancel => self.cancel = name,
            KeyRole::Search => self.search = name,
            KeyRole::OpenWorkdir => self.open_workdir = name,
            KeyRole::DigitBack => self.digit_back = name,
        }
    }

    fn resolved_vk(&self, role: KeyRole) -> u16 {
        let r = self.resolved();
        match role {
            KeyRole::Start => r.start,
            KeyRole::Confirm => r.confirm,
            KeyRole::Cancel => r.cancel,
            KeyRole::Search => r.search,
            KeyRole::OpenWorkdir => r.open_workdir,
            KeyRole::DigitBack => r.digit_back,
        }
    }

    /// Set a role's display label (clamped). Empty clears to default-at-use.
    pub fn set_label(&mut self, role: KeyRole, raw: &str) {
        let v = clamp_key_display(raw);
        match role {
            KeyRole::Start => self.start_label = v,
            KeyRole::Confirm => self.confirm_label = v,
            KeyRole::Cancel => self.cancel_label = v,
            KeyRole::Search => self.search_label = v,
            KeyRole::OpenWorkdir => self.open_workdir_label = v,
            KeyRole::DigitBack => self.digit_back_label = v,
        }
    }

    /// Typing-window / Keys legend: custom display or VK short form.
    pub fn display_label(&self, role: KeyRole) -> String {
        let custom = clamp_key_display(self.label_raw(role));
        if custom.is_empty() {
            let r = self.resolved();
            let vk = match role {
                KeyRole::Start => r.start,
                KeyRole::Confirm => r.confirm,
                KeyRole::Cancel => r.cancel,
                KeyRole::Search => r.search,
                KeyRole::OpenWorkdir => r.open_workdir,
                KeyRole::DigitBack => r.digit_back,
            };
            vk_short_label(vk)
        } else {
            custom
        }
    }

    fn snapshot(&self, id: &str, chord: &str) -> KeySetStored {
        KeySetStored {
            id: id.to_string(),
            chord: chord.to_string(),
            start: self.start.clone(),
            confirm: self.confirm.clone(),
            cancel: self.cancel.clone(),
            search: self.search.clone(),
            open_workdir: self.open_workdir.clone(),
            digit_back: self.digit_back.clone(),
            start_label: self.start_label.clone(),
            confirm_label: self.confirm_label.clone(),
            cancel_label: self.cancel_label.clone(),
            search_label: self.search_label.clone(),
            open_workdir_label: self.open_workdir_label.clone(),
            digit_back_label: self.digit_back_label.clone(),
            wake_enabled: self.wake_enabled,
            wake: self.wake.clone(),
        }
    }

    fn apply_stored(&mut self, stored: &KeySetStored) {
        self.start = stored.start.clone();
        self.confirm = stored.confirm.clone();
        self.cancel = stored.cancel.clone();
        self.search = stored.search.clone();
        self.open_workdir = stored.open_workdir.clone();
        self.digit_back = stored.digit_back.clone();
        self.start_label = stored.start_label.clone();
        self.confirm_label = stored.confirm_label.clone();
        self.cancel_label = stored.cancel_label.clone();
        self.search_label = stored.search_label.clone();
        self.open_workdir_label = stored.open_workdir_label.clone();
        self.digit_back_label = stored.digit_back_label.clone();
        self.wake_enabled = stored.wake_enabled;
        self.wake = if stored.wake.is_empty() {
            if stored.id == KEY_SET_KEYBOARD {
                "VK_OEM_MINUS".into()
            } else {
                default_wake_key()
            }
        } else {
            stored.wake.clone()
        };
        self.active = stored.id.clone();
    }

    fn keyboard_shipped() -> KeySetStored {
        KeySetStored {
            id: KEY_SET_KEYBOARD.into(),
            chord: default_keyboard_chord(),
            start: "VK_OEM_PLUS".into(),
            confirm: "VK_RETURN".into(),
            cancel: "VK_ESCAPE".into(),
            search: "VK_OEM_5".into(),
            open_workdir: default_open_workdir_key(),
            digit_back: default_digit_back_key(),
            start_label: default_start_label(),
            confirm_label: default_confirm_label(),
            cancel_label: default_cancel_label(),
            search_label: default_search_label(),
            open_workdir_label: default_open_workdir_label(),
            digit_back_label: default_digit_back_label(),
            wake_enabled: false,
            wake: "VK_OEM_MINUS".into(),
        }
    }

    /// Fill shipped sets from a 0.15-style file; keep live fields as the active set.
    pub fn normalize(&mut self) {
        if self.active.is_empty() {
            self.active = KEY_SET_NUMPAD.into();
        }
        if self.wake.is_empty() {
            self.wake = if self.active == KEY_SET_KEYBOARD {
                "VK_OEM_MINUS".into()
            } else {
                default_wake_key()
            };
        }
        if self.sets.is_empty() {
            let numpad = self.snapshot(KEY_SET_NUMPAD, &default_numpad_chord());
            self.sets = vec![numpad, Self::keyboard_shipped()];
        } else {
            self.ensure_shipped_sets();
            if let Some(stored) = self.sets.iter().find(|s| s.id == self.active).cloned() {
                self.apply_stored(&stored);
            } else if let Some(first) = self.sets.first().cloned() {
                self.apply_stored(&first);
            }
        }
        self.sync_live_into_sets();
    }

    fn ensure_shipped_sets(&mut self) {
        if !self.sets.iter().any(|s| s.id == KEY_SET_NUMPAD) {
            self.sets
                .insert(0, self.snapshot(KEY_SET_NUMPAD, &default_numpad_chord()));
        }
        if !self.sets.iter().any(|s| s.id == KEY_SET_KEYBOARD) {
            self.sets.push(Self::keyboard_shipped());
        }
        for set in &mut self.sets {
            if set.chord.is_empty() {
                set.chord = if set.id == KEY_SET_KEYBOARD {
                    default_keyboard_chord()
                } else {
                    default_numpad_chord()
                };
            }
            if set.wake.is_empty() {
                set.wake = if set.id == KEY_SET_KEYBOARD {
                    "VK_OEM_MINUS".into()
                } else {
                    default_wake_key()
                };
            }
        }
    }

    /// Copy the live role fields into `sets[active]`.
    pub fn sync_live_into_sets(&mut self) {
        let chord = self
            .sets
            .iter()
            .find(|s| s.id == self.active)
            .map(|s| s.chord.clone())
            .unwrap_or_else(|| {
                if self.active == KEY_SET_KEYBOARD {
                    default_keyboard_chord()
                } else {
                    default_numpad_chord()
                }
            });
        let snap = self.snapshot(&self.active, &chord);
        if let Some(slot) = self.sets.iter_mut().find(|s| s.id == self.active) {
            *slot = snap;
        } else {
            self.sets.push(snap);
        }
    }

    /// Switch the live fields to `id`. Returns false if the set is missing.
    pub fn activate(&mut self, id: &str) -> bool {
        self.sync_live_into_sets();
        let Some(stored) = self.sets.iter().find(|s| s.id == id).cloned() else {
            return false;
        };
        self.apply_stored(&stored);
        true
    }

    pub fn active_chord(&self) -> char {
        self.sets
            .iter()
            .find(|s| s.id == self.active)
            .and_then(|s| normalize_key_chord(&s.chord))
            .unwrap_or(if self.active == KEY_SET_KEYBOARD {
                'k'
            } else {
                't'
            })
    }

    pub fn chord_for_id(&self, id: &str) -> char {
        self.sets
            .iter()
            .find(|s| s.id == id)
            .and_then(|s| normalize_key_chord(&s.chord))
            .unwrap_or(if id == KEY_SET_KEYBOARD { 'k' } else { 't' })
    }

    pub fn set_chord_for_id(&mut self, id: &str, letter: char) -> Result<(), KeyBindingError> {
        let letter = letter.to_ascii_lowercase();
        if !letter.is_ascii_lowercase() {
            return Err(KeyBindingError::UnknownName {
                role: "chord",
                name: letter.to_string(),
            });
        }
        if let Some(other) = self
            .sets
            .iter()
            .find(|s| s.id != id && normalize_key_chord(&s.chord) == Some(letter))
        {
            return Err(KeyBindingError::DuplicateChord {
                set_a: id.to_string(),
                set_b: other.id.clone(),
                letter,
            });
        }
        if let Some(slot) = self.sets.iter_mut().find(|s| s.id == id) {
            slot.chord = letter.to_string();
        }
        Ok(())
    }

    pub fn set_active_chord(&mut self, letter: char) -> Result<(), KeyBindingError> {
        let id = self.active.clone();
        self.set_chord_for_id(&id, letter)
    }

    /// Letter → set id for X1 chords.
    pub fn chord_map(&self) -> Vec<(char, String)> {
        let mut out = Vec::new();
        for set in &self.sets {
            if let Some(c) = normalize_key_chord(&set.chord) {
                if !out.iter().any(|(x, _)| *x == c) {
                    out.push((c, set.id.clone()));
                }
            }
        }
        out
    }

    pub fn reset_current_set(&mut self) {
        let id = self.active.clone();
        let shipped = if id == KEY_SET_KEYBOARD {
            Self::keyboard_shipped()
        } else {
            let mut n = KeyBindings::default();
            n.normalize();
            n.snapshot(KEY_SET_NUMPAD, &default_numpad_chord())
        };
        self.apply_stored(&shipped);
        self.active = id;
        if let Some(slot) = self.sets.iter_mut().find(|s| s.id == self.active) {
            *slot = shipped;
            if slot.id != self.active {
                slot.id = self.active.clone();
            }
        }
    }

    /// Restore shipped X1 letters (`t` / `k`) without touching Action-key VKs.
    pub fn reset_switch_chords(&mut self) {
        if let Some(slot) = self.sets.iter_mut().find(|s| s.id == KEY_SET_NUMPAD) {
            slot.chord = default_numpad_chord();
        }
        if let Some(slot) = self.sets.iter_mut().find(|s| s.id == KEY_SET_KEYBOARD) {
            slot.chord = default_keyboard_chord();
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedKeys {
    pub start: u16,
    pub confirm: u16,
    pub cancel: u16,
    pub search: u16,
    pub open_workdir: u16,
    pub digit_back: u16,
    pub wake: u16,
    pub wake_enabled: bool,
}

pub const VK_ADD: u16 = 0x6B;
pub const VK_DIVIDE: u16 = 0x6F;
pub const VK_OEM_PLUS: u16 = 0xBB;
/// Main-keyboard `/` (US); also the usual `/` key on JIS.
pub const VK_OEM_2: u16 = 0xBF;
/// Main-keyboard `\` (US `VK_OEM_5`). Spec default for Search.
pub const VK_OEM_5: u16 = 0xDC;
/// Numpad `.` (NumLock on).
pub const VK_DECIMAL: u16 = 0x6E;
/// Numpad `.` with NumLock off.
pub const VK_DELETE: u16 = 0x2E;
/// Numpad `-`. NumLock does not remap this key.
pub const VK_SUBTRACT: u16 = 0x6D;
/// Main-keyboard `-`.
pub const VK_OEM_MINUS: u16 = 0xBD;

/// Ctrl (generic). LL hooks usually report `VK_LCONTROL` / `VK_RCONTROL`.
pub const VK_CONTROL: u16 = 0x11;
pub const VK_LCONTROL: u16 = 0xA2;
pub const VK_RCONTROL: u16 = 0xA3;

impl ResolvedKeys {
    /// Whether `vk` should act as the start (`+`) key.
    ///
    /// Debug builds also accept both numpad `VK_ADD` and main-keyboard
    /// `VK_OEM_PLUS` so notebook + external numpad can be used while developing.
    /// Release builds honor only the configured binding (spec default: VK_ADD).
    pub fn is_start(&self, vk: u16) -> bool {
        if vk == self.start {
            return true;
        }
        #[cfg(debug_assertions)]
        {
            let start_is_plus_family = self.start == VK_ADD || self.start == VK_OEM_PLUS;
            if start_is_plus_family && (vk == VK_ADD || vk == VK_OEM_PLUS) {
                return true;
            }
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = vk;
        }
        false
    }

    /// Whether `vk` should open Search (`\` by default).
    ///
    /// Spec default is main-keyboard `VK_OEM_5` (`\`). When the binding is still
    /// the legacy slash family (`VK_DIVIDE` / `VK_OEM_2`), both slash keys are
    /// accepted so older settings keep working.
    pub fn is_search(&self, vk: u16) -> bool {
        if vk == self.search {
            return true;
        }
        let search_is_slash_family = self.search == VK_DIVIDE || self.search == VK_OEM_2;
        search_is_slash_family && (vk == VK_DIVIDE || vk == VK_OEM_2)
    }

    /// Whether `vk` should launch + open working folder (Ctrl by default).
    ///
    /// Spec default is `VK_CONTROL`. Low-level hooks typically deliver
    /// `VK_LCONTROL` / `VK_RCONTROL`, so any of the three match when the binding
    /// is in that family.
    pub fn is_open_workdir(&self, vk: u16) -> bool {
        if vk == self.open_workdir {
            return true;
        }
        let family = self.open_workdir == VK_CONTROL
            || self.open_workdir == VK_LCONTROL
            || self.open_workdir == VK_RCONTROL;
        family && (vk == VK_CONTROL || vk == VK_LCONTROL || vk == VK_RCONTROL)
    }

    /// Configured wake VK when the checkbox is on.
    pub fn is_wake(&self, vk: u16) -> bool {
        self.wake_enabled && vk == self.wake
    }
}

fn vk_name_table() -> HashMap<&'static str, u16> {
    let mut t: HashMap<&str, u16> = [
        ("VK_ADD", VK_ADD),
        ("VK_RETURN", 0x0D),
        ("VK_ESCAPE", 0x1B),
        ("VK_DIVIDE", VK_DIVIDE),
        ("VK_MULTIPLY", 0x6A),
        ("VK_SUBTRACT", VK_SUBTRACT),
        ("VK_DECIMAL", VK_DECIMAL),
        ("VK_SPACE", 0x20),
        ("VK_TAB", 0x09),
        ("VK_CONTROL", VK_CONTROL),
        ("VK_LCONTROL", VK_LCONTROL),
        ("VK_RCONTROL", VK_RCONTROL),
        ("VK_BACK", 0x08),
        ("VK_DELETE", VK_DELETE),
        ("VK_INSERT", 0x2D),
        ("VK_HOME", 0x24),
        ("VK_END", 0x23),
        ("VK_PRIOR", 0x21),
        ("VK_NEXT", 0x22),
        ("VK_LEFT", 0x25),
        ("VK_UP", 0x26),
        ("VK_RIGHT", 0x27),
        ("VK_DOWN", 0x28),
        ("VK_OEM_PLUS", VK_OEM_PLUS),
        ("VK_OEM_MINUS", VK_OEM_MINUS),
        ("VK_OEM_COMMA", 0xBC),
        ("VK_OEM_PERIOD", 0xBE),
        ("VK_OEM_1", 0xBA),
        ("VK_OEM_2", VK_OEM_2),
        ("VK_OEM_3", 0xC0),
        ("VK_OEM_4", 0xDB),
        ("VK_OEM_5", VK_OEM_5),
        ("VK_OEM_6", 0xDD),
        ("VK_OEM_7", 0xDE),
        ("VK_PAUSE", 0x13),
        ("VK_CAPITAL", 0x14),
        ("VK_APPS", 0x5D),
    ]
    .into_iter()
    .collect();
    for (name, vk) in [
        ("VK_F1", 0x70u16),
        ("VK_F2", 0x71),
        ("VK_F3", 0x72),
        ("VK_F4", 0x73),
        ("VK_F5", 0x74),
        ("VK_F6", 0x75),
        ("VK_F7", 0x76),
        ("VK_F8", 0x77),
        ("VK_F9", 0x78),
        ("VK_F10", 0x79),
        ("VK_F11", 0x7A),
        ("VK_F12", 0x7B),
        ("VK_F13", 0x7C),
        ("VK_F14", 0x7D),
        ("VK_F15", 0x7E),
        ("VK_F16", 0x7F),
        ("VK_F17", 0x80),
        ("VK_F18", 0x81),
        ("VK_F19", 0x82),
        ("VK_F20", 0x83),
        ("VK_F21", 0x84),
        ("VK_F22", 0x85),
        ("VK_F23", 0x86),
        ("VK_F24", 0x87),
    ] {
        t.insert(name, vk);
    }
    // A–Z (VK_A = 0x41). Digits are reserved for dial bindings but still parsed.
    for (name, vk) in [
        ("VK_A", 0x41u16),
        ("VK_B", 0x42),
        ("VK_C", 0x43),
        ("VK_D", 0x44),
        ("VK_E", 0x45),
        ("VK_F", 0x46),
        ("VK_G", 0x47),
        ("VK_H", 0x48),
        ("VK_I", 0x49),
        ("VK_J", 0x4A),
        ("VK_K", 0x4B),
        ("VK_L", 0x4C),
        ("VK_M", 0x4D),
        ("VK_N", 0x4E),
        ("VK_O", 0x4F),
        ("VK_P", 0x50),
        ("VK_Q", 0x51),
        ("VK_R", 0x52),
        ("VK_S", 0x53),
        ("VK_T", 0x54),
        ("VK_U", 0x55),
        ("VK_V", 0x56),
        ("VK_W", 0x57),
        ("VK_X", 0x58),
        ("VK_Y", 0x59),
        ("VK_Z", 0x5A),
        ("VK_0", 0x30),
        ("VK_1", 0x31),
        ("VK_2", 0x32),
        ("VK_3", 0x33),
        ("VK_4", 0x34),
        ("VK_5", 0x35),
        ("VK_6", 0x36),
        ("VK_7", 0x37),
        ("VK_8", 0x38),
        ("VK_9", 0x39),
        ("VK_NUMPAD0", 0x60),
        ("VK_NUMPAD1", 0x61),
        ("VK_NUMPAD2", 0x62),
        ("VK_NUMPAD3", 0x63),
        ("VK_NUMPAD4", 0x64),
        ("VK_NUMPAD5", 0x65),
        ("VK_NUMPAD6", 0x66),
        ("VK_NUMPAD7", 0x67),
        ("VK_NUMPAD8", 0x68),
        ("VK_NUMPAD9", 0x69),
    ] {
        t.insert(name, vk);
    }
    t
}

/// Parse a `VK_*` name from settings into a virtual-key code.
pub fn parse_vk_name(name: &str) -> Option<u16> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    if let Some(hex) = name
        .strip_prefix("VK_0x")
        .or_else(|| name.strip_prefix("VK_0X"))
    {
        return u16::from_str_radix(hex, 16).ok();
    }
    let upper = name.to_ascii_uppercase();
    let table = vk_name_table();
    if let Some(vk) = table.get(upper.as_str()).copied() {
        return Some(vk);
    }
    // Bare letter / F-key convenience: "F9", "A"
    if let Some(rest) = upper.strip_prefix('F') {
        if let Ok(n) = rest.parse::<u16>() {
            if (1..=24).contains(&n) {
                return Some(0x6F + n); // F1 = 0x70
            }
        }
    }
    if upper.len() == 1 {
        let c = upper.as_bytes()[0];
        if (b'A'..=b'Z').contains(&c) {
            return Some(c as u16);
        }
    }
    None
}

/// Convert a virtual-key code to a stable `VK_*` name for settings.json.
pub fn vk_to_name(vk: u16) -> String {
    for (name, code) in vk_name_table() {
        if code == vk {
            return name.to_string();
        }
    }
    format!("VK_0x{vk:02X}")
}

/// Short label for typing-window legend (numpad-oriented).
pub fn vk_short_label(vk: u16) -> String {
    match vk {
        VK_ADD | VK_OEM_PLUS => "+".into(),
        VK_DIVIDE | VK_OEM_2 => "/".into(),
        VK_OEM_5 => "\\".into(),
        VK_CONTROL | VK_LCONTROL | VK_RCONTROL => "Ctr".into(),
        0x1B => "ESC".into(),
        0x0D => "Enter".into(),
        0x08 => "Bksp".into(),
        0x20 => "Space".into(),
        0x09 => "Tab".into(),
        0x27 => "→".into(),
        0x25 => "←".into(),
        _ => {
            let n = vk_to_name(vk);
            n.strip_prefix("VK_")
                .unwrap_or(&n)
                .replace("NUMPAD", "")
                .replace('_', "")
        }
    }
}

/// Roles that must not share the same virtual key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRole {
    Start,
    Confirm,
    Cancel,
    Search,
    OpenWorkdir,
    DigitBack,
}

impl KeyRole {
    pub const ALL: [KeyRole; 6] = [
        KeyRole::Start,
        KeyRole::Confirm,
        KeyRole::Cancel,
        KeyRole::Search,
        KeyRole::OpenWorkdir,
        KeyRole::DigitBack,
    ];

    pub fn label(self) -> &'static str {
        match self {
            KeyRole::Start => "start",
            KeyRole::Confirm => "confirm",
            KeyRole::Cancel => "cancel",
            KeyRole::Search => "search",
            KeyRole::OpenWorkdir => "openWorkdir",
            KeyRole::DigitBack => "digitBack",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyBindingError {
    /// Digits 0–9 (and numpad / NumLock-off equivalents) are reserved.
    ReservedDigit { role: &'static str },
    /// Two roles mapped to the same VK.
    Duplicate {
        role_a: &'static str,
        role_b: &'static str,
        name: String,
    },
    /// Two roles would show the same Display text (empty = VK short name).
    DuplicateDisplay {
        role_a: &'static str,
        role_b: &'static str,
        label: String,
    },
    /// Name could not be parsed.
    UnknownName { role: &'static str, name: String },
    /// Two key sets share the same X1 letter.
    DuplicateChord {
        set_a: String,
        set_b: String,
        letter: char,
    },
}

/// Validate exclusivity and digit reservation for action keys.
///
/// `digitBack` may use NumLock-off nav VKs (e.g. `VK_RIGHT`) so → can backspace
/// digits; those VKs are otherwise reserved as digits for other roles.
pub fn validate_key_bindings(keys: &KeyBindings) -> Result<(), KeyBindingError> {
    let roles = [
        (KeyRole::Start, keys.start.as_str()),
        (KeyRole::Confirm, keys.confirm.as_str()),
        (KeyRole::Cancel, keys.cancel.as_str()),
        (KeyRole::Search, keys.search.as_str()),
        (KeyRole::OpenWorkdir, keys.open_workdir.as_str()),
        (KeyRole::DigitBack, keys.digit_back.as_str()),
    ];
    let mut resolved: Vec<(KeyRole, u16, &str)> = Vec::with_capacity(6);
    for (role, name) in roles {
        let Some(vk) = parse_vk_name(name) else {
            return Err(KeyBindingError::UnknownName {
                role: role.label(),
                name: name.to_string(),
            });
        };
        let allow_digit_vk = role == KeyRole::DigitBack;
        if !allow_digit_vk && digit_from_vk(vk).is_some() {
            return Err(KeyBindingError::ReservedDigit { role: role.label() });
        }
        for (other, ovk, oname) in &resolved {
            if *ovk == vk {
                return Err(KeyBindingError::Duplicate {
                    role_a: other.label(),
                    role_b: role.label(),
                    name: oname.to_string(),
                });
            }
        }
        resolved.push((role, vk, name));
    }
    if keys.wake_enabled {
        let Some(wvk) = parse_vk_name(&keys.wake) else {
            return Err(KeyBindingError::UnknownName {
                role: "wake",
                name: keys.wake.clone(),
            });
        };
        if digit_from_vk(wvk).is_some() {
            return Err(KeyBindingError::ReservedDigit { role: "wake" });
        }
        for (other, ovk, oname) in &resolved {
            if *ovk == wvk {
                return Err(KeyBindingError::Duplicate {
                    role_a: other.label(),
                    role_b: "wake",
                    name: oname.to_string(),
                });
            }
        }
    }
    Ok(())
}

/// X1 letters must be unique `a`–`z` across sets.
pub fn validate_key_set_chords(keys: &KeyBindings) -> Result<(), KeyBindingError> {
    let mut seen: Vec<(char, String)> = Vec::new();
    for set in &keys.sets {
        let Some(letter) = normalize_key_chord(&set.chord) else {
            return Err(KeyBindingError::UnknownName {
                role: "chord",
                name: set.chord.clone(),
            });
        };
        if let Some((c, other)) = seen.iter().find(|(c, _)| *c == letter) {
            return Err(KeyBindingError::DuplicateChord {
                set_a: other.clone(),
                set_b: set.id.clone(),
                letter: *c,
            });
        }
        seen.push((letter, set.id.clone()));
    }
    Ok(())
}

/// Display text must be unique (empty field = VK short name).
pub fn validate_display_labels(keys: &KeyBindings) -> Result<(), KeyBindingError> {
    let mut seen: Vec<(KeyRole, String)> = Vec::with_capacity(6);
    for role in KeyRole::ALL {
        let label = keys.display_label(role);
        for (other, olabel) in &seen {
            if olabel == &label {
                return Err(KeyBindingError::DuplicateDisplay {
                    role_a: other.label(),
                    role_b: role.label(),
                    label: label.clone(),
                });
            }
        }
        seen.push((role, label));
    }
    Ok(())
}

/// Assign `vk` to `role`. Same VK on another role swaps the two bindings.
/// Changed roles get Display overwritten with the clamped VK short name.
pub fn apply_captured_vk(
    keys: &mut KeyBindings,
    role: KeyRole,
    vk: u16,
) -> Result<Option<KeyRole>, KeyBindingError> {
    let name = vk_to_name(vk);
    let allow_digit = role == KeyRole::DigitBack;
    if !allow_digit && digit_from_vk(vk).is_some() {
        return Err(KeyBindingError::ReservedDigit { role: role.label() });
    }
    let other = KeyRole::ALL
        .iter()
        .copied()
        .find(|&r| r != role && keys.resolved_vk(r) == vk);
    if let Some(other) = other {
        let old_self = keys.vk_name(role).to_string();
        keys.set_vk(role, name);
        keys.set_vk(other, old_self);
        keys.set_label(role, &vk_short_label(vk));
        keys.set_label(other, &vk_short_label(keys.resolved_vk(other)));
    } else {
        keys.set_vk(role, name);
        keys.set_label(role, &vk_short_label(vk));
    }
    validate_key_bindings(keys)?;
    validate_display_labels(keys)?;
    Ok(other)
}

/// Assign `vk` as the ON (wake) key. Never swaps with action roles.
/// When `wake_enabled`, a clash with another role is refused.
pub fn apply_captured_wake(keys: &mut KeyBindings, vk: u16) -> Result<(), KeyBindingError> {
    if digit_from_vk(vk).is_some() {
        return Err(KeyBindingError::ReservedDigit { role: "wake" });
    }
    let name = vk_to_name(vk);
    if keys.wake_enabled {
        let mut trial = keys.clone();
        trial.wake = name.clone();
        validate_key_bindings(&trial)?;
    }
    keys.wake = name;
    Ok(())
}

/// Map a virtual-key code to a digit 0–9.
/// Accepts top-row, numpad (NumLock ON), and nav cluster (NumLock OFF).
pub fn digit_from_vk(vk: u16) -> Option<u8> {
    match vk {
        0x30..=0x39 => Some((vk - 0x30) as u8), // VK_0..VK_9
        0x60..=0x69 => Some((vk - 0x60) as u8), // VK_NUMPAD0..9
        0x2D => Some(0),                        // VK_INSERT
        0x23 => Some(1),                        // VK_END
        0x28 => Some(2),                        // VK_DOWN
        0x22 => Some(3),                        // VK_NEXT
        0x25 => Some(4),                        // VK_LEFT
        0x0C => Some(5),                        // VK_CLEAR
        0x27 => Some(6),                        // VK_RIGHT
        0x24 => Some(7),                        // VK_HOME
        0x26 => Some(8),                        // VK_UP
        0x21 => Some(9),                        // VK_PRIOR
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_vks() {
        assert_eq!(parse_vk_name("VK_ADD"), Some(0x6B));
        assert_eq!(parse_vk_name("VK_RETURN"), Some(0x0D));
        assert_eq!(parse_vk_name("NOPE"), None);
    }

    #[test]
    fn digits_numlock_both() {
        assert_eq!(digit_from_vk(0x61), Some(1)); // NUMPAD1
        assert_eq!(digit_from_vk(0x23), Some(1)); // END
        assert_eq!(digit_from_vk(0x31), Some(1)); // top-row 1
    }

    #[test]
    fn debug_start_accepts_add_and_oem_plus() {
        let keys = ResolvedKeys {
            start: VK_ADD,
            confirm: 0x0D,
            cancel: 0x1B,
            search: VK_OEM_5,
            open_workdir: VK_CONTROL,
            digit_back: 0x27,
            wake: VK_SUBTRACT,
            wake_enabled: false,
        };
        assert!(keys.is_start(VK_ADD));
        #[cfg(debug_assertions)]
        assert!(keys.is_start(VK_OEM_PLUS));
        #[cfg(not(debug_assertions))]
        assert!(!keys.is_start(VK_OEM_PLUS));
    }

    #[test]
    fn search_default_is_backslash() {
        let keys = KeyBindings::default().resolved();
        assert_eq!(keys.search, VK_OEM_5);
        assert!(keys.is_search(VK_OEM_5));
        assert!(!keys.is_search(VK_DIVIDE));
        assert!(!keys.is_search(VK_OEM_2));
        assert_eq!(vk_short_label(VK_OEM_5), "\\");
        assert_eq!(vk_short_label(0x1B), "ESC");
    }

    #[test]
    fn display_label_falls_back_to_vk_short() {
        let mut keys = KeyBindings::default();
        assert_eq!(keys.display_label(KeyRole::Search), "\\");
        assert_eq!(keys.display_label(KeyRole::Cancel), "ESC");
        keys.set_label(KeyRole::Search, "Find");
        assert_eq!(keys.display_label(KeyRole::Search), "Fin"); // clamped to 3
        keys.set_label(KeyRole::Search, "");
        assert_eq!(keys.display_label(KeyRole::Search), "\\");
    }

    #[test]
    fn missing_label_fields_deserialize_to_shipped_defaults() {
        let keys: KeyBindings = serde_json::from_str(
            r#"{"start":"VK_ADD","confirm":"VK_RETURN","cancel":"VK_ESCAPE","search":"VK_OEM_5"}"#,
        )
        .unwrap();
        assert_eq!(keys.start_label, "+");
        assert_eq!(keys.confirm_label, "Ent");
        assert_eq!(keys.cancel_label, "ESC");
        assert_eq!(keys.search_label, "\\");
        assert_eq!(keys.open_workdir_label, "Ctr");
        assert_eq!(keys.digit_back_label, "→");
    }

    #[test]
    fn wake_key_is_numpad_minus_when_enabled() {
        let off = KeyBindings::default().resolved();
        assert!(!off.wake_enabled);
        assert!(!off.is_wake(VK_SUBTRACT));
        let on = ResolvedKeys {
            start: VK_ADD,
            confirm: 0x0D,
            cancel: 0x1B,
            search: VK_OEM_5,
            open_workdir: VK_CONTROL,
            digit_back: 0x27,
            wake: VK_SUBTRACT,
            wake_enabled: true,
        };
        assert!(on.is_wake(VK_SUBTRACT));
        assert!(!on.is_wake(VK_ADD));
        assert!(!on.is_wake(VK_DECIMAL));
    }

    #[test]
    fn open_workdir_default_is_ctrl() {
        let keys = KeyBindings::default().resolved();
        assert_eq!(keys.open_workdir, VK_CONTROL);
        assert!(keys.is_open_workdir(VK_CONTROL));
        assert!(keys.is_open_workdir(VK_LCONTROL));
        assert!(keys.is_open_workdir(VK_RCONTROL));
        assert_eq!(vk_short_label(VK_CONTROL), "Ctr");
        assert_eq!(
            KeyBindings::default().display_label(KeyRole::OpenWorkdir),
            "Ctr"
        );
    }

    #[test]
    fn search_accepts_divide_and_oem_2_when_slash_bound() {
        let keys = ResolvedKeys {
            start: VK_ADD,
            confirm: 0x0D,
            cancel: 0x1B,
            search: VK_DIVIDE,
            open_workdir: VK_CONTROL,
            digit_back: 0x27,
            wake: VK_SUBTRACT,
            wake_enabled: false,
        };
        assert!(keys.is_search(VK_DIVIDE));
        assert!(keys.is_search(VK_OEM_2));

        let rebound = ResolvedKeys {
            start: VK_ADD,
            confirm: 0x0D,
            cancel: 0x1B,
            search: VK_OEM_2,
            open_workdir: VK_CONTROL,
            digit_back: 0x27,
            wake: VK_SUBTRACT,
            wake_enabled: false,
        };
        assert!(rebound.is_search(VK_DIVIDE));
        assert!(rebound.is_search(VK_OEM_2));

        let other = ResolvedKeys {
            start: VK_ADD,
            confirm: 0x0D,
            cancel: 0x1B,
            search: 0x78, // F9
            open_workdir: VK_CONTROL,
            digit_back: 0x27,
            wake: VK_SUBTRACT,
            wake_enabled: false,
        };
        assert!(other.is_search(0x78));
        assert!(!other.is_search(VK_OEM_2));
    }

    #[test]
    fn accepts_legacy_phonebook_json_key() {
        let v: KeyBindings = serde_json::from_str(
            r#"{"start":"VK_ADD","confirm":"VK_RETURN","cancel":"VK_ESCAPE","phonebook":"VK_F9"}"#,
        )
        .unwrap();
        assert_eq!(v.search, "VK_F9");
        assert_eq!(v.open_workdir, "VK_CONTROL");
        let out = serde_json::to_string(&v).unwrap();
        assert!(out.contains("\"search\""));
        assert!(!out.contains("phonebook"));
        assert!(out.contains("openWorkdir"));
    }

    #[test]
    fn vk_roundtrip_defaults() {
        for name in [
            "VK_ADD",
            "VK_RETURN",
            "VK_ESCAPE",
            "VK_OEM_5",
            "VK_DIVIDE",
            "VK_CONTROL",
            "VK_LCONTROL",
            "VK_F9",
            "VK_TAB",
        ] {
            let vk = parse_vk_name(name).unwrap();
            assert_eq!(parse_vk_name(&vk_to_name(vk)), Some(vk));
        }
    }

    #[test]
    fn rejects_digit_and_duplicate_bindings() {
        let mut keys = KeyBindings::default();
        keys.start = "VK_1".into();
        assert!(matches!(
            validate_key_bindings(&keys),
            Err(KeyBindingError::ReservedDigit { .. })
        ));

        keys = KeyBindings::default();
        keys.cancel = "VK_RETURN".into();
        assert!(matches!(
            validate_key_bindings(&keys),
            Err(KeyBindingError::Duplicate { .. })
        ));

        keys = KeyBindings::default();
        keys.open_workdir = "VK_RETURN".into();
        assert!(matches!(
            validate_key_bindings(&keys),
            Err(KeyBindingError::Duplicate { .. })
        ));

        assert!(validate_key_bindings(&KeyBindings::default()).is_ok());
    }

    #[test]
    fn display_labels_are_not_validated_as_bindings() {
        let mut keys = KeyBindings::default();
        keys.search_label = "1".into();
        keys.start_label = "9+".into();
        keys.set_label(KeyRole::Search, "Win");
        assert_eq!(keys.search, "VK_OEM_5");
        assert!(validate_key_bindings(&keys).is_ok());
    }

    #[test]
    fn capture_escape_on_confirm_swaps_with_cancel() {
        let mut keys = KeyBindings::default();
        keys.confirm_label = "OK".into();
        keys.cancel_label = "No".into();
        let other = apply_captured_vk(&mut keys, KeyRole::Confirm, 0x1B).unwrap();
        assert_eq!(other, Some(KeyRole::Cancel));
        assert_eq!(keys.confirm, "VK_ESCAPE");
        assert_eq!(keys.cancel, "VK_RETURN");
        assert_eq!(keys.confirm_label, "ESC");
        assert_eq!(keys.cancel_label, "Ent");
    }

    #[test]
    fn capture_overwrites_display_with_short_name() {
        let mut keys = KeyBindings::default();
        keys.confirm_label = "OK".into();
        let other = apply_captured_vk(&mut keys, KeyRole::Confirm, 0x0D).unwrap();
        assert_eq!(other, None);
        assert_eq!(keys.confirm, "VK_RETURN");
        assert_eq!(keys.confirm_label, "Ent");
    }

    #[test]
    fn rejects_duplicate_display_labels() {
        let mut keys = KeyBindings::default();
        keys.start_label = "Same".into();
        keys.confirm_label = "Same".into();
        assert!(matches!(
            validate_display_labels(&keys),
            Err(KeyBindingError::DuplicateDisplay { .. })
        ));
    }

    #[test]
    fn wake_on_conflicts_with_start_subtract() {
        let mut keys = KeyBindings::default();
        keys.wake_enabled = true;
        keys.wake = "VK_SUBTRACT".into();
        keys.start = "VK_SUBTRACT".into();
        assert!(matches!(
            validate_key_bindings(&keys),
            Err(KeyBindingError::Duplicate { role_b: "wake", .. })
        ));
        keys.start = "VK_ADD".into();
        assert!(validate_key_bindings(&keys).is_ok());
        keys.wake_enabled = false;
        keys.start = "VK_SUBTRACT".into();
        assert!(validate_key_bindings(&keys).is_ok());
    }

    #[test]
    fn capture_wake_refuses_swap_when_enabled() {
        let mut keys = KeyBindings::default();
        keys.wake_enabled = true;
        let err = apply_captured_wake(&mut keys, 0x1B).unwrap_err();
        assert!(matches!(
            err,
            KeyBindingError::Duplicate { role_b: "wake", .. }
        ));
        assert_eq!(keys.cancel, "VK_ESCAPE");
        assert_eq!(keys.wake, "VK_SUBTRACT");
    }

    #[test]
    fn capture_wake_stores_vk_when_disabled() {
        let mut keys = KeyBindings::default();
        apply_captured_wake(&mut keys, 0x1B).unwrap();
        assert_eq!(keys.wake, "VK_ESCAPE");
        assert!(!keys.wake_enabled);
        assert_eq!(keys.cancel, "VK_ESCAPE");
        keys.wake_enabled = true;
        assert!(matches!(
            validate_key_bindings(&keys),
            Err(KeyBindingError::Duplicate { role_b: "wake", .. })
        ));
    }

    #[test]
    fn capture_wake_rejects_digit() {
        let mut keys = KeyBindings::default();
        assert!(matches!(
            apply_captured_wake(&mut keys, 0x61),
            Err(KeyBindingError::ReservedDigit { role: "wake" })
        ));
    }

    #[test]
    fn normalize_old_json_builds_two_sets() {
        let mut keys: KeyBindings = serde_json::from_str(
            r#"{"start":"VK_ADD","confirm":"VK_RETURN","cancel":"VK_ESCAPE","search":"VK_OEM_5"}"#,
        )
        .unwrap();
        keys.normalize();
        assert_eq!(keys.active, KEY_SET_NUMPAD);
        assert_eq!(keys.sets.len(), 2);
        assert_eq!(keys.active_chord(), 't');
        assert_eq!(keys.chord_for_id(KEY_SET_KEYBOARD), 'k');
        assert!(keys.activate(KEY_SET_KEYBOARD));
        assert_eq!(keys.start, "VK_OEM_PLUS");
        assert_eq!(keys.wake, "VK_OEM_MINUS");
        assert_eq!(keys.active_chord(), 'k');
        assert!(keys.activate(KEY_SET_NUMPAD));
        assert_eq!(keys.start, "VK_ADD");
    }

    #[test]
    fn key_set_chords_must_be_unique() {
        let mut keys = KeyBindings::default();
        keys.normalize();
        assert!(validate_key_set_chords(&keys).is_ok());
        assert!(keys.set_active_chord('k').is_err());
        assert!(keys.set_active_chord('n').is_ok());
        assert_eq!(keys.active_chord(), 'n');
    }

    #[test]
    fn set_chord_for_id_rejects_duplicate_letter() {
        let mut keys = KeyBindings::default();
        keys.normalize();
        assert!(keys.set_chord_for_id(KEY_SET_KEYBOARD, 'm').is_ok());
        assert_eq!(keys.chord_for_id(KEY_SET_KEYBOARD), 'm');
        assert!(keys.set_chord_for_id(KEY_SET_NUMPAD, 'm').is_err());
        assert_eq!(keys.chord_for_id(KEY_SET_NUMPAD), 't');
    }

    #[test]
    fn each_set_keeps_its_own_wake() {
        let mut keys = KeyBindings::default();
        keys.normalize();
        keys.wake_enabled = true;
        keys.wake = "VK_ESCAPE".into();
        keys.sync_live_into_sets();
        assert!(keys.activate(KEY_SET_KEYBOARD));
        assert!(!keys.wake_enabled);
        keys.wake_enabled = true;
        keys.wake = "VK_OEM_MINUS".into();
        keys.sync_live_into_sets();
        assert!(keys.activate(KEY_SET_NUMPAD));
        assert!(keys.wake_enabled);
        assert_eq!(keys.wake, "VK_ESCAPE");
        assert!(keys.activate(KEY_SET_KEYBOARD));
        assert!(keys.wake_enabled);
        assert_eq!(keys.wake, "VK_OEM_MINUS");
        assert!(keys.resolved().is_wake(VK_OEM_MINUS));
    }
}
