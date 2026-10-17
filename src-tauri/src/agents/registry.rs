//! Live session registry.
//!
//! Sessions are keyed by `(agent, session_id)` so ten terminals of Claude Code
//! and three Codex threads all show up separately. Hook-driven sessions are
//! authoritative; process-detected sessions are flagged approximate.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::{AgentId, folder_name, process::ProcessDetection};

/// Hard cap per agent, oldest idle sessions are evicted first.
pub const MAX_SESSIONS_PER_AGENT: usize = 50;

/// A session whose pid is missing from the last process-table sample is only
/// treated as dead once it has also been quiet for this long. The sample is
/// refreshed every couple of seconds, so a hook that lands moments after its
/// agent process starts can carry a pid the current sample has never seen;
/// the next sample includes it and the session stops being a suspect.
pub const PID_GRACE: Duration = Duration::from_secs(6);

/// Hook-driven sessions with a live owning process stay in the decision's
/// grace window this long after their last event. Agents routinely go quiet
/// for 26–34 s mid-turn (model thinking, network waits) and several of them
/// fire settle-style events between steps of one run; on a 15 s window that
/// flaps the hold mid-turn. 90 s is ~2.5× the longest observed in-turn gap,
/// so a live session never releases while its turn is plausibly running.
/// Release stays bounded: once the agent truly ends, the hold drops 90 s
/// after the final event (plus the engine's release floor).
pub const LIVE_PID_IDLE_TRUST: Duration = Duration::from_secs(90);

/// Approximate (process-detected) sessions keep the hold alive this long
/// after the last sample where their tree was actually classified Working.
/// The CPU classifier's own `IDLE_HOLD` is tuned to release promptly; the
/// decision layer adds its own, longer window so that a hookless agent's
/// think-gap cannot flap the hold. Bounded the same way: quiet for this long
/// and the row stops counting even while the process is still open.
pub const APPROX_DWELL: Duration = Duration::from_secs(60);

/// How long a removed session's last event keeps counting toward the flat
/// grace window. Removal (an `End` hook, or the dead-pid sweep) must not
/// cancel the grace the session had already earned — otherwise a finished
/// one-shot task releases the hold the instant its row disappears.
const ENDED_KEEP: Duration = Duration::from_secs(60);

/// Hook messages whose timestamp is older than this are treated as replays and
/// dropped. The comparison is relative to "now", so a system clock correction
/// after resume-from-sleep can never mute every future hook.
pub const STALE_EVENT_WINDOW: u64 = 60 * 60;

#[derive(serde::Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Working,
    Idle,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HookEvent {
    Start,
    Working,
    Idle,
    End,
    Heartbeat,
}

impl HookEvent {
    pub fn parse(raw: &str) -> Option<HookEvent> {
        match raw {
            "start" => Some(HookEvent::Start),
            "working" => Some(HookEvent::Working),
            "idle" => Some(HookEvent::Idle),
            "end" => Some(HookEvent::End),
            "heartbeat" => Some(HookEvent::Heartbeat),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            HookEvent::Start => "start",
            HookEvent::Working => "working",
            HookEvent::Idle => "idle",
            HookEvent::End => "end",
            HookEvent::Heartbeat => "heartbeat",
        }
    }
}

#[derive(Clone, Debug)]
pub struct HookMessage {
    pub agent: AgentId,
    pub event: HookEvent,
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub pid: Option<u32>,
    pub ts: u64,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub agent: AgentId,
    pub session_id: String,
    pub folder: String,
    pub pid: Option<u32>,
    pub state: SessionState,
    pub state_since: Instant,
    pub last_event: Instant,
    pub last_event_unix: u64,
    /// Last time this session was classified Working (hook event or process
    /// sample). Feeds [`APPROX_DWELL`] for process-detected rows.
    pub last_working: Option<Instant>,
    /// Working but silent past the stale timeout.
    pub stale: bool,
    /// Detected from the process tree rather than a hook.
    pub approximate: bool,
}

impl Session {
    pub fn key(&self) -> String {
        format!("{}::{}", self.agent.id(), self.session_id)
    }
}

/// What a hook message did to the registry. Returned so callers can log
/// state transitions — accepted hooks are otherwise invisible in the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEffect {
    /// Nothing observable changed (state refresh, duplicate event).
    None,
    /// A new session row appeared.
    Created(SessionState),
    /// An existing session flipped state.
    Changed { from: SessionState, to: SessionState },
    /// An `End` hook removed the session.
    Removed,
}

pub struct Registry {
    sessions: HashMap<(AgentId, String), Session>,
    /// Recently removed hook sessions and the last event they had. Keeps the
    /// grace window alive across a removal (see [`ENDED_KEEP`]).
    ended: HashMap<(AgentId, String), Instant>,
    anon: u64,
}

impl Default for Registry {
    fn default() -> Self {
        Registry::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Registry {
            sessions: HashMap::new(),
            ended: HashMap::new(),
            anon: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn get(&self, agent: AgentId, session_id: &str) -> Option<&Session> {
        self.sessions.get(&(agent, session_id.to_string()))
    }

    pub fn sessions_for(&self, agent: AgentId) -> Vec<&Session> {
        let mut list: Vec<&Session> = self
            .sessions
            .values()
            .filter(|s| s.agent == agent)
            .collect();
        list.sort_by_key(|s| std::cmp::Reverse(s.last_event));
        list
    }

    pub fn all_sessions(&self) -> Vec<&Session> {
        let mut list: Vec<&Session> = self.sessions.values().collect();
        list.sort_by_key(|s| std::cmp::Reverse(s.last_event));
        list
    }

    pub fn working_count(&self, agent: AgentId) -> usize {
        self.sessions
            .values()
            .filter(|s| s.agent == agent && s.state == SessionState::Working && !s.stale)
            .count()
    }

    pub fn any_working(&self, watch: impl Fn(AgentId) -> bool) -> bool {
        self.sessions
            .values()
            .any(|s| watch(s.agent) && s.state == SessionState::Working && !s.stale)
    }

    /// Sessions currently keeping the hold in grace, in the same shape the
    /// decision uses (see [`any_working_or_in_grace`]). Used for decision
    /// logging so the log states exactly why the hold is still up.
    pub fn in_grace(
        &self,
        watch: impl Fn(AgentId) -> bool,
        flat: Duration,
        alive: &HashSet<u32>,
        now: Instant,
    ) -> Vec<&Session> {
        if flat.is_zero() {
            return Vec::new();
        }
        self.sessions
            .values()
            .filter(|s| {
                watch(s.agent)
                    && if s.approximate {
                        s.last_working
                            .is_some_and(|t| now.duration_since(t) < APPROX_DWELL)
                    } else {
                        let grace = match s.pid {
                            Some(pid) if alive.contains(&pid) => {
                                flat.max(LIVE_PID_IDLE_TRUST)
                            }
                            _ => flat,
                        };
                        now.duration_since(s.last_event) < grace
                    }
            })
            .collect()
    }

    /// Removed sessions whose flat grace window is still open (tombstones).
    pub fn ended_in_grace(
        &self,
        watch: impl Fn(AgentId) -> bool,
        flat: Duration,
        now: Instant,
    ) -> bool {
        !flat.is_zero()
            && self
                .ended
                .iter()
                .any(|((agent, _), t)| watch(*agent) && now.duration_since(*t) < flat)
    }

    /// Apply a hook message. `End` removes the session.
    pub fn on_hook(&mut self, msg: HookMessage, now: Instant) -> HookEffect {
        let session_id = match msg.session_id.clone() {
            Some(id) if !id.is_empty() => id,
            _ => match msg.pid {
                Some(pid) => format!("pid:{pid}"),
                None => {
                    // Anonymous invocations (no id, no pid) still count while
                    // they keep arriving, so give them a stable slot.
                    format!("anon:{}", msg.agent.id())
                }
            },
        };

        let key = (msg.agent, session_id.clone());

        if msg.event == HookEvent::End {
            if let Some(removed) = self.sessions.remove(&key) {
                // Tombstone: removal must not cancel the grace the session
                // had already earned (a dead-pid sweep or a wrapper's `End`
                // used to release the hold on the very same tick).
                if !removed.approximate {
                    self.ended.insert(key, removed.last_event);
                }
                return HookEffect::Removed;
            }
            return HookEffect::None;
        }

        let now_unix = unix_now();
        // Drop replayed events that are clearly old (for example a plugin
        // flushing yesterday's queue). The comparison is relative to "now"
        // rather than app start: a clock correction after resume-from-sleep
        // used to mute every future hook for the rest of the run, because its
        // timestamps stayed behind the recorded app-start time.
        if msg.ts > 0 && msg.ts < now_unix.saturating_sub(STALE_EVENT_WINDOW) {
            return HookEffect::None;
        }
        let old_state = self.sessions.get(&key).map(|s| s.state);
        let entry = self.sessions.entry(key).or_insert_with(|| {
            self.anon += 1;
            Session {
                agent: msg.agent,
                session_id: session_id.clone(),
                folder: msg
                    .cwd
                    .as_deref()
                    .map(folder_name)
                    .unwrap_or_else(|| "unknown".to_string()),
                pid: msg.pid,
                state: SessionState::Idle,
                state_since: now,
                last_event: now,
                last_event_unix: now_unix,
                last_working: None,
                stale: false,
                approximate: false,
            }
        });

        if let Some(cwd) = msg.cwd.as_deref()
            && !cwd.is_empty()
        {
            entry.folder = folder_name(cwd);
        }
        if let Some(pid) = msg.pid {
            entry.pid = Some(pid);
        }

        let next_state = match msg.event {
            HookEvent::Start | HookEvent::Idle | HookEvent::End => SessionState::Idle,
            HookEvent::Working | HookEvent::Heartbeat => SessionState::Working,
        };
        // A heartbeat proves the agent process is still alive: it rescues a
        // session a previous tick demoted as suspiciously silent (stale), but
        // it never resurrects a session the agent itself idled.
        let next_state = if msg.event == HookEvent::Heartbeat {
            if entry.stale {
                SessionState::Working
            } else {
                entry.state
            }
        } else {
            next_state
        };

        if next_state != entry.state {
            entry.state = next_state;
            entry.state_since = now;
        }
        if next_state == SessionState::Working {
            entry.last_working = Some(now);
        }
        // Heartbeats from an idle session only prove the process is alive;
        // they must not keep refreshing the grace clock, or an open-but-idle
        // agent could hold the PC awake forever.
        if msg.event != HookEvent::Heartbeat || entry.state == SessionState::Working {
            entry.last_event = now;
            entry.last_event_unix = now_unix;
        }
        entry.stale = false;
        entry.approximate = false;
        let new_state = entry.state;

        // A hook session supersedes any approximate row for the same process:
        // two rows for one terminal would flap independently. If the hook had
        // no cwd of its own, inherit the folder the process detector already
        // resolved from the process tree.
        if let Some(pid) = msg.pid {
            let approx_folder = self
                .sessions
                .values()
                .find(|s| {
                    s.approximate
                        && s.agent == msg.agent
                        && s.pid == Some(pid)
                        && s.folder != "unknown"
                })
                .map(|s| s.folder.clone());
            if let Some(folder) = approx_folder
                && let Some(entry) = self.sessions.get_mut(&(msg.agent, session_id))
                && entry.folder == "unknown"
            {
                entry.folder = folder;
            }
            self.sessions
                .retain(|_, s| !(s.approximate && s.agent == msg.agent && s.pid == Some(pid)));
        }

        self.enforce_cap(msg.agent);

        match old_state {
            None => HookEffect::Created(new_state),
            Some(old) if old != new_state => HookEffect::Changed {
                from: old,
                to: new_state,
            },
            Some(_) => HookEffect::None,
        }
    }

    /// Fold in process-detection results. Only fills gaps: hook sessions win.
    pub fn on_process(&mut self, detections: &[ProcessDetection], now: Instant) {
        let now_unix = unix_now();
        let mut seen: HashSet<(AgentId, String)> = HashSet::new();

        for det in detections {
            let session_id = format!("pid:{}", det.pid);
            let key = (det.agent, session_id.clone());
            seen.insert(key.clone());

            let state = if det.working {
                SessionState::Working
            } else {
                SessionState::Idle
            };

            match self.sessions.get_mut(&key) {
                Some(existing) => {
                    // A hook-managed session for the same pid takes precedence.
                    if !existing.approximate {
                        continue;
                    }
                    if existing.state != state {
                        existing.state = state;
                        existing.state_since = now;
                    }
                    if state == SessionState::Working {
                        existing.last_working = Some(now);
                    }
                    existing.last_event = now;
                    existing.last_event_unix = now_unix;
                    existing.folder = det.folder.clone();
                    existing.stale = false;
                }
                None => {
                    // A hook session may already own this process under a
                    // different key (e.g. `ses_...` instead of `pid:NNN`). It
                    // is authoritative; a second approximate row would flap on
                    // CPU alone and fight the hook's own state. Still, if the
                    // hook session has no folder yet ("unknown"), enrich it
                    // from the process tree's resolved cwd.
                    let mut covered = false;
                    for s in self.sessions.values_mut() {
                        if s.agent == det.agent && !s.approximate && s.pid == Some(det.pid) {
                            covered = true;
                            if s.folder == "unknown" && det.folder != "unknown" {
                                s.folder = det.folder.clone();
                            }
                        }
                    }
                    if covered {
                        continue;
                    }
                    self.sessions.insert(
                        key,
                        Session {
                            agent: det.agent,
                            session_id,
                            folder: det.folder.clone(),
                            pid: Some(det.pid),
                            state,
                            state_since: now,
                            last_event: now,
                            last_event_unix: now_unix,
                            last_working: det.working.then_some(now),
                            stale: false,
                            approximate: true,
                        },
                    );
                }
            }
        }

        // Drop approximate sessions whose process vanished — but only after
        // PID_GRACE, exactly like hook sessions: one flaky enumeration pass
        // must not release the hold the moment it misses a pid.
        self.sessions.retain(|_, s| {
            !s.approximate
                || seen.contains(&(s.agent, s.session_id.clone()))
                || now.duration_since(s.last_event) < PID_GRACE
        });

        for agent in AgentId::ALL {
            self.enforce_cap(agent);
        }
    }

    /// Demote silent working sessions, drop dead PIDs.
    pub fn tick(&mut self, stale_timeout: Duration, alive: &HashSet<u32>, now: Instant) {
        for session in self.sessions.values_mut() {
            // A live agent process keeps its session Working: a long reasoning
            // or tool phase can easily outlive the stale window, and the
            // process table is a far better liveness signal than silence. The
            // dead-pid sweep below still removes sessions whose process exits.
            let pid_alive = session.pid.map(|p| alive.contains(&p)).unwrap_or(false);
            if session.state == SessionState::Working
                && !pid_alive
                && now.duration_since(session.last_event) >= stale_timeout
            {
                session.state = SessionState::Idle;
                session.state_since = now;
                session.stale = true;
            }
        }
        // Sessions whose owning process is gone are removed outright — but
        // only after [`PID_GRACE`]: the process table is sampled every couple
        // of seconds, so "not in the sample" is not yet "dead" for a pid that
        // was just born (see the fresh-hook-pid regression test below).
        let dead: Vec<(AgentId, String, Instant, bool)> = self
            .sessions
            .values()
            .filter(|s| {
                s.pid
                    .map(|p| !alive.contains(&p) && now.duration_since(s.last_event) >= PID_GRACE)
                    .unwrap_or(false)
            })
            .map(|s| (s.agent, s.session_id.clone(), s.last_event, s.approximate))
            .collect();
        for (agent, session_id, last_event, approximate) in dead {
            let key = (agent, session_id);
            self.sessions.remove(&key);
            if !approximate {
                self.ended.insert(key, last_event);
            }
        }
        // A tombstone only matters while its flat grace could still be open.
        self.ended
            .retain(|_, t| now.duration_since(*t) < ENDED_KEEP);
    }

    fn enforce_cap(&mut self, agent: AgentId) {
        let mut ids: Vec<(Instant, (AgentId, String))> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.agent == agent)
            .map(|(k, s)| (s.last_event, k.clone()))
            .collect();
        if ids.len() <= MAX_SESSIONS_PER_AGENT {
            return;
        }
        ids.sort_by_key(|(t, _)| *t);
        let drop_count = ids.len() - MAX_SESSIONS_PER_AGENT;
        for (_, key) in ids.into_iter().take(drop_count) {
            self.sessions.remove(&key);
        }
    }

    #[cfg(test)]
    pub fn insert_for_test(&mut self, session: Session) {
        self.sessions
            .insert((session.agent, session.session_id.clone()), session);
    }
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `any working` widened by the anti-flap grace window.
///
/// Two classes of quiet are tolerated beyond `any_working`:
/// * hook sessions — flat `keep_awake_after`, extended to
///   [`LIVE_PID_IDLE_TRUST`] while their process is alive (mid-turn think
///   gaps and step-settle events must not release the hold);
/// * approximate rows — [`APPROX_DWELL`] after their last Working sample.
///
/// Recently removed sessions keep their flat grace via tombstones.
pub fn any_working_or_in_grace(
    registry: &Registry,
    watch: impl Fn(AgentId) -> bool,
    keep_awake_after: Duration,
    alive: &HashSet<u32>,
    now: Instant,
) -> bool {
    if registry.any_working(&watch) {
        return true;
    }
    if keep_awake_after.is_zero() {
        return false;
    }
    if !registry
        .in_grace(&watch, keep_awake_after, alive, now)
        .is_empty()
    {
        return true;
    }
    registry.ended_in_grace(&watch, keep_awake_after, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(agent: AgentId, event: HookEvent, id: &str, pid: Option<u32>) -> HookMessage {
        HookMessage {
            agent,
            event,
            session_id: Some(id.to_string()),
            cwd: Some(format!("C:\\work\\{id}")),
            pid,
            ts: 0,
        }
    }

    fn always(_: AgentId) -> bool {
        true
    }

    #[test]
    fn start_then_working_then_idle() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        r.on_hook(
            msg(AgentId::ClaudeCode, HookEvent::Start, "a", Some(10)),
            t0,
        );
        assert_eq!(r.working_count(AgentId::ClaudeCode), 0);
        assert_eq!(
            r.get(AgentId::ClaudeCode, "a").unwrap().state,
            SessionState::Idle
        );
        assert_eq!(r.get(AgentId::ClaudeCode, "a").unwrap().folder, "a");

        r.on_hook(
            msg(AgentId::ClaudeCode, HookEvent::Working, "a", Some(10)),
            t0,
        );
        assert_eq!(r.working_count(AgentId::ClaudeCode), 1);
        assert_eq!(
            r.get(AgentId::ClaudeCode, "a").unwrap().state,
            SessionState::Working
        );

        r.on_hook(msg(AgentId::ClaudeCode, HookEvent::Idle, "a", Some(10)), t0);
        assert_eq!(r.working_count(AgentId::ClaudeCode), 0);
        assert_eq!(
            r.get(AgentId::ClaudeCode, "a").unwrap().state,
            SessionState::Idle
        );

        r.on_hook(msg(AgentId::ClaudeCode, HookEvent::End, "a", Some(10)), t0);
        assert!(r.get(AgentId::ClaudeCode, "a").is_none());
    }

    #[test]
    fn heartbeat_does_not_resurrect_idle_session() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        r.on_hook(msg(AgentId::Codex, HookEvent::Working, "t", None), t0);
        r.on_hook(msg(AgentId::Codex, HookEvent::Idle, "t", None), t0);
        r.on_hook(msg(AgentId::Codex, HookEvent::Heartbeat, "t", None), t0);
        assert_eq!(
            r.get(AgentId::Codex, "t").unwrap().state,
            SessionState::Idle
        );
    }

    #[test]
    fn stale_working_is_demoted_and_flagged() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        r.on_hook(msg(AgentId::OpenCode, HookEvent::Working, "s", None), t0);
        let alive: HashSet<u32> = HashSet::new();
        let later = t0 + Duration::from_secs(21 * 60);
        r.tick(Duration::from_secs(20 * 60), &alive, later);

        let s = r.get(AgentId::OpenCode, "s").unwrap();
        assert_eq!(s.state, SessionState::Idle);
        assert!(s.stale);
        assert_eq!(r.working_count(AgentId::OpenCode), 0);
    }

    #[test]
    fn fresh_working_is_not_demoted() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        r.on_hook(msg(AgentId::OpenCode, HookEvent::Working, "s", None), t0);
        let alive: HashSet<u32> = HashSet::new();
        r.tick(
            Duration::from_secs(20 * 60),
            &alive,
            t0 + Duration::from_secs(60),
        );
        assert_eq!(r.working_count(AgentId::OpenCode), 1);
    }

    #[test]
    fn dead_pid_sessions_are_removed_after_the_grace_window() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        r.on_hook(msg(AgentId::Pi, HookEvent::Working, "p", Some(4242)), t0);
        let mut alive: HashSet<u32> = HashSet::new();
        alive.insert(1);
        // Right after creation the missing pid is only a suspect: the process
        // sample may simply predate the process.
        r.tick(
            Duration::from_secs(20 * 60),
            &alive,
            t0 + Duration::from_millis(500),
        );
        assert!(r.get(AgentId::Pi, "p").is_some());
        // Once the pid has been missing beyond the grace window, it goes.
        r.tick(
            Duration::from_secs(20 * 60),
            &alive,
            t0 + PID_GRACE + Duration::from_secs(1),
        );
        assert!(r.get(AgentId::Pi, "p").is_none());

        // Still-alive PID survives, no matter how long it has been quiet.
        alive.insert(4242);
        r.on_hook(msg(AgentId::Pi, HookEvent::Working, "p", Some(4242)), t0);
        r.tick(
            Duration::from_secs(20 * 60),
            &alive,
            t0 + Duration::from_secs(60),
        );
        assert!(r.get(AgentId::Pi, "p").is_some());
    }

    #[test]
    fn fresh_hook_pid_survives_a_stale_process_sample() {
        // The bug the probe caught: a hook (e.g. session.created) that arrives
        // moments after its agent process starts carries a pid the current,
        // up-to-two-seconds-old process sample has never seen. The next tick
        // must not remove the session — by the time the grace window lapses,
        // the refreshed sample includes the pid and the session is safe.
        let mut r = Registry::new();
        let t0 = Instant::now();
        r.on_hook(
            msg(AgentId::OpenCode, HookEvent::Start, "fresh", Some(9001)),
            t0,
        );
        let stale_sample: HashSet<u32> = HashSet::new();
        r.tick(
            Duration::from_secs(20 * 60),
            &stale_sample,
            t0 + Duration::from_millis(300),
        );
        r.tick(
            Duration::from_secs(20 * 60),
            &stale_sample,
            t0 + Duration::from_secs(2),
        );
        assert!(
            r.get(AgentId::OpenCode, "fresh").is_some(),
            "a just-created session must survive a stale process sample"
        );
        // The refreshed sample now includes the pid: the session stays.
        let fresh_sample: HashSet<u32> = HashSet::from([9001]);
        r.tick(
            Duration::from_secs(20 * 60),
            &fresh_sample,
            t0 + PID_GRACE + Duration::from_secs(2),
        );
        assert!(r.get(AgentId::OpenCode, "fresh").is_some());
    }

    #[test]
    fn grace_window_keeps_us_awake_after_last_event() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        let none_alive: HashSet<u32> = HashSet::new();
        r.on_hook(msg(AgentId::Cline, HookEvent::Working, "c", None), t0);
        r.on_hook(msg(AgentId::Cline, HookEvent::Idle, "c", None), t0);

        assert!(any_working_or_in_grace(
            &r,
            always,
            Duration::from_secs(15),
            &none_alive,
            t0 + Duration::from_secs(10)
        ));
        assert!(!any_working_or_in_grace(
            &r,
            always,
            Duration::from_secs(15),
            &none_alive,
            t0 + Duration::from_secs(16)
        ));
        // Zero grace means "no grace at all".
        assert!(!any_working_or_in_grace(
            &r,
            always,
            Duration::from_secs(0),
            &none_alive,
            t0
        ));
    }

    #[test]
    fn live_pid_idle_outlives_the_flat_grace() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        let alive: HashSet<u32> = HashSet::from([42]);
        r.on_hook(msg(AgentId::ClaudeCode, HookEvent::Working, "a", Some(42)), t0);
        r.on_hook(msg(AgentId::ClaudeCode, HookEvent::Idle, "a", Some(42)), t0);
        // A live process keeps the trust window open well past the flat 15s:
        // this is what carries a mid-turn think gap without releasing.
        assert!(any_working_or_in_grace(
            &r,
            always,
            Duration::from_secs(15),
            &alive,
            t0 + Duration::from_secs(80)
        ));
        // Bounded: quiet past the trust window still releases.
        assert!(!any_working_or_in_grace(
            &r,
            always,
            Duration::from_secs(15),
            &alive,
            t0 + LIVE_PID_IDLE_TRUST + Duration::from_secs(1)
        ));
    }

    #[test]
    fn dead_pid_falls_back_to_the_flat_grace() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        let none_alive: HashSet<u32> = HashSet::new();
        r.on_hook(msg(AgentId::ClaudeCode, HookEvent::Working, "a", Some(42)), t0);
        r.on_hook(msg(AgentId::ClaudeCode, HookEvent::Idle, "a", Some(42)), t0);
        assert!(any_working_or_in_grace(
            &r,
            always,
            Duration::from_secs(15),
            &none_alive,
            t0 + Duration::from_secs(14)
        ));
        assert!(!any_working_or_in_grace(
            &r,
            always,
            Duration::from_secs(15),
            &none_alive,
            t0 + Duration::from_secs(16)
        ));
    }

    #[test]
    fn end_hook_removal_keeps_the_grace_window() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        let none_alive: HashSet<u32> = HashSet::new();
        r.on_hook(msg(AgentId::Cline, HookEvent::Working, "c", None), t0);
        r.on_hook(msg(AgentId::Cline, HookEvent::End, "c", None), t0 + Duration::from_secs(1));
        assert!(r.get(AgentId::Cline, "c").is_none());
        // The row is gone, but its grace keeps covering the next moments so a
        // finished one-shot task cannot release the hold the same tick.
        assert!(any_working_or_in_grace(
            &r,
            always,
            Duration::from_secs(15),
            &none_alive,
            t0 + Duration::from_secs(10)
        ));
        assert!(!any_working_or_in_grace(
            &r,
            always,
            Duration::from_secs(15),
            &none_alive,
            t0 + Duration::from_secs(16)
        ));
    }

    #[test]
    fn dead_pid_sweep_preserves_the_grace_window() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        let none_alive: HashSet<u32> = HashSet::new();
        r.on_hook(msg(AgentId::Codex, HookEvent::Working, "x", Some(77)), t0);
        // The process is gone from the sample: the sweep removes the row only
        // after PID_GRACE.
        let t_sweep = t0 + PID_GRACE + Duration::from_millis(1);
        r.tick(Duration::from_secs(20 * 60), &none_alive, t_sweep);
        assert!(r.get(AgentId::Codex, "x").is_none());
        // The tombstone still covers the remaining grace window.
        assert!(any_working_or_in_grace(
            &r,
            always,
            Duration::from_secs(15),
            &none_alive,
            t_sweep
        ));
        assert!(!any_working_or_in_grace(
            &r,
            always,
            Duration::from_secs(15),
            &none_alive,
            t0 + Duration::from_secs(16)
        ));
    }

    #[test]
    fn approximate_rows_dwell_past_their_own_idle_hold() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        let none_alive: HashSet<u32> = HashSet::new();
        let dets = vec![ProcessDetection {
            agent: AgentId::Cline,
            pid: 777,
            folder: "proj".into(),
            working: true,
        }];
        r.on_process(&dets, t0);
        // The classifier flips the row idle, but the decision layer keeps the
        // hold for APPROX_DWELL after the last working sample.
        let idle = vec![ProcessDetection {
            agent: AgentId::Cline,
            pid: 777,
            folder: "proj".into(),
            working: false,
        }];
        r.on_process(&idle, t0 + Duration::from_secs(20));
        assert!(any_working_or_in_grace(
            &r,
            always,
            Duration::from_secs(15),
            &none_alive,
            t0 + APPROX_DWELL - Duration::from_secs(1)
        ));
        assert!(!any_working_or_in_grace(
            &r,
            always,
            Duration::from_secs(15),
            &none_alive,
            t0 + APPROX_DWELL + Duration::from_secs(1)
        ));
    }

    #[test]
    fn watching_filter_is_respected() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        r.on_hook(msg(AgentId::Codex, HookEvent::Working, "x", None), t0);
        assert!(r.any_working(|a| a == AgentId::Codex));
        assert!(!r.any_working(|a| a == AgentId::Cline));
    }

    #[test]
    fn cap_evicts_oldest() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        for i in 0..60u32 {
            r.on_hook(
                msg(
                    AgentId::ClaudeCode,
                    HookEvent::Working,
                    &format!("s{i}"),
                    None,
                ),
                t0 + Duration::from_millis(i as u64),
            );
        }
        assert_eq!(
            r.sessions_for(AgentId::ClaudeCode).len(),
            MAX_SESSIONS_PER_AGENT
        );
        assert!(r.get(AgentId::ClaudeCode, "s0").is_none());
        assert!(r.get(AgentId::ClaudeCode, "s59").is_some());
    }

    #[test]
    fn process_detection_creates_approximate_sessions() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        let dets = vec![ProcessDetection {
            agent: AgentId::Cline,
            pid: 777,
            folder: "proj".into(),
            working: true,
        }];
        r.on_process(&dets, t0);
        let s = r.get(AgentId::Cline, "pid:777").unwrap();
        assert!(s.approximate);
        assert_eq!(s.state, SessionState::Working);
        assert_eq!(s.last_working, Some(t0));

        // A single missed sample keeps the row (PID_GRACE); a miss past the
        // grace window removes it.
        r.on_process(&[], t0 + Duration::from_secs(1));
        assert!(r.get(AgentId::Cline, "pid:777").is_some());
        r.on_process(&[], t0 + PID_GRACE);
        assert!(r.get(AgentId::Cline, "pid:777").is_none());
    }

    #[test]
    fn process_detection_never_overrides_hook_sessions() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        r.insert_for_test(Session {
            agent: AgentId::Codex,
            session_id: "pid:555".into(),
            folder: "hooked".into(),
            pid: Some(555),
            state: SessionState::Working,
            state_since: t0,
            last_event: t0,
            last_event_unix: 0,
            last_working: Some(t0),
            stale: false,
            approximate: false,
        });
        let dets = vec![ProcessDetection {
            agent: AgentId::Codex,
            pid: 555,
            folder: "guessed".into(),
            working: false,
        }];
        r.on_process(&dets, t0);
        let s = r.get(AgentId::Codex, "pid:555").unwrap();
        assert!(!s.approximate);
        assert_eq!(s.state, SessionState::Working);
        assert_eq!(s.folder, "hooked");
    }

    #[test]
    fn sessions_without_id_or_pid_are_kept_apart_per_agent() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        r.on_hook(
            HookMessage {
                agent: AgentId::Codex,
                event: HookEvent::Working,
                session_id: None,
                cwd: None,
                pid: None,
                ts: 0,
            },
            t0,
        );
        assert_eq!(r.working_count(AgentId::Codex), 1);
        assert_eq!(
            r.get(AgentId::Codex, "anon:codex").unwrap().folder,
            "unknown"
        );
    }

    #[test]
    fn replayed_events_relative_to_now_are_dropped() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        let now = unix_now();

        // A message timestamped two hours ago (a replayed queue) is ignored.
        r.on_hook(
            HookMessage {
                agent: AgentId::ClaudeCode,
                event: HookEvent::Working,
                session_id: Some("stale-1".into()),
                cwd: None,
                pid: Some(123),
                ts: now.saturating_sub(2 * 60 * 60),
            },
            t0,
        );
        assert_eq!(r.working_count(AgentId::ClaudeCode), 0);
        assert!(r.get(AgentId::ClaudeCode, "stale-1").is_none());

        // A recent timestamp is accepted even when the wall clock was
        // corrected backwards after launch (the old app-start comparison
        // muted every hook in that case).
        r.on_hook(
            HookMessage {
                agent: AgentId::ClaudeCode,
                event: HookEvent::Working,
                session_id: Some("fresh-1".into()),
                cwd: None,
                pid: Some(123),
                ts: now.saturating_sub(30),
            },
            t0,
        );
        assert_eq!(r.working_count(AgentId::ClaudeCode), 1);
        assert!(r.get(AgentId::ClaudeCode, "fresh-1").is_some());
    }

    #[test]
    fn a_live_pid_keeps_a_working_session_through_the_stale_window() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        r.on_hook(
            msg(AgentId::OpenCode, HookEvent::Working, "long", Some(777)),
            t0,
        );
        let alive: HashSet<u32> = HashSet::from([777]);

        // Thirty hours of silence: the process is still alive, the run may
        // simply be thinking. The session must stay Working.
        r.tick(
            Duration::from_secs(20 * 60),
            &alive,
            t0 + Duration::from_secs(30 * 3600),
        );
        let s = r.get(AgentId::OpenCode, "long").unwrap();
        assert_eq!(s.state, SessionState::Working);
        assert!(!s.stale);
    }

    #[test]
    fn a_pidless_working_session_still_demotes_when_silent() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        r.on_hook(msg(AgentId::OpenCode, HookEvent::Working, "quiet", None), t0);
        let alive: HashSet<u32> = HashSet::new();
        r.tick(
            Duration::from_secs(20 * 60),
            &alive,
            t0 + Duration::from_secs(21 * 60),
        );
        let s = r.get(AgentId::OpenCode, "quiet").unwrap();
        assert_eq!(s.state, SessionState::Idle);
        assert!(s.stale);
    }

    #[test]
    fn a_heartbeat_rescues_a_stale_session_but_not_an_idle_one() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        r.on_hook(msg(AgentId::OpenCode, HookEvent::Working, "w", None), t0);
        let alive: HashSet<u32> = HashSet::new();
        let later = t0 + Duration::from_secs(21 * 60);
        r.tick(Duration::from_secs(20 * 60), &alive, later);
        assert!(r.get(AgentId::OpenCode, "w").unwrap().stale);

        // The plugin's heartbeat says the run is still alive: resurrect.
        r.on_hook(msg(AgentId::OpenCode, HookEvent::Heartbeat, "w", None), later);
        let s = r.get(AgentId::OpenCode, "w").unwrap();
        assert_eq!(s.state, SessionState::Working);
        assert!(!s.stale);

        // An idle session stays idle: heartbeats never resurrect it.
        r.on_hook(msg(AgentId::OpenCode, HookEvent::Idle, "w", None), later);
        r.on_hook(msg(AgentId::OpenCode, HookEvent::Heartbeat, "w", None), later);
        assert_eq!(
            r.get(AgentId::OpenCode, "w").unwrap().state,
            SessionState::Idle
        );
    }

    #[test]
    fn a_hook_session_removes_the_approximate_row_for_its_pid() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        let dets = vec![ProcessDetection {
            agent: AgentId::OpenCode,
            pid: 888,
            folder: "proj".into(),
            working: true,
        }];
        r.on_process(&dets, t0);
        assert!(r.get(AgentId::OpenCode, "pid:888").is_some());

        // The session.created hook arrives under its real session id.
        r.on_hook(
            msg(AgentId::OpenCode, HookEvent::Working, "ses_x", Some(888)),
            t0,
        );
        assert!(
            r.get(AgentId::OpenCode, "pid:888").is_none(),
            "the approximate row hands over to the hook session"
        );
        let s = r.get(AgentId::OpenCode, "ses_x").unwrap();
        assert!(!s.approximate);
        assert_eq!(s.state, SessionState::Working);
    }

    #[test]
    fn process_detection_does_not_shadow_a_hook_session_with_another_key() {
        let mut r = Registry::new();
        let t0 = Instant::now();
        r.on_hook(
            msg(AgentId::OpenCode, HookEvent::Idle, "ses_y", Some(999)),
            t0,
        );
        let dets = vec![ProcessDetection {
            agent: AgentId::OpenCode,
            pid: 999,
            folder: "guessed".into(),
            working: true,
        }];
        r.on_process(&dets, t0);
        assert!(
            r.get(AgentId::OpenCode, "pid:999").is_none(),
            "a covered process must not grow a second row"
        );
        assert_eq!(
            r.get(AgentId::OpenCode, "ses_y").unwrap().state,
            SessionState::Idle
        );
    }

    #[test]
    fn hook_session_without_cwd_inherits_folder_from_process_detection() {
        let mut r = Registry::new();
        let t0 = Instant::now();

        // Case 1: process detection already resolved the folder before the hook arrived
        let dets = vec![ProcessDetection {
            agent: AgentId::OpenCode,
            pid: 5448,
            folder: "Windows Tool".into(),
            working: false,
        }];
        r.on_process(&dets, t0);
        r.on_hook(
            HookMessage {
                agent: AgentId::OpenCode,
                event: HookEvent::Working,
                session_id: Some("ses_1".into()),
                cwd: None,
                pid: Some(5448),
                ts: 0,
            },
            t0,
        );
        assert_eq!(
            r.get(AgentId::OpenCode, "ses_1").unwrap().folder,
            "Windows Tool"
        );

        // Case 2: hook arrived first with no cwd, then process detection resolves it
        r.on_hook(
            HookMessage {
                agent: AgentId::OpenCode,
                event: HookEvent::Working,
                session_id: Some("ses_2".into()),
                cwd: None,
                pid: Some(6000),
                ts: 0,
            },
            t0,
        );
        assert_eq!(r.get(AgentId::OpenCode, "ses_2").unwrap().folder, "unknown");
        let dets2 = vec![ProcessDetection {
            agent: AgentId::OpenCode,
            pid: 6000,
            folder: "my-repo".into(),
            working: true,
        }];
        r.on_process(&dets2, t0);
        assert_eq!(r.get(AgentId::OpenCode, "ses_2").unwrap().folder, "my-repo");
    }
}
