//! Process-tree detection — the fallback for agents with no hook mechanism.
//!
//! Polled every 2s. A matched process tree must survive the settle window
//! (`MIN_REPORT_AGE`) before it is reported at all, so short-lived helpers can
//! never flash a row into the session list. After settling it requires
//! sustained activity before it counts as "working": every sample above
//! [`CPU_FLOOR`] adds up to [`CPU_SAMPLE_CAP`] energy, the tree engages once
//! the accumulator holds [`CPU_ENERGY`], and every quiet sample removes a full
//! cap so isolated spikes decay instead of stacking. Once quiet, it remains in
//! working state for a 15s anti-flap idle hold. A row lives exactly as long as
//! its process: idle agents stay listed, and the row disappears one poll after
//! the process exits.
//!
//! Folder labels come from the working directories of the whole tree, read
//! straight out of each process's PEB (sysinfo cannot read cwds on Windows):
//! the agent itself often sits in its install directory ("cli",
//! "x86_64-pc-windows-msvc") while its shells and tools sit in the project,
//! so the most plausible project cwd wins the label.
//!
//! The gate input is deliberately the instantaneous per-poll tree reading, NOT
//! a window average: averaging smears a single 2s burst across three polls,
//! letting one spike satisfy the consecutive gate by itself.
//!
//! Process detection ignores everything for the first 10 seconds after launch and
//! requires at least two valid CPU refreshes before state is trusted.
//! Matching is strictly limited to exact executables and script entrypoints,
//! excluding shells, multiplexers, editors, WebView2, and Ember itself.
//!
//! Tree walks only follow *real* parent links. Windows records a parent PID once
//! at creation and never clears it, so a reused PID can make an unrelated
//! orphan (e.g. a Radeon helper) look like a child of an agent. Because a real
//! parent always starts at or before its child, a parent that started later is
//! treated as the end of the chain, keeping stray processes out of the tree sum.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use sysinfo::{Process, ProcessesToUpdate, System};

use super::{AgentId, folder_name};

/// The per-sample CPU energy model for the matched process tree, on the
/// sysinfo scale where 100 = one fully-used core (tree readings sum every
/// descendant). Calibrated Oct 2026 on a 12-core box: live idle trees sit at
/// 0–2.3 across minutes of sampling, so anything at or below [`CPU_FLOOR`]
/// adds no energy; genuinely working trees sit at ~7–13+, and even a
/// modestly-busy stream of samples accumulates. A single sample contributes at
/// most [`CPU_SAMPLE_CAP`] and one quiet sample removes exactly that much, so
/// an isolated spike can never engage on its own (a repeated spike/quiet
/// alternation always decays back to zero) while a sustained modest load
/// engages within [`CPU_ENERGY`] worth of samples.
pub const CPU_FLOOR: f32 = 2.0;
pub const CPU_SAMPLE_CAP: f32 = 4.0;
pub const CPU_ENERGY: f32 = 6.0;
pub const IDLE_HOLD: Duration = Duration::from_secs(15);
pub const STARTUP_IGNORE_WINDOW: Duration = Duration::from_secs(10);

/// Every matched process stays invisible for this long after it is first seen.
/// Transient helpers (one-shot launchers, updaters, spawn bursts) burn out
/// inside this window, so they can never flash a row into the session list.
pub const MIN_REPORT_AGE: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessDetection {
    pub agent: AgentId,
    pub pid: u32,
    pub folder: String,
    pub working: bool,
}

#[derive(Clone, Debug)]
pub struct Track {
    pub agent: AgentId,
    pub folder: String,
    /// Accumulated CPU energy (see [`CPU_FLOOR`] and friends).
    pub energy: f32,
    pub working: bool,
    pub below_since: Option<Instant>,
    /// First poll that saw this pid (the settle clock).
    pub first_seen: Instant,
}

/// Pure hysteresis step with the CPU energy gate.
pub fn advance_hysteresis(
    working: bool,
    energy: f32,
    below_since: Option<Instant>,
    sample_cpu: f32,
    idle_hold: Duration,
    now: Instant,
) -> (bool, f32, Option<Instant>) {
    let excess = (sample_cpu - CPU_FLOOR).clamp(0.0, CPU_SAMPLE_CAP);
    let next_energy = if excess > 0.0 {
        (energy + excess).min(CPU_ENERGY)
    } else {
        (energy - CPU_SAMPLE_CAP).max(0.0)
    };
    if !working {
        // Engage once the accumulator holds a full budget; a quiet sample
        // decays it, so isolated spikes fade instead of stacking.
        if next_energy >= CPU_ENERGY {
            return (true, next_energy, None);
        }
        return (false, next_energy, None);
    }
    if next_energy > 0.0 {
        return (true, next_energy, None);
    }
    match below_since {
        None => (true, 0.0, Some(now)),
        Some(since) => {
            if now.duration_since(since) >= idle_hold {
                (false, 0.0, None)
            } else {
                (true, 0.0, below_since)
            }
        }
    }
}

/// Whether a track belongs in the session list right now.
pub fn should_report(first_seen: Instant, now: Instant) -> bool {
    now.duration_since(first_seen) >= MIN_REPORT_AGE
}

/// One poll step for a matched process tree: settle window, sustained-activity
/// gate and anti-flap idle hold.
///
/// Inside the settle window the state machine is frozen (no counts, no report),
/// so a transient helper can never flash a row no matter how hot it burns.
/// Once admitted, the ordinary hysteresis applies, and the row then lives until
/// the process exits — idle agents stay listed.
pub fn step_track(track: &mut Track, sample_cpu: f32, now: Instant) {
    if now.duration_since(track.first_seen) >= MIN_REPORT_AGE {
        let (working, energy, below_since) = advance_hysteresis(
            track.working,
            track.energy,
            track.below_since,
            sample_cpu,
            IDLE_HOLD,
            now,
        );
        track.working = working;
        track.energy = energy;
        track.below_since = below_since;
    } else {
        track.working = false;
        track.energy = 0.0;
        track.below_since = None;
    }
}

/// The parent of `pid`, but only when the recorded link can be real.
///
/// Windows stores a process's parent PID once, at creation, and never updates
/// it. When that parent exits, its PID can be reused by an unrelated process,
/// which then looks like the parent of the orphan. A genuine parent always
/// starts at or before its child, so a "parent" that started after the child is
/// a stale link: return `None` so the walk stops instead of picking up a stray.
fn real_parent(sys: &System, pid: u32) -> Option<u32> {
    let child = sys.process(sysinfo::Pid::from_u32(pid))?;
    let parent_pid = child.parent()?.as_u32();
    let parent = sys.process(sysinfo::Pid::from_u32(parent_pid))?;
    if parent.start_time() > child.start_time() {
        return None;
    }
    Some(parent_pid)
}

pub struct ProcessDetector {
    sys: System,
    tracks: HashMap<u32, Track>,
    created_at: Instant,
    last_refresh: Option<Instant>,
    refreshes: u32,
    /// Locations that can never be a project folder (Windows, Program Files,
    /// per-user AppData and temp). Computed once at startup.
    skip_prefixes: Vec<PathBuf>,
}

impl Default for ProcessDetector {
    fn default() -> Self {
        ProcessDetector::new()
    }
}

impl ProcessDetector {
    pub fn new() -> Self {
        ProcessDetector {
            sys: System::new(),
            tracks: HashMap::new(),
            created_at: Instant::now(),
            last_refresh: None,
            refreshes: 0,
            skip_prefixes: install_prefixes(),
        }
    }

    /// Poll and return detections plus the set of live PIDs.
    pub fn poll(
        &mut self,
        agents: &[AgentId],
        now: Instant,
    ) -> (Vec<ProcessDetection>, HashSet<u32>) {
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        self.refreshes = self.refreshes.saturating_add(1);
        self.last_refresh = Some(now);

        let alive: HashSet<u32> = self.sys.processes().keys().map(|p| p.as_u32()).collect();

        // Identify Ember's own process tree so we never match ourselves.
        let self_pid = std::process::id();
        let mut self_tree = HashSet::new();
        self_tree.insert(self_pid);
        for pid in self.sys.processes().keys() {
            let p_u32 = pid.as_u32();
            let mut cursor = Some(p_u32);
            for _ in 0..12 {
                if let Some(curr) = cursor {
                    if curr == self_pid {
                        self_tree.insert(p_u32);
                        break;
                    }
                    cursor = real_parent(&self.sys, curr);
                } else {
                    break;
                }
            }
        }

        // Which agent does each process belong to?
        let mut owner: HashMap<u32, AgentId> = HashMap::new();
        for (pid, proc_) in self.sys.processes() {
            let pid = pid.as_u32();
            if self_tree.contains(&pid) {
                continue;
            }
            if let Some(agent) = match_agent(proc_, agents) {
                owner.insert(pid, agent);
            }
        }
        // Prune runtime-host wrappers (`node.exe ... codex.js` / `cline`) when
        // they already spawned the real native binary for the same agent.
        let mut wrapper_pids = HashSet::new();
        for (&pid, &agent) in &owner {
            let mut cursor = real_parent(&self.sys, pid);
            for _ in 0..4 {
                let Some(parent) = cursor else { break };
                if owner.get(&parent) == Some(&agent)
                    && let Some(parent_proc) = self.sys.process(sysinfo::Pid::from_u32(parent))
                {
                    let parent_exe = parent_proc
                        .exe()
                        .and_then(|p| p.file_name())
                        .map(|s| s.to_string_lossy().to_lowercase())
                        .unwrap_or_else(|| parent_proc.name().to_string_lossy().to_lowercase());
                    if RUNTIME_HOSTS.contains(&parent_exe.as_str()) {
                        wrapper_pids.insert(parent);
                    }
                }
                cursor = real_parent(&self.sys, parent);
            }
        }
        for wrapper in wrapper_pids {
            owner.remove(&wrapper);
        }


        // Sum CPU across each matched process and its descendants, and remember
        // which processes belong to which matched tree (for folder resolution).
        let mut tree_cpu: HashMap<u32, f32> = HashMap::new();
        let mut tree_members: HashMap<u32, Vec<u32>> = HashMap::new();
        for pid in owner.keys() {
            tree_cpu.insert(*pid, 0.0);
        }
        for (pid, proc_) in self.sys.processes() {
            let pid = pid.as_u32();
            if self_tree.contains(&pid) {
                continue;
            }
            let mut cursor = Some(pid);
            let mut hops = 0;
            while let Some(current) = cursor {
                if owner.contains_key(&current) {
                    *tree_cpu.entry(current).or_insert(0.0) += proc_.cpu_usage();
                    tree_members.entry(current).or_default().push(pid);
                    break;
                }
                hops += 1;
                if hops > 12 {
                    break;
                }
                cursor = real_parent(&self.sys, current);
            }
        }

        // Is CPU trustworthy yet? Ignore the first 10 seconds and need >= 2 refreshes.
        let trustworthy =
            now.duration_since(self.created_at) >= STARTUP_IGNORE_WINDOW && self.refreshes >= 2;

        // Update per-pid tracks.
        for (pid, agent) in &owner {
            let cpu = if trustworthy {
                tree_cpu.get(pid).copied().unwrap_or(0.0)
            } else {
                0.0
            };
            let folder = self.tree_folder(*pid, &tree_members);

            let track = self.tracks.entry(*pid).or_insert_with(|| Track {
                agent: *agent,
                folder: folder.clone(),
                energy: 0.0,
                working: false,
                below_since: None,
                first_seen: now,
            });
            // A pid reused by a different agent (or a tree that changed hands)
            // must start its history over: hysteresis from the previous owner
            // would otherwise leak into the new one.
            if track.agent != *agent {
                track.energy = 0.0;
                track.working = false;
                track.below_since = None;
                track.first_seen = now;
            }
            track.agent = *agent;
            track.folder = folder;

            // Instantaneous reading: one burst counts once. (A window average
            // would smear a single 2s spike across ~3 polls and satisfy the
            // energy gate by itself, pinning stopped runtimes as working.)
            step_track(track, cpu, now);
        }

        // Forget processes that exited or were pruned as runtime wrappers.
        self.tracks
            .retain(|pid, _| alive.contains(pid) && owner.contains_key(pid));

        let detections = self
            .tracks
            .iter()
            .filter(|(_, t)| should_report(t.first_seen, now))
            .map(|(pid, t)| ProcessDetection {
                agent: t.agent,
                pid: *pid,
                folder: t.folder.clone(),
                working: if trustworthy { t.working } else { false },
            })
            .collect();

        (detections, alive)
    }

    /// The project folder for one matched tree: the most plausible working
    /// directory across the whole tree. The agent's own process often sits in
    /// its install directory ("cli", "x86_64-pc-windows-msvc") while the work
    /// really happens in the directories its shells and tools run from.
    /// Falls back to the root process's own cwd, then the exe/name heuristics.
    fn tree_folder(&self, root: u32, members: &HashMap<u32, Vec<u32>>) -> String {
        let root_proc = self.sys.process(sysinfo::Pid::from_u32(root));
        let exe_dir = root_proc
            .and_then(|p| p.exe())
            .and_then(|e| e.parent().map(PathBuf::from));
        let cwds: Vec<PathBuf> = members
            .get(&root)
            .map(|pids| {
                pids.iter()
                    .filter_map(|p| process_cwd(sysinfo::Pid::from_u32(*p)))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(folder) = best_project_folder(&cwds, exe_dir.as_deref(), &self.skip_prefixes) {
            return folder_name(&folder.to_string_lossy());
        }
        root_proc
            .map(guess_folder)
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// Excluded executables that must never be classified as coding agents.
const EXCLUDED_EXES: &[&str] = &[
    "ember.exe",
    "msedgewebview2.exe",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "bash.exe",
    "sh.exe",
    "wsl.exe",
    "wslhost.exe",
    "conhost.exe",
    "openconsole.exe",
    "windowsterminal.exe",
    "wt.exe",
    "tmux.exe",
    "zellij.exe",
    "wezterm.exe",
    "wezterm-gui.exe",
    "wezterm-mux-server.exe",
    "psmux.exe",
    "explorer.exe",
    "code.exe",
    "code - insiders.exe",
    "cursor.exe",
    "windsurf.exe",
    "vscodium.exe",
    "electron.exe",
];

const RUNTIME_HOSTS: &[&str] = &["node.exe", "bun.exe", "deno.exe"];

pub fn match_agent(proc_: &Process, agents: &[AgentId]) -> Option<AgentId> {
    let exe_name = proc_
        .exe()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| proc_.name().to_string_lossy().to_lowercase());

    if EXCLUDED_EXES.contains(&exe_name.as_str()) {
        return None;
    }

    let cmd_tokens: Vec<String> = proc_
        .cmd()
        .iter()
        .map(|s| s.to_string_lossy().to_lowercase())
        .collect();

    if is_background_daemon(&exe_name, &cmd_tokens) {
        return None;
    }

    match_agent_tokens(&exe_name, &cmd_tokens, agents)
}

/// Idle background daemons (`codex.exe app-server --managed-daemon`,
/// `opencode.exe serve --service`) stay resident across sessions and are not
/// interactive agent runs; their active sessions report via hooks or their
/// own client processes.
pub fn is_background_daemon(exe_name: &str, cmd_tokens: &[String]) -> bool {
    let args: Vec<&str> = cmd_tokens
        .iter()
        .skip(1)
        .map(|s| s.trim_matches(['"', '\'']))
        .collect();
    if exe_name.starts_with("codex")
        && args.first().copied() == Some("app-server")
        && (args.contains(&"--managed-daemon") || args.contains(&"daemon"))
    {
        return true;
    }
    if exe_name.starts_with("opencode")
        && args.contains(&"serve")
        && args.contains(&"--service")
    {
        return true;
    }
    false
}

pub fn match_agent_tokens(
    exe_name: &str,
    cmd_tokens: &[String],
    agents: &[AgentId],
) -> Option<AgentId> {
    if EXCLUDED_EXES.contains(&exe_name) {
        return None;
    }

    let is_runtime = RUNTIME_HOSTS.contains(&exe_name);

    for agent in agents {
        let def = agent.def();
        let exe_hit = def.executables.iter().any(|e| e.to_lowercase() == exe_name);

        if exe_hit && !is_runtime {
            return Some(*agent);
        }

        // If it's a runtime host (node/bun/deno), look for exact package/entrypoint in args.
        // Never match on raw directory paths.
        if is_runtime && !cmd_tokens.is_empty() {
            let matched_entrypoint = def.cmdline_matchers.iter().any(|matcher| {
                let m = matcher.to_lowercase();
                let m_norm = m.replace('\\', "/");
                cmd_tokens.iter().skip(1).any(|arg| {
                    let arg_clean = arg.trim_matches(['"', '\'']);
                    let arg_norm = arg_clean.replace('\\', "/");
                    // Scoped package / path segment match (e.g. `@openai/codex` in `.../node_modules/@openai/codex/bin/codex.js`)
                    if m_norm.contains('/') {
                        let seg = m_norm.trim_matches('/');
                        if arg_norm == seg
                            || arg_norm.starts_with(&format!("{seg}/"))
                            || arg_norm.contains(&format!("/{seg}/"))
                            || arg_norm.ends_with(&format!("/{seg}"))
                        {
                            return true;
                        }
                    }
                    // Match package name or script basename
                    if arg_norm == m_norm || arg_norm.starts_with(&format!("{m_norm}/")) {
                        return true;
                    }
                    if let Some(file_name) = std::path::Path::new(arg_clean).file_name() {
                        let fname = file_name.to_string_lossy().to_lowercase();
                        if fname == m
                            || fname == format!("{m}.js")
                            || fname == format!("{m}.mjs")
                            || fname == format!("{m}.cjs")
                            || fname == format!("{m}.ts")
                        {
                            return true;
                        }
                    }
                    false
                })
            });
            if matched_entrypoint {
                return Some(*agent);
            }
        }
    }
    None
}

/// Reads a live process's working directory out of its PEB.
///
/// sysinfo returns `None` for cwds on Windows, which used to push every folder
/// label onto the executable's install path ("cli", "x86_64-pc-windows-msvc",
/// "old-install"). Walking PEB → ProcessParameters → CurrentDirectory is what
/// process explorers do. Protected, elevated and WOW64 targets fail softly
/// (`None`) and fall back to the exe path.
pub fn process_cwd(pid: sysinfo::Pid) -> Option<PathBuf> {
    use std::ffi::c_void;
    use std::mem::size_of;

    use windows::Wdk::System::Threading::{
        NtQueryInformationProcess, ProcessBasicInformation, ProcessWow64Information,
    };
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_BASIC_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
    };

    // x64 offsets: PEB.ProcessParameters, then
    // RTL_USER_PROCESS_PARAMETERS.CurrentDirectory.DosPath { Length, Buffer }.
    const OFF_PARAMS: usize = 0x20;
    const OFF_CWD_LEN: usize = 0x38;
    const OFF_CWD_BUF: usize = 0x40;

    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ,
            false.into(),
            pid.as_u32(),
        )
    }
    .ok()?;

    let read = |addr: usize, dst: *mut c_void, len: usize| -> Option<()> {
        let mut got = 0usize;
        unsafe {
            ReadProcessMemory(handle, addr as *const c_void, dst, len, Some(&mut got)).ok()?;
        }
        (got == len).then_some(())
    };
    let read_ptr = |addr: usize| -> Option<usize> {
        let mut out = 0usize;
        read(addr, &mut out as *mut usize as *mut c_void, size_of::<usize>())?;
        Some(out)
    };

    let cwd = (|| -> Option<PathBuf> {
        let mut basic: PROCESS_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        let status = unsafe {
            NtQueryInformationProcess(
                handle,
                ProcessBasicInformation,
                &mut basic as *mut _ as *mut c_void,
                size_of::<PROCESS_BASIC_INFORMATION>() as u32,
                std::ptr::null_mut(),
            )
        };
        if status.0 < 0 {
            return None;
        }

        // WOW64 processes use 32-bit PEB offsets; skip them.
        let mut wow64 = 0usize;
        let status = unsafe {
            NtQueryInformationProcess(
                handle,
                ProcessWow64Information,
                &mut wow64 as *mut usize as *mut c_void,
                size_of::<usize>() as u32,
                std::ptr::null_mut(),
            )
        };
        if status.0 >= 0 && wow64 != 0 {
            return None;
        }

        let peb = basic.PebBaseAddress as usize;
        if peb < 0x10000 {
            return None;
        }
        let params = read_ptr(peb + OFF_PARAMS)?;
        if params < 0x10000 {
            return None;
        }
        let mut len = 0u16;
        read(
            params + OFF_CWD_LEN,
            &mut len as *mut u16 as *mut c_void,
            size_of::<u16>(),
        )?;
        if len == 0 || len % 2 != 0 || len > 0x7FFE {
            return None;
        }
        let buf = read_ptr(params + OFF_CWD_BUF)?;
        if buf < 0x10000 {
            return None;
        }
        let mut raw = vec![0u16; (len / 2) as usize];
        read(buf, raw.as_mut_ptr() as *mut c_void, len as usize)?;
        let text = String::from_utf16_lossy(&raw);
        let text = text.trim_matches('\0').trim();
        if text.is_empty() || (!text.contains('\\') && !text.contains('/')) {
            return None;
        }
        Some(PathBuf::from(text))
    })();

    unsafe {
        let _ = CloseHandle(handle);
    }
    cwd
}

/// Normalises a path for comparisons: lowercase, forward slashes to
/// backslashes, no trailing separator.
fn norm_key(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

/// Is `child` the same as, or inside, `parent`? Both are `norm_key` strings.
fn is_same_or_child(child: &str, parent: &str) -> bool {
    child == parent
        || (child.starts_with(parent) && child.as_bytes().get(parent.len()) == Some(&b'\\'))
}

/// Picks the most plausible project folder out of a tree's working
/// directories. Candidates under system/install locations, or inside the
/// executable's own directory chain, never win a vote; subfolders of another
/// candidate collapse into it. The most frequent remaining cwd wins, with the
/// longest path and the name breaking ties (deterministically).
pub fn best_project_folder(
    candidates: &[PathBuf],
    exe_dir: Option<&std::path::Path>,
    skip_prefixes: &[PathBuf],
) -> Option<PathBuf> {
    let skip: Vec<String> = skip_prefixes.iter().map(|p| norm_key(p)).collect();
    let exe = exe_dir.map(norm_key);

    let viable: Vec<(PathBuf, String)> = candidates
        .iter()
        .filter(|p| p.parent().is_some()) // never a drive root
        .map(|p| (p.clone(), norm_key(p)))
        .filter(|(_, key)| {
            if key.is_empty() {
                return false;
            }
            if skip
                .iter()
                .any(|s| !s.is_empty() && is_same_or_child(key, s))
            {
                return false;
            }
            if let Some(exe) = &exe {
                if !exe.is_empty() && (is_same_or_child(key, exe) || is_same_or_child(exe, key)) {
                    return false;
                }
            }
            true
        })
        .collect();

    if viable.is_empty() {
        return None;
    }

    // Vote: every candidate stands for its shallowest parent that is also a
    // candidate, so a busy subfolder never drowns out the project root.
    let mut votes: HashMap<String, (&PathBuf, usize)> = HashMap::new();
    for (path, key) in &viable {
        let mut rep = (path, key);
        for (other, other_key) in &viable {
            if other_key.len() < rep.1.len() && is_same_or_child(rep.1, other_key) {
                rep = (other, other_key);
            }
        }
        votes.entry(rep.1.clone()).or_insert((rep.0, 0)).1 += 1;
    }

    let mut ranked: Vec<(usize, usize, String, PathBuf)> = votes
        .into_iter()
        .map(|(key, (path, count))| (count, path.as_os_str().len(), key, path.clone()))
        .collect();
    ranked.sort_by(|a, b| (a.0, a.1, &a.2).cmp(&(b.0, b.1, &b.2)));
    ranked.pop().map(|(_, _, _, path)| path)
}

/// Windows, program and per-user app-data locations: never a project folder.
fn install_prefixes() -> Vec<PathBuf> {
    const VARS: [&str; 7] = [
        "windir",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
        "LOCALAPPDATA",
        "APPDATA",
        "TEMP",
    ];
    VARS.iter()
        .filter_map(|name| std::env::var_os(name))
        .map(PathBuf::from)
        .collect()
}

pub fn guess_folder(proc_: &Process) -> String {
    // sysinfo's cwd is always None on Windows; read the PEB instead so the
    // fallback at least names the directory the process actually runs in.
    if let Some(cwd) = process_cwd(proc_.pid()) {
        let name = cwd.to_string_lossy();
        if !name.trim().is_empty() {
            let f = folder_name(&name);
            if f.to_lowercase() == "bin" {
                if let Some(parent) = cwd.parent() {
                    let pf = folder_name(&parent.to_string_lossy());
                    if !pf.is_empty() && pf != "unknown" && pf.to_lowercase() != "bin" {
                        return pf;
                    }
                }
            } else {
                return f;
            }
        }
    }
    if let Some(exe) = proc_.exe()
        && let Some(dir) = exe.parent()
    {
        let name = dir.to_string_lossy();
        if !name.trim().is_empty() {
            let f = folder_name(&name);
            if f.to_lowercase() == "bin" {
                if let Some(parent) = dir.parent() {
                    let pf = folder_name(&parent.to_string_lossy());
                    if !pf.is_empty() && pf != "unknown" && pf.to_lowercase() != "bin" {
                        return pf;
                    }
                }
            } else {
                return f;
            }
        }
    }
    let fallback = folder_name(&proc_.name().to_string_lossy());
    if fallback.to_lowercase() == "bin" {
        "workspace".to_string()
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_cpu_spike_does_not_engage() {
        let now = Instant::now();
        // One spike is a single capped sample; the gate needs a full budget.
        let (working, energy, below) = advance_hysteresis(false, 0.0, None, 15.0, IDLE_HOLD, now);
        assert!(!working, "single spike must not engage");
        assert_eq!(energy, CPU_SAMPLE_CAP);
        assert!(below.is_none());

        // The next quiet sample decays it back to zero.
        let (working, energy, _) = advance_hysteresis(
            working,
            energy,
            None,
            0.0,
            IDLE_HOLD,
            now + Duration::from_secs(2),
        );
        assert!(!working);
        assert_eq!(energy, 0.0, "a quiet sample cancels one capped spike");
    }

    #[test]
    fn brief_spike_followed_by_idle_decays_away() {
        let now = Instant::now();
        let (working, energy, _) = advance_hysteresis(false, 0.0, None, 10.0, IDLE_HOLD, now);
        assert!(!working);
        assert_eq!(energy, CPU_SAMPLE_CAP);

        // Flaps back down before a second above-floor sample arrives.
        let (working, energy, _) = advance_hysteresis(
            working,
            energy,
            None,
            0.5,
            IDLE_HOLD,
            now + Duration::from_secs(2),
        );
        assert!(!working);
        assert_eq!(energy, 0.0, "one quiet sample cancels one capped spike");
    }

    #[test]
    fn ambient_lingering_noise_never_engages() {
        // Measured profile of stopped Codex/Node trees on a 12-core box
        // (100 = one full core): 0–2.3 sustained once stale parent links are
        // excluded. Every sample sits at or below the floor, so nothing
        // accumulates and the tree must never count as work.
        let now = Instant::now();
        let (mut working, mut energy, mut below) = (false, 0.0, None);
        for (i, sample) in [0.3, 1.2, 0.9, 0.0, 0.8, 1.25, 0.4].iter().enumerate() {
            (working, energy, below) = advance_hysteresis(
                working,
                energy,
                below,
                *sample,
                IDLE_HOLD,
                now + Duration::from_secs(2 * i as u64),
            );
            assert!(!working, "ambient sample {sample} must not engage");
            assert_eq!(energy, 0.0, "ambient sample {sample} must not accumulate");
        }
        assert!(below.is_none());
    }

    #[test]
    fn zero_cpu_never_counts_as_working() {
        let now = Instant::now();
        let (mut working, mut energy, mut below) = (false, 0.0, None);
        for i in 0..5 {
            (working, energy, below) = advance_hysteresis(
                working,
                energy,
                below,
                0.0,
                IDLE_HOLD,
                now + Duration::from_secs(2 * i),
            );
            assert!(!working);
            assert_eq!(energy, 0.0);
        }
        assert!(below.is_none());
    }

    #[test]
    fn sustained_moderate_cpu_engages() {
        // ~12 matches a genuinely working single process on the same box;
        // builds/tests pile much higher via tree-summed descendants. At the
        // capped rate the accumulator fills in two samples (~4s).
        let now = Instant::now();
        let (mut working, mut energy, mut below) = (false, 0.0, None);
        for i in 0..2u64 {
            (working, energy, below) = advance_hysteresis(
                working,
                energy,
                below,
                12.0,
                IDLE_HOLD,
                now + Duration::from_secs(2 * i),
            );
        }
        assert!(working, "sustained moderate CPU must engage");
        assert_eq!(energy, CPU_ENERGY);
        assert!(below.is_none());
    }

    #[test]
    fn sustained_low_streaming_load_engages() {
        // The reason the energy model exists: a tree that sits just above the
        // ambient floor while the agent streams a long answer must eventually
        // count, instead of never crossing a single high threshold at any
        // instant. 3.5 adds 1.5 per sample — engages on the fourth (~8s).
        let now = Instant::now();
        let (mut working, mut energy, mut below) = (false, 0.0, None);
        for i in 0..4u64 {
            (working, energy, below) = advance_hysteresis(
                working,
                energy,
                below,
                3.5,
                IDLE_HOLD,
                now + Duration::from_secs(2 * i),
            );
        }
        assert!(working, "a sustained modest load must engage within ~8s");
        assert!(below.is_none());
    }

    #[test]
    fn single_burst_counts_only_once() {
        // One 2s burst — however large — is a single capped sample. (The old
        // window average re-counted one spike three times.)
        let now = Instant::now();
        let (working, energy, below) = advance_hysteresis(false, 0.0, None, 90.0, IDLE_HOLD, now);
        assert!(!working, "one burst must not engage");
        assert_eq!(energy, CPU_SAMPLE_CAP);
        assert!(below.is_none());

        let (working, energy, _) = advance_hysteresis(
            working,
            energy,
            None,
            0.0,
            IDLE_HOLD,
            now + Duration::from_secs(2),
        );
        assert!(!working);
        assert_eq!(energy, 0.0, "quiet sample resets the single count");
    }

    #[test]
    fn low_cpu_keeps_working_until_idle_hold_elapses() {
        let t0 = Instant::now();
        // Assume a fully charged, working tree. The first quiet samples drain
        // the accumulator before the idle hold even starts.
        let (working, energy, below) =
            advance_hysteresis(true, CPU_ENERGY, None, 0.1, IDLE_HOLD, t0);
        assert!(working, "first quiet sample must not drop to idle");
        assert!(energy > 0.0);
        let (working, energy, below) = advance_hysteresis(
            working,
            energy,
            below,
            0.1,
            IDLE_HOLD,
            t0 + Duration::from_secs(2),
        );
        assert!(working, "still draining the accumulator");
        let below = below.expect("below_since recorded once the accumulator empties");

        // Half the idle hold later -> still working.
        let (working, _, below) = advance_hysteresis(
            working,
            energy,
            Some(below),
            0.1,
            IDLE_HOLD,
            t0 + Duration::from_secs(2) + IDLE_HOLD / 2,
        );
        assert!(working, "half the idle hold is not enough to release");

        // Just past the idle hold -> drops to idle.
        let (working, _, below) = advance_hysteresis(
            working,
            energy,
            below,
            0.1,
            IDLE_HOLD,
            t0 + Duration::from_secs(2) + IDLE_HOLD + Duration::from_secs(1),
        );
        assert!(!working, "past the idle hold the tree drops to idle");
        assert!(below.is_none());
    }

    #[test]
    fn cpu_spike_during_idle_hold_cancels_hold() {
        let t0 = Instant::now();
        let (_, _, below) = advance_hysteresis(true, 0.0, None, 0.0, IDLE_HOLD, t0);
        let (working, energy, below) = advance_hysteresis(
            true,
            0.0,
            below,
            20.0,
            IDLE_HOLD,
            t0 + Duration::from_secs(5),
        );
        assert!(working);
        assert_eq!(energy, CPU_SAMPLE_CAP);
        assert!(below.is_none(), "below_since cancelled on new activity");
    }

    #[test]
    fn idle_tree_with_single_sample_spikes_never_engages() {
        // Regression for the reported false positive: a mostly-idle tree still
        // emits an occasional single 2s spike. One quiet sample removes
        // exactly one capped spike, so alternating spike/quiet never
        // accumulates — every quiet sample cancels the spike before it.
        let now = Instant::now();
        let spike = 20.0; // clearly above the cap
        let (mut working, mut energy, mut below) = (false, 0.0, None);
        for (i, sample) in [spike, 0.2, spike, 0.2, spike, 0.2, spike, 0.2]
            .iter()
            .enumerate()
        {
            (working, energy, below) = advance_hysteresis(
                working,
                energy,
                below,
                *sample,
                IDLE_HOLD,
                now + Duration::from_secs(2 * i as u64),
            );
            assert!(!working, "isolated spike {sample} must not engage");
            assert!(
                energy < CPU_ENERGY,
                "isolated spike {sample} must not reach the gate"
            );
        }
        assert!(below.is_none());
        assert_eq!(energy, 0.0);
    }

    #[test]
    fn steady_moderate_work_engages_then_releases() {
        // Both halves of the tuning: a sustained load fills the accumulator
        // within a couple of samples, and the tree releases within IDLE_HOLD
        // of the accumulator emptying.
        let now = Instant::now();
        let work = 12.0; // clearly above the cap on the calibrated box

        let (mut working, mut energy, mut below) = (false, 0.0, None);
        for i in 0..2u64 {
            let last = i == 1;
            (working, energy, below) = advance_hysteresis(
                working,
                energy,
                below,
                work,
                IDLE_HOLD,
                now + Duration::from_secs(2 * i),
            );
            assert_eq!(working, last, "engages exactly when the budget fills");
        }
        assert_eq!(energy, CPU_ENERGY);
        assert!(below.is_none());

        // Going quiet drains the accumulator first (two samples), then starts
        // the idle hold; release happens within IDLE_HOLD of the drain.
        let drain_at = now + Duration::from_secs(4);
        let (working, energy, below) =
            advance_hysteresis(working, energy, below, 0.1, IDLE_HOLD, drain_at);
        assert!(working);
        let (working, energy, below) = advance_hysteresis(
            working,
            energy,
            below,
            0.1,
            IDLE_HOLD,
            drain_at + Duration::from_secs(2),
        );
        assert!(working);
        let below = below.expect("idle hold started after the drain");

        // Still inside the hold -> still working.
        let (working, _, below) = advance_hysteresis(
            working,
            energy,
            Some(below),
            0.1,
            IDLE_HOLD,
            drain_at + Duration::from_secs(2) + IDLE_HOLD - Duration::from_secs(1),
        );
        assert!(working, "must not release before the idle hold elapses");
        assert!(
            IDLE_HOLD <= Duration::from_secs(15),
            "release must happen within 15s of going quiet"
        );

        // Just past the hold -> idle.
        let (working, _, below) = advance_hysteresis(
            working,
            energy,
            below,
            0.1,
            IDLE_HOLD,
            drain_at + Duration::from_secs(2) + IDLE_HOLD,
        );
        assert!(!working, "released once the idle hold elapsed");
        assert!(below.is_none());
    }

    fn test_track(now: Instant) -> Track {
        Track {
            agent: AgentId::Codex,
            folder: "boxed".to_string(),
            energy: 0.0,
            working: false,
            below_since: None,
            first_seen: now,
        }
    }

    #[test]
    fn transient_helper_inside_settle_window_never_reports() {
        // The reported false positive: a short-lived helper burning hot enough
        // for the old gate. Inside the settle window it must never surface and
        // never turn working, however hot it runs.
        let t0 = Instant::now();
        let mut t = test_track(t0);
        for i in 1..=5u64 {
            let now = t0 + Duration::from_secs(2 * i); // 2s..=10s of hot samples
            step_track(&mut t, 40.0, now);
            assert!(!t.working, "settling helper must never turn working");
            if now.duration_since(t0) < MIN_REPORT_AGE {
                assert!(
                    !should_report(t.first_seen, now),
                    "settling helper must stay invisible"
                );
            }
        }
    }

    #[test]
    fn sustained_work_engages_only_after_settle_and_full_run() {
        let t0 = Instant::now();
        let mut t = test_track(t0);
        let mut engaged_at = None;
        for i in 1..=30u64 {
            let now = t0 + Duration::from_secs(2 * i);
            step_track(&mut t, 12.0, now);
            let admitted = now.duration_since(t0) >= MIN_REPORT_AGE;
            assert_eq!(
                should_report(t.first_seen, now),
                admitted,
                "reporting follows the settle window"
            );
            if t.working {
                engaged_at = Some(now);
                break;
            }
        }
        let engaged = engaged_at.expect("sustained work engages");
        assert!(
            engaged >= t0 + MIN_REPORT_AGE + Duration::from_secs(2),
            "working is earned with a full energy budget after settling"
        );
    }

    #[test]
    fn idle_alive_track_stays_reported_after_long_quiet() {
        // Rows live exactly as long as their process: an agent that is alive but
        // quiet must stay listed (running, idle) however long the quiet spell.
        let t0 = Instant::now();
        let mut t = test_track(t0);

        // Admit + engage, then stay quiet for ten minutes of polls.
        let mut now;
        for i in 1..=15u64 {
            now = t0 + Duration::from_secs(2 * i);
            step_track(&mut t, 12.0, now);
        }
        assert!(t.working, "engaged while hot");

        for i in 16..=315u64 {
            now = t0 + Duration::from_secs(2 * i);
            step_track(&mut t, 0.1, now);
            assert!(
                should_report(t.first_seen, now),
                "an alive quiet process must stay in the session list"
            );
        }
        assert!(!t.working, "idle after the quiet spell");
    }

    #[test]
    fn guess_folder_never_returns_bin() {
        let folder = folder_name("C:\\tools\\codex\\bin");
        assert_eq!(folder, "bin");
        // But our fallback in guess_folder walks up to parent
        let path = std::path::Path::new("C:\\tools\\codex\\bin");
        if let Some(parent) = path.parent() {
            let pf = folder_name(&parent.to_string_lossy());
            assert_eq!(pf, "codex");
        }
    }

    #[test]
    fn project_folder_beats_install_locations_and_folds_subfolders() {
        let project = PathBuf::from("C:\\Users\\sanir\\Windows Tool");
        let candidates = vec![
            // The agent itself sits in its package dir (filtered out).
            PathBuf::from("C:\\Users\\sanir\\AppData\\Local\\Programs\\antigravity"),
            project.clone(),
            project.join("src-tauri"),
            project.join("src-tauri"),
        ];
        let picked = best_project_folder(&candidates, None, &install_prefixes()).unwrap();
        assert_eq!(picked, project, "subfolders vote for the project root");
    }

    #[test]
    fn exe_install_chain_is_never_a_project() {
        let exe_dir = PathBuf::from("C:\\tools\\opencode\\bin");
        let project = PathBuf::from("C:\\work\\proj");
        let candidates = vec![
            PathBuf::from("C:\\tools\\opencode"),
            PathBuf::from("C:\\tools\\opencode\\bin"),
            project.clone(),
        ];
        let picked = best_project_folder(&candidates, Some(&exe_dir), &[]).unwrap();
        assert_eq!(picked, project);
        // Only install-chain evidence left: no project at all.
        assert!(
            best_project_folder(&[PathBuf::from("C:\\tools\\opencode")], Some(&exe_dir), &[])
                .is_none()
        );
    }

    #[test]
    fn no_project_evidence_yields_none() {
        assert!(best_project_folder(&[], None, &[]).is_none());
        assert!(best_project_folder(&[PathBuf::from("C:\\")], None, &[]).is_none());
        let skip = vec![PathBuf::from("C:\\Windows")];
        assert!(
            best_project_folder(&[PathBuf::from("C:\\Windows\\System32")], None, &skip).is_none()
        );
    }

    #[test]
    fn process_cwd_reads_a_live_child_process() {
        // Spawn a child in a known directory and read its cwd straight from the
        // PEB — this is what gives every provider a real project folder.
        let dir = std::env::temp_dir().join("ember-cwd-test");
        std::fs::create_dir_all(&dir).unwrap();
        let mut child = std::process::Command::new("cmd")
            .args(["/c", "ping", "-n", "30", "127.0.0.1"])
            .current_dir(&dir)
            .spawn()
            .expect("spawn child");

        let mut got = None;
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            if let Some(cwd) = process_cwd(sysinfo::Pid::from_u32(child.id())) {
                got = Some(cwd);
                break;
            }
        }
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .output();
        let _ = child.wait();

        let got = got.expect("child cwd must be readable from its PEB");
        assert_eq!(norm_key(&got), norm_key(&dir));
    }

    #[test]
    fn match_agent_tokens_handles_full_paths_and_excludes_daemons() {
        let all = AgentId::ALL;
        // Node wrapper with full path to @openai/codex/bin/codex.js
        let codex_wrapper = vec![
            "node.exe".to_string(),
            r"c:\users\me\node_modules\@openai\codex\bin\codex.js".to_string(),
        ];
        assert_eq!(
            match_agent_tokens("node.exe", &codex_wrapper, &all),
            Some(AgentId::Codex)
        );

        // Node wrapper with full path to cline
        let cline_wrapper = vec![
            "node.exe".to_string(),
            r"c:\users\me\node_modules\cline\bin\cline".to_string(),
        ];
        assert_eq!(
            match_agent_tokens("node.exe", &cline_wrapper, &all),
            Some(AgentId::Cline)
        );

        // Antigravity.exe (desktop Electron app) must NOT match Antigravity CLI
        let agy_desktop = vec![
            r"c:\users\me\appdata\local\programs\antigravity\antigravity.exe".to_string(),
            "dist/server.cjs".to_string(),
        ];
        assert_eq!(
            match_agent_tokens("antigravity.exe", &agy_desktop, &all),
            None
        );
        // agy.exe DOES match Antigravity CLI
        let agy_cli = vec!["agy.exe".to_string()];
        assert_eq!(
            match_agent_tokens("agy.exe", &agy_cli, &all),
            Some(AgentId::Antigravity)
        );

        // Background daemons are excluded
        let codex_daemon = vec![
            "codex.exe".to_string(),
            "app-server".to_string(),
            "--listen".to_string(),
            "unix://".to_string(),
            "--managed-daemon".to_string(),
        ];
        assert!(is_background_daemon("codex.exe", &codex_daemon));

        let opencode_service = vec![
            "opencode.exe".to_string(),
            "serve".to_string(),
            "--service".to_string(),
        ];
        assert!(is_background_daemon("opencode.exe", &opencode_service));

        // Interactive `opencode serve --stdio` is NOT a background daemon
        let opencode_stdio = vec![
            "opencode.exe".to_string(),
            "serve".to_string(),
            "--stdio".to_string(),
        ];
        assert!(!is_background_daemon("opencode.exe", &opencode_stdio));
    }
}
