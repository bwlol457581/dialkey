//! OS-independent core.
//!
//! Nothing in this module may reference Windows APIs.

pub mod chain;
pub mod config;
pub mod date_tokens;
pub mod drop_path;
pub mod focus_match;
pub mod i18n;
pub mod keys;
pub mod launch;
pub mod legend;
pub mod logging;
pub mod mode_map;
pub mod search;
pub mod sequence;
pub mod slots;
pub mod winpath;

pub use config::{load_app_config, ConfigError};
