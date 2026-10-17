//! Settings model, defaults and JSON persistence.
//!
//! Settings are stored as a single JSON document in the app config dir so a
//! user can inspect, back up or hand-edit them. Every load is clamped so a
//! corrupt or hand-edited file can never produce out-of-range behaviour.
//!
//! After the simplification Ember only keeps a handful of user options; every
//! other policy is hardcoded. The constants below are the single source of
//! truth for those fixed values.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::paths;

// ---------------------------------------------------------------------------
// Always-on policy (hardcoded, no user toggles)
// ---------------------------------------------------------------------------

/// Anti-flap: stay awake this long after the last agent activity.
pub const GRACE_SECS: u64 = 15;
/// How long a session may go silent before it is demoted to idle.
pub const STALE_TIMEOUT_MIN: u64 = 20;
/// Battery floor: releases the hold below this percentage.
pub const STOP_BELOW_PERCENT: u8 = 15;
/// Chime volume, 0.0 ..= 1.0.
pub const NOTIFY_VOLUME: f32 = 0.7;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// The ten chimes. All are synthesised in Rust; nothing is shipped as audio.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Chime {
    Glass,
    Droplet,
    Bell,
    Marimba,
    Pulse,
    Sparkle,
    LowTone,
    DoubleTap,
    Rise,
    Fall,
}

impl Chime {
    pub const ALL: [Chime; 10] = [
        Chime::Glass,
        Chime::Droplet,
        Chime::Bell,
        Chime::Marimba,
        Chime::Pulse,
        Chime::Sparkle,
        Chime::LowTone,
        Chime::DoubleTap,
        Chime::Rise,
        Chime::Fall,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Chime::Glass => "glass",
            Chime::Droplet => "droplet",
            Chime::Bell => "bell",
            Chime::Marimba => "marimba",
            Chime::Pulse => "pulse",
            Chime::Sparkle => "sparkle",
            Chime::LowTone => "low_tone",
            Chime::DoubleTap => "double_tap",
            Chime::Rise => "rise",
            Chime::Fall => "fall",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Chime::Glass => "Glass",
            Chime::Droplet => "Droplet",
            Chime::Bell => "Bell",
            Chime::Marimba => "Marimba",
            Chime::Pulse => "Pulse",
            Chime::Sparkle => "Sparkle",
            Chime::LowTone => "Low Tone",
            Chime::DoubleTap => "Double Tap",
            Chime::Rise => "Rise",
            Chime::Fall => "Fall",
        }
    }
}

fn default_true() -> bool {
    true
}

/// The battery floor's default source of truth. The field below is a user
/// option, but its default is fixed policy.
fn default_battery_floor_percent() -> u8 {
    STOP_BELOW_PERCENT
}

/// Notifications are all-or-nothing now: a single mute switch gates both the
/// toast and the chime.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Notifications {
    pub mute: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WindowState {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
    /// False until the user has moved or resized the window once.
    pub placed: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        WindowState {
            x: None,
            y: None,
            width: 920,
            height: 640,
            maximized: false,
            placed: false,
        }
    }
}

/// The surviving user options. Everything else Ember does is fixed policy.
///
/// `#[serde(default)]` at the container level means an old settings.json (with
/// the removed options) still loads: unknown keys are ignored and missing keys
/// fall back to the default.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    // Appearance
    pub theme: Theme,
    // General
    #[serde(default = "default_true")]
    pub launch_at_login: bool,
    // Watching
    /// The Overview master toggle.
    #[serde(default = "default_true")]
    pub watching: bool,
    // Lid
    pub lid_proof: bool,
    /// Require AC power before lid-proof changes the lid-close action. Unplugging
    /// restores the user's lid action while the normal keep-awake hold continues.
    pub require_charger_for_lid: bool,
    // Power
    /// Release the hold below this battery percentage (5..=50). Defaults to
    /// [`STOP_BELOW_PERCENT`].
    #[serde(default = "default_battery_floor_percent")]
    pub battery_floor_percent: u8,
    /// Also hold a `DisplayRequired` power request, keeping the screen on.
    pub keep_display_on: bool,
    // Notifications
    pub notifications: Notifications,
    // Window geometry, remembered between runs (physical pixels).
    pub window: WindowState,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: Theme::System,
            launch_at_login: true,
            watching: true,
            lid_proof: false,
            require_charger_for_lid: false,
            battery_floor_percent: STOP_BELOW_PERCENT,
            keep_display_on: false,
            notifications: Notifications::default(),
            window: WindowState::default(),
        }
    }
}

impl Settings {
    pub fn clamp(&mut self) {
        self.window.width = self.window.width.clamp(760, 10000);
        self.window.height = self.window.height.clamp(520, 10000);
        // The floor is a user knob, so a hand-edited file cannot push it to a
        // value that would either never trip or trip immediately.
        self.battery_floor_percent = self.battery_floor_percent.clamp(5, 50);
    }

    pub fn load() -> Settings {
        let path = paths::settings_path();
        let mut settings = match std::fs::read_to_string(&path) {
            Ok(text) => parse_document(&text),
            Err(_) => Settings::default(),
        };
        settings.clamp();
        settings
    }

    pub fn save(&self) -> Result<(), String> {
        let path = paths::settings_path();
        paths::ensure_parent(&path);
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        write_atomic(&path, text.as_bytes())
    }
}

/// Parse a settings document, tolerating a leading byte-order mark: many
/// Windows editors add one, `serde_json` rejects it, and the fallback used to
/// silently reset every preference in the file to its default.
fn parse_document(text: &str) -> Settings {
    serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_default()
}

/// Write via a temp file + rename so a crash can never truncate settings.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp-ember");
    paths::ensure_parent(path);
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let s = Settings::default();
        assert_eq!(s.theme, Theme::System);
        assert!(s.launch_at_login);
        assert!(s.watching);
        assert!(!s.lid_proof);
        assert!(!s.require_charger_for_lid);
        assert_eq!(s.battery_floor_percent, 15);
        assert!(!s.keep_display_on);
        assert!(!s.notifications.mute);
    }

    #[test]
    fn policy_constants_match_the_spec() {
        assert_eq!(GRACE_SECS, 15);
        assert_eq!(STALE_TIMEOUT_MIN, 20);
        assert_eq!(STOP_BELOW_PERCENT, 15);
        assert!((NOTIFY_VOLUME - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn clamp_bounds() {
        let mut s = Settings::default();
        s.window.width = 10;
        s.window.height = 10;
        s.clamp();
        assert_eq!(s.window.width, 760);
        assert_eq!(s.window.height, 520);

        s.window.width = 99_999;
        s.window.height = 99_999;
        s.clamp();
        assert_eq!(s.window.width, 10_000);
        assert_eq!(s.window.height, 10_000);

        // The battery floor is clamped to the documented 5..=50 range.
        s.battery_floor_percent = 1;
        s.clamp();
        assert_eq!(s.battery_floor_percent, 5);
        s.battery_floor_percent = 200;
        s.clamp();
        assert_eq!(s.battery_floor_percent, 50);
    }

    #[test]
    fn round_trip_json() {
        let s = Settings::default();
        let text = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.theme, s.theme);
        assert!(!back.notifications.mute);
        assert!(back.watching);
    }

    #[test]
    fn a_byte_order_mark_does_not_reset_settings() {
        // Editors love adding a BOM; serde_json rejects it and the loader's
        // fallback used to silently reset the whole document to defaults.
        let marked = "\u{feff}{\"lid_proof\":true}";
        assert!(parse_document(marked).lid_proof);
        // Garbage still falls back to defaults instead of panicking.
        assert!(!parse_document("not json").lid_proof);
    }

    #[test]
    fn an_old_settings_file_still_loads() {
        // A settings.json from before the simplification: unknown keys are
        // ignored and the missing new keys fall back to their defaults.
        let old = r#"{
            "theme": "dark",
            "material": "mica",
            "launch_at_login": false,
            "always_on_top": true,
            "show_title_status": false,
            "watching": false,
            "agent_watch": { "codex": false },
            "keep_awake_after_sec": 15,
            "stale_timeout_min": 20,
            "lid_proof": true,
            "lid_cap_hours": 6,
            "only_when_plugged": true,
            "stop_below_enabled": true,
            "stop_below_percent": 15,
            "respect_battery_saver": true,
            "display_policy": "let_sleep",
            "turn_off_on_lid_close": false,
            "turn_off_after_enabled": false,
            "turn_off_after_sec": 300,
            "keep_on_for_manual": true,
            "notifications": { "master_mute": true, "volume": 0.5 },
            "window": { "x": null, "y": null, "width": 920, "height": 640, "maximized": false, "placed": false }
        }"#;
        let s: Settings = serde_json::from_str(old).unwrap();
        assert_eq!(s.theme, Theme::Dark);
        assert!(!s.launch_at_login);
        assert!(!s.watching);
        assert!(s.lid_proof);
        // The old `stop_below_percent` key is ignored; the new `battery_floor_percent`
        // falls back to its default. Likewise the new power/lid toggles default off.
        assert_eq!(s.battery_floor_percent, 15);
        assert!(!s.require_charger_for_lid);
        assert!(!s.keep_display_on);
        // The old notifications blob has no `mute`, so it defaults to false.
        assert!(!s.notifications.mute);
    }

    #[test]
    fn chime_ids_are_unique() {
        let mut ids: Vec<&str> = Chime::ALL.iter().map(|c| c.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 10);
    }
}
