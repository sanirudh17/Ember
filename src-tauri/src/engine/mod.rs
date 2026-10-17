//! The decision engine.
//!
//! One [`Engine`] owns the settings, the session registry, the process
//! detector, the manual session, the Windows sleep request and the lid-proof
//! hold. [`Engine::tick`] is called about twice a second; it re-evaluates
//! [`decision::want_awake`], engages or releases the OS hold, and returns a
//! [`Snapshot`] for the UI.
//!
//! Nothing in here talks to Tauri, so all of the interesting logic is testable.

pub mod decision;
pub mod manual;
pub mod snapshot;
pub mod until;

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::agents::process::ProcessDetector;
use crate::agents::registry::{Registry, SessionState, any_working_or_in_grace};
use crate::agents::{AgentId, installers};
use crate::battery::{self, BatteryStatus, LID_CLOSED, POWER_DIRTY};
use crate::lid::{self, LidHold, RealPowerApi};
use crate::power::{self, SleepBlocker};
use crate::settings::{GRACE_SECS, STALE_TIMEOUT_MIN, Settings};

pub use decision::{DecisionInput, Guard, Guards};
pub use manual::{Manual, ManualMode, PRESETS_MIN, clock, preset_label};
pub use snapshot::*;

/// How often the process tree is polled.
const PROCESS_POLL: Duration = Duration::from_secs(2);
/// How often the battery is re-read even without a power broadcast.
const BATTERY_POLL: Duration = Duration::from_secs(10);
/// How often install/detection state is recomputed while agents work.
pub const AGENT_INFO_POLL: Duration = Duration::from_secs(5);
/// The slower cadence used while nothing is working.
pub const AGENT_INFO_IDLE_POLL: Duration = Duration::from_secs(60);
/// Snapshots are never emitted faster than this, except when something marks
/// the engine dirty (user actions, hook events), which must not wait it out.
const EMIT_INTERVAL: Duration = Duration::from_secs(1);
/// How long a hold must last before the UI, tray, overlay dot, notifications,
/// and log show it engaged (filters sub-second process blips while feeling
/// effectively immediate).
pub const VISIBILITY_HOLD_DELAY: Duration = Duration::from_secs(1);
/// One chime per notification episode: within this window of the last chime
/// neither another engage chime nor a matching finish chime re-fires, so a
/// burst of short holds (settle events, dropped hooks, a thinking agent
/// re-engaging) stays visible but silent. Manual holds and battery releases
/// always notify.
pub const NOTIFY_EPISODE_DEBOUNCE: Duration = Duration::from_secs(60);
/// Once a hold is up, the agent signal must stay dark for this long before
/// the hold drops. Absorbs sub-second flicker from multi-row races and
/// one-sample detector glitches without touching any classification gate.
pub const RELEASE_FLOOR: Duration = Duration::from_secs(3);

/// A user-visible transition worth a notification.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Notify {
    /// Ember started holding the PC awake.
    Engaged,
    /// Everything Ember was watching went idle.
    Finished,
    /// A battery guardrail released the hold.
    Battery,
}

pub struct TickOutcome {
    pub snapshot: Snapshot,
    /// True when the caller should emit `state-changed`.
    pub emit: bool,
    /// True when window chrome (title, overlay icon) needs a push.
    pub chrome_changed: bool,
    pub notifications: Vec<Notify>,
}

/// The cached, slow-moving half of an agent's snapshot.
#[derive(Clone)]
struct AgentInfo {
    id: AgentId,
    detected: bool,
    detect_detail: String,
    installed: bool,
    needs_repair: bool,
    supported: bool,
    install_file: Option<String>,
    install_detail: String,
}

pub struct Engine {
    pub settings: Settings,
    pub registry: Registry,
    pub detector: ProcessDetector,
    pub manual: Manual,
    pub paused: bool,
    pub paused_until_unix: Option<u64>,
    /// Set whenever something changed outside `tick` and the UI must be told.
    pub dirty: bool,

    blocker: SleepBlocker,
    pub lid_hold: Option<LidHold>,
    pub lid_engaged_unix: Option<u64>,
    pub lid_cap_tripped: bool,
    pub lid_error: Option<String>,
    pub blocker_error: Option<String>,
    /// The crash watchdog is spawned at most once per app run, the first time
    /// the lid override goes up. It outlives us and restores from the session
    /// state file if we die with the override still applied.
    lid_watchdog_spawned: bool,

    pub engaged_since_unix: Option<u64>,

    /// Test seams for the hardcoded always-on policy. Production keeps the
    /// defaults below; tests override them to exercise long-horizon behaviour.
    pub grace_secs: u64,
    pub stale_timeout_min: u64,
    pub lid_cap_hours: u32,

    /// Turned off by tests so the real process tree cannot influence results.
    pub process_detection: bool,

    /// Test seam: when set, the engine reads the wall clock from here instead of
    /// the OS. Production never sets it; it exists so the manual countdown and
    /// the 1 Hz emit budget can be exercised deterministically.
    pub unix_override: Option<u64>,

    pub battery: BatteryStatus,
    battery_at: Option<Instant>,
    process_at: Option<Instant>,
    agent_info_at: Option<Instant>,
    alive: HashSet<u32>,

    agent_info: Vec<AgentInfo>,
    capabilities: Capabilities,

    pub last: Snapshot,
    last_emit: Option<Instant>,
    last_chrome: String,

    hold_started_at: Option<Instant>,
    user_visible_engaged: bool,
    /// When the last engage/finish/battery chime actually played. Gates the
    /// next chime for [`NOTIFY_EPISODE_DEBOUNCE`].
    last_notified_at: Option<Instant>,
    /// When the agent signal last went dark while a hold was up; feeds
    /// [`RELEASE_FLOOR`].
    agent_dark_since: Option<Instant>,
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        let settings = Settings::load();
        let battery = battery::poll();
        let now = Instant::now();
        let mut engine = Engine {
            settings,
            registry: Registry::new(),
            detector: ProcessDetector::new(),
            manual: Manual::default(),
            paused: false,
            paused_until_unix: None,
            dirty: true,
            blocker: SleepBlocker::new(),
            lid_hold: None,
            lid_engaged_unix: None,
            lid_cap_tripped: false,
            lid_error: None,
            blocker_error: None,
            lid_watchdog_spawned: false,
            engaged_since_unix: None,
            grace_secs: GRACE_SECS,
            stale_timeout_min: STALE_TIMEOUT_MIN,
            lid_cap_hours: 6,
            process_detection: true,
            unix_override: None,
            battery,
            battery_at: Some(now),
            process_at: None,
            agent_info_at: None,
            alive: HashSet::new(),
            agent_info: Vec::new(),
            capabilities: Capabilities::default(),
            last: Snapshot::default(),
            last_emit: None,
            last_chrome: String::new(),
            hold_started_at: None,
            user_visible_engaged: false,
            last_notified_at: None,
            agent_dark_since: None,
        };
        engine.capabilities = engine.detect_capabilities();
        engine.refresh_agent_info();
        // Prime the registry so `alive` is populated before the first hygiene
        // pass, and so the first snapshot already shows real sessions.
        engine.poll_processes(now);
        engine
    }

    // -----------------------------------------------------------------------
    // Capabilities
    // -----------------------------------------------------------------------

    fn detect_capabilities(&self) -> Capabilities {
        let (build, os_name) = detect_windows_build();
        let lid_present = lid::lid_present();
        let modern_standby = lid::modern_standby();
        // The backdrop is always solid now, so Mica availability no longer
        // affects anything; the field is kept for the frontend's capability card.
        let mica_available = is_windows_11_or_greater(build);

        Capabilities {
            lid_present,
            modern_standby,
            mica_available,
            os_build: build,
            os_name,
            blocking: if modern_standby {
                "Windows power requests and the lid-close action (Modern Standby)".to_string()
            } else {
                "Windows power requests and the lid-close action".to_string()
            },
            power_requests: true,
            lid_proof_available: lid_present,
        }
    }
}

pub fn parse_build_number(raw: &str) -> Option<u32> {
    if let Some(start) = raw.find('(')
        && let Some(end) = raw[start..].find(')')
    {
        let inner = &raw[start + 1..start + end];
        if let Ok(b) = inner.trim().parse::<u32>() {
            return Some(b);
        }
    }
    if let Some(last) = raw.rsplit('.').next()
        && let Ok(b) = last.trim().parse::<u32>()
    {
        return Some(b);
    }
    raw.trim().parse::<u32>().ok()
}

pub fn is_windows_11_or_greater(build: u32) -> bool {
    build >= 22_000
}

#[repr(C)]
#[allow(non_snake_case)]
struct OsVersionInfoExW {
    dwOSVersionInfoSize: u32,
    dwMajorVersion: u32,
    dwMinorVersion: u32,
    dwBuildNumber: u32,
    dwPlatformId: u32,
    szCSDVersion: [u16; 128],
    wServicePackMajor: u16,
    wServicePackMinor: u16,
    wSuiteMask: u16,
    wProductType: u8,
    wReserved: u8,
}

type RtlGetVersionFn = unsafe extern "system" fn(*mut OsVersionInfoExW) -> i32;

fn rtl_get_version_build() -> Option<u32> {
    unsafe {
        use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
        use windows::core::s;

        let ntdll = GetModuleHandleA(s!("ntdll.dll")).ok()?;
        let func = GetProcAddress(ntdll, s!("RtlGetVersion"))?;
        let rtl_get_version: RtlGetVersionFn = std::mem::transmute(func);

        let mut info: OsVersionInfoExW = std::mem::zeroed();
        info.dwOSVersionInfoSize = std::mem::size_of::<OsVersionInfoExW>() as u32;

        if rtl_get_version(&mut info) == 0 {
            Some(info.dwBuildNumber)
        } else {
            None
        }
    }
}

fn registry_build() -> Option<u32> {
    use windows::Win32::System::Registry::{
        HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
    };
    use windows::core::w;

    unsafe {
        let mut key = Default::default();
        let subkey = w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
        if RegOpenKeyExW(HKEY_LOCAL_MACHINE, subkey, Some(0), KEY_READ, &mut key).is_err() {
            return None;
        }

        let val_name = w!("CurrentBuildNumber");
        let mut buf = [0u16; 64];
        let mut len = (buf.len() * 2) as u32;
        let mut val_type = REG_SZ;

        let status = RegQueryValueExW(
            key,
            val_name,
            None,
            Some(&mut val_type),
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut len),
        );
        let _ = RegCloseKey(key);

        if status.is_ok() {
            let u16_len = (len as usize) / 2;
            let end = buf[..u16_len]
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(u16_len);
            let s = String::from_utf16_lossy(&buf[..end]);
            parse_build_number(&s)
        } else {
            None
        }
    }
}

pub fn detect_windows_build() -> (u32, String) {
    let build = rtl_get_version_build().or_else(registry_build).unwrap_or(0);
    let os_name = if build >= 22_000 {
        format!("Windows 11, build {build}")
    } else if build > 0 {
        format!("Windows 10, build {build}")
    } else {
        "Windows".to_string()
    };
    (build, os_name)
}

impl Engine {
    // -----------------------------------------------------------------------
    // Agent info cache
    // -----------------------------------------------------------------------

    fn refresh_agent_info(&mut self) {
        let mut info = Vec::with_capacity(AgentId::ALL.len());
        for agent in AgentId::ALL {
            let (detected, detect_detail) = crate::agents::detect(agent);
            let st = installers::status(agent);
            info.push(AgentInfo {
                id: agent,
                detected,
                detect_detail,
                installed: st.installed,
                needs_repair: st.needs_repair,
                supported: installers::supported(agent),
                install_file: st.file,
                install_detail: st.detail,
            });
        }
        self.agent_info = info;
        self.agent_info_at = Some(Instant::now());
    }

    // -----------------------------------------------------------------------
    // Watching
    // -----------------------------------------------------------------------

    /// The single Overview master toggle now gates every agent.
    pub fn is_watched(&self, _agent: AgentId) -> bool {
        self.settings.watching
    }

    fn watch_test(&self) -> impl Fn(AgentId) -> bool + '_ {
        |agent: AgentId| self.is_watched(agent)
    }

    pub fn working_agents(&self) -> usize {
        AgentId::ALL
            .iter()
            .filter(|a| self.is_watched(**a) && self.registry.working_count(**a) > 0)
            .count()
    }

    pub fn watched_agents(&self) -> usize {
        AgentId::ALL.iter().filter(|a| self.is_watched(**a)).count()
    }

    // -----------------------------------------------------------------------
    // Manual session control
    // -----------------------------------------------------------------------

    pub fn start_manual_secs(&mut self, secs: u64) {
        self.manual.start_secs(self.clock_now(), secs.max(1));
        self.lid_cap_tripped = false;
        self.dirty = true;
    }

    /// Resolve "Until HH:MM" against the local zone (DST-aware) and start.
    pub fn start_manual_until(&mut self, hour: u32, minute: u32) -> until::UntilTarget {
        let now = chrono::Local::now();
        let target = until::resolve_until(&chrono::Local, &now, hour, minute);
        let secs = target.seconds.max(1) as u64;
        self.manual.start_until(self.clock_now(), secs);
        self.lid_cap_tripped = false;
        self.dirty = true;
        target
    }

    pub fn start_manual_unlimited(&mut self) {
        self.manual.start_unlimited(self.clock_now());
        self.lid_cap_tripped = false;
        self.dirty = true;
    }

    pub fn stop_manual(&mut self) {
        self.manual.stop();
        self.dirty = true;
    }

    pub fn extend_manual(&mut self, secs: u64) {
        self.manual.extend(secs);
        self.dirty = true;
    }

    /// Pause releases everything, including a running manual session.
    pub fn set_paused(&mut self, paused: bool) {
        if paused && !self.paused {
            self.manual.stop();
        }
        self.paused = paused;
        self.paused_until_unix = None;
        self.dirty = true;
    }

    /// Timed pause (e.g. 30 minutes / 1 hour). Releases everything and resumes
    /// automatically when the timer expires, or immediately on `set_paused(false)`.
    pub fn pause_for(&mut self, secs: u64) {
        self.manual.stop();
        self.paused = true;
        self.paused_until_unix = Some(self.clock_now().saturating_add(secs.max(1)));
        self.dirty = true;
    }

    pub fn clear_lid_cap(&mut self) {
        self.lid_cap_tripped = false;
        self.dirty = true;
    }

    /// Put the lid action back immediately (used by the Reliability card).
    pub fn release_lid_now(&mut self) {
        if let Some(hold) = self.lid_hold.take()
            && let Err(e) = lid::restore(&RealPowerApi, &hold)
        {
            self.lid_error = Some(e);
        }
        self.lid_engaged_unix = None;
        self.dirty = true;
    }

    // -----------------------------------------------------------------------
    // Polling
    // -----------------------------------------------------------------------

    fn poll_battery(&mut self, now: Instant) {
        let broadcast = POWER_DIRTY.swap(false, Ordering::Relaxed);
        let elapsed = self
            .battery_at
            .map(|t| now.duration_since(t) >= BATTERY_POLL)
            .unwrap_or(true);
        if broadcast || elapsed {
            self.battery = battery::poll();
            self.battery_at = Some(now);
        }
    }

    fn poll_processes(&mut self, now: Instant) {
        let agents = AgentId::ALL.to_vec();
        let (detections, alive) = self.detector.poll(&agents, now);
        self.alive = alive;
        self.registry.on_process(&detections, now);
        self.process_at = Some(now);
    }

    // -----------------------------------------------------------------------
    // Guards
    // -----------------------------------------------------------------------

    fn guards(&self, lid_cap_exceeded: bool) -> Guards {
        Guards {
            plugged_in: self.battery.plugged_in,
            battery_percent: self.battery.percent,
            saver_on: self.battery.saver_on,
            lid_proof: self.settings.lid_proof,
            lid_cap_exceeded,
            battery_floor: self.settings.battery_floor_percent,
        }
    }

    fn lid_capped(&self, now_unix: u64, base_want: bool, battery_blocked: bool) -> bool {
        // Nothing is holding the machine awake, so the cap cannot matter.
        if !base_want || battery_blocked || !self.settings.lid_proof {
            return false;
        }
        // While a hold is still wanted, the latch keeps the cap tripped: it is
        // cleared in `tick` once the hold is released, not here.
        if self.lid_cap_tripped {
            return true;
        }
        let Some(started) = self.lid_engaged_unix else {
            return false;
        };
        if self.lid_hold.is_none() {
            return false;
        }
        now_unix.saturating_sub(started) >= u64::from(self.lid_cap_hours) * 3_600
    }

    // -----------------------------------------------------------------------
    // Main tick
    // -----------------------------------------------------------------------

    /// The current wall-clock second, or the test override when one is set.
    fn clock_now(&self) -> u64 {
        self.unix_override.unwrap_or_else(now_unix)
    }

    pub fn tick(&mut self, now: Instant) -> TickOutcome {
        let now_u = self.clock_now();

        if self.paused
            && let Some(until) = self.paused_until_unix
            && now_u >= until
        {
            self.paused = false;
            self.paused_until_unix = None;
            self.dirty = true;
        }

        self.poll_battery(now);

        if self.process_detection
            && self
                .process_at
                .map(|t| now.duration_since(t) >= PROCESS_POLL)
                .unwrap_or(true)
        {
            self.poll_processes(now);
        }

        // Install/detection state touches the disk (PATH walks, config file
        // reads): poll fast only while something is actually working; when
        // fully idle the drift is invisible and the idle cost matters.
        let info_interval = if self.manual.active || self.registry.any_working(|_| true) {
            AGENT_INFO_POLL
        } else {
            AGENT_INFO_IDLE_POLL
        };
        if self
            .agent_info_at
            .map(|t| now.duration_since(t) >= info_interval)
            .unwrap_or(true)
        {
            self.refresh_agent_info();
        }

        self.registry.tick(
            Duration::from_secs(self.stale_timeout_min * 60),
            &self.alive,
            now,
        );

        if self.manual.expired(now_u) {
            self.manual.stop();
        }

        // --- decision -----------------------------------------------------
        let grace = Duration::from_secs(self.grace_secs);
        let agents_working = {
            let watch = self.watch_test();
            any_working_or_in_grace(&self.registry, watch, grace, &self.alive, now)
        };
        // Release floor: once an agent-driven hold is up, the agent signal
        // must stay dark for RELEASE_FLOOR before the hold drops. Hook
        // drops, row-removal races and one-sample detector glitches cannot
        // flip an active hold on their own anymore; a system that never held
        // releases instantly, and a lapsed manual session drops at once.
        let agent_signal = if agents_working {
            self.agent_dark_since = None;
            true
        } else if (self.hold_started_at.is_some() || self.user_visible_engaged)
            && !self.last.manual.active
        {
            let dark_since = *self.agent_dark_since.get_or_insert(now);
            now.duration_since(dark_since) < RELEASE_FLOOR
        } else {
            self.agent_dark_since = None;
            false
        };
        let base_want =
            self.manual.active || (self.settings.watching && !self.paused && agent_signal);

        let battery_blocked = self.guards(false).blocker().is_some();
        let lid_cap_exceeded = self.lid_capped(now_u, base_want, battery_blocked);
        // The lid safety cap only retires the lid override (see
        // `should_release_lid`); it must never veto the keep-awake hold
        // itself, or a single long session would drop the hold while agents
        // are still working. Only battery guards stop the hold.
        let blocker = self.guards(false).blocker();
        let want = base_want && blocker.is_none();

        // Lid-proof can be gated on AC power. Unplugging restores the lid
        // action while the normal keep-awake hold continues; plugging back in
        // re-engages the override on the next tick.
        let charger_ok = !self.settings.require_charger_for_lid || self.battery.plugged_in;

        if lid_cap_exceeded && !self.lid_cap_tripped {
            self.lid_cap_tripped = true;
        }
        if !base_want || battery_blocked {
            self.lid_cap_tripped = false;
        }

        // --- apply --------------------------------------------------------
        let working_agents = self.working_agents();
        // Remember when the hold ended so the DISENGAGE line can report how
        // long it lasted (the release branch used to clear it first, making
        // every line say `held_for=0s`).
        let mut released_since_unix: Option<u64> = None;

        if want {
            // Fix for the reported lid bug: a hold that is still wanted must
            // still release a stale lid override the moment lid-proof is turned
            // off (or the lid vanishes, or the cap trips). Previously the
            // `if want` branch could only ever *engage* the lid hold, so the
            // "Do nothing" lid action lingered until the whole hold ended.
            if should_release_lid(
                self.settings.lid_proof,
                self.capabilities.lid_present,
                lid_cap_exceeded,
                self.lid_hold.is_some(),
                charger_ok,
            ) {
                self.release_lid_now();
            }

            if self.engaged_since_unix.is_none() {
                self.engaged_since_unix = Some(now_u);
            }
            if self.hold_started_at.is_none() {
                self.hold_started_at = Some(now);
            }
            let reason = power::reason_string(self.manual.active, working_agents);
            match self.blocker.engage(&reason, self.settings.keep_display_on) {
                Ok(()) => self.blocker_error = None,
                Err(e) => self.blocker_error = Some(e),
            }

            if self.settings.lid_proof
                && self.capabilities.lid_present
                && !lid_cap_exceeded
                && charger_ok
                && self.lid_hold.is_none()
            {
                match lid::engage(&RealPowerApi) {
                    Ok(hold) => {
                        self.lid_hold = Some(hold);
                        self.lid_engaged_unix = Some(now_u);
                        self.lid_error = None;
                        // One watchdog per app run: it blocks on our PID and
                        // restores from the session state file if we die while
                        // the override is up. Normal exits delete that file
                        // first, so the watchdog no-ops.
                        if !self.lid_watchdog_spawned {
                            self.lid_watchdog_spawned = crate::watchdog::spawn_detached(
                                &crate::paths::session_state_path(),
                            );
                            crate::log::append(
                                "LID",
                                if self.lid_watchdog_spawned {
                                    "lid watchdog spawned"
                                } else {
                                    "lid watchdog failed to spawn"
                                },
                            );
                        }
                    }
                    Err(e) => {
                        self.lid_error = Some(e);
                        // Do not thrash: back off for a whole cap period.
                        self.lid_engaged_unix = None;
                    }
                }
            }
        } else {
            self.blocker.release();
            released_since_unix = self.engaged_since_unix.take();
            if self.lid_hold.is_some() {
                self.release_lid_now();
            }
        }

        // --- transitions --------------------------------------------------
        let is_user_visible = if want {
            if self.manual.active {
                true
            } else if let Some(started) = self.hold_started_at {
                now.duration_since(started) >= VISIBILITY_HOLD_DELAY
            } else {
                false
            }
        } else {
            false
        };

        let mut notifications = Vec::new();
        // One chime per episode: within NOTIFY_EPISODE_DEBOUNCE of the last
        // chime neither another engage nor a matching finish re-fires sound
        // or toast. The orb/UI still flips — only the noise is gated, so a
        // burst of short holds is visible but silent. Manual holds always
        // notify; battery releases always notify.
        let episode_open = self
            .last_notified_at
            .map(|t| now.duration_since(t) < NOTIFY_EPISODE_DEBOUNCE)
            .unwrap_or(false);
        if is_user_visible && !self.user_visible_engaged {
            self.user_visible_engaged = true;
            if self.manual.active || !episode_open {
                notifications.push(Notify::Engaged);
                self.last_notified_at = Some(now);
            }
            let reason = power::reason_string(self.manual.active, working_agents);
            let mut matched_pids: Vec<String> = self
                .registry
                .all_sessions()
                .iter()
                .filter(|s| s.state == SessionState::Working && !s.stale)
                .map(|s| format!("{}:{:?}(folder={})", s.agent.id(), s.pid, s.folder))
                .collect();
            if matched_pids.is_empty() && !self.manual.active {
                matched_pids = self
                    .registry
                    .in_grace(|_| true, grace, &self.alive, now)
                    .iter()
                    .map(|s| {
                        format!(
                            "{}:{:?}(folder={},grace)",
                            s.agent.id(),
                            s.pid,
                            s.folder
                        )
                    })
                    .collect();
            }
            crate::log::append(
                "DECISION",
                &format!(
                    "ENGAGE: reason='{reason}', matched=[{}]",
                    matched_pids.join(", ")
                ),
            );
        } else if !is_user_visible && self.user_visible_engaged {
            self.user_visible_engaged = false;
            let held_secs = released_since_unix
                .map(|since| now_u.saturating_sub(since))
                .unwrap_or(0);
            if blocker.is_some() {
                notifications.push(Notify::Battery);
                self.last_notified_at = Some(now);
            } else if !self.paused && !episode_open {
                notifications.push(Notify::Finished);
                self.last_notified_at = Some(now);
            }
            crate::log::append("DECISION", &format!("DISENGAGE: held_for={}s", held_secs));
            self.hold_started_at = None;
        } else if !want && self.hold_started_at.is_some() {
            let held_ms = self
                .hold_started_at
                .map(|t| now.duration_since(t).as_millis())
                .unwrap_or(0);
            crate::log::append(
                "DECISION",
                &format!("DISENGAGE_EARLY (silent, <1s): held_for={}ms", held_ms),
            );
            self.hold_started_at = None;
        }
        // --- snapshot -----------------------------------------------------
        let snapshot = self.build_snapshot(now, now_u, want, blocker, working_agents);
        let emit = self.should_emit(now, &snapshot);
        let chrome_changed = self.push_chrome(&snapshot);

        self.last = snapshot.clone();
        TickOutcome {
            snapshot,
            emit,
            chrome_changed,
            notifications,
        }
    }

    // -----------------------------------------------------------------------
    // Snapshot construction
    // -----------------------------------------------------------------------

    fn build_snapshot(
        &self,
        now: Instant,
        now_u: u64,
        want: bool,
        blocker: Option<Guard>,
        working_agents: usize,
    ) -> Snapshot {
        // `engaged` mirrors the actual OS request, not just intent: if the
        // power request failed, the UI must not claim the PC is held awake.
        let engaged = self.user_visible_engaged && self.blocker.is_engaged();
        let engaged_for_secs = if engaged {
            self.engaged_since_unix
                .map(|since| now_u.saturating_sub(since))
                .unwrap_or(0)
        } else {
            0
        };
        let paused_remaining_secs = if self.paused {
            self.paused_until_unix
                .map(|until| until.saturating_sub(now_u))
        } else {
            None
        };
        let paused_countdown = paused_remaining_secs.map(clock);

        let title = snapshot::taskbar_title(engaged, self.paused, working_agents);
        let reason = snapshot::reason_line(
            engaged,
            self.paused,
            self.manual.active,
            working_agents,
            self.settings.watching,
        );

        let mut snapshot = Snapshot {
            unix_ms: now_u.saturating_mul(1_000),
            engaged,
            engaged_for_secs,
            want_awake: want,
            paused: self.paused,
            paused_remaining_secs,
            paused_countdown,
            watching: self.settings.watching,
            agents_working: working_agents,
            watched_agents: self.watched_agents(),
            blocker,
            blocker_label: blocker.map(|b| b.label().to_string()),
            blocker_detail: blocker.map(|b| b.detail(self.settings.battery_floor_percent)),
            reason,
            status_word: if self.paused {
                "Paused".to_string()
            } else if engaged {
                "Awake".to_string()
            } else if self.settings.watching {
                "Active".to_string()
            } else {
                "Idle".to_string()
            },
            title,
            battery: BatterySnapshot {
                present: self.battery.present,
                percent: self.battery.percent,
                plugged_in: self.battery.plugged_in,
                saver_on: self.battery.saver_on,
                rule: blocker.map(|b| b.label().to_string()),
                percent_label: match self.battery.percent {
                    Some(p) if self.battery.present => format!("{p}%"),
                    _ if self.battery.plugged_in => "AC power".to_string(),
                    _ => "Unknown".to_string(),
                },
            },
            lid: self.lid_snapshot(now_u),
            display: self.display_snapshot(),
            capabilities: self.capabilities.clone(),
            sessions: self.session_snapshots(now),
            agents: self.agent_snapshots(),
            settings: self.settings.clone(),
            ..Snapshot::default()
        };
        snapshot.set_manual_from(&self.manual, now_u);
        snapshot
    }

    fn lid_snapshot(&self, now_u: u64) -> LidSnapshot {
        let held = self.lid_hold.is_some();
        let held_secs = self
            .lid_engaged_unix
            .map(|s| now_u.saturating_sub(s))
            .unwrap_or(0);
        let cap_secs = u64::from(self.lid_cap_hours) * 3_600;
        let note = if !self.capabilities.lid_present {
            "This machine reports no lid switch. Lid-proof is unavailable here.".to_string()
        } else if let Some(err) = self.lid_error.as_deref() {
            format!("Lid-proof failed: {err}")
        } else if self.lid_cap_tripped {
            Guard::LidCap.detail(self.settings.battery_floor_percent)
        } else if held {
            format!(
                "Lid close is temporarily set to \"Do nothing\" and will be restored automatically ({} left).",
                clock(cap_secs.saturating_sub(held_secs))
            )
        } else if self.settings.lid_proof {
            "Armed: the next hold also changes the lid-close action.".to_string()
        } else {
            "Off: closing the lid follows your Windows power plan.".to_string()
        };
        LidSnapshot {
            present: self.capabilities.lid_present,
            proof_enabled: self.settings.lid_proof,
            held,
            modern_standby: self.capabilities.modern_standby,
            cap_hours: self.lid_cap_hours,
            held_secs,
            cap_remaining_secs: if held {
                cap_secs.saturating_sub(held_secs)
            } else {
                cap_secs
            },
            closed: match LID_CLOSED.load(Ordering::Relaxed) {
                1 => Some(true),
                2 => Some(false),
                _ => None,
            },
            note,
        }
    }

    /// Display power is only ever touched when the `keep_display_on` option is
    /// on: Ember then holds a `DisplayRequired` power request for as long as it
    /// holds the PC awake. Otherwise the display follows the Windows plan.
    fn display_snapshot(&self) -> DisplaySnapshot {
        let keep = self.settings.keep_display_on;
        DisplaySnapshot {
            policy_label: if keep {
                "The screen stays on".to_string()
            } else {
                "Windows manages the display".to_string()
            },
            display_kept_on: self.blocker.is_engaged() && keep,
            turned_off: false,
            note: if keep {
                "Ember also keeps the display awake while it holds the PC.".to_string()
            } else {
                "Windows may turn the screen off while Ember keeps the PC awake.".to_string()
            },
        }
    }

    fn session_snapshots(&self, now: Instant) -> Vec<SessionSnapshot> {
        self.registry
            .all_sessions()
            .into_iter()
            .map(|s| {
                let def = s.agent.def();
                SessionSnapshot {
                    agent: s.agent.id().to_string(),
                    agent_name: def.name.to_string(),
                    monogram: def.monogram.to_string(),
                    hue: def.hue,
                    session_id: s.session_id.clone(),
                    folder: s.folder.clone(),
                    state: s.state,
                    working_for_secs: now.duration_since(s.state_since).as_secs(),
                    last_event_unix: s.last_event_unix,
                    approximate: s.approximate,
                    stale: s.stale,
                    pid: s.pid,
                }
            })
            .collect()
    }

    fn agent_snapshots(&self) -> Vec<AgentSnapshot> {
        self.agent_info
            .iter()
            .map(|info| {
                let def = info.id.def();
                let sessions = self.registry.sessions_for(info.id);
                let last_event_unix = sessions.iter().map(|s| s.last_event_unix).max();
                AgentSnapshot {
                    id: info.id.id().to_string(),
                    name: def.name.to_string(),
                    monogram: def.monogram.to_string(),
                    hue: def.hue,
                    detected: info.detected,
                    detect_detail: info.detect_detail.clone(),
                    installed: info.installed,
                    needs_repair: info.needs_repair,
                    supported: info.supported,
                    install_file: info.install_file.clone(),
                    install_detail: info.install_detail.clone(),
                    mode: def.primary_mode(),
                    note: def.integration_note().to_string(),
                    watching: self.is_watched(info.id),
                    working: self.registry.working_count(info.id),
                    sessions: sessions.len(),
                    last_event_unix,
                }
            })
            .collect()
    }

    fn should_emit(&mut self, now: Instant, snapshot: &Snapshot) -> bool {
        let too_soon = self
            .last_emit
            .map(|t| now.duration_since(t) < EMIT_INTERVAL)
            .unwrap_or(false);
        if too_soon && !self.dirty {
            return false;
        }
        let changed = self.dirty || snapshot.signature() != self.last.signature();
        if !changed {
            return false;
        }
        self.last_emit = Some(now);
        self.dirty = false;
        true
    }

    /// Returns true when the window title changed.
    fn push_chrome(&mut self, snapshot: &Snapshot) -> bool {
        let title = snapshot.window_title();
        if title == self.last_chrome {
            return false;
        }
        self.last_chrome = title;
        true
    }

    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// The most recent snapshot, without running a tick.
    pub fn current_snapshot(&self) -> Snapshot {
        self.last.clone()
    }

    /// Force the slow-moving agent/install information to be recomputed.
    pub fn force_agent_refresh(&mut self) {
        self.refresh_agent_info();
        self.dirty = true;
    }

    pub fn sleep_blocker(&self) -> &SleepBlocker {
        &self.blocker
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Pure decision for whether a currently-held lid override has to be released
/// even while Ember keeps holding the PC awake.
///
/// The lid override is only warranted while lid-proof is on, the machine has a
/// lid, the safety cap has not tripped, and (when required) the charger is
/// connected. The moment any of those stops being true the override must go —
/// that is the bug where turning lid-proof off mid-hold left the Windows lid
/// action on "Do nothing".
///
/// Kept free of the real power API so it can be unit-tested without touching
/// the machine's power scheme.
pub fn should_release_lid(
    lid_proof: bool,
    lid_present: bool,
    cap_exceeded: bool,
    has_hold: bool,
    charger_ok: bool,
) -> bool {
    has_hold && (!lid_proof || !lid_present || cap_exceeded || !charger_ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::registry::{HookEvent, HookMessage};

    fn engine() -> Engine {
        let mut e = Engine::new();
        // Never let the real machine influence a test: no process detection,
        // no leftover sessions, and agent info pre-filled.
        e.process_detection = false;
        e.registry = Registry::new();
        e.alive.clear();
        e.process_at = Some(Instant::now());
        e.agent_info = AgentId::ALL
            .iter()
            .map(|a| AgentInfo {
                id: *a,
                detected: true,
                detect_detail: "test".into(),
                installed: false,
                needs_repair: false,
                supported: installers::supported(*a),
                install_file: None,
                install_detail: "test".into(),
            })
            .collect();
        e.agent_info_at = Some(Instant::now());
        // Start every test from a neutral power picture; tests that care about
        // a guard switch it on explicitly.
        e.battery = BatteryStatus {
            present: false,
            percent: None,
            plugged_in: true,
            saver_on: false,
        };
        e.battery_at = Some(Instant::now());
        POWER_DIRTY.store(false, Ordering::Relaxed);
        e
    }

    /// Freeze the battery reading so `tick` does not re-read the real machine.
    fn freeze_battery(e: &mut Engine, now: Instant) {
        e.battery_at = Some(now);
        POWER_DIRTY.store(false, Ordering::Relaxed);
    }

    #[test]
    fn a_clean_engine_starts_idle_and_does_not_hold_the_pc_awake() {
        let mut e = engine();
        e.settings.watching = false;
        let out = e.tick(Instant::now());
        assert!(!out.snapshot.engaged);
        assert!(!out.snapshot.want_awake);
        assert_eq!(out.snapshot.status_word, "Idle");
        assert_eq!(out.snapshot.title, "Ember·Idle");
    }

    #[test]
    fn a_hook_makes_the_engine_engage_and_emit_engaged() {
        let mut e = engine();
        e.settings.watching = true;
        let now = Instant::now();
        e.registry.on_hook(
            HookMessage {
                agent: AgentId::ClaudeCode,
                event: HookEvent::Working,
                session_id: Some("s1".into()),
                cwd: Some("C:\\work\\proj".into()),
                pid: None,
                ts: 0,
            },
            now,
        );
        // Under 1 second, visibility gate holds back user-visible state and notifications
        let out0 = e.tick(now);
        assert!(out0.notifications.is_empty());
        assert!(!out0.snapshot.engaged);

        // At 1 second of sustained activity, visibility gate and Notify::Engaged trigger together
        let out = e.tick(now + Duration::from_secs(1));
        assert_eq!(out.notifications, vec![Notify::Engaged]);
        assert!(out.snapshot.engaged);
        assert_eq!(out.snapshot.agents_working, 1);
        assert_eq!(out.snapshot.title, "Ember·Awake·1 agent working");
        assert!(out.snapshot.sessions.iter().any(|s| s.folder == "proj"));

        // Going idle inside the grace window keeps the hold...
        e.registry.on_hook(
            HookMessage {
                agent: AgentId::ClaudeCode,
                event: HookEvent::Idle,
                session_id: Some("s1".into()),
                cwd: None,
                pid: None,
                ts: 0,
            },
            now + Duration::from_secs(5),
        );
        e.grace_secs = 15;
        let out = e.tick(now + Duration::from_secs(10));
        assert!(!out.notifications.contains(&Notify::Finished));

        // ...and the release after the grace window (plus the floor) is
        // silent: the episode chimed once at +1s, so its finish chime is
        // suppressed inside the episode window — only the state drops.
        let out = e.tick(now + Duration::from_secs(30));
        assert!(!out.notifications.contains(&Notify::Finished));
        let out = e.tick(now + Duration::from_secs(34));
        assert!(!out.snapshot.engaged, "grace + floor lapsed: released");
        assert!(
            out.notifications.is_empty(),
            "short episode stays silent end to end"
        );
    }

    #[test]
    fn long_hold_past_the_episode_window_finishes_with_a_chime() {
        let mut e = engine();
        e.settings.watching = true;
        let now = Instant::now();
        e.registry.on_hook(
            HookMessage {
                agent: AgentId::ClaudeCode,
                event: HookEvent::Working,
                session_id: Some("s1".into()),
                cwd: None,
                pid: None,
                ts: 0,
            },
            now,
        );
        e.tick(now);
        let out = e.tick(now + Duration::from_secs(1));
        assert_eq!(out.notifications, vec![Notify::Engaged]);

        // The turn keeps running well past the episode window (a live,
        // working session never releases), then ends at +120s.
        e.registry.on_hook(
            HookMessage {
                agent: AgentId::ClaudeCode,
                event: HookEvent::End,
                session_id: Some("s1".into()),
                cwd: None,
                pid: None,
                ts: 0,
            },
            now + Duration::from_secs(120),
        );
        // Tombstone grace (15s) + release floor (3s) delay the drop.
        e.tick(now + Duration::from_secs(120));
        let out = e.tick(now + Duration::from_secs(140));
        assert!(!out.snapshot.engaged);
        assert!(
            out.notifications.contains(&Notify::Finished),
            "a long hold past the episode window chimes when it finishes"
        );
    }

    #[test]
    fn pausing_stops_a_manual_session_and_releases_everything() {
        let mut e = engine();
        let now = Instant::now();
        e.start_manual_unlimited();
        let out = e.tick(now);
        assert!(out.snapshot.engaged);
        assert!(out.snapshot.manual.active);

        e.set_paused(true);
        let out = e.tick(now + Duration::from_secs(1));
        assert!(!out.snapshot.engaged);
        assert!(
            !out.snapshot.manual.active,
            "pause stops the manual session"
        );
        assert_eq!(out.snapshot.title, "Ember·Paused");
        assert_eq!(out.snapshot.status_word, "Paused");

        e.set_paused(false);
        let out = e.tick(now + Duration::from_secs(2));
        assert!(!out.snapshot.engaged);
        assert_eq!(out.snapshot.title, "Ember·Idle");
    }

    #[test]
    fn manual_timer_expires_and_drops_the_hold() {
        let mut e = engine();
        let now = Instant::now();
        // Manual timers are wall-clock based (so "Until" can be DST-aware), so
        // pin the clock: `Instant` deltas alone would never expire them.
        e.unix_override = Some(1_000);
        e.start_manual_secs(61);

        assert!(e.tick(now).snapshot.engaged);
        e.unix_override = Some(1_060);
        assert!(e.tick(now + Duration::from_secs(1)).snapshot.engaged);
        e.unix_override = Some(1_061);
        let out = e.tick(now + Duration::from_secs(2));
        assert!(!out.snapshot.engaged);
        assert!(!out.snapshot.manual.active);
    }

    #[test]
    fn manual_until_uses_the_dst_aware_resolver() {
        let mut e = engine();
        let target = e.start_manual_until(23, 59);
        assert!(target.seconds > 0);
        assert!(target.seconds <= 86_400);
        assert!(target.label.starts_with("Today") || target.label.starts_with("Tomorrow"));
        assert_eq!(e.manual.mode, ManualMode::Until);
    }

    #[test]
    fn a_battery_guard_releases_the_hold_and_notifies() {
        let mut e = engine();
        let now = Instant::now();
        freeze_battery(&mut e, now);
        e.battery.present = true;
        e.battery.percent = Some(50);
        e.battery.plugged_in = false;
        e.start_manual_unlimited();
        assert!(e.tick(now).snapshot.engaged);

        // The battery drains below the fixed 15% floor.
        e.battery.percent = Some(9);
        let out = e.tick(now + Duration::from_secs(2));
        assert!(!out.snapshot.engaged);
        assert_eq!(out.snapshot.blocker, Some(Guard::BatteryLow));
        assert!(out.notifications.contains(&Notify::Battery));
        assert!(out.snapshot.blocker_detail.is_some());
    }

    #[test]
    fn battery_floor_and_saver_are_reported_with_reasons() {
        let mut e = engine();
        let now = Instant::now();
        freeze_battery(&mut e, now);
        e.battery.present = true;
        e.battery.percent = Some(10);
        e.start_manual_unlimited();
        let out = e.tick(now);
        assert!(!out.snapshot.engaged);
        assert_eq!(out.snapshot.blocker, Some(Guard::BatteryLow));
        assert!(out.snapshot.blocker_detail.unwrap().contains("below"));

        e.battery.percent = Some(90);
        e.battery.saver_on = true;
        let out = e.tick(now + Duration::from_secs(1));
        assert_eq!(out.snapshot.blocker, Some(Guard::BatterySaver));
    }

    #[test]
    fn lid_cap_latches_until_it_is_cleared() {
        let mut e = engine();
        e.settings.lid_proof = true;
        e.lid_cap_hours = 1;
        // Pretend a hold has been running for two hours. The hold is never
        // passed to the real power API in this test.
        e.lid_hold = Some(LidHold {
            scheme: crate::lid::GuidData::from_guid(&windows::core::GUID::from_u128(1)),
            ac: 1,
            dc: 1,
            started_unix: 0,
            owner_pid: 0,
        });
        e.lid_engaged_unix = Some(now_unix().saturating_sub(7_200));

        let now = now_unix();
        assert!(e.lid_capped(now, true, false), "2h past a 1h cap");
        e.lid_cap_tripped = true;
        assert!(
            e.lid_capped(now, true, false),
            "the latch keeps the cap tripped"
        );
        assert!(
            !e.lid_capped(now, false, false),
            "nothing wants awake, so the cap no longer matters"
        );

        e.clear_lid_cap();
        assert!(!e.lid_cap_tripped);
    }

    #[test]
    fn the_lid_cap_only_releases_the_lid_override_not_the_whole_hold() {
        // The critical review finding: once the cap tripped, `want` collapsed
        // and the OS hold was dropped while agents were still working — the
        // machine could sleep mid-session. The cap must only hand the lid
        // action back; the keep-awake hold continues.
        let mut e = engine();
        e.settings.lid_proof = true;
        e.capabilities.lid_present = true;
        e.lid_cap_hours = 1;
        e.start_manual_unlimited();
        e.lid_hold = Some(LidHold {
            scheme: crate::lid::GuidData::from_guid(&windows::core::GUID::from_u128(1)),
            ac: 1,
            dc: 1,
            started_unix: 0,
            owner_pid: 0,
        });
        e.lid_engaged_unix = Some(now_unix().saturating_sub(7_200));

        let out = e.tick(Instant::now());
        assert!(
            out.snapshot.want_awake,
            "the keep-awake hold continues past the lid cap"
        );
        assert!(out.snapshot.engaged);
        assert!(e.lid_cap_tripped, "the cap latch stays tripped");
        assert!(
            !out.snapshot.lid.held,
            "the lid override itself was released"
        );
        assert!(e.lid_hold.is_none());
    }

    #[test]
    fn a_machine_without_a_lid_never_reports_a_hold() {
        let mut e = engine();
        e.capabilities.lid_present = false;
        e.settings.lid_proof = true;
        e.start_manual_unlimited();
        let out = e.tick(Instant::now());
        assert!(out.snapshot.lid.note.contains("no lid switch"));
        assert!(!out.snapshot.lid.held);
    }

    #[test]
    fn snapshots_are_emitted_at_most_once_a_second() {
        let mut e = engine();
        let now = Instant::now();
        // Anchor the countdown to a pinned wall clock so "a new second" is
        // deterministic without sleeping.
        e.unix_override = Some(10_000);
        e.start_manual_secs(600);

        assert!(e.tick(now).emit, "a real change emits");
        assert!(!e.tick(now + Duration::from_millis(200)).emit);
        assert!(!e.tick(now + Duration::from_millis(900)).emit);
        // The countdown changed, and a second has passed, so it emits again.
        e.unix_override = Some(10_001);
        assert!(e.tick(now + Duration::from_millis(1_100)).emit);
    }

    #[test]
    fn chrome_changes_only_when_the_title_changes() {
        let mut e = engine();
        let now = Instant::now();
        let first = e.tick(now);
        assert!(first.chrome_changed);
        assert!(!e.tick(now + Duration::from_secs(1)).chrome_changed);
    }

    #[test]
    fn turning_lid_proof_off_mid_hold_releases_the_lid_override() {
        // The reported bug: switching lid-proof off while Ember is still holding
        // the PC awake used to leave the Windows lid action on "Do nothing"
        // until the whole hold ended. The pure decision below is what `tick`
        // consults at the top of its engage branch; here we prove it releases
        // the override even though the hold itself is still wanted.
        assert!(should_release_lid(false, true, false, true, true));
        assert!(should_release_lid(true, false, false, true, true));
        assert!(should_release_lid(true, true, true, true, true));
        assert!(!should_release_lid(true, true, false, true, true));
        assert!(!should_release_lid(false, true, false, false, true));

        // And the engine end of the contract: a wanted manual hold no longer
        // warrants a lid override once lid-proof is off.
        let mut e = engine();
        e.settings.lid_proof = false;
        e.capabilities.lid_present = true;
        e.start_manual_unlimited();
        let out = e.tick(Instant::now());
        assert!(out.snapshot.want_awake, "the manual hold is still wanted");
        assert!(
            should_release_lid(
                e.settings.lid_proof,
                e.capabilities.lid_present,
                false,
                true,
                true,
            ),
            "a held lid override must be released even while the hold continues"
        );
    }

    #[test]
    fn lid_proof_stays_engaged_while_it_is_still_warranted() {
        assert!(!should_release_lid(true, true, false, true, true));
        assert!(!should_release_lid(true, true, false, false, true));
    }

    #[test]
    fn a_lid_hold_is_released_when_the_charger_requirement_is_unmet() {
        // With `require_charger_for_lid`, unplugging releases the override while
        // the plain keep-awake hold continues.
        assert!(should_release_lid(true, true, false, true, false));
        // Plugging back in keeps it (and re-engages on the next tick).
        assert!(!should_release_lid(true, true, false, true, true));
        // No hold means there is nothing to release, charger or not.
        assert!(!should_release_lid(true, true, false, false, false));
    }

    #[test]
    fn manual_sessions_work_with_every_agent_switched_off() {
        let mut e = engine();
        e.settings.watching = false;
        e.start_manual_secs(600);
        let out = e.tick(Instant::now());
        assert!(out.snapshot.engaged);
        assert_eq!(out.snapshot.reason, "Manual session");
    }

    #[test]
    fn timed_pause_counts_down_and_resumes_automatically() {
        let mut e = engine();
        let now = Instant::now();
        e.unix_override = Some(5_000);
        e.settings.watching = true;
        e.registry.on_hook(
            HookMessage {
                agent: AgentId::ClaudeCode,
                event: HookEvent::Working,
                session_id: Some("p1".into()),
                cwd: None,
                pid: None,
                ts: 0,
            },
            now,
        );
        e.tick(now);
        assert!(e.tick(now + Duration::from_secs(1)).snapshot.engaged);

        e.pause_for(1_800);
        let out = e.tick(now + Duration::from_secs(2));
        assert!(!out.snapshot.engaged);
        assert!(out.snapshot.paused);
        assert_eq!(out.snapshot.paused_remaining_secs, Some(1_800));
        assert_eq!(out.snapshot.paused_countdown.as_deref(), Some("30:00"));

        e.unix_override = Some(6_800);
        let out = e.tick(now + Duration::from_secs(3));
        assert!(!out.snapshot.paused, "timed pause expired");
        assert!(
            out.snapshot.want_awake,
            "working session wants awake after pause expires"
        );
        let out_engaged = e.tick(now + Duration::from_secs(4));
        assert!(
            out_engaged.snapshot.engaged,
            "working session re-engages after pause expires and 1s visibility gate"
        );
    }

    #[test]
    fn build_number_comparison_and_parsing() {
        assert_eq!(parse_build_number("19045"), Some(19045));
        assert_eq!(parse_build_number("22000"), Some(22000));
        assert_eq!(parse_build_number("22621"), Some(22621));
        assert_eq!(parse_build_number("26100"), Some(26100));
        assert_eq!(parse_build_number("26200"), Some(26200));
        assert_eq!(parse_build_number("11 (26100)"), Some(26100));
        assert_eq!(parse_build_number("10.0.22621"), Some(22621));

        assert!(!is_windows_11_or_greater(19045));
        assert!(is_windows_11_or_greater(22000));
        assert!(is_windows_11_or_greater(22621));
        assert!(is_windows_11_or_greater(26100));
        assert!(is_windows_11_or_greater(26200));
        assert!(is_windows_11_or_greater(28000));
    }

    #[test]
    fn fake_clock_startup_cpu_spike_never_engages() {
        use crate::agents::process::{CPU_SAMPLE_CAP, IDLE_HOLD, advance_hysteresis};
        let t0 = Instant::now();
        // Startup spike: a single reading far above the floor.
        let (working, energy, below) = advance_hysteresis(false, 0.0, None, 45.0, IDLE_HOLD, t0);
        // One sample alone is never enough.
        assert!(!working);
        assert_eq!(energy, CPU_SAMPLE_CAP);
        assert_eq!(below, None);

        // Next sample is quiet -> the capped spike decays away.
        let (working2, energy2, _) = advance_hysteresis(
            working,
            energy,
            below,
            0.5,
            IDLE_HOLD,
            t0 + Duration::from_secs(2),
        );
        assert!(!working2);
        assert_eq!(energy2, 0.0);
    }

    #[test]
    fn fake_clock_sustained_activity_engages_after_configured_time() {
        use crate::agents::process::{CPU_ENERGY, IDLE_HOLD, advance_hysteresis};
        let t0 = Instant::now();
        let (mut working, mut energy, mut below) = (false, 0.0, None);
        for i in 0..2u64 {
            let last = i == 1;
            (working, energy, below) = advance_hysteresis(
                working,
                energy,
                below,
                15.0 + i as f32,
                IDLE_HOLD,
                t0 + Duration::from_secs(2 * i),
            );
            assert_eq!(working, last, "engages exactly when the budget fills");
        }
        assert!(working, "sustained samples above the floor engage");
        assert_eq!(energy, CPU_ENERGY);
        assert_eq!(below, None);
    }

    #[test]
    fn engage_flips_and_notifies_at_one_second_and_subsecond_blips_are_silent() {
        // Sub-second blips (<1s) must produce neither an "Ember is awake" toast
        // nor a phantom "finished" toast, while real turns flip visible state
        // and notify together at 1s.
        let mut e = engine();
        e.settings.watching = true;
        let now = Instant::now();
        e.registry.on_hook(
            HookMessage {
                agent: AgentId::Codex,
                event: HookEvent::Working,
                session_id: Some("short".into()),
                cwd: Some("C:\\work\\boxed".into()),
                pid: None,
                ts: 0,
            },
            now,
        );

        // 0..900ms: still inside the 1s visibility delay -> silent.
        for ms in [0u64, 400, 900] {
            let out = e.tick(now + Duration::from_millis(ms));
            assert!(out.notifications.is_empty(), "no toast at {ms}ms");
            assert!(!out.snapshot.engaged, "not user-visible yet at {ms}ms");
        }

        // The agent goes idle before the 1s delay elapses.
        e.registry.on_hook(
            HookMessage {
                agent: AgentId::Codex,
                event: HookEvent::Idle,
                session_id: Some("short".into()),
                cwd: None,
                pid: None,
                ts: 0,
            },
            now + Duration::from_millis(900),
        );
        e.grace_secs = 0;
        let out = e.tick(now + Duration::from_millis(950));
        assert!(
            out.notifications.is_empty(),
            "a sub-second blip must not produce a phantom release toast"
        );
        assert!(!out.snapshot.engaged);
    }

    #[test]
    fn fake_clock_brief_flapping_stays_silent() {
        let mut e = engine();
        e.settings.watching = true;
        let now = Instant::now();

        // Agent begins working
        e.registry.on_hook(
            HookMessage {
                agent: AgentId::ClaudeCode,
                event: HookEvent::Working,
                session_id: Some("flap".into()),
                cwd: None,
                pid: None,
                ts: 0,
            },
            now,
        );
        let out1 = e.tick(now);
        // Under 1s: sleep blocker is engaged, but no toast, no chime, snapshot.engaged = false
        assert!(out1.notifications.is_empty());
        assert!(!out1.snapshot.engaged);

        // Tick at 500ms
        let out2 = e.tick(now + Duration::from_millis(500));
        assert!(out2.notifications.is_empty());
        assert!(!out2.snapshot.engaged);

        // Flap ends at 800ms before reaching 1s visibility threshold
        e.registry.on_hook(
            HookMessage {
                agent: AgentId::ClaudeCode,
                event: HookEvent::Idle,
                session_id: Some("flap".into()),
                cwd: None,
                pid: None,
                ts: 0,
            },
            now + Duration::from_millis(800),
        );
        e.grace_secs = 0;
        let out3 = e.tick(now + Duration::from_millis(900));
        // Flap ended early: silent, zero notifications, no finish tone, no engage tone!
        assert!(out3.notifications.is_empty());
        assert!(!out3.snapshot.engaged);
    }

    #[test]
    fn short_hold_bursts_flip_visible_state_without_rechiming() {
        let mut e = engine();
        e.settings.watching = true;
        e.grace_secs = 15;
        let now = Instant::now();

        // First turn engages at +1s and fires Notify::Engaged: the episode
        // starts here and its window is NOTIFY_EPISODE_DEBOUNCE (60s).
        e.registry.on_hook(
            HookMessage {
                agent: AgentId::ClaudeCode,
                event: HookEvent::Working,
                session_id: Some("s1".into()),
                cwd: None,
                pid: None,
                ts: 0,
            },
            now,
        );
        e.tick(now);
        let out_engage1 = e.tick(now + Duration::from_secs(1));
        assert!(out_engage1.snapshot.engaged);
        assert_eq!(out_engage1.notifications, vec![Notify::Engaged]);

        // The turn ends at +2s. The tombstone keeps the grace window alive and
        // the release floor adds 3s: still engaged, and nothing is announced.
        e.registry.on_hook(
            HookMessage {
                agent: AgentId::ClaudeCode,
                event: HookEvent::End,
                session_id: Some("s1".into()),
                cwd: None,
                pid: None,
                ts: 0,
            },
            now + Duration::from_secs(2),
        );
        let out_hold = e.tick(now + Duration::from_secs(2));
        assert!(out_hold.snapshot.engaged);
        assert!(out_hold.notifications.is_empty());

        // Release once grace + floor lapse (+18s+): still inside the episode,
        // so the finish chime is suppressed — only the visible state drops.
        let out_floor = e.tick(now + Duration::from_secs(18));
        assert!(out_floor.notifications.is_empty());
        let out_release = e.tick(now + Duration::from_secs(23));
        assert!(!out_release.snapshot.engaged);
        assert!(
            out_release.notifications.is_empty(),
            "short hold: finish chime suppressed inside the episode"
        );

        // Re-engage at +24s: visible flip after 1s, no engage chime either.
        e.registry.on_hook(
            HookMessage {
                agent: AgentId::ClaudeCode,
                event: HookEvent::Working,
                session_id: Some("s2".into()),
                cwd: None,
                pid: None,
                ts: 0,
            },
            now + Duration::from_secs(24),
        );
        e.tick(now + Duration::from_secs(24));
        let out_reengage = e.tick(now + Duration::from_secs(25));
        assert!(
            out_reengage.snapshot.engaged,
            "visible state still flips after 1s"
        );
        assert!(
            out_reengage.notifications.is_empty(),
            "still the same episode: no engage chime"
        );

        // The second hold ends quietly inside the same episode.
        e.registry.on_hook(
            HookMessage {
                agent: AgentId::ClaudeCode,
                event: HookEvent::End,
                session_id: Some("s2".into()),
                cwd: None,
                pid: None,
                ts: 0,
            },
            now + Duration::from_secs(26),
        );
        e.tick(now + Duration::from_secs(27));
        e.tick(now + Duration::from_secs(40));
        let out_quiet = e.tick(now + Duration::from_secs(45));
        assert!(!out_quiet.snapshot.engaged);
        assert!(out_quiet.notifications.is_empty());

        // A fresh engage well past the episode window (+70s) chimes again.
        e.registry.on_hook(
            HookMessage {
                agent: AgentId::ClaudeCode,
                event: HookEvent::Working,
                session_id: Some("s3".into()),
                cwd: None,
                pid: None,
                ts: 0,
            },
            now + Duration::from_secs(70),
        );
        e.tick(now + Duration::from_secs(70));
        let out_fresh = e.tick(now + Duration::from_secs(71));
        assert!(out_fresh.snapshot.engaged);
        assert_eq!(out_fresh.notifications, vec![Notify::Engaged]);
    }

    #[test]
    fn poll_cadence_constants_match_fast_detection_targets() {
        assert_eq!(VISIBILITY_HOLD_DELAY, Duration::from_secs(1));
        assert_eq!(NOTIFY_EPISODE_DEBOUNCE, Duration::from_secs(60));
        assert_eq!(RELEASE_FLOOR, Duration::from_secs(3));
        assert_eq!(AGENT_INFO_POLL, Duration::from_secs(5));
        assert_eq!(AGENT_INFO_IDLE_POLL, Duration::from_secs(60));
    }
}
