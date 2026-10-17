//! Canonical locations and identifiers for Ember.
//!
//! `APP_NAME` is the single source of truth for the window title, installer
//! name, autostart entry, named pipe and data folder.

use std::fs;
use std::path::{Path, PathBuf};

/// The one place the app name lives.
pub const APP_NAME: &str = "Ember";

/// Hook transport used by `ember.exe hook ...` to reach the running app.
pub const PIPE_NAME: &str = r"\\.\pipe\ember-hooks";

/// `%APPDATA%\Ember` — user-visible settings live here.
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME)
}

/// `%LOCALAPPDATA%\Ember` — machine-local runtime state lives here.
pub fn data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME)
}

pub fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn token_path() -> PathBuf {
    config_dir().join("hook-token")
}

/// Written *before* the lid action is changed so a crash can be recovered.
pub fn session_state_path() -> PathBuf {
    data_dir().join("session-state.json")
}

pub fn ensure_dir(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)
}

/// Best-effort directory creation that swallows errors (callers degrade).
pub fn ensure_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
}
