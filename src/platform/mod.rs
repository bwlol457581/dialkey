//! Platform abstraction.
//!
//! Windows is the only implementation for now. macOS is a future target
//! (CGEventTap / NSStatusItem / LaunchAgent).

#![allow(dead_code)]

pub trait TriggerSource {
    /// Install the trigger. Suppression is per configuration.
    fn install(&mut self) -> anyhow::Result<()>;
    fn uninstall(&mut self);
}

pub trait InputCapture {
    /// Install while the window is shown; MUST be uninstalled on close,
    /// including on panic (manage with RAII).
    fn install(&mut self) -> anyhow::Result<()>;
    fn uninstall(&mut self);
}

pub trait TrayIcon {
    fn add(&mut self) -> anyhow::Result<()>;
    /// Re-register on the `TaskbarCreated` broadcast (Explorer restart).
    fn reregister(&mut self) -> anyhow::Result<()>;
}

pub trait Notifier {
    fn error(&self, message: &str);
}

pub trait AutoStart {
    fn enable(&self) -> anyhow::Result<()>;
    fn disable(&self) -> anyhow::Result<()>;
    fn heal(&self) -> anyhow::Result<()>;
}

#[cfg(windows)]
pub mod windows;
