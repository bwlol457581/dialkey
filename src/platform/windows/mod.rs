//! Windows platform: hooks, tray, hotkey, autostart, single instance.

mod app;
mod autostart;
mod focus_existing;
mod hotkey;
mod icon;
mod launch_ipc;
mod launch_queue;
mod locale;
mod messages;
mod mode_runtime;
mod mouse_hook;
mod placement;
mod search;
mod settings;
mod shell;
mod single_instance;
mod tray;
mod wide;

pub use app::run;
pub use autostart::uninstall_run_key;
pub use launch_ipc::{
    try_signal_launch, try_signal_reload, SignalLaunchError, LAUNCH_BAD_REQUEST, LAUNCH_FAILED,
    LAUNCH_OK, LAUNCH_UNKNOWN,
};
pub use locale::os_ui_language;
pub use single_instance::{AlreadyRunning, SingleInstance};
