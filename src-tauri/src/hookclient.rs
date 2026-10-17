//! The `Ember.exe hook <agent> <event>` fast path.
//!
//! This runs *before* Tauri, before the WebView, and before single-instance —
//! it must be cheap, quiet and never fail the host agent. The contract:
//!
//!   * never write to stdout or stderr
//!   * always exit 0, in roughly 300 ms
//!   * read the agent's stdin payload with a hard 200 ms / 1 MiB cap
//!   * find the agent's own PID by walking our ancestor chain
//!   * post exactly one JSON line to `\\.\pipe\ember-hooks`

use std::collections::HashMap;
use std::io::Read;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use windows::Win32::Foundation::{CloseHandle, GENERIC_WRITE, GetLastError};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, WriteFile,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Pipes::WaitNamedPipeW;
use windows::core::PCWSTR;

use crate::agents::AgentId;
use crate::agents::hookserver::{HookWire, PROTOCOL_VERSION};
use crate::paths;

/// The whole invocation has to fit in this.
const TOTAL_BUDGET: Duration = Duration::from_millis(300);
/// How long we are willing to wait for the agent's stdin payload.
const STDIN_BUDGET: Duration = Duration::from_millis(200);
const MAX_STDIN: usize = 1024 * 1024;
const PIPE_WAIT_MS: u32 = 150;

/// Shells and launchers that sit between the hook and the agent itself.
const LAUNCHERS: &[&str] = &[
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "sh.exe",
    "bash.exe",
    "wsl.exe",
    "wslhost.exe",
    "conhost.exe",
    "openconsole.exe",
    "windowsterminal.exe",
    "wt.exe",
    "git.exe",
    "npx.cmd",
    "npm.cmd",
    "tmux.exe",
    "zellij.exe",
    "wezterm.exe",
    "wezterm-gui.exe",
    "wezterm-mux-server.exe",
    "psmux.exe",
];

/// Runtimes that host a JS/TS coding agent. Used only as a fallback when no
/// ancestor matches the agent's own executable names.
const GENERIC_HOSTS: &[&str] = &[
    "node.exe",
    "bun.exe",
    "deno.exe",
    "python.exe",
    "python3.exe",
    "electron.exe",
    "code.exe",
    "code - insiders.exe",
    "cursor.exe",
    "windsurf.exe",
];

/// Entry point for the hook CLI. Always returns 0.
pub fn run(agent_id: &str, event: &str) -> i32 {
    let started = Instant::now();
    let agent = AgentId::parse(agent_id);
    let payload = read_stdin_bounded(STDIN_BUDGET);
    if let Some(agent) = agent {
        let remaining = TOTAL_BUDGET.saturating_sub(started.elapsed());
        if let Err(e) = post(agent, event, &payload, remaining) {
            // Never fail the host agent, but leave a trace: a silently
            // dropped hook shortens the coverage window and used to be
            // invisible everywhere.
            crate::log::append("HOOK", &format!("{} {event} dropped: {e}", agent.id()));
        }
    }
    // Never print, never fail.
    0
}

/// True when `args` is a hook invocation, e.g. `hook claude-code working`.
pub fn parse_args(args: &[String]) -> Option<(&str, &str)> {
    if args.len() != 4 || args[1] != "hook" {
        return None;
    }
    Some((args[2].as_str(), args[3].as_str()))
}

/// Read stdin on a helper thread with a hard deadline, so a console with no
/// input can never wedge a hook.
fn read_stdin_bounded(budget: Duration) -> String {
    let (tx, rx) = channel::<Vec<u8>>();
    let spawned = std::thread::Builder::new()
        .name("ember-hook-stdin".into())
        .spawn(move || {
            let mut collected: Vec<u8> = Vec::new();
            let stdin = std::io::stdin();
            let mut lock = stdin.lock();
            let mut chunk = [0u8; 8192];
            while collected.len() < MAX_STDIN {
                match lock.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => collected.extend_from_slice(&chunk[..n]),
                    Err(_) => break,
                }
            }
            let _ = tx.send(collected);
        });

    if spawned.is_err() {
        return String::new();
    }
    match rx.recv_timeout(budget) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => String::new(),
    }
}

fn post(agent: AgentId, event: &str, payload: &str, budget: Duration) -> Result<(), String> {
    if budget.is_zero() {
        return Err("hook budget exhausted before connect".into());
    }

    let token = std::fs::read_to_string(paths::token_path())
        .map_err(|e| format!("no hook token: {e}"))?
        .trim()
        .to_string();
    if token.is_empty() {
        return Err("empty hook token".into());
    }

    let ctx = crate::agents::parse_hook_payload(agent, payload);
    let resolved_pid = ctx.pid.or_else(|| find_agent_pid(agent));

    let wire = HookWire {
        v: PROTOCOL_VERSION,
        token,
        agent: agent.id().to_string(),
        event: event.to_string(),
        payload: payload.to_string(),
        pid: resolved_pid,
        ts: now_unix(),
    };
    let mut line = serde_json::to_string(&wire).map_err(|e| e.to_string())?;
    line.push('\n');

    let name: Vec<u16> = paths::PIPE_NAME
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    // Hooks arrive in bursts (PreToolUse/PostToolUse pairs, event storms).
    // Retry the connect until the budget runs out instead of dropping a cast
    // that merely found the pipe momentarily busy between instances.
    let deadline = Instant::now() + budget;
    let mut last_error = String::from("Ember is not listening on the hook pipe");
    // When the app is closed there is no pipe at all: retry only a moment in
    // case Ember is mid-startup, then fail fast instead of burning the whole
    // budget on every hook of the host agent. A busy pipe (instances exist,
    // all in use) keeps the full retry budget.
    let mut missing_since: Option<Instant> = None;
    let handle = loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(last_error);
        }
        let wait_ms = PIPE_WAIT_MS.min(remaining.as_millis() as u32).max(1);
        if unsafe { WaitNamedPipeW(PCWSTR(name.as_ptr()), wait_ms) }.as_bool() {
            missing_since = None;
            match unsafe {
                CreateFileW(
                    PCWSTR(name.as_ptr()),
                    GENERIC_WRITE.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    None,
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    None,
                )
            } {
                Ok(handle) => break handle,
                Err(e) => last_error = format!("open hook pipe: {e}"),
            }
        } else {
            // A timeout means instances exist but are all busy; "file not
            // found" means the pipe is not there at all (Ember is not
            // running) — a much more honest message than "busy".
            let code = unsafe { GetLastError() }.0;
            if code == 2 {
                let since = *missing_since.get_or_insert_with(Instant::now);
                last_error = "Ember is not running".to_string();
                if since.elapsed() >= Duration::from_millis(150) {
                    return Err(last_error);
                }
            } else {
                missing_since = None;
                last_error = "hook pipe was busy".to_string();
            }
        }
        // A tiny backoff so a burst does not spin a core.
        std::thread::sleep(Duration::from_millis(5));
    };

    let mut written: u32 = 0;
    let result = unsafe { WriteFile(handle, Some(line.as_bytes()), Some(&mut written), None) };
    unsafe {
        let _ = CloseHandle(handle);
    }
    result.map_err(|e| format!("write hook pipe: {e}"))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Snapshot the whole process table: pid -> parent pid, and pid -> exe name.
fn snapshot_processes() -> (HashMap<u32, u32>, HashMap<u32, String>) {
    let mut parents = HashMap::new();
    let mut names = HashMap::new();

    let snapshot = match unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) } {
        Ok(handle) => handle,
        Err(_) => return (parents, names),
    };
    if snapshot.is_invalid() {
        return (parents, names);
    }

    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            let end = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_lowercase();
            parents.insert(entry.th32ProcessID, entry.th32ParentProcessID);
            names.insert(entry.th32ProcessID, name);
            if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }

    unsafe {
        let _ = CloseHandle(snapshot);
    }
    (parents, names)
}

/// Walk our own ancestor chain looking for the agent's host process.
///
/// A hook spawned by a shell is two or three hops below the agent, so launchers
/// are skipped. If no ancestor matches the agent's own executable names, the
/// nearest non-launcher runtime (`node.exe` and friends) is used, because that
/// is what an npm-installed agent actually runs as.
fn find_agent_pid(agent: AgentId) -> Option<u32> {
    let (parents, names) = snapshot_processes();
    agent_pid_from(&parents, &names, std::process::id(), agent)
}

/// The testable core of [`find_agent_pid`].
pub fn agent_pid_from(
    parents: &HashMap<u32, u32>,
    names: &HashMap<u32, String>,
    start_pid: u32,
    agent: AgentId,
) -> Option<u32> {
    let wanted: Vec<String> = agent
        .def()
        .executables
        .iter()
        .map(|s| s.to_lowercase())
        .collect();

    let mut cursor = start_pid;
    let mut fallback: Option<u32> = None;

    for _ in 0..12 {
        let Some(parent) = parents.get(&cursor).copied() else {
            break;
        };
        if parent == 0 || parent == cursor {
            break;
        }
        let name = names.get(&parent).cloned().unwrap_or_default();
        if name.is_empty() {
            break;
        }
        if LAUNCHERS.contains(&name.as_str()) {
            cursor = parent;
            continue;
        }
        if wanted.contains(&name) {
            return Some(parent);
        }
        if fallback.is_none() && GENERIC_HOSTS.contains(&name.as_str()) {
            fallback = Some(parent);
        }
        cursor = parent;
    }

    fallback
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn argument_parsing_is_strict() {
        assert_eq!(
            parse_args(&args(&["ember.exe", "hook", "claude-code", "working"])),
            Some(("claude-code", "working"))
        );
        assert_eq!(parse_args(&args(&["ember.exe"])), None);
        assert_eq!(parse_args(&args(&["ember.exe", "hook"])), None);
        assert_eq!(
            parse_args(&args(&["ember.exe", "watchdog", "1", "2"])),
            None
        );
        assert_eq!(
            parse_args(&args(&["ember.exe", "hook", "x", "y", "z"])),
            None
        );
    }

    fn chain(entries: &[(u32, u32, &str)]) -> (HashMap<u32, u32>, HashMap<u32, String>) {
        let mut parents = HashMap::new();
        let mut names = HashMap::new();
        for (pid, parent, name) in entries {
            parents.insert(*pid, *parent);
            names.insert(*pid, name.to_string());
        }
        (parents, names)
    }

    #[test]
    fn direct_parent_agent_is_found() {
        // 100 (us) -> 200 claude.exe
        let (parents, names) = chain(&[(100, 200, "ember.exe"), (200, 1, "claude.exe")]);
        assert_eq!(
            agent_pid_from(&parents, &names, 100, AgentId::ClaudeCode),
            Some(200)
        );
    }

    #[test]
    fn launchers_are_skipped() {
        // 100 (us) -> 300 cmd.exe -> 200 codex.exe
        let (parents, names) = chain(&[
            (100, 300, "ember.exe"),
            (300, 200, "cmd.exe"),
            (200, 1, "codex.exe"),
        ]);
        assert_eq!(
            agent_pid_from(&parents, &names, 100, AgentId::Codex),
            Some(200)
        );
    }

    #[test]
    fn npm_installed_agents_fall_back_to_their_runtime() {
        // 100 (us) -> 300 cmd.exe -> 400 node.exe -> 1
        let (parents, names) = chain(&[
            (100, 300, "ember.exe"),
            (300, 400, "cmd.exe"),
            (400, 1, "node.exe"),
        ]);
        assert_eq!(
            agent_pid_from(&parents, &names, 100, AgentId::ClaudeCode),
            Some(400)
        );
    }

    #[test]
    fn an_exact_match_beats_the_runtime_fallback() {
        // 100 (us) -> 400 node.exe -> 200 omp.exe
        let (parents, names) = chain(&[
            (100, 400, "ember.exe"),
            (400, 200, "node.exe"),
            (200, 1, "omp.exe"),
        ]);
        assert_eq!(
            agent_pid_from(&parents, &names, 100, AgentId::OhMyPi),
            Some(200)
        );
    }

    #[test]
    fn unrelated_ancestors_yield_nothing() {
        // 100 (us) -> 300 explorer.exe
        let (parents, names) = chain(&[(100, 300, "ember.exe"), (300, 1, "explorer.exe")]);
        assert_eq!(agent_pid_from(&parents, &names, 100, AgentId::Codex), None);
    }

    #[test]
    fn cycles_and_missing_parents_terminate() {
        let (parents, names) = chain(&[(100, 100, "ember.exe")]);
        assert_eq!(agent_pid_from(&parents, &names, 100, AgentId::Codex), None);

        let (parents, names) = chain(&[(100, 999, "ember.exe")]);
        assert_eq!(agent_pid_from(&parents, &names, 100, AgentId::Codex), None);

        let empty: HashMap<u32, u32> = HashMap::new();
        let names: HashMap<u32, String> = HashMap::new();
        assert_eq!(agent_pid_from(&empty, &names, 1234, AgentId::Cline), None);
    }

    #[test]
    fn a_long_chain_is_bounded() {
        // 20 shell hops with no agent anywhere.
        let mut entries = vec![(100u32, 101u32, "ember.exe")];
        for i in 101..121u32 {
            entries.push((i, i + 1, "cmd.exe"));
        }
        entries.push((121, 1, "cmd.exe"));
        let (parents, names) = chain(&entries);
        assert_eq!(agent_pid_from(&parents, &names, 100, AgentId::Pi), None);
    }
}
