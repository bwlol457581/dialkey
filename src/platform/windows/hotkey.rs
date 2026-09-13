//! Global hotkey trigger via RegisterHotKey.

use tracing::{info, warn};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN,
    VIRTUAL_KEY,
};

use crate::core::keys::parse_vk_name;

use super::messages::HOTKEY_ID;

pub struct HotkeyTrigger {
    hwnd: HWND,
    registered: bool,
}

impl HotkeyTrigger {
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            registered: false,
        }
    }

    /// Parse settings hotkey string like `"F9"`, `"VK_F9"`, `"Ctrl+Alt+D"`.
    /// Empty string disables the hotkey.
    pub fn install_from_string(&mut self, key: &str) -> anyhow::Result<()> {
        self.uninstall();
        let key = key.trim();
        if key.is_empty() {
            info!("hotkey trigger disabled (empty)");
            return Ok(());
        }
        let Some((mods, vk)) = parse_hotkey(key) else {
            warn!(key, "could not parse hotkey; leaving disabled");
            return Ok(());
        };
        unsafe {
            if RegisterHotKey(self.hwnd, HOTKEY_ID, mods, vk.0 as u32).is_err() {
                anyhow::bail!("RegisterHotKey failed for '{key}'");
            }
        }
        self.registered = true;
        info!(key, "hotkey registered");
        Ok(())
    }

    pub fn uninstall(&mut self) {
        if self.registered {
            unsafe {
                let _ = UnregisterHotKey(self.hwnd, HOTKEY_ID);
            }
            self.registered = false;
        }
    }
}

impl Drop for HotkeyTrigger {
    fn drop(&mut self) {
        self.uninstall();
    }
}

fn parse_hotkey(spec: &str) -> Option<(HOT_KEY_MODIFIERS, VIRTUAL_KEY)> {
    let mut mods = HOT_KEY_MODIFIERS(0);
    let mut vk: Option<u16> = None;
    for part in spec.split('+') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        match p.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= MOD_CONTROL,
            "alt" => mods |= MOD_ALT,
            "shift" => mods |= MOD_SHIFT,
            "win" | "windows" | "super" => mods |= MOD_WIN,
            other => {
                let name = if other.starts_with("vk_") {
                    other.to_ascii_uppercase()
                } else if other.len() == 1 {
                    let ch = other.chars().next()?.to_ascii_uppercase();
                    format!("VK_{ch}")
                } else {
                    format!("VK_{}", other.to_ascii_uppercase())
                };
                // F-keys and letters via extended table
                vk = parse_vk_name(&name).or_else(|| parse_vk_extended(&name));
            }
        }
    }
    vk.map(|v| (mods, VIRTUAL_KEY(v)))
}

fn parse_vk_extended(name: &str) -> Option<u16> {
    match name {
        "VK_F1" => Some(0x70),
        "VK_F2" => Some(0x71),
        "VK_F3" => Some(0x72),
        "VK_F4" => Some(0x73),
        "VK_F5" => Some(0x74),
        "VK_F6" => Some(0x75),
        "VK_F7" => Some(0x76),
        "VK_F8" => Some(0x77),
        "VK_F9" => Some(0x78),
        "VK_F10" => Some(0x79),
        "VK_F11" => Some(0x7A),
        "VK_F12" => Some(0x7B),
        "VK_A" => Some(0x41),
        "VK_B" => Some(0x42),
        "VK_C" => Some(0x43),
        "VK_D" => Some(0x44),
        "VK_E" => Some(0x45),
        "VK_F" => Some(0x46),
        "VK_G" => Some(0x47),
        "VK_H" => Some(0x48),
        "VK_I" => Some(0x49),
        "VK_J" => Some(0x4A),
        "VK_K" => Some(0x4B),
        "VK_L" => Some(0x4C),
        "VK_M" => Some(0x4D),
        "VK_N" => Some(0x4E),
        "VK_O" => Some(0x4F),
        "VK_P" => Some(0x50),
        "VK_Q" => Some(0x51),
        "VK_R" => Some(0x52),
        "VK_S" => Some(0x53),
        "VK_T" => Some(0x54),
        "VK_U" => Some(0x55),
        "VK_V" => Some(0x56),
        "VK_W" => Some(0x57),
        "VK_X" => Some(0x58),
        "VK_Y" => Some(0x59),
        "VK_Z" => Some(0x5A),
        _ => None,
    }
}

pub fn hotkey_string_from_triggers(triggers: &[serde_json::Value]) -> String {
    for t in triggers {
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
