//! UI language packs (`lang/<code>.toml`) — see `docs/spec.md` §13.

mod catalog;
mod english;
pub mod keys;
mod simple_toml;

pub use catalog::{
    active, extract_language_code, install, lang_dir_next_to_exe, list_available_packs, resolve, t,
    tf,
};

/// Key-set major version that language packs must match (`meta.target`).
pub const LANGUAGE_KEY_TARGET_MAJOR: &str = "1.0";
