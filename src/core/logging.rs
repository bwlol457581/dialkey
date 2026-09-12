//! File logging with size rotation (spec: 1MB × 3 generations).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

const MAX_BYTES: u64 = 1024 * 1024;
const GENERATIONS: u32 = 3;

/// Initialize tracing: stderr (debug builds) + rotating file under `logs/`.
///
/// Fallback: `%LOCALAPPDATA%\DialKey\logs\` then give up (app still runs).
pub fn init(exe_dir: &Path, log_level: &str) -> Option<PathBuf> {
    let level = parse_level(log_level);
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level_to_str(level)));

    let log_dir = choose_log_dir(exe_dir);
    let file_layer = log_dir.as_ref().and_then(|dir| {
        let path = dir.join("dialkey.log");
        match RotatingFile::open(&path, MAX_BYTES, GENERATIONS) {
            Ok(writer) => {
                let layer = fmt::layer()
                    .with_ansi(false)
                    .with_target(false)
                    .with_writer(Mutex::new(writer));
                Some(layer)
            }
            Err(_) => None,
        }
    });

    let stderr_layer = fmt::layer().with_ansi(true).with_target(false);

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer);
    if let Some(file_layer) = file_layer {
        let _ = subscriber.with(file_layer).try_init();
    } else {
        let _ = subscriber.try_init();
    }

    log_dir
}

fn parse_level(s: &str) -> LevelFilter {
    match s.to_ascii_lowercase().as_str() {
        "error" => LevelFilter::ERROR,
        "warn" => LevelFilter::WARN,
        "info" => LevelFilter::INFO,
        "debug" | "trace" => LevelFilter::DEBUG,
        _ => LevelFilter::WARN,
    }
}

fn level_to_str(level: LevelFilter) -> &'static str {
    match level {
        LevelFilter::ERROR => "error",
        LevelFilter::WARN => "warn",
        LevelFilter::INFO => "info",
        _ => "debug",
    }
}

fn choose_log_dir(exe_dir: &Path) -> Option<PathBuf> {
    let primary = exe_dir.join("logs");
    if ensure_writable_dir(&primary) {
        return Some(primary);
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let fallback = PathBuf::from(local).join("DialKey").join("logs");
        if ensure_writable_dir(&fallback) {
            return Some(fallback);
        }
    }
    None
}

fn ensure_writable_dir(dir: &Path) -> bool {
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".write_probe");
    match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&probe)
    {
        Ok(mut f) => {
            let ok = f.write_all(b"ok").is_ok();
            drop(f);
            let _ = fs::remove_file(&probe);
            ok
        }
        Err(_) => false,
    }
}

struct RotatingFile {
    path: PathBuf,
    file: File,
    max_bytes: u64,
    generations: u32,
    written: u64,
}

impl RotatingFile {
    fn open(path: &Path, max_bytes: u64, generations: u32) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let written = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            path: path.to_path_buf(),
            file,
            max_bytes,
            generations,
            written,
        })
    }

    fn rotate_if_needed(&mut self) -> io::Result<()> {
        if self.written < self.max_bytes {
            return Ok(());
        }
        self.file.flush()?;
        drop(std::mem::replace(
            &mut self.file,
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?,
        ));

        // Shift: .(n-1) -> .n, current -> .1
        let base = &self.path;
        if self.generations > 0 {
            let oldest = with_gen(base, self.generations);
            let _ = fs::remove_file(&oldest);
            for gen in (1..self.generations).rev() {
                let from = with_gen(base, gen);
                let to = with_gen(base, gen + 1);
                let _ = fs::rename(&from, &to);
            }
            let _ = fs::rename(base, with_gen(base, 1));
        }

        self.file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(base)?;
        self.written = 0;
        Ok(())
    }
}

fn with_gen(path: &Path, gen: u32) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{gen}"));
    path.with_file_name(name)
}

impl Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.rotate_if_needed()?;
        let n = self.file.write(buf)?;
        self.written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
