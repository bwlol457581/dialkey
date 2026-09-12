//! Registry Run-key autostart with path self-healing.

use std::path::Path;

use tracing::{info, warn};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
};

use crate::platform::AutoStart as AutoStartTrait;

const RUN_SUBKEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: PCWSTR = w!("DialKey");

pub struct WindowsAutoStart {
    exe_path: String,
}

impl WindowsAutoStart {
    pub fn new(exe_path: &Path) -> Self {
        Self {
            exe_path: exe_path.to_string_lossy().into_owned(),
        }
    }

    /// Apply settings: enable+heal or disable.
    pub fn apply(&self, enabled: bool) -> anyhow::Result<()> {
        if enabled {
            self.heal()
        } else {
            // Only delete if present — ignore missing.
            match self.disable() {
                Ok(()) => Ok(()),
                Err(e) => {
                    warn!("autostart disable: {e}");
                    Ok(())
                }
            }
        }
    }

    fn quoted_command(&self) -> String {
        format!("\"{}\"", self.exe_path)
    }

    fn read_run_value(&self) -> anyhow::Result<Option<String>> {
        unsafe {
            let mut key = std::mem::zeroed();
            let open = RegOpenKeyExW(HKEY_CURRENT_USER, RUN_SUBKEY, 0, KEY_READ, &mut key);
            if open.is_err() {
                return Ok(None);
            }
            let mut kind = REG_SZ;
            let mut size = 0u32;
            let q = RegQueryValueExW(
                key,
                VALUE_NAME,
                None,
                Some(&mut kind),
                None,
                Some(&mut size),
            );
            if q == ERROR_FILE_NOT_FOUND {
                let _ = RegCloseKey(key);
                return Ok(None);
            }
            if q.is_err() || size == 0 {
                let _ = RegCloseKey(key);
                return Ok(None);
            }
            let mut buf = vec![0u8; size as usize];
            let q2 = RegQueryValueExW(
                key,
                VALUE_NAME,
                None,
                Some(&mut kind),
                Some(buf.as_mut_ptr()),
                Some(&mut size),
            );
            let _ = RegCloseKey(key);
            if q2.is_err() {
                return Ok(None);
            }
            // REG_SZ is UTF-16
            let wide: Vec<u16> = buf
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .take_while(|&u| u != 0)
                .collect();
            Ok(Some(String::from_utf16_lossy(&wide)))
        }
    }
}

impl AutoStartTrait for WindowsAutoStart {
    fn enable(&self) -> anyhow::Result<()> {
        let cmd = self.quoted_command();
        unsafe {
            let mut key = std::mem::zeroed();
            RegOpenKeyExW(HKEY_CURRENT_USER, RUN_SUBKEY, 0, KEY_WRITE, &mut key).ok()?;
            let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
            RegSetValueExW(key, VALUE_NAME, 0, REG_SZ, Some(bytes)).ok()?;
            let _ = RegCloseKey(key);
        }
        info!(path = %self.exe_path, "autostart enabled");
        Ok(())
    }

    fn disable(&self) -> anyhow::Result<()> {
        unsafe {
            let mut key = std::mem::zeroed();
            let open = RegOpenKeyExW(HKEY_CURRENT_USER, RUN_SUBKEY, 0, KEY_WRITE, &mut key);
            if open.is_err() {
                return Ok(());
            }
            let del = RegDeleteValueW(key, VALUE_NAME);
            let _ = RegCloseKey(key);
            if del == ERROR_FILE_NOT_FOUND {
                return Ok(());
            }
            del.ok()?;
        }
        info!("autostart disabled (Run key removed)");
        Ok(())
    }

    fn heal(&self) -> anyhow::Result<()> {
        let expected = self.quoted_command();
        match self.read_run_value()? {
            None => {
                // Not registered yet — enable for the first time when autostart is on.
                self.enable()
            }
            Some(current) if current == expected => {
                info!("autostart Run key already correct");
                Ok(())
            }
            Some(current) => {
                info!(old = %current, new = %expected, "autostart path changed — rewriting Run key");
                self.enable()
            }
        }
    }
}

/// `--uninstall`: remove the Run key only (never touches config files).
pub fn uninstall_run_key(exe_path: &Path) -> anyhow::Result<()> {
    WindowsAutoStart::new(exe_path).disable()
}
