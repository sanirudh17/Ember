//! The serialisable snapshot pushed to the UI (and to the window chrome).
//!
//! One snapshot describes absolutely everything a view needs, so the frontend
//! never has to poll piecemeal. It is emitted at most once a second.

use serde::Serialize;

use crate::agents::registry::SessionState;
use crate::agents::{AgentId, IntegrationMode};
use crate::paths::APP_NAME;
use crate::settings::Settings;

use super::decision::Guard;
use super::manual::{ManualMode, clock};

/// The taskbar/window title. `Ember·Awake·2 agents working`, `Ember·Idle`,
/// `Ember·Paused`.
pub fn taskbar_title(engaged: bool, paused: bool, agents_working: usize) -> String {
    if paused {
        return format!("{APP_NAME}·Paused");
    }
    if !engaged {
        return format!("{APP_NAME}·Idle");
    }
    match agents_working {
        0 => format!("{APP_NAME}·Awake"),
        1 => format!("{APP_NAME}·Awake·1 agent working"),
        n => format!("{APP_NAME}·Awake·{n} agents working"),
    }
}

/// The short human reason Ember is awake (or isn't).
pub fn reason_line(
    engaged: bool,
    paused: bool,
    manual_active: bool,
    agents_working: usize,
    watching: bool,
) -> String {
    if paused {
        return "Paused".to_string();
    }
    if !engaged {
        if watching {
            return "Active · watching for unsolicited agent runs".to_string();
        }
        return "Not holding the PC awake".to_string();
    }
    match (manual_active, agents_working) {
        (true, 0) => "Manual session".to_string(),
        (false, 0) => "Holding the PC awake".to_string(),
        (false, 1) => "1 agent working".to_string(),
        (false, n) => format!("{n} agents working"),
        (true, 1) => "Manual session + 1 agent working".to_string(),
        (true, n) => format!("Manual session + {n} agents working"),
    }
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct ManualSnapshot {
    pub active: bool,
    pub mode: ManualMode,
    pub ends_unix: Option<u64>,
    pub remaining_secs: u64,
    pub total_secs: u64,
    pub elapsed_secs: u64,
    pub countdown: String,
    pub unlimited: bool,
    /// `Timer` / `Until` / `Unlimited`
    pub mode_label: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct BatterySnapshot {
    pub present: bool,
    pub percent: Option<u8>,
    pub plugged_in: bool,
    pub saver_on: bool,
    /// The guardrail that is currently applying, if any.
    pub rule: Option<String>,
    pub percent_label: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct LidSnapshot {
    pub present: bool,
    pub proof_enabled: bool,
    pub held: bool,
    pub modern_standby: bool,
    pub cap_hours: u32,
    pub held_secs: u64,
    pub cap_remaining_secs: u64,
    /// `Some(true)` closed, `Some(false)` open, `None` unknown/no lid.
    pub closed: Option<bool>,
    pub note: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct DisplaySnapshot {
    pub policy_label: String,
    pub display_kept_on: bool,
    pub turned_off: bool,
    pub note: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct SessionSnapshot {
    pub agent: String,
    pub agent_name: String,
    pub monogram: String,
    pub hue: u16,
    pub session_id: String,
    pub folder: String,
    pub state: SessionState,
    pub working_for_secs: u64,
    pub last_event_unix: u64,
    pub approximate: bool,
    pub stale: bool,
    pub pid: Option<u32>,
}

#[derive(Serialize, Clone, Debug)]
pub struct AgentSnapshot {
    pub id: String,
    pub name: String,
    pub monogram: String,
    pub hue: u16,
    pub detected: bool,
    pub detect_detail: String,
    pub installed: bool,
    pub needs_repair: bool,
    pub supported: bool,
    pub install_file: Option<String>,
    pub install_detail: String,
    pub mode: IntegrationMode,
    pub note: String,
    pub watching: bool,
    pub working: usize,
    pub sessions: usize,
    pub last_event_unix: Option<u64>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Capabilities {
    pub lid_present: bool,
    pub modern_standby: bool,
    pub mica_available: bool,
    pub os_build: u32,
    pub os_name: String,
    /// Free-text summary of what Ember can actually do on this machine.
    pub blocking: String,
    pub power_requests: bool,
    pub lid_proof_available: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Snapshot {
    pub unix_ms: u64,
    pub engaged: bool,
    pub engaged_for_secs: u64,
    pub want_awake: bool,
    pub paused: bool,
    pub paused_remaining_secs: Option<u64>,
    pub paused_countdown: Option<String>,
    pub watching: bool,
    pub agents_working: usize,
    pub watched_agents: usize,
    pub blocker: Option<Guard>,
    pub blocker_label: Option<String>,
    pub blocker_detail: Option<String>,
    pub reason: String,
    pub status_word: String,
    pub title: String,
    pub manual: ManualSnapshot,
    pub battery: BatterySnapshot,
    pub lid: LidSnapshot,
    pub display: DisplaySnapshot,
    pub capabilities: Capabilities,
    pub sessions: Vec<SessionSnapshot>,
    pub agents: Vec<AgentSnapshot>,
    pub settings: Settings,
}

impl Snapshot {
    /// The exact window title. The status suffix is always shown now; there is
    /// no "show status in the title bar" option any more.
    pub fn window_title(&self) -> String {
        self.title.clone()
    }

    /// Stable comparison used for change detection — volatile timestamps must
    /// not make every tick look like a change, but the one-second UI clocks
    /// (elapsed hold, per-session seconds) are included: a steady hold must
    /// still tick the display. `EMIT_INTERVAL` keeps this at ≤1/s.
    pub fn signature(&self) -> String {
        format!(
            "{}|{}|{}|{}|{:?}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{:?}|{}|{}|{}|{}|{}",
            self.engaged,
            self.engaged_for_secs,
            self.want_awake,
            self.paused,
            self.paused_remaining_secs,
            self.watching,
            self.agents_working,
            self.watched_agents,
            self.blocker.map(|b| b.label()).unwrap_or(""),
            self.reason,
            self.title,
            self.manual.active,
            self.manual.mode_label,
            self.manual.remaining_secs,
            self.battery.percent,
            self.battery.plugged_in,
            self.battery.saver_on,
            self.lid.held,
            self.lid.held_secs,
            self.session_signature()
        )
    }

    fn session_signature(&self) -> String {
        let mut parts: Vec<String> = self
            .sessions
            .iter()
            .map(|s| {
                // A working session's clock ticks and must reach the UI; idle
                // rows must not tick, or a quiet session would emit forever.
                let elapsed = if s.state == SessionState::Working {
                    format!(":{}", s.working_for_secs)
                } else {
                    String::new()
                };
                format!("{}:{}:{:?}{}", s.agent, s.folder, s.state, elapsed)
            })
            .collect();
        parts.sort_unstable();
        parts.join(",")
    }

    pub fn agent_for(&self, id: &str) -> Option<&AgentSnapshot> {
        self.agents.iter().find(|a| a.id == id)
    }

    pub fn set_manual_from(&mut self, manual: &super::manual::Manual, now_unix: u64) {
        let remaining = manual.remaining(now_unix).unwrap_or(0);
        self.manual = ManualSnapshot {
            active: manual.active,
            mode: manual.mode,
            ends_unix: manual.ends_unix,
            remaining_secs: remaining,
            total_secs: manual.total_secs,
            elapsed_secs: now_unix.saturating_sub(manual.started_unix),
            countdown: if manual.active {
                clock(remaining)
            } else {
                "00:00".to_string()
            },
            unlimited: manual.mode == ManualMode::Unlimited && manual.active,
            mode_label: manual.mode.label().to_string(),
        };
    }

    pub fn agent_names() -> Vec<(AgentId, &'static str)> {
        AgentId::ALL.iter().map(|a| (*a, a.def().name)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taskbar_titles_match_the_spec() {
        assert_eq!(taskbar_title(false, false, 0), "Ember·Idle");
        assert_eq!(taskbar_title(true, true, 3), "Ember·Paused");
        assert_eq!(taskbar_title(true, false, 0), "Ember·Awake");
        assert_eq!(taskbar_title(true, false, 1), "Ember·Awake·1 agent working");
        assert_eq!(
            taskbar_title(true, false, 2),
            "Ember·Awake·2 agents working"
        );
        assert_eq!(
            taskbar_title(true, false, 9),
            "Ember·Awake·9 agents working"
        );
    }

    #[test]
    fn app_name_flows_into_the_title() {
        assert!(taskbar_title(true, false, 2).starts_with(APP_NAME));
    }

    #[test]
    fn reasons_read_naturally() {
        assert_eq!(
            reason_line(false, false, false, 0, false),
            "Not holding the PC awake"
        );
        assert_eq!(
            reason_line(false, false, false, 0, true),
            "Active · watching for unsolicited agent runs"
        );
        assert_eq!(reason_line(true, true, false, 3, false), "Paused");
        assert_eq!(reason_line(true, false, true, 0, false), "Manual session");
        assert_eq!(
            reason_line(true, false, false, 2, false),
            "2 agents working"
        );
        assert_eq!(
            reason_line(true, false, true, 1, false),
            "Manual session + 1 agent working"
        );
    }

    #[test]
    fn window_title_always_shows_the_status_suffix() {
        let snap = Snapshot {
            title: "Ember·Awake".into(),
            ..Snapshot::default()
        };
        assert_eq!(snap.window_title(), "Ember·Awake");
        let idle = Snapshot {
            title: "Ember·Idle".into(),
            ..Snapshot::default()
        };
        assert_eq!(idle.window_title(), "Ember·Idle");
    }

    #[test]
    fn signature_ignores_countdown_ticks_but_sees_real_changes() {
        let mut a = Snapshot::default();
        let mut b = Snapshot::default();
        a.manual.countdown = "05:00".into();
        b.manual.countdown = "04:59".into();
        assert_eq!(a.signature(), b.signature());

        b.engaged = true;
        assert_ne!(a.signature(), b.signature());
    }
}
