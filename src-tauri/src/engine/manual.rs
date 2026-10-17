//! The manual "keep awake" session: Timer, Until, or Unlimited.
//!
//! Manual sessions are completely independent of agent watching — you can hold
//! the PC awake for two hours with every agent switched off, and you can watch
//! agents with no manual session at all.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum ManualMode {
    /// Countdown from a duration.
    #[default]
    Timer,
    /// Count up to a wall-clock time (Today/Tomorrow HH:MM).
    Until,
    /// Until you press End.
    Unlimited,
}

impl ManualMode {
    pub fn label(self) -> &'static str {
        match self {
            ManualMode::Timer => "Timer",
            ManualMode::Until => "Until",
            ManualMode::Unlimited => "Unlimited",
        }
    }
}

/// The duration presets offered on the Sessions page, in minutes: 15m .. 8h.
pub const PRESETS_MIN: [u64; 9] = [15, 30, 45, 60, 120, 180, 240, 360, 480];

pub const EXTEND_SHORT_SECS: u64 = 15 * 60;
pub const EXTEND_LONG_SECS: u64 = 60 * 60;

/// `15m`, `45m`, `1h`, `1h 30m`, `8h`.
pub fn preset_label(minutes: u64) -> String {
    let h = minutes / 60;
    let m = minutes % 60;
    if h == 0 {
        format!("{m}m")
    } else if m == 0 {
        format!("{h}h")
    } else {
        format!("{h}h {m}m")
    }
}

/// `2h 05m 09s`, `05:09`, `1d 03:00:00` — the big countdown.
pub fn clock(seconds: u64) -> String {
    let s = seconds % 60;
    let m = (seconds / 60) % 60;
    let h = (seconds / 3600) % 24;
    let d = seconds / 86_400;
    if d > 0 {
        format!("{d}d {h:02}:{m:02}:{s:02}")
    } else if seconds >= 3600 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Manual {
    pub active: bool,
    pub mode: ManualMode,
    pub started_unix: u64,
    /// `None` only for an Unlimited session.
    pub ends_unix: Option<u64>,
    /// Total planned seconds (surfaced to the UI as elapsed/total).
    pub total_secs: u64,
}

impl Default for Manual {
    fn default() -> Self {
        Manual {
            active: false,
            mode: ManualMode::Timer,
            started_unix: 0,
            ends_unix: None,
            total_secs: 0,
        }
    }
}

impl Manual {
    pub fn start_secs(&mut self, now_unix: u64, secs: u64) {
        *self = Manual {
            active: true,
            mode: ManualMode::Timer,
            started_unix: now_unix,
            ends_unix: Some(now_unix.saturating_add(secs)),
            total_secs: secs,
        };
    }

    pub fn start_until(&mut self, now_unix: u64, secs: u64) {
        *self = Manual {
            active: true,
            mode: ManualMode::Until,
            started_unix: now_unix,
            ends_unix: Some(now_unix.saturating_add(secs)),
            total_secs: secs,
        };
    }

    pub fn start_unlimited(&mut self, now_unix: u64) {
        *self = Manual {
            active: true,
            mode: ManualMode::Unlimited,
            started_unix: now_unix,
            ends_unix: None,
            total_secs: 0,
        };
    }

    /// `+15m` / `+1h`. Unlimited stays unlimited.
    pub fn extend(&mut self, secs: u64) {
        if !self.active {
            return;
        }
        if let Some(ends) = self.ends_unix {
            self.ends_unix = Some(ends.saturating_add(secs));
            self.total_secs = self.total_secs.saturating_add(secs);
        }
    }

    pub fn stop(&mut self) {
        let mode = self.mode;
        *self = Manual {
            mode,
            ..Manual::default()
        };
    }

    pub fn remaining(&self, now_unix: u64) -> Option<u64> {
        self.ends_unix.map(|e| e.saturating_sub(now_unix))
    }

    pub fn expired(&self, now_unix: u64) -> bool {
        self.active && self.ends_unix.map(|e| now_unix >= e).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_span_fifteen_minutes_to_eight_hours() {
        assert_eq!(PRESETS_MIN[0], 15);
        assert_eq!(PRESETS_MIN[PRESETS_MIN.len() - 1], 480);
        assert!(PRESETS_MIN.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(preset_label(15), "15m");
        assert_eq!(preset_label(60), "1h");
        assert_eq!(preset_label(90), "1h 30m");
        assert_eq!(preset_label(480), "8h");
    }

    #[test]
    fn timer_counts_down_and_expires() {
        let mut m = Manual::default();
        m.start_secs(1_000, 900);
        assert_eq!(m.remaining(1_000), Some(900));
        assert_eq!(m.remaining(1_300), Some(600));
        assert!(!m.expired(1_899));
        assert!(m.expired(1_900));
        assert_eq!(m.mode, ManualMode::Timer);
    }

    #[test]
    fn until_behaves_like_a_timer_with_a_wall_clock_end() {
        let mut m = Manual::default();
        m.start_until(5_000, 5_400);
        assert_eq!(m.mode, ManualMode::Until);
        assert_eq!(m.ends_unix, Some(10_400));
        assert_eq!(m.remaining(10_000), Some(400));
    }

    #[test]
    fn unlimited_never_expires() {
        let mut m = Manual::default();
        m.start_unlimited(42);
        assert_eq!(m.remaining(u64::MAX / 2), None);
        assert!(!m.expired(u64::MAX / 2));
        m.extend(EXTEND_LONG_SECS);
        assert_eq!(m.ends_unix, None, "extending unlimited keeps it unlimited");
    }

    #[test]
    fn extend_pushes_the_end_and_the_total() {
        let mut m = Manual::default();
        m.start_secs(0, 3_600);
        m.extend(EXTEND_SHORT_SECS);
        assert_eq!(m.ends_unix, Some(3_600 + 900));
        assert_eq!(m.total_secs, 4_500);
    }

    #[test]
    fn stop_clears_state_but_remembers_the_mode() {
        let mut m = Manual::default();
        m.start_unlimited(7);
        m.stop();
        assert!(!m.active);
        assert_eq!(m.ends_unix, None);
        assert_eq!(m.total_secs, 0);
        assert_eq!(m.mode, ManualMode::Unlimited);
        assert!(!m.expired(9_999_999));
    }

    #[test]
    fn extend_on_an_inactive_session_is_a_no_op() {
        let mut m = Manual::default();
        m.extend(EXTEND_LONG_SECS);
        assert!(!m.active);
        assert_eq!(m.ends_unix, None);
    }

    #[test]
    fn clock_formats_read_naturally() {
        assert_eq!(clock(9), "00:09");
        assert_eq!(clock(540), "09:00");
        assert_eq!(clock(3_599), "59:59");
        assert_eq!(clock(3_600), "1:00:00");
        assert_eq!(clock(8 * 3600), "8:00:00");
        assert_eq!(clock(86_400 + 3600 * 3), "1d 03:00:00");
    }
}
