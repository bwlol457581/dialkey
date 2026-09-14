//! Process launching and path resolution (OS-independent logic).
//!
//! Actual process spawn uses `std::process`; env expansion uses `std::env`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use thiserror::Error;
use tracing::{error, info, warn};

use super::chain::{is_chain_path, parse_chain_ids};
use super::date_tokens::{
    contains_date_token, deepest_existing_ancestor, expand_date_tokens, DateParts,
};
use super::slots::{Slot, SlotRegistry};
use super::winpath::{path_exists, path_for_win32, path_is_dir};

/// Best-effort: if the target is already open, bring it to the foreground.
/// Installed by the Windows platform at startup. Never waits / never polls.
type FocusExistingFn = fn(&Path) -> bool;

static FOCUS_EXISTING: OnceLock<FocusExistingFn> = OnceLock::new();

/// Open a document / folder via the platform shell (optional).
type ShellOpenFn = fn(file: &Path, workdir: Option<&Path>) -> Result<(), std::io::Error>;

static SHELL_OPEN: OnceLock<ShellOpenFn> = OnceLock::new();

/// Register the platform helper for "already open → focus" (optional).
pub fn set_focus_existing_handler(f: FocusExistingFn) {
    let _ = FOCUS_EXISTING.set(f);
}

/// Register the platform helper for shell-open (documents / folders).
pub fn set_shell_open_handler(f: ShellOpenFn) {
    let _ = SHELL_OPEN.set(f);
}

fn try_focus_existing(path: &Path) -> bool {
    FOCUS_EXISTING.get().is_some_and(|f| f(path))
}

#[derive(Debug, Error)]
pub enum LaunchError {
    #[error("path is empty")]
    EmptyPath,
    #[error("working folder is missing or not a directory")]
    WorkdirUnavailable,
    #[error("failed to spawn process: {0}")]
    Spawn(#[from] std::io::Error),
}

/// Expand `%VAR%` / `$VAR` style environment variables in a path string.
pub fn expand_env(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let chars: Vec<char> = path.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '%') {
                let name: String = chars[i + 1..i + 1 + end].iter().collect();
                if let Ok(val) = std::env::var(&name) {
                    out.push_str(&val);
                } else {
                    out.push('%');
                    out.push_str(&name);
                    out.push('%');
                }
                i += end + 2;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Resolve `path` against the DialKey executable directory (not CWD).
///
/// Order: date tokens → environment variables → absolute / relative.
/// A bare filename (`notepad.exe`) is left as-is so Windows can search PATH.
/// Relative paths with a directory component (`tools/foo.bat`) resolve
/// against the executable's folder.
#[cfg_attr(not(test), allow(dead_code))]
pub fn resolve_path(path: &str, exe_dir: &Path) -> PathBuf {
    resolve_path_at(path, exe_dir, &DateParts::now())
}

/// Same as [`resolve_path`], with an explicit instant for date tokens (tests).
pub fn resolve_path_at(path: &str, exe_dir: &Path, when: &DateParts) -> PathBuf {
    let dated = expand_date_tokens(path, when);
    let expanded = expand_env(&dated);
    let p = PathBuf::from(&expanded);
    if p.is_absolute() {
        p
    } else if is_bare_filename(&p) {
        p
    } else {
        exe_dir.join(p)
    }
}

/// Resolve for launch: after normal resolution, if the raw string had date
/// tokens and the target is missing, walk up to the deepest existing ancestor.
/// Never creates folders. Not applied to URL / `chain:` (those skip resolution).
fn resolve_launch_path(raw: &str, exe_dir: &Path, when: &DateParts) -> PathBuf {
    let resolved = resolve_path_at(raw, exe_dir, when);
    if path_exists(&resolved) || !contains_date_token(raw) {
        return resolved;
    }
    let fallback = deepest_existing_ancestor(&resolved);
    if fallback != resolved {
        info!(
            from = %resolved.display(),
            to = %fallback.display(),
            "date-token path missing; opening deepest existing ancestor"
        );
    }
    fallback
}

fn is_bare_filename(p: &Path) -> bool {
    p.components().count() == 1
}

fn default_workdir(resolved: &Path, exe_dir: &Path) -> PathBuf {
    match resolved.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => exe_dir.to_path_buf(),
    }
}

/// `http://` or `https://` — opened with the default browser (not path-resolved).
pub fn is_http_url(path: &str) -> bool {
    let t = path.trim();
    let b = t.as_bytes();
    if b.len() < 7 {
        return false;
    }
    t.get(..7)
        .map(|s| s.eq_ignore_ascii_case("http://"))
        .unwrap_or(false)
        || t.get(..8)
            .map(|s| s.eq_ignore_ascii_case("https://"))
            .unwrap_or(false)
}

/// Fire-and-forget launch. Never waits on the child.
///
/// `chain:` paths launch referenced slots one level deep (no recursion).
/// Failures inside a chain are logged; remaining targets still run.
pub fn launch_slot(
    slot: &Slot,
    exe_dir: &Path,
    registry: &SlotRegistry,
) -> Result<(), LaunchError> {
    launch_slot_opts(slot, exe_dir, registry, false)
}

/// Open the slot's working folder only. Does not launch Path.
///
/// Folder is `workdir` if set, otherwise Path's parent. The slot `openWorkdir`
/// flag is ignored. A top-level URL has no folder. A `chain:` slot opens each
/// member's folder in order (same as opening 101 then 102). Nested chain / URL
/// / missing ids are skipped; the chain's own `workdir` is unused.
pub fn open_slot_workdir(
    slot: &Slot,
    exe_dir: &Path,
    registry: &SlotRegistry,
) -> Result<(), LaunchError> {
    let folders = workdir_open_targets(slot, exe_dir, registry);
    if folders.is_empty() {
        info!(
            slot_id = %slot.id,
            "open workdir skipped (no folder); path not launched"
        );
        return Err(LaunchError::WorkdirUnavailable);
    }
    let mut any_ok = false;
    for folder in &folders {
        info!(
            slot_id = %slot.id,
            folder = %folder.display(),
            "opening workdir only"
        );
        #[cfg(windows)]
        {
            if shell_open_path(folder).is_ok() {
                any_ok = true;
            }
        }
        #[cfg(not(windows))]
        {
            let _ = folder;
            any_ok = true;
        }
    }
    if any_ok {
        Ok(())
    } else {
        Err(LaunchError::WorkdirUnavailable)
    }
}

/// Folders that Open-workdir would try, in order. Empty → no folder to open.
pub fn workdir_open_targets(slot: &Slot, exe_dir: &Path, registry: &SlotRegistry) -> Vec<PathBuf> {
    if is_http_url(&slot.path) {
        return Vec::new();
    }
    if is_chain_path(&slot.path) {
        return chain_workdir_targets(slot, exe_dir, registry);
    }
    plain_workdir_if_dir(slot, exe_dir).into_iter().collect()
}

fn chain_workdir_targets(slot: &Slot, exe_dir: &Path, registry: &SlotRegistry) -> Vec<PathBuf> {
    let targets = match parse_chain_ids(&slot.path) {
        Ok(ids) => ids,
        Err(e) => {
            error!(slot_id = %slot.id, error = %e, "invalid chain path");
            return Vec::new();
        }
    };
    info!(slot_id = %slot.id, targets = ?targets, "opening chain member folders");
    let mut out = Vec::new();
    for tid in &targets {
        let Some(target) = registry.get(tid) else {
            error!(slot_id = %slot.id, target = %tid, "chain target missing");
            continue;
        };
        if is_chain_path(&target.path) {
            warn!(
                slot_id = %slot.id,
                target = %tid,
                "skipping nested chain (one level only)"
            );
            continue;
        }
        if is_http_url(&target.path) {
            info!(slot_id = %slot.id, target = %tid, "open workdir skip url member");
            continue;
        }
        if let Some(folder) = plain_workdir_if_dir(target, exe_dir) {
            out.push(folder);
        }
    }
    out
}

fn plain_workdir_if_dir(slot: &Slot, exe_dir: &Path) -> Option<PathBuf> {
    let when = DateParts::now();
    let folder = workdir_folder(slot, exe_dir, &when);
    if path_is_dir(&folder) {
        Some(folder)
    } else {
        error!(
            slot_id = %slot.id,
            folder = %folder.display(),
            "workdir is not an existing folder"
        );
        None
    }
}

fn launch_slot_opts(
    slot: &Slot,
    exe_dir: &Path,
    registry: &SlotRegistry,
    force_open_workdir: bool,
) -> Result<(), LaunchError> {
    if slot.path.trim().is_empty() {
        return Err(LaunchError::EmptyPath);
    }

    if is_chain_path(&slot.path) {
        return launch_chain(slot, exe_dir, registry);
    }

    // URLs must not go through path resolution (would join against exe_dir).
    if is_http_url(&slot.path) {
        return launch_url(slot);
    }

    // One instant for path + workdir + openWorkdir (spec: fixed to "today").
    let when = DateParts::now();
    let result = launch_path_slot(slot, exe_dir, &when);
    if result.is_ok() {
        maybe_open_workdir(slot, exe_dir, &when, force_open_workdir);
    }
    result
}

fn launch_chain(slot: &Slot, exe_dir: &Path, registry: &SlotRegistry) -> Result<(), LaunchError> {
    let targets = match parse_chain_ids(&slot.path) {
        Ok(ids) => ids,
        Err(e) => {
            error!(slot_id = %slot.id, error = %e, "invalid chain path");
            return Err(LaunchError::EmptyPath);
        }
    };

    info!(slot_id = %slot.id, targets = ?targets, "launching chain");
    let mut any_ok = false;
    for tid in &targets {
        let Some(target) = registry.get(tid) else {
            error!(
                slot_id = %slot.id,
                target = %tid,
                "chain target missing"
            );
            continue;
        };
        if is_chain_path(&target.path) {
            warn!(
                slot_id = %slot.id,
                target = %tid,
                "skipping nested chain (one level only)"
            );
            continue;
        }
        match launch_slot(target, exe_dir, registry) {
            Ok(()) => any_ok = true,
            Err(e) => {
                error!(
                    slot_id = %slot.id,
                    target = %tid,
                    error = %e,
                    "chain target launch failed; continuing"
                );
            }
        }
    }
    if any_ok || targets.is_empty() {
        Ok(())
    } else {
        // All targets failed or were skipped — still Ok so the typing window
        // can show acceptance feedback; failures were already logged/toasted
        // per target. Spec: continue on failure; no chain-specific error type.
        Ok(())
    }
}

fn launch_path_slot(slot: &Slot, exe_dir: &Path, when: &DateParts) -> Result<(), LaunchError> {
    let resolved = resolve_launch_path(&slot.path, exe_dir, when);
    let workdir = if slot.workdir.trim().is_empty() {
        default_workdir(&resolved, exe_dir)
    } else {
        resolve_launch_path(&slot.workdir, exe_dir, when)
    };

    // Best-effort reuse: if already open, foreground and skip a new spawn.
    // Failure to find/focus falls through to normal launch (macropad-like).
    // Per-slot `focusExisting: false` skips the scan entirely.
    if slot.focus_existing && try_focus_existing(&resolved) {
        info!(
            slot_id = %slot.id,
            path = %resolved.display(),
            "focused existing window; skipped new launch"
        );
        return Ok(());
    }

    let ext = resolved
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let spawn_path = path_for_win32(&resolved);
    let spawn_dir = path_for_win32(&workdir);
    let path_str = spawn_path.to_string_lossy();

    // Documents, folders, and other associated types (`.xlsx`, `.ahk`, …) open
    // via the shell (file association). Scripts, `.exe`, and bare PATH names
    // use process spawn. (core stays free of Win32 APIs.)
    #[cfg(windows)]
    if path_is_dir(&resolved) || !matches!(ext.as_str(), "bat" | "cmd" | "ps1" | "exe" | "") {
        return shell_open(&resolved, &workdir, slot);
    }

    let mut cmd = match ext.as_str() {
        "bat" | "cmd" => {
            let mut c = Command::new("cmd");
            c.arg("/c").arg(path_str.as_ref());
            c
        }
        "ps1" => {
            let mut c = Command::new("powershell");
            c.arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-File")
                .arg(path_str.as_ref());
            c
        }
        _ => Command::new(spawn_path.as_os_str()),
    };

    cmd.current_dir(&spawn_dir);

    info!(
        slot_id = %slot.id,
        path = %resolved.display(),
        "launching"
    );

    match cmd.spawn() {
        Ok(child) => {
            info!(pid = child.id(), slot_id = %slot.id, "process launched");
            // Intentionally drop Child without wait.
            Ok(())
        }
        Err(e) => {
            error!(
                slot_id = %slot.id,
                path = %resolved.display(),
                error = %e,
                "process launch failed"
            );
            Err(LaunchError::Spawn(e))
        }
    }
}

fn workdir_folder(slot: &Slot, exe_dir: &Path, when: &DateParts) -> PathBuf {
    let resolved = resolve_launch_path(&slot.path, exe_dir, when);
    if slot.workdir.trim().is_empty() {
        default_workdir(&resolved, exe_dir)
    } else {
        resolve_launch_path(&slot.workdir, exe_dir, when)
    }
}

/// Open the effective working directory alongside the launch (B-1 / Phase 9-2).
/// When `force` is true, open even if the slot's `openWorkdir` flag is false.
fn maybe_open_workdir(slot: &Slot, exe_dir: &Path, when: &DateParts, force: bool) {
    if !force && !slot.open_workdir {
        return;
    }
    if is_http_url(&slot.path) || is_chain_path(&slot.path) {
        return;
    }

    let folder = workdir_folder(slot, exe_dir, when);

    info!(
        slot_id = %slot.id,
        folder = %folder.display(),
        force,
        "opening workdir alongside launch"
    );

    if !path_is_dir(&folder) {
        error!(
            slot_id = %slot.id,
            folder = %folder.display(),
            "workdir is not an existing folder; skipped open"
        );
        return;
    }

    #[cfg(windows)]
    {
        if let Err(e) = shell_open_path(&folder) {
            error!(
                slot_id = %slot.id,
                folder = %folder.display(),
                error = %e,
                "failed to open workdir"
            );
        }
    }
}

/// Open a document / associated file / folder with the platform shell.
#[cfg(windows)]
fn shell_open(resolved: &Path, workdir: &Path, slot: &Slot) -> Result<(), LaunchError> {
    info!(
        slot_id = %slot.id,
        path = %resolved.display(),
        "shell-opening"
    );
    match platform_shell_open(resolved, Some(workdir)) {
        Ok(()) => {
            info!(slot_id = %slot.id, "shell open started");
            Ok(())
        }
        Err(e) => {
            error!(
                slot_id = %slot.id,
                path = %resolved.display(),
                error = %e,
                "shell open failed"
            );
            Err(e)
        }
    }
}

/// Open a folder/path with the shell (fire-and-forget).
#[cfg(windows)]
fn shell_open_path(path: &Path) -> Result<(), LaunchError> {
    platform_shell_open(path, None)
}

fn platform_shell_open(file: &Path, workdir: Option<&Path>) -> Result<(), LaunchError> {
    if let Some(f) = SHELL_OPEN.get() {
        return f(file, workdir).map_err(LaunchError::Spawn);
    }
    #[cfg(windows)]
    {
        return fallback_cmd_start(file, workdir);
    }
    #[cfg(not(windows))]
    {
        let _ = (file, workdir);
        Err(LaunchError::Spawn(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "shell open requires Windows",
        )))
    }
}

#[cfg(windows)]
fn fallback_cmd_start(file: &Path, workdir: Option<&Path>) -> Result<(), LaunchError> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let spawn_file = path_for_win32(file);
    let path_str = spawn_file.to_string_lossy();
    let mut cmd = Command::new("cmd");
    cmd.arg("/c")
        .arg("start")
        .arg("")
        .arg(path_str.as_ref())
        .creation_flags(CREATE_NO_WINDOW);
    if let Some(dir) = workdir {
        cmd.current_dir(path_for_win32(dir));
    }
    cmd.spawn().map(|_| ()).map_err(LaunchError::Spawn)
}

fn launch_url(slot: &Slot) -> Result<(), LaunchError> {
    let url = slot.path.trim();
    info!(slot_id = %slot.id, url, "opening URL in default browser");

    #[cfg(windows)]
    {
        // `start "" <url>` — empty title so `start` does not treat the URL as a title.
        // CREATE_NO_WINDOW avoids a flashing console.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut cmd = Command::new("cmd");
        cmd.arg("/c")
            .arg("start")
            .arg("")
            .arg(url)
            .creation_flags(CREATE_NO_WINDOW);
        match cmd.spawn() {
            Ok(child) => {
                info!(pid = child.id(), slot_id = %slot.id, "URL open started");
                Ok(())
            }
            Err(e) => {
                error!(slot_id = %slot.id, url, error = %e, "URL open failed");
                Err(LaunchError::Spawn(e))
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = url;
        Err(LaunchError::Spawn(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "URL launch requires Windows",
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_resolves_against_exe_dir() {
        let exe = Path::new("D:/apps/dialkey");
        let p = resolve_path("tools/foo.bat", exe);
        assert_eq!(p, PathBuf::from("D:/apps/dialkey/tools/foo.bat"));
    }

    #[test]
    fn bare_filename_left_for_path_search() {
        let exe = Path::new("D:/apps/dialkey");
        let p = resolve_path("notepad.exe", exe);
        assert_eq!(p, PathBuf::from("notepad.exe"));
    }

    #[test]
    fn absolute_unchanged() {
        let exe = Path::new("D:/apps/dialkey");
        let p = resolve_path("C:/Windows/notepad.exe", exe);
        assert_eq!(p, PathBuf::from("C:/Windows/notepad.exe"));
    }

    #[test]
    fn expand_userprofile() {
        std::env::set_var("DIALKEY_TEST_VAR", "hello");
        let s = expand_env("%DIALKEY_TEST_VAR%/x");
        assert_eq!(s, "hello/x");
        std::env::remove_var("DIALKEY_TEST_VAR");
    }

    #[test]
    fn detects_http_urls() {
        assert!(is_http_url("https://example.com"));
        assert!(is_http_url("  HTTP://Example.COM/x  "));
        assert!(is_http_url("http://localhost:8080"));
        assert!(!is_http_url("notepad.exe"));
        assert!(!is_http_url("C:/tools/app.exe"));
        assert!(!is_http_url("ftp://example.com"));
        assert!(!is_http_url("www.example.com"));
    }

    #[test]
    fn date_tokens_expand_before_env() {
        std::env::set_var("DIALKEY_DATE_TEST", "base");
        let when = DateParts {
            year: 2026,
            month: 8,
            day: 10,
            hour: 14,
            minute: 5,
        };
        let exe = Path::new("D:/apps/dialkey");
        let p = resolve_path_at(
            "%DIALKEY_DATE_TEST%/{yyyyMM}/report_{yyyyMM}.xlsx",
            exe,
            &when,
        );
        assert_eq!(
            p,
            exe.join("base").join("202608").join("report_202608.xlsx")
        );
        std::env::remove_var("DIALKEY_DATE_TEST");
    }

    #[test]
    fn date_tokens_on_absolute_path() {
        let when = DateParts {
            year: 2026,
            month: 8,
            day: 10,
            hour: 14,
            minute: 5,
        };
        let exe = Path::new("D:/apps/dialkey");
        let p = resolve_path_at(r"D:\work\{yyyy}\{yyyyMM}\a.xlsx", exe, &when);
        assert_eq!(p, PathBuf::from(r"D:\work\2026\202608\a.xlsx"));
    }

    fn slot(path: &str, workdir: &str) -> Slot {
        Slot {
            id: "1".into(),
            name: "t".into(),
            path: path.into(),
            description: String::new(),
            workdir: workdir.into(),
            open_workdir: false,
            focus_existing: true,
        }
    }

    #[test]
    fn open_workdir_skips_url_and_walks_chain() {
        let exe = Path::new("D:/apps/dialkey");
        let folder = std::env::temp_dir().join("dialkey-workdir-open-test");
        std::fs::create_dir_all(&folder).expect("create test folder");
        assert!(
            crate::core::winpath::path_is_dir(&folder),
            "Open needs a real folder; got {}",
            folder.display()
        );
        let folder_s = folder.to_string_lossy().into_owned();
        let empty = SlotRegistry::new(vec![]);
        assert!(matches!(
            open_slot_workdir(&slot("https://example.com", ""), exe, &empty),
            Err(LaunchError::WorkdirUnavailable)
        ));
        let chain = slot("chain:11,20", "");
        let mut chain = chain;
        chain.id = "12".into();
        let a = {
            let mut s = slot(r"D:\apps\tool.exe", &folder_s);
            s.id = "11".into();
            s
        };
        let b = {
            let mut s = slot("https://example.com", "");
            s.id = "20".into();
            s
        };
        let nested = {
            let mut s = slot("chain:11", "");
            s.id = "99".into();
            s
        };
        let chain_nested = {
            let mut s = slot("chain:11,99,20,88", "");
            s.id = "12".into();
            s
        };
        let reg = SlotRegistry::new(vec![a, b, nested, chain_nested.clone()]);
        let folders = workdir_open_targets(&chain_nested, exe, &reg);
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0], folder);
        assert!(workdir_open_targets(&chain, exe, &empty).is_empty());
    }

    #[test]
    fn workdir_folder_uses_explicit_then_path_parent() {
        let exe = Path::new("D:/apps/dialkey");
        let when = DateParts::now();
        let with_wd = slot(r"D:\apps\tool.exe", r"D:\work");
        assert_eq!(
            workdir_folder(&with_wd, exe, &when),
            PathBuf::from(r"D:\work")
        );
        let parent = slot(r"D:\apps\tool.exe", "");
        assert_eq!(
            workdir_folder(&parent, exe, &when),
            PathBuf::from(r"D:\apps")
        );
    }
}
