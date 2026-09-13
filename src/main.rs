//! DialKey — a software macropad.
//!
//! See `docs/spec.md` for the full specification.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod platform;

use std::path::PathBuf;
use std::process::ExitCode;

use crate::core::slots::is_valid_slot_id;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if is_cli_command(&args) {
        attach_parent_console();
    }
    match run_main(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn is_cli_command(args: &[String]) -> bool {
    args.iter().any(|a| {
        matches!(
            a.as_str(),
            "--help"
                | "-h"
                | "--version"
                | "-V"
                | "--uninstall"
                | "--list-slots"
                | "--launch"
                | "--reload"
        ) || a.starts_with("--launch=")
    })
}

/// Release builds use the Windows subsystem (no console). Attach to the parent
/// so `--launch` / `--list-slots` / `--help` still print when run from a shell.
fn attach_parent_console() {
    #[cfg(all(windows, not(debug_assertions)))]
    unsafe {
        use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn run_main(args: &[String]) -> anyhow::Result<ExitCode> {
    let args = args.to_vec();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("DialKey {}", env!("CARGO_PKG_VERSION"));
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        let locale = peek_ui_locale(&exe_dir, parse_config_arg(&args).as_deref());
        #[cfg(windows)]
        let os = platform::windows::os_ui_language();
        #[cfg(not(windows))]
        let os: Option<String> = None;
        let cat = core::i18n::resolve(
            &locale,
            &core::i18n::lang_dir_next_to_exe(&exe_dir),
            os.as_deref(),
        );
        if let Some(line) = cat.version_line() {
            println!("{line}");
        }
        return Ok(ExitCode::SUCCESS);
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(ExitCode::SUCCESS);
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let exe_path = std::env::current_exe().unwrap_or_else(|_| exe_dir.join("dialkey.exe"));
    let cli_config = parse_config_arg(&args);

    if args.iter().any(|a| a == "--uninstall") {
        let _ = core::logging::init(&exe_dir, "info");
        #[cfg(windows)]
        {
            platform::windows::uninstall_run_key(&exe_path)?;
            println!("Autostart Run key removed. Configuration files were left untouched.");
            return Ok(ExitCode::SUCCESS);
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("--uninstall is only supported on Windows");
        }
    }

    if args.iter().any(|a| a == "--list-slots") {
        return list_slots(&exe_dir, cli_config.as_deref());
    }

    if args.iter().any(|a| a == "--reload") {
        return reload_cli();
    }

    if let Some(id) = parse_launch_arg(&args)? {
        return launch_slot_cli(&exe_dir, cli_config.as_deref(), &id);
    }

    #[cfg(windows)]
    {
        let instance = match platform::windows::SingleInstance::acquire() {
            Ok(g) => g,
            Err(platform::windows::AlreadyRunning) => return Ok(ExitCode::SUCCESS),
        };

        // Init logging before config load so load warnings reach the file.
        let peek_level = peek_log_level(&exe_dir, cli_config.as_deref());
        let log_dir = core::logging::init(&exe_dir, &peek_level);

        let config = match core::load_app_config(&exe_dir, cli_config.as_deref()) {
            Ok(c) => c,
            Err(core::ConfigError::SettingsTooNew { found }) => {
                anyhow::bail!(
                    "settings.json schemaVersion {found} is newer than this DialKey build supports"
                );
            }
            Err(core::ConfigError::SlotsTooNew { found }) => {
                anyhow::bail!(
                    "slots JSON schemaVersion {found} is newer than this DialKey build supports"
                );
            }
            Err(e) => return Err(e.into()),
        };

        tracing::info!(
            version = env!("CARGO_PKG_VERSION"),
            log_dir = ?log_dir,
            log_level = %config.settings.log_level,
            "DialKey starting"
        );

        let result = platform::windows::run(config);
        drop(instance);
        result?;
        return Ok(ExitCode::SUCCESS);
    }

    #[cfg(not(windows))]
    {
        let _ = (exe_dir, cli_config, exe_path);
        anyhow::bail!(
            "DialKey requires Windows. WSL cannot see host input devices — build and run on Windows."
        );
    }
}

fn print_help() {
    println!(
        "DialKey {} — a software macropad\n\n\
         Usage:\n\
           dialkey [OPTIONS]\n\n\
         Options:\n\
           --launch <id>    Launch slot by ID (IPC if resident, else one-shot)\n\
           --list-slots     Print registered slots (id<TAB>name), from disk\n\
           --reload         Ask resident DialKey to reload settings/slots\n\
           --config <dir>   Config folder (settings.json + slots*.json)\n\
           --uninstall      Remove autostart Run key (keeps config files)\n\
           --version        Print version\n\
           --help           Show this help\n\n\
         Exit codes for --launch:\n\
           0  success\n\
           1  usage / invalid id\n\
           2  unknown slot\n\
           3  launch failed\n",
        env!("CARGO_PKG_VERSION")
    );
}

/// `--launch <id>` / `--launch=<id>`. `Ok(None)` if the flag is absent.
fn parse_launch_arg(args: &[String]) -> anyhow::Result<Option<String>> {
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        if a == "--launch" {
            let Some(id) = iter.next() else {
                anyhow::bail!("--launch requires a slot id (digits only)");
            };
            return Ok(Some(validate_launch_id(id)?));
        }
        if let Some(id) = a.strip_prefix("--launch=") {
            return Ok(Some(validate_launch_id(id)?));
        }
    }
    Ok(None)
}

fn validate_launch_id(id: &str) -> anyhow::Result<String> {
    let id = id.trim();
    if !is_valid_slot_id(id) {
        anyhow::bail!(
            "invalid slot id {id:?}: use digits only (leading zeros matter; \"01\" ≠ \"1\")"
        );
    }
    Ok(id.to_string())
}

fn list_slots(
    exe_dir: &std::path::Path,
    cli_config: Option<&std::path::Path>,
) -> anyhow::Result<ExitCode> {
    let config = core::load_app_config(exe_dir, cli_config)?;
    for slot in config.slots.sorted() {
        println!("{}\t{}", slot.id, slot.name);
    }
    Ok(ExitCode::SUCCESS)
}

fn reload_cli() -> anyhow::Result<ExitCode> {
    #[cfg(windows)]
    {
        match platform::windows::try_signal_reload() {
            Ok(()) => {
                println!("Reload requested.");
                return Ok(ExitCode::SUCCESS);
            }
            Err(platform::windows::SignalLaunchError::NotRunning) => {
                eprintln!("error: DialKey is not running (nothing to reload)");
                return Ok(ExitCode::from(1));
            }
            Err(platform::windows::SignalLaunchError::Host(_)) => {
                eprintln!("error: failed to signal resident DialKey");
                return Ok(ExitCode::from(1));
            }
        }
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("--reload is only supported on Windows");
    }
}

fn launch_slot_cli(
    exe_dir: &std::path::Path,
    cli_config: Option<&std::path::Path>,
    id: &str,
) -> anyhow::Result<ExitCode> {
    let _ = core::logging::init(exe_dir, &peek_log_level(exe_dir, cli_config));

    #[cfg(windows)]
    {
        match platform::windows::try_signal_launch(id) {
            Ok(()) => return Ok(ExitCode::SUCCESS),
            Err(platform::windows::SignalLaunchError::NotRunning) => {}
            Err(platform::windows::SignalLaunchError::Host(code)) => {
                return Ok(exit_from_launch_code(code, id));
            }
        }
    }

    // One-shot: no tray / hooks. `--config` applies here only.
    let config = match core::load_app_config(exe_dir, cli_config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to load config: {e}");
            return Ok(ExitCode::from(3));
        }
    };
    let Some(slot) = config.slots.get(id) else {
        eprintln!("error: unknown slot id {id:?}");
        eprintln!("hint: run dialkey --list-slots to see registered ids");
        return Ok(ExitCode::from(2));
    };
    match core::launch::launch_slot(slot, exe_dir, &config.slots) {
        Ok(()) => {
            tracing::info!(slot_id = %id, "launched via CLI one-shot");
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            eprintln!("error: launch failed for slot {id:?}: {e}");
            tracing::error!(slot_id = %id, error = %e, "CLI one-shot launch failed");
            Ok(ExitCode::from(3))
        }
    }
}

#[cfg(windows)]
fn exit_from_launch_code(code: isize, id: &str) -> ExitCode {
    match code {
        platform::windows::LAUNCH_OK => ExitCode::SUCCESS,
        platform::windows::LAUNCH_UNKNOWN => {
            eprintln!("error: unknown slot id {id:?}");
            eprintln!("hint: run dialkey --list-slots to see registered ids");
            ExitCode::from(2)
        }
        platform::windows::LAUNCH_FAILED => {
            eprintln!("error: launch failed for slot {id:?}");
            ExitCode::from(3)
        }
        platform::windows::LAUNCH_BAD_REQUEST | _ => {
            eprintln!("error: resident DialKey rejected --launch {id:?} (code {code})");
            ExitCode::from(1)
        }
    }
}

fn parse_config_arg(args: &[String]) -> Option<PathBuf> {
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        if a == "--config" {
            return iter.next().map(|s| PathBuf::from(s));
        }
        if let Some(path) = a.strip_prefix("--config=") {
            return Some(PathBuf::from(path));
        }
    }
    None
}

fn peek_ui_locale(exe_dir: &std::path::Path, cli_config: Option<&std::path::Path>) -> String {
    let dir = match core::config::resolve_config_dir(exe_dir, cli_config) {
        Ok(d) => d,
        Err(_) => return String::new(),
    };
    let path = dir.join("settings.json");
    let Ok(text) = core::config::read_utf8_bom(&path) else {
        return String::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return String::new();
    };
    v.get("ui")
        .and_then(|u| u.get("locale"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

fn peek_log_level(exe_dir: &std::path::Path, cli_config: Option<&std::path::Path>) -> String {
    let dir = match core::config::resolve_config_dir(exe_dir, cli_config) {
        Ok(d) => d,
        Err(_) => return "warn".into(),
    };
    let path = dir.join("settings.json");
    let Ok(text) = core::config::read_utf8_bom(&path) else {
        return "warn".into();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return "warn".into();
    };
    v.get("logLevel")
        .and_then(|x| x.as_str())
        .unwrap_or("warn")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_launch_separate_and_equals() {
        let a = vec!["--launch".into(), "71".into()];
        assert_eq!(parse_launch_arg(&a).unwrap().as_deref(), Some("71"));
        let b = vec!["--launch=01".into()];
        assert_eq!(parse_launch_arg(&b).unwrap().as_deref(), Some("01"));
    }

    #[test]
    fn parse_launch_rejects_non_digits() {
        let a = vec!["--launch".into(), "ab".into()];
        assert!(parse_launch_arg(&a).is_err());
    }

    #[test]
    fn parse_launch_absent() {
        let a = vec!["--version".into()];
        assert_eq!(parse_launch_arg(&a).unwrap(), None);
    }
}
