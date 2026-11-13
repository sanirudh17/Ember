//! Per-agent hook/plugin installers.
//!
//! Every installer follows the same contract:
//!   * it is never silent — the UI shows the exact file and the exact change
//!   * it writes a `<file>.ember-backup-<timestamp>` backup first
//!   * it merges rather than replaces, preserving everything it did not add
//!   * it is idempotent and can be removed cleanly later
//!   * it is recorded in `%APPDATA%\Ember\installs.json` as a marker/ledger
//!
//! Format notes (all verified against upstream docs — see
//! `docs/AGENT_INTEGRATIONS.md`):
//!   * Claude Code — JSON `settings.json`, `hooks.<Event>` = matcher groups
//!   * Codex — TOML `config.toml`, `[[hooks.<Event>]]` arrays of tables;
//!     commands point at a space-free launcher `.cmd` because Codex runs
//!     hooks through `cmd /C "<command>"` and a command that starts with a
//!     quoted path gets its quotes eaten by cmd, silently never running
//!   * Antigravity — JSON `hooks.json`, keyed by hook name ("ember")
//!   * OpenCode / Pi / Oh My Pi — a single plugin/extension file we own
//!   * Cline: managed Windows PowerShell hook bundle; existing policies are preserved

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

use super::registry::HookEvent;
use super::{AgentId, IntegrationMode, current_exe};
use crate::paths;
mod cline;

// ---------------------------------------------------------------------------
// Hook bindings (which agent event maps to which Ember event)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct Binding {
    pub config_event: &'static str,
    pub ember_event: HookEvent,
    pub matcher: Option<&'static str>,
}

const fn b(
    config_event: &'static str,
    ember_event: HookEvent,
    matcher: Option<&'static str>,
) -> Binding {
    Binding {
        config_event,
        ember_event,
        matcher,
    }
}

static CLAUDE_CODE_BINDINGS: [Binding; 11] = [
    b("SessionStart", HookEvent::Start, None),
    b("UserPromptSubmit", HookEvent::Working, None),
    b("PreToolUse", HookEvent::Working, None),
    b("PostToolUse", HookEvent::Resume, None),
    b("Notification", HookEvent::Idle, Some("idle_prompt")),
    b("Stop", HookEvent::Idle, None),
    b("StopFailure", HookEvent::Idle, None),
    b("PostToolUseFailure", HookEvent::Resume, None),
    b("Elicitation", HookEvent::Waiting, None),
    b("ElicitationResult", HookEvent::Resume, None),
    b("SessionEnd", HookEvent::End, None),
];

static CODEX_BINDINGS: [Binding; 7] = [
    b("SessionStart", HookEvent::Start, None),
    b("UserPromptSubmit", HookEvent::Working, None),
    b("PreToolUse", HookEvent::Working, None),
    b("PostToolUse", HookEvent::Resume, None),
    b("Stop", HookEvent::Idle, None),
    b("Interrupt", HookEvent::Idle, None),
    b("SessionEnd", HookEvent::End, None),
];

static ANTIGRAVITY_BINDINGS: [Binding; 5] = [
    b("SessionStart", HookEvent::Start, None),
    b("PreInvocation", HookEvent::Working, None),
    b("PostInvocation", HookEvent::Heartbeat, None),
    b("PostToolUse", HookEvent::Heartbeat, Some("*")),
    b("Stop", HookEvent::Idle, None),
];

// Plugin-driven agents get a single file; the event table below is only used to
// describe the integration in the UI and ledger. The event names come from the
// verified matrix in `docs/AGENT_INTEGRATIONS.md`.
static OPENCODE_BINDINGS: [Binding; 8] = [
    b("session.created", HookEvent::Start, None),
    b("session.prompt", HookEvent::Working, None),
    b("session.execution.started", HookEvent::Working, None),
    b("session.execution.* ended", HookEvent::Idle, None),
    b("session.status busy/retry", HookEvent::Working, None),
    b("session.deleted", HookEvent::End, None),
    b(
        "permission/question/form requested",
        HookEvent::Waiting,
        None,
    ),
    b("permission/question/form replied", HookEvent::Resume, None),
];

static PI_BINDINGS: [Binding; 10] = [
    b("session_start", HookEvent::Start, None),
    b("before_agent_start", HookEvent::Working, None),
    b("agent_start", HookEvent::Working, None),
    b("turn_start", HookEvent::Working, None),
    b("tool_call", HookEvent::Working, None),
    b("tool_execution_start", HookEvent::Working, None),
    b("agent_settled", HookEvent::Idle, None),
    b("ui_prompt_start", HookEvent::Waiting, None),
    b("ui_prompt_end", HookEvent::Resume, None),
    b("session_shutdown", HookEvent::End, None),
];

static OH_MY_PI_BINDINGS: [Binding; 7] = [
    b("session_start", HookEvent::Start, None),
    b("before_agent_start", HookEvent::Working, None),
    b("agent_start", HookEvent::Working, None),
    b("tool_call", HookEvent::Working, None),
    b("tool_execution_start", HookEvent::Working, None),
    b("agent_end", HookEvent::Idle, None),
    b("session_shutdown", HookEvent::End, None),
];

pub fn bindings(agent: AgentId) -> &'static [Binding] {
    match agent {
        AgentId::ClaudeCode => &CLAUDE_CODE_BINDINGS,
        AgentId::Codex => &CODEX_BINDINGS,
        AgentId::Antigravity => &ANTIGRAVITY_BINDINGS,
        AgentId::OpenCode => &OPENCODE_BINDINGS,
        AgentId::Pi => &PI_BINDINGS,
        AgentId::OhMyPi => &OH_MY_PI_BINDINGS,
        AgentId::Cline => cline::BINDINGS,
    }
}

// ---------------------------------------------------------------------------
// Ledger ("marked tag")
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct InstalledRecord {
    pub file: String,
    pub format: String,
    /// config event -> the exact command / marker we wrote.
    pub entries: BTreeMap<String, String>,
    pub installed_unix: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Manifest {
    pub agents: BTreeMap<String, InstalledRecord>,
}

pub fn manifest_path() -> PathBuf {
    paths::config_dir().join("installs.json")
}

pub fn load_manifest() -> Manifest {
    std::fs::read_to_string(manifest_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_manifest(m: &Manifest) -> Result<(), String> {
    let text = serde_json::to_string_pretty(m).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&manifest_path(), text.as_bytes())
}

// ---------------------------------------------------------------------------
// Command shape
// ---------------------------------------------------------------------------

/// The one command shape Ember installs: `"<Ember.exe>" hook <agent> <event>`.
pub fn hook_command(exe: &Path, agent: AgentId, event: HookEvent) -> String {
    format!(
        "\"{}\" hook {} {}",
        exe.display(),
        agent.id(),
        event.as_str()
    )
}

/// File name of the Windows launcher Ember installs for Codex.
///
/// Codex executes hook commands as `cmd /C "<command>"` (see codex-rs
/// `hooks/src/engine/command_runner.rs` → `build_command`). When the command
/// itself starts with a quoted path (anything under a directory with a space,
/// like `"C:\Users\me\Windows Tool\...\ember.exe" hook codex start`), cmd's
/// `/C` quote stripping removes the command's own quotes and the hook fails
/// silently. Ember therefore points Codex at a launcher `.cmd` whose path
/// contains no spaces, so the command needs no quotes at all.
pub const CODEX_LAUNCHER_FILENAME: &str = "ember-hook.cmd";

/// Space-free launcher location: `%USERPROFILE%\.ember\hooks\ember-hook.cmd`.
pub fn codex_launcher_path() -> Result<PathBuf, String> {
    let path = home()?
        .join(".ember")
        .join("hooks")
        .join(CODEX_LAUNCHER_FILENAME);
    if path.display().to_string().contains(' ') {
        return Err("no space-free launcher path (user home contains spaces)".into());
    }
    Ok(path)
}

fn launcher_contents(exe: &Path) -> String {
    format!("@echo off\r\n\"{}\" hook %*\r\n", exe.display())
}

/// The command Ember installs for Codex: `<launcher> codex <event>`, falling
/// back to the classic quoted shape when no space-free launcher path exists.
fn codex_hook_command(exe: &Path, event: HookEvent) -> String {
    match codex_launcher_path() {
        Ok(launcher) => format!("{} codex {}", launcher.display(), event.as_str()),
        Err(_) => hook_command(exe, AgentId::Codex, event),
    }
}

/// Recognise an Ember hook command and recover which Ember event it fires.
pub fn parse_our_command(agent: AgentId, command: &str) -> Option<HookEvent> {
    let command = command.trim();

    // Launcher shape: `<launcher path> <agent> <event>` (path has no spaces).
    if !command.starts_with('"')
        && let Some(first) = command.split_whitespace().next()
        && first
            .rsplit(['\\', '/'])
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case(CODEX_LAUNCHER_FILENAME))
    {
        let mut parts = command[first.len()..].split_whitespace();
        if parts.next()? != agent.id() {
            return None;
        }
        return HookEvent::parse(parts.next()?);
    }

    let (exe, tail) = if let Some(rest) = command.strip_prefix('"') {
        let end = rest.find('"')?;
        (&rest[..end], rest[end + 1..].trim())
    } else {
        // Unquoted variant: split right after the .exe token.
        let lower = command.to_lowercase();
        let idx = lower.find("ember.exe")?;
        let split = idx + "ember.exe".len();
        (&command[..split], command[split..].trim())
    };

    if !exe.to_lowercase().ends_with("ember.exe") {
        return None;
    }

    let mut parts = tail.split_whitespace();
    if parts.next()? != "hook" {
        return None;
    }
    if parts.next()? != agent.id() {
        return None;
    }
    let event = parts.next()?;
    HookEvent::parse(event)
}

/// Does an installed command still point at the running executable?
pub fn command_matches_running_exe(command: &str, exe: &Path) -> bool {
    let want = exe.display().to_string().to_lowercase();
    let have = command.to_lowercase();
    if have.contains(&want) {
        return true;
    }
    // Launcher commands keep the executable path inside the launcher file.
    if !command.trim_start().starts_with('"')
        && let Some(first) = command.split_whitespace().next()
        && first
            .rsplit(['\\', '/'])
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case(CODEX_LAUNCHER_FILENAME))
    {
        return std::fs::read_to_string(first)
            .map(|text| text.to_lowercase().contains(&want))
            .unwrap_or(false);
    }
    false
}

// ---------------------------------------------------------------------------
// Target files
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Install,
    Remove,
}

#[derive(Clone, Debug)]
pub struct Rendered {
    pub path: PathBuf,
    pub format: &'static str,
    pub before: String,
    pub after: String,
    pub entries: BTreeMap<String, String>,
    pub creates: bool,
}

fn home() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "could not resolve the user home directory".to_string())
}

pub fn target_path(agent: AgentId) -> Result<PathBuf, String> {
    let h = home()?;
    Ok(match agent {
        AgentId::ClaudeCode => h.join(".claude").join("settings.json"),
        AgentId::Codex => h.join(".codex").join("config.toml"),
        AgentId::OpenCode => h
            .join(".config")
            .join("opencode")
            .join("plugins")
            .join("ember-hooks.js"),
        AgentId::Pi => h
            .join(".pi")
            .join("agent")
            .join("extensions")
            .join("ember-hooks.ts"),
        AgentId::OhMyPi => h
            .join(".omp")
            .join("agent")
            .join("extensions")
            .join("ember-hooks.ts"),
        AgentId::Antigravity => h.join(".gemini").join("config").join("hooks.json"),
        AgentId::Cline => cline::directory()?.join("UserPromptSubmit.ps1"),
    })
}

pub fn supported(_agent: AgentId) -> bool {
    true
}

fn read_or_empty(path: &Path) -> Result<(String, bool), String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok((text, false)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((String::new(), true)),
        Err(e) => Err(format!("could not read {}: {e}", path.display())),
    }
}

pub fn render(agent: AgentId, op: Op) -> Result<Rendered, String> {
    if agent == AgentId::Cline {
        return cline::render(op, &current_exe());
    }
    let exe = current_exe();
    let path = target_path(agent)?;
    let (before, creates) = read_or_empty(&path)?;
    let (after, entries) = match agent {
        AgentId::ClaudeCode => render_claude_json(&before, agent, &exe, op)?,
        AgentId::Codex => render_codex_toml(&before, agent, &exe, op)?,
        AgentId::Antigravity => render_antigravity_json(&before, agent, &exe, op)?,
        AgentId::OpenCode | AgentId::Pi | AgentId::OhMyPi => {
            render_plugin_file(&before, agent, &exe, op)?
        }
        AgentId::Cline => unreachable!("Cline uses its bundle renderer"),
    };
    Ok(Rendered {
        path,
        format: format_of(agent),
        before,
        after,
        entries,
        creates,
    })
}

fn format_of(agent: AgentId) -> &'static str {
    match agent {
        AgentId::ClaudeCode | AgentId::Antigravity => "json",
        AgentId::Codex => "toml",
        AgentId::OpenCode | AgentId::Pi | AgentId::OhMyPi => "typescript",
        AgentId::Cline => "powershell bundle",
    }
}

// ---------------------------------------------------------------------------
// Claude Code + Antigravity: JSON
// ---------------------------------------------------------------------------

fn group_has_our_command(group: &serde_json::Value, agent: AgentId) -> bool {
    group
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|handlers| {
            handlers.iter().any(|h| {
                h.get("command")
                    .and_then(|c| c.as_str())
                    .map(|c| parse_our_command(agent, c).is_some())
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn render_claude_json(
    before: &str,
    agent: AgentId,
    exe: &Path,
    op: Op,
) -> Result<(String, BTreeMap<String, String>), String> {
    let mut doc: serde_json::Value = if before.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(before).map_err(|e| format!("settings.json is not valid JSON: {e}"))?
    };

    if !doc.is_object() {
        return Err("settings.json does not contain a JSON object".into());
    }

    let mut entries = BTreeMap::new();
    if op == Op::Install {
        let root = doc.as_object_mut().unwrap();
        let hooks = root.entry("hooks").or_insert_with(|| serde_json::json!({}));
        let hooks = hooks
            .as_object_mut()
            .ok_or_else(|| "settings.json `hooks` is not an object".to_string())?;

        for binding in bindings(agent) {
            let command = hook_command(exe, agent, binding.ember_event);
            let arr = hooks
                .entry(binding.config_event)
                .or_insert_with(|| serde_json::json!([]));
            let arr = arr.as_array_mut().ok_or_else(|| {
                format!(
                    "settings.json hooks.{} is not an array",
                    binding.config_event
                )
            })?;

            // Refresh our own handlers in place so command-shape upgrades,
            // matcher updates and moved executables propagate; install doubles
            // as repair.
            let mut found = false;
            for group in arr.iter_mut() {
                if !group_has_our_command(group, agent) {
                    continue;
                }
                found = true;
                if let Some(obj) = group.as_object_mut() {
                    match binding.matcher {
                        Some(m) => {
                            obj.insert("matcher".to_string(), serde_json::json!(m));
                        }
                        None => {
                            obj.remove("matcher");
                        }
                    }
                    if let Some(handlers) = obj.get_mut("hooks").and_then(|h| h.as_array_mut()) {
                        for h in handlers.iter_mut() {
                            let ours = h
                                .get("command")
                                .and_then(|c| c.as_str())
                                .map(|c| parse_our_command(agent, c).is_some())
                                .unwrap_or(false);
                            if ours && let Some(h_obj) = h.as_object_mut() {
                                h_obj.insert("command".to_string(), serde_json::json!(command));
                            }
                        }
                    }
                }
            }

            if !found {
                let handler = serde_json::json!({
                    "type": "command",
                    "command": command.clone(),
                    "timeout": 10,
                });
                let group = match binding.matcher {
                    Some(m) => serde_json::json!({ "matcher": m, "hooks": [handler] }),
                    None => serde_json::json!({ "hooks": [handler] }),
                };
                arr.push(group);
            }
            entries.insert(binding.config_event.to_string(), command);
        }
    } else {
        entries = load_manifest()
            .agents
            .get(agent.id())
            .map(|r| r.entries.clone())
            .unwrap_or_default();

        if let Some(hooks) = doc.get_mut("hooks").and_then(|h| h.as_object_mut()) {
            let events: Vec<String> = hooks.keys().cloned().collect();
            for event in events {
                if let Some(arr) = hooks.get_mut(&event).and_then(|a| a.as_array_mut()) {
                    arr.retain(|g| !group_has_our_command(g, agent));
                    if arr.is_empty() {
                        hooks.remove(&event);
                    }
                }
            }
            if hooks.is_empty() {
                doc.as_object_mut().unwrap().remove("hooks");
            }
        }
    }

    let after = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    Ok((after, entries))
}

fn render_antigravity_json(
    before: &str,
    agent: AgentId,
    exe: &Path,
    op: Op,
) -> Result<(String, BTreeMap<String, String>), String> {
    let mut doc: serde_json::Value = if before.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(before).map_err(|e| format!("hooks.json is not valid JSON: {e}"))?
    };
    if !doc.is_object() {
        return Err("hooks.json does not contain a JSON object".into());
    }

    // Antigravity keys hooks.json by hook *name*; "ember" is both the tag and
    // the removal handle.
    let hook_name = "ember";
    let mut entries = BTreeMap::new();
    let root = doc.as_object_mut().unwrap();

    if op == Op::Install {
        let mut hook: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
        for binding in bindings(agent) {
            let command = hook_command(exe, agent, binding.ember_event);
            let handler = serde_json::json!({
                "type": "command",
                "command": command,
                "timeout": 10,
            });
            let entry = match binding.matcher {
                Some(m) => serde_json::json!([{ "matcher": m, "hooks": [handler] }]),
                None => serde_json::json!([handler]),
            };
            hook.insert(binding.config_event.to_string(), entry);
            entries.insert(binding.config_event.to_string(), command);
        }
        root.insert(hook_name.to_string(), serde_json::Value::Object(hook));
    } else {
        entries = load_manifest()
            .agents
            .get(agent.id())
            .map(|r| r.entries.clone())
            .unwrap_or_default();
        root.remove(hook_name);
    }

    let after = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    Ok((after, entries))
}

// ---------------------------------------------------------------------------
// Codex: TOML (formatting and comments preserved by toml_edit)
// ---------------------------------------------------------------------------

fn render_codex_toml(
    before: &str,
    agent: AgentId,
    exe: &Path,
    op: Op,
) -> Result<(String, BTreeMap<String, String>), String> {
    let mut doc: DocumentMut = if before.trim().is_empty() {
        DocumentMut::new()
    } else {
        before
            .parse::<DocumentMut>()
            .map_err(|e| format!("config.toml is not valid TOML: {e}"))?
    };

    let mut entries = BTreeMap::new();

    if op == Op::Install {
        let root = doc.as_table_mut();
        let hooks_item = root.entry("hooks").or_insert(Item::Table(Table::new()));
        let hooks = hooks_item
            .as_table_mut()
            .ok_or_else(|| "config.toml `hooks` is not a table".to_string())?;

        for binding in bindings(agent) {
            let command = codex_hook_command(exe, binding.ember_event);
            let aot_item = hooks
                .entry(binding.config_event)
                .or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
            let aot = aot_item.as_array_of_tables_mut().ok_or_else(|| {
                format!(
                    "config.toml hooks.{} is not an array of tables",
                    binding.config_event
                )
            })?;

            // Refresh our own handlers in place so command-shape upgrades and
            // moved executables propagate; install doubles as repair.
            let mut found = false;
            for group in aot.iter_mut() {
                if let Some(handlers) = group
                    .get_mut("hooks")
                    .and_then(Item::as_array_of_tables_mut)
                {
                    for handler in handlers.iter_mut() {
                        let ours = handler
                            .get("command")
                            .and_then(Item::as_str)
                            .map(|c| parse_our_command(agent, c).is_some())
                            .unwrap_or(false);
                        if ours {
                            handler["command"] = value(command.clone());
                            handler["commandWindows"] = value(command.clone());
                            handler.remove("command_windows");
                            found = true;
                        }
                    }
                }
            }

            if !found {
                let mut handler = Table::new();
                handler["type"] = value("command");
                handler["command"] = value(command.clone());
                handler["commandWindows"] = value(command.clone());
                let mut handlers = ArrayOfTables::new();
                handlers.push(handler);

                let mut group = Table::new();
                if let Some(m) = binding.matcher {
                    group["matcher"] = value(m);
                }
                group["hooks"] = Item::ArrayOfTables(handlers);
                aot.push(group);
            }
            entries.insert(binding.config_event.to_string(), command);
        }
    } else {
        entries = load_manifest()
            .agents
            .get(agent.id())
            .map(|r| r.entries.clone())
            .unwrap_or_default();

        let root = doc.as_table_mut();
        if let Some(hooks) = root.get_mut("hooks").and_then(Item::as_table_mut) {
            let events: Vec<String> = hooks.iter().map(|(k, _)| k.to_string()).collect();
            for event in events {
                if let Some(aot) = hooks.get_mut(&event).and_then(Item::as_array_of_tables_mut) {
                    let mut keep: Vec<Table> = Vec::new();
                    for group in aot.iter() {
                        let ours = group
                            .get("hooks")
                            .and_then(Item::as_array_of_tables)
                            .map(|handlers| {
                                handlers.iter().any(|h| {
                                    h.get("command")
                                        .and_then(Item::as_str)
                                        .map(|c| parse_our_command(agent, c).is_some())
                                        .unwrap_or(false)
                                })
                            })
                            .unwrap_or(false);
                        if !ours {
                            keep.push(group.clone());
                        }
                    }
                    if keep.is_empty() {
                        hooks.remove(&event);
                    } else {
                        let mut replacement = ArrayOfTables::new();
                        for t in keep {
                            replacement.push(t);
                        }
                        hooks.insert(&event, Item::ArrayOfTables(replacement));
                    }
                }
            }
            if hooks.is_empty() {
                root.remove("hooks");
            }
        }
    }

    Ok((doc.to_string(), entries))
}

// ---------------------------------------------------------------------------
// OpenCode / Pi / Oh My Pi: a file we own end to end
// ---------------------------------------------------------------------------

const MANAGED_HEADER: &str =
    "// ember-hook-managed — installed by Ember. Safe to delete; Ember removes it cleanly.";

fn render_plugin_file(
    before: &str,
    agent: AgentId,
    exe: &Path,
    op: Op,
) -> Result<(String, BTreeMap<String, String>), String> {
    let mut entries = BTreeMap::new();
    for binding in bindings(agent) {
        entries.insert(
            binding.config_event.to_string(),
            hook_command(exe, agent, binding.ember_event),
        );
    }

    if op == Op::Remove {
        // Our file, our name: removing is deleting it.
        return Ok((String::new(), entries));
    }

    if !before.trim().is_empty() && before.starts_with(MANAGED_HEADER) {
        // Already ours; rewriting in place is the idempotent path.
    } else if !before.trim().is_empty() {
        return Err(format!(
            "{} already exists and was not created by Ember; refusing to overwrite it",
            target_path(agent)?.display()
        ));
    }

    let exe_js = exe.display().to_string().replace('\\', "\\\\");
    let body = match agent {
        AgentId::OpenCode => opencode_plugin(&exe_js),
        AgentId::Pi => pi_extension(&exe_js),
        AgentId::OhMyPi => ohmy_pi_extension(&exe_js),
        _ => return Err("no plugin template for this agent".into()),
    };
    Ok((format!("{MANAGED_HEADER}\n{body}"), entries))
}

fn opencode_plugin(exe: &str) -> String {
    format!(
        r#"import {{ spawnSync }} from "node:child_process";
const EMBER = "{exe}";
const AGENT = "opencode";
function fire(event, sessionId, cwd) {{
  try {{ spawnSync(EMBER, ["hook", AGENT, event], {{
    input: JSON.stringify({{ session_id: sessionId, cwd: cwd || "", pid: process.pid, ts: Math.floor(Date.now()/1000) }}),
    timeout: 600, windowsHide: true, stdio: ["pipe", "ignore", "ignore"],
  }}); }} catch {{ /* never break the agent */ }}
}}
const store = (globalThis.__emberOpenCodeHooks ||= {{ sessions: new Map(), heartbeat: null, exitHooked: false }});
function stateFor(id, cwd) {{
  let st=store.sessions.get(id);
  if (!st) {{ st={{working:false, active:false, executionOwned:false, pending:new Set(), cwd:cwd||"", lastFireAt:0}}; store.sessions.set(id,st); }}
  st.pending ||= new Set();
  if(cwd)st.cwd=cwd;
  return st;
}}
function markWorking(id, cwd, begin=false) {{
  if(!id)return;
  const st=stateFor(id,cwd);
  if(begin)st.active=true;
  if(!st.active || st.pending.size)return;
  st.revision=(st.revision||0)+1;
  const now=Date.now();
  if(st.working && now-st.lastFireAt<20000)return;
  st.working=true; st.lastFireAt=now; fire("working",id,st.cwd);
}}
function markIdle(id,cwd,terminal=true) {{
  if(!id)return;
  const st=stateFor(id,cwd), wasWorking=st.working;
  st.revision=(st.revision||0)+1;
  if(terminal){{st.active=false;st.executionOwned=false;st.pending.clear();}}
  st.working=false;
  if(wasWorking || terminal){{st.lastFireAt=Date.now();fire("idle",id,st.cwd);}}
}}
function markEnd(id,cwd) {{
  if(!id)return;
  const st=store.sessions.get(id);store.sessions.delete(id);fire("end",id,cwd||st?.cwd);
}}
function waiting(id,cwd,key) {{
  if(!id)return;
  const st=stateFor(id,cwd);
  st.pending.add(key);markIdle(id,cwd,false);
}}
function replied(id,cwd,key) {{
  const st=store.sessions.get(id);
  if(!st || !st.pending.delete(key))return;
  if(st.active && !st.pending.size)markWorking(id,cwd);
}}
function startHeartbeat() {{
  if(store.heartbeat)return;
  store.heartbeat=setInterval(()=>{{
    for(const [id,st] of store.sessions){{
      if(st.active && st.working && !st.pending?.size && Date.now()-st.lastFireAt>30000){{
        st.lastFireAt=Date.now();fire("working",id,st.cwd);
      }}
    }}
  }},30000);
  store.heartbeat.unref?.();
}}
if(!store.exitHooked){{
  store.exitHooked=true;
  const flush=()=>{{for(const [id,st] of store.sessions)fire("end",id,st.cwd);store.sessions.clear();}};
  process.once("exit",flush); process.once("SIGINT",flush); process.once("SIGTERM",flush);
}}
async function handle(ctx,input,v1=false){{
  const type=input?.type||"", data=input?.data||input?.properties||{{}};
  const info=data.info||{{}}, form=data.form||{{}};
  const id=data.sessionID||data.sessionId||form.sessionID||info.sessionID||info.id||(type.startsWith("session.")?data.id:"")||"";
  const cwd=data.cwd||data.directory||data.location?.directory||input?.location?.directory||info.directory||ctx?.location?.directory||ctx?.directory||"";
  const family=type.startsWith("permission.")?"permission":type.startsWith("form.")?"form":"question";
  const request=family+":"+(form.id||data.requestID||data.id||"default");
  switch(type){{
    case "session.created":
      if(id&&!store.sessions.has(id)){{stateFor(id,cwd);fire("start",id,cwd);}}break;
    case "session.deleted": markEnd(id,cwd);break;
    case "session.status":
      if(["busy","retry"].includes(data.status?.type))markWorking(id,cwd,true);
      else if(v1&&data.status?.type==="idle"&&!store.sessions.get(id)?.executionOwned)markIdle(id,cwd);
      break;
    case "session.idle": if(v1&&!store.sessions.get(id)?.executionOwned)markIdle(id,cwd);break;
    case "session.execution.started": if(id)stateFor(id,cwd).executionOwned=true;markWorking(id,cwd,true);break;
    case "session.execution.succeeded":
    case "session.execution.failed":
    case "session.execution.interrupted": markIdle(id,cwd);break;
    case "session.error": if(v1&&!store.sessions.get(id)?.executionOwned)markIdle(id,cwd);break;
    // Progress can sustain/resume an active run, but cannot resurrect a completed one.
    case "session.reasoning.started":
    case "session.reasoning.delta":
    case "session.tool.called":
    case "session.tool.progress":
    case "session.text.delta":
    case "session.retry.scheduled": markWorking(id,cwd);break;
    case "permission.asked":
    case "question.asked":
    case "form.created": waiting(id,cwd,request);break;
    case "permission.replied":
    case "question.replied":
    case "question.rejected":
    case "form.replied":
    case "form.cancelled": replied(id,cwd,request);break;
  }}
}}
export const EmberHooks=async(ctx)=>{{
  startHeartbeat();
  return {{
    event:async({{event}})=>{{try{{await handle(ctx,event,true);}}catch{{}}}},
    "tool.execute.before":async(input)=>{{
      const id=input?.sessionID||input?.sessionId||"";
      if(input?.tool==="question")waiting(id,ctx?.directory,"question:tool");
      else markWorking(id,ctx?.directory,true);
    }},
    "tool.execute.after":async(input)=>{{
      if(input?.tool==="question")replied(input?.sessionID||input?.sessionId,ctx?.directory,"question:tool");
    }},
  }};
}};
async function setup(ctx){{
  if(typeof ctx?.event?.subscribe!=="function")return async()=>{{}};
  startHeartbeat();
  const controller=new AbortController();
  const owned=new Set();
  let promptReg;
  if(typeof ctx.session?.hook==="function"){{
    promptReg=await ctx.session.hook("prompt",async p=>{{const id=p?.sessionID||p?.sessionId;if(id)owned.add(id);markWorking(id,p?.cwd||ctx.location?.directory,true);}});
  }}
  // A transport disconnect is not a completed execution. Reattach, and compare
  // the known sessions with the host's durable busy/idle times to catch a
  // terminal event missed while the stream was unavailable.
  let reconciling=false;
  const reconcile=async()=>{{
    if(reconciling||controller.signal.aborted)return;
    if(typeof ctx.session?.get!=="function")return;
    reconciling=true;
    try{{for(const id of owned){{
      const st=store.sessions.get(id);if(!st)continue;
      const revision=st.revision;
      try{{
        const result=await ctx.session.get({{sessionID:id}}),info=result?.data||result;
        if(controller.signal.aborted||store.sessions.get(id)!==st||st.revision!==revision)continue;
        const millis=v=>v==null?NaN:typeof v==="number"?v:typeof v.epochMillis==="number"?v.epochMillis:new Date(v).getTime();
        const busy=millis(info?.time?.busy),idle=millis(info?.time?.idle);
        if(Number.isFinite(idle)&&(!Number.isFinite(busy)||idle>=busy))markIdle(id,st.cwd);
        // Keep pending user input paused; silence never implies completion.
      }}catch{{ /* a temporary metadata failure is not a terminal event */ }}
    }}}}finally{{reconciling=false;}}
  }};
  const checking=setInterval(()=>{{void reconcile();}},2000);checking.unref?.();
  const delay=()=>new Promise(resolve=>{{
    const done=()=>{{clearTimeout(timer);controller.signal.removeEventListener("abort",done);resolve();}};
    const timer=setTimeout(done,500);controller.signal.addEventListener("abort",done,{{once:true}});
    if(controller.signal.aborted)done();
  }});
  const consuming=(async()=>{{while(!controller.signal.aborted){{
    try{{
      for await(const input of ctx.event.subscribe({{signal:controller.signal}})){{
        if(controller.signal.aborted)break;
        const data=input?.data||input?.properties||{{}};
        const id=data.sessionID||data.sessionId||data.form?.sessionID||data.info?.sessionID||data.info?.id;
        if(id)owned.add(id);
        try{{await handle(ctx,input);}}catch{{}}
      }}
    }}catch{{ /* reconnect below without releasing ongoing reasoning/tools */ }}
    if(!controller.signal.aborted){{await reconcile();await delay();}}
  }}}})();
  return async()=>{{controller.abort();clearInterval(checking);await promptReg?.dispose?.();await consuming;for(const id of owned)markEnd(id,store.sessions.get(id)?.cwd);}};
}}
export default {{ id: "ember-hooks", server: EmberHooks, setup }};
"#
    )
}

fn pi_extension(exe: &str) -> String {
    // Pi loads extensions from ~/.pi/agent/extensions and dispatches events
    // through `pi.on(name, handler)`. Event names follow the verified matrix in
    // docs/AGENT_INTEGRATIONS.md; Ember also watches the process tree, so
    // detection keeps working even if a name changes between releases.
    format!(
        r#"import {{ spawnSync }} from "node:child_process";

const EMBER = "{exe}";
const AGENT = "pi";

const running = new Set();
const waiting = new Set();
function fire(event, sessionId) {{
  if (event === "waiting") {{
    if (!running.has(sessionId)) return;
    waiting.add(sessionId);
    running.delete(sessionId);
  }}
  if (event === "resume") {{
    if (!waiting.delete(sessionId)) return;
    running.add(sessionId);
  }}
  if (event === "idle" || event === "end" || event === "start" || event === "working") waiting.delete(sessionId);
  if (event === "working") running.add(sessionId);
  if (event === "idle" || event === "end" || event === "start") running.delete(sessionId);
  try {{
    spawnSync(EMBER, ["hook", AGENT, event], {{
      input: JSON.stringify({{
        session_id: sessionId || "",
        cwd: process.cwd(),
        pid: process.pid,
        ts: Math.floor(Date.now() / 1000),
      }}),
      timeout: 600,
      windowsHide: true,
      stdio: ["pipe", "ignore", "ignore"],
    }});
  }} catch {{
    /* never break the host agent */
  }}
}}

export default function emberExtension(pi) {{
  let lastSessionId = "";
  const heartbeat = setInterval(() => {{
    for (const sessionId of running) fire("working", sessionId);
  }}, 30000);
  heartbeat.unref?.();
  const cleanup = () => {{
    clearInterval(heartbeat);
    for (const sessionId of [...running]) fire("end", sessionId);
    fire("end", lastSessionId);
  }};
  process.once("exit", cleanup);
  process.once("SIGINT", cleanup);
  process.once("SIGTERM", cleanup);

  const on = (name, handler) => {{
    try {{
      if (typeof pi?.on === "function") pi.on(name, handler);
    }} catch {{
      /* ignore */
    }}
  }};
  const id = (ctx, event) => {{
    try {{
      const fromManager =
        typeof ctx?.sessionManager?.getSessionId === "function"
          ? ctx.sessionManager.getSessionId()
          : "";
      const res =
        (typeof fromManager === "string" && fromManager) ||
        ctx?.sessionId ||
        ctx?.session_id ||
        ctx?.id ||
        event?.sessionId ||
        event?.session_id ||
        event?.id ||
        lastSessionId ||
        "";
      if (res) lastSessionId = String(res);
      return lastSessionId;
    }} catch {{
      return lastSessionId;
    }}
  }};
  on("session_start", (event, ctx) => fire("start", id(ctx, event)));
  on("before_agent_start", (event, ctx) => fire("working", id(ctx, event)));
  on("agent_start", (event, ctx) => fire("working", id(ctx, event)));
  on("turn_start", (event, ctx) => fire("working", id(ctx, event)));
  on("tool_call", (event, ctx) => {{
    const name = event?.toolName || event?.tool_name;
    const sessionId=id(ctx,event);
    if (["ask", "ask_user", "question", "AskUserQuestion"].includes(name)) fire("waiting",sessionId);
    else if (!waiting.has(sessionId)) fire("working",sessionId);
  }});
  on("tool_execution_end", (event, ctx) => {{
    if (["ask", "ask_user", "question", "AskUserQuestion"].includes(event?.toolName || event?.tool_name)) fire("resume", id(ctx, event));
  }});
  on("tool_execution_start", (event, ctx) => {{const sessionId=id(ctx,event);if(!waiting.has(sessionId))fire("working",sessionId);}});
  on("agent_settled", (event, ctx) => fire("idle", id(ctx, event)));
  on("ui_prompt_start", (event, ctx) => fire("waiting", id(ctx, event)));
  on("ui_prompt_end", (event, ctx) => fire("resume", id(ctx, event)));
  on("session_shutdown", (event, ctx) => fire("end", id(ctx, event)));
}}
"#
    )
}

fn ohmy_pi_extension(exe: &str) -> String {
    // Oh My Pi loads extensions from ~/.omp/agent/extensions and dispatches
    // events as `pi.on(name, (event, ctx) => ...)`: the first argument is the
    // event payload and the second is the extension context (whose
    // `sessionManager.getSessionId()` exposes the session id). Only terminal
    // `agent_end` (`event.willContinue !== true`) releases the hold.
    format!(
        r#"import {{ spawnSync }} from "node:child_process";

const EMBER = "{exe}";
const AGENT = "oh-my-pi";

const running = new Set();
const waiting = new Set();
function fire(event, sessionId) {{
  if (event === "waiting") {{
    if (!running.has(sessionId)) return;
    waiting.add(sessionId);
    running.delete(sessionId);
  }}
  if (event === "resume") {{
    if (!waiting.delete(sessionId)) return;
    running.add(sessionId);
  }}
  if (event === "idle" || event === "end" || event === "start" || event === "working") waiting.delete(sessionId);
  if (event === "working") running.add(sessionId);
  if (event === "idle" || event === "end" || event === "start") running.delete(sessionId);
  try {{
    spawnSync(EMBER, ["hook", AGENT, event], {{
      input: JSON.stringify({{
        session_id: sessionId || "",
        cwd: process.cwd(),
        pid: process.pid,
        ts: Math.floor(Date.now() / 1000),
      }}),
      timeout: 600,
      windowsHide: true,
      stdio: ["pipe", "ignore", "ignore"],
    }});
  }} catch {{
    /* never break the host agent */
  }}
}}

export default function emberExtension(pi) {{
  let lastSessionId = "";
  const heartbeat = setInterval(() => {{
    for (const sessionId of running) fire("working", sessionId);
  }}, 30000);
  heartbeat.unref?.();
  const cleanup = () => {{
    clearInterval(heartbeat);
    for (const sessionId of [...running]) fire("end", sessionId);
    fire("end", lastSessionId);
  }};
  process.once("exit", cleanup);
  process.once("SIGINT", cleanup);
  process.once("SIGTERM", cleanup);

  const on = (name, handler) => {{
    try {{
      if (typeof pi?.on === "function") pi.on(name, handler);
    }} catch {{
      /* ignore */
    }}
  }};
  const id = (ctx, event) => {{
    try {{
      const fromManager =
        typeof ctx?.sessionManager?.getSessionId === "function"
          ? ctx.sessionManager.getSessionId()
          : "";
      const res =
        (typeof fromManager === "string" && fromManager) ||
        ctx?.sessionId ||
        ctx?.session_id ||
        ctx?.id ||
        event?.sessionId ||
        event?.session_id ||
        event?.id ||
        lastSessionId ||
        "";
      if (res) lastSessionId = String(res);
      return lastSessionId;
    }} catch {{
      return lastSessionId;
    }}
  }};
  on("session_start", (event, ctx) => fire("start", id(ctx, event)));
  on("before_agent_start", (event, ctx) => fire("working", id(ctx, event)));
  on("agent_start", (event, ctx) => fire("working", id(ctx, event)));
  on("tool_call", (event, ctx) => {{
    const name = event?.toolName || event?.tool_name;
    const sessionId=id(ctx,event);
    if (["ask", "ask_user", "question", "AskUserQuestion"].includes(name)) fire("waiting",sessionId);
    else if (!waiting.has(sessionId)) fire("working",sessionId);
  }});
  on("tool_execution_end", (event, ctx) => {{
    if (["ask", "ask_user", "question", "AskUserQuestion"].includes(event?.toolName || event?.tool_name)) fire("resume", id(ctx, event));
  }});
  on("tool_execution_start", (event, ctx) => {{const sessionId=id(ctx,event);if(!waiting.has(sessionId))fire("working",sessionId);}});
  on("agent_end", (event, ctx) => {{
    if (event?.willContinue === true) return;
    fire("idle", id(ctx, event));
  }});
  on("session_shutdown", (event, ctx) => fire("end", id(ctx, event)));
}}
"#
    )
}

// ---------------------------------------------------------------------------
// Backup + apply
// ---------------------------------------------------------------------------

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn backup_path(file: &Path) -> PathBuf {
    let mut name = file.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".ember-backup-{}", timestamp()));
    file.with_file_name(name)
}

fn write_backup(file: &Path, contents: &str) -> Result<Option<String>, String> {
    if !file.exists() {
        return Ok(None);
    }
    let backup = backup_path(file);
    paths::ensure_parent(&backup);
    std::fs::write(&backup, contents).map_err(|e| format!("backup failed: {e}"))?;
    Ok(Some(backup.display().to_string()))
}

// ---------------------------------------------------------------------------
// Preview / apply API
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Add,
    Remove,
    Context,
}

#[derive(Serialize, Clone, Debug)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub text: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct ChangePreview {
    pub agent: String,
    pub name: String,
    pub mode: IntegrationMode,
    pub supported: bool,
    pub file: Option<String>,
    pub format: String,
    pub creates_file: bool,
    pub summary: String,
    pub note: String,
    pub backup: Option<String>,
    pub diff: Vec<DiffLine>,
    pub installed: bool,
    pub needs_repair: bool,
    pub can_install: bool,
    pub can_remove: bool,
    pub can_repair: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct InstallResult {
    pub ok: bool,
    pub message: String,
    pub file: Option<String>,
    pub backup: Option<String>,
    pub diff: Vec<DiffLine>,
    pub installed: bool,
    pub needs_repair: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct InstallStatus {
    pub installed: bool,
    pub needs_repair: bool,
    pub file: Option<String>,
    pub detail: String,
    pub last_event_unix: Option<u64>,
}

/// Compute the install/remove status of an agent from its config file.
pub fn status(agent: AgentId) -> InstallStatus {
    if agent == AgentId::Cline {
        return cline::status(&current_exe());
    }
    if !supported(agent) {
        return InstallStatus {
            installed: false,
            needs_repair: false,
            file: None,
            detail: "No global Windows hook surface; Ember watches the process tree instead."
                .into(),
            last_event_unix: None,
        };
    }
    let Ok(path) = target_path(agent) else {
        return InstallStatus {
            installed: false,
            needs_repair: false,
            file: None,
            detail: "could not resolve config path".into(),
            last_event_unix: None,
        };
    };
    let file = Some(path.display().to_string());

    let Ok(text) = std::fs::read_to_string(&path) else {
        return InstallStatus {
            installed: false,
            needs_repair: false,
            file,
            detail: format!("{} does not exist yet", path.display()),
            last_event_unix: None,
        };
    };

    let exe = current_exe();
    let (installed, stale) = match agent {
        AgentId::ClaudeCode => {
            let cmds = collect_json_commands(&text, agent);
            if cmds.is_empty() {
                (false, false)
            } else {
                let missing = cmds.len() < bindings(agent).len();
                let bad_exe = cmds.iter().any(|c| !command_matches_running_exe(c, &exe));
                let schema_drift = claude_needs_schema_repair(&text, agent);
                (true, missing || bad_exe || schema_drift)
            }
        }
        AgentId::Antigravity => {
            let cmds = collect_antigravity_commands(&text);
            if cmds.is_empty() {
                (false, false)
            } else {
                let missing = cmds.len() < bindings(agent).len();
                let bad_exe = cmds.iter().any(|c| !command_matches_running_exe(c, &exe));
                // Older installs wrote a flat array for PostToolUse (which
                // Antigravity requires to be matcher-grouped) or gated
                // PreToolUse without a decision on stdout.
                let schema_drift = antigravity_needs_schema_repair(&text);
                (true, missing || bad_exe || schema_drift)
            }
        }
        AgentId::Codex => {
            let cmds = collect_toml_commands(&text, agent);
            if cmds.is_empty() {
                (false, false)
            } else {
                let missing = codex_missing_bindings(&text);
                let bad_exe = cmds.iter().any(|c| !command_matches_running_exe(c, &exe));
                let bad_key = codex_has_legacy_windows_key(&text, agent);
                (true, missing || bad_exe || bad_key)
            }
        }
        AgentId::OpenCode | AgentId::Pi | AgentId::OhMyPi => {
            if !text.starts_with(MANAGED_HEADER) {
                (false, false)
            } else {
                let expected = render_plugin_file("", agent, &exe, Op::Install)
                    .map(|(after, _)| after)
                    .unwrap_or_default();
                let norm_have = text.replace("\r\n", "\n");
                let norm_want = expected.replace("\r\n", "\n");
                (true, norm_have != norm_want)
            }
        }
        AgentId::Cline => (false, false),
    };

    if !installed {
        return InstallStatus {
            installed: false,
            needs_repair: false,
            file,
            detail: "not installed".into(),
            last_event_unix: None,
        };
    }
    InstallStatus {
        installed: true,
        needs_repair: stale,
        file,
        detail: if stale {
            "installed, but the hook points at a different executable — repair to update it"
                .to_string()
        } else {
            "installed".to_string()
        },
        last_event_unix: None,
    }
}

fn collect_json_commands(text: &str, agent: AgentId) -> Vec<String> {
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(hooks) = doc.get("hooks").and_then(|h| h.as_object()) {
        for arr in hooks.values() {
            if let Some(arr) = arr.as_array() {
                for group in arr {
                    if let Some(handlers) = group.get("hooks").and_then(|h| h.as_array()) {
                        for h in handlers {
                            if let Some(c) = h.get("command").and_then(|c| c.as_str())
                                && parse_our_command(agent, c).is_some()
                            {
                                out.push(c.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn collect_antigravity_commands(text: &str) -> Vec<String> {
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let Some(hook) = doc.get("ember").and_then(|h| h.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in hook.values() {
        if let Some(arr) = entry.as_array() {
            for item in arr {
                if let Some(c) = item.get("command").and_then(|c| c.as_str()) {
                    out.push(c.to_string());
                }
                if let Some(handlers) = item.get("hooks").and_then(|h| h.as_array()) {
                    for h in handlers {
                        if let Some(c) = h.get("command").and_then(|c| c.as_str()) {
                            out.push(c.to_string());
                        }
                    }
                }
            }
        }
    }
    out
}

fn collect_toml_commands(text: &str, agent: AgentId) -> Vec<String> {
    let Ok(doc) = text.parse::<DocumentMut>() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let Some(hooks) = doc.get("hooks").and_then(Item::as_table) else {
        return out;
    };
    for (_, item) in hooks.iter() {
        if let Some(aot) = item.as_array_of_tables() {
            for group in aot.iter() {
                if let Some(handlers) = group.get("hooks").and_then(Item::as_array_of_tables) {
                    for h in handlers.iter() {
                        if let Some(c) = h.get("command").and_then(Item::as_str)
                            && parse_our_command(agent, c).is_some()
                        {
                            out.push(c.to_string());
                        }
                    }
                }
            }
        }
    }
    out
}

fn claude_needs_schema_repair(text: &str, agent: AgentId) -> bool {
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(text) else {
        return false;
    };
    let Some(hooks) = doc.get("hooks").and_then(|h| h.as_object()) else {
        return false;
    };
    for binding in bindings(agent) {
        let Some(arr) = hooks.get(binding.config_event).and_then(|a| a.as_array()) else {
            return true;
        };
        for group in arr {
            if group_has_our_command(group, agent) {
                let have_matcher = group.get("matcher").and_then(|m| m.as_str());
                if have_matcher != binding.matcher {
                    return true;
                }
            }
        }
    }
    false
}

fn antigravity_needs_schema_repair(text: &str) -> bool {
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(text) else {
        return false;
    };
    let Some(hook) = doc.get("ember").and_then(|h| h.as_object()) else {
        return false;
    };
    // Legacy installs included PreToolUse (which blocks tool calls without a
    // stdout decision) or wrote PostToolUse as a flat array without a matcher.
    if hook.contains_key("PreToolUse") {
        return true;
    }
    for binding in bindings(AgentId::Antigravity) {
        let Some(arr) = hook.get(binding.config_event).and_then(|a| a.as_array()) else {
            return true;
        };
        for item in arr {
            let has_matcher = item.get("matcher").and_then(|m| m.as_str());
            if has_matcher != binding.matcher {
                return true;
            }
        }
    }
    false
}

fn codex_missing_bindings(text: &str) -> bool {
    let Ok(doc) = text.parse::<DocumentMut>() else {
        return true;
    };
    bindings(AgentId::Codex).iter().any(|binding| {
        !doc.get("hooks")
            .and_then(Item::as_table)
            .and_then(|h| h.get(binding.config_event))
            .and_then(Item::as_array_of_tables)
            .is_some_and(|groups| {
                groups.iter().any(|g| {
                    g.get("hooks")
                        .and_then(Item::as_array_of_tables)
                        .is_some_and(|handlers| {
                            handlers.iter().any(|h| {
                                h.get("command")
                                    .and_then(Item::as_str)
                                    .is_some_and(|command| {
                                        parse_our_command(AgentId::Codex, command)
                                            == Some(binding.ember_event)
                                    })
                            })
                        })
                })
            })
    })
}

fn codex_has_legacy_windows_key(text: &str, agent: AgentId) -> bool {
    let Ok(doc) = text.parse::<DocumentMut>() else {
        return false;
    };
    let Some(hooks) = doc.get("hooks").and_then(Item::as_table) else {
        return false;
    };
    for (_, item) in hooks.iter() {
        if let Some(aot) = item.as_array_of_tables() {
            for group in aot.iter() {
                if let Some(handlers) = group.get("hooks").and_then(Item::as_array_of_tables) {
                    for h in handlers.iter() {
                        let ours = h
                            .get("command")
                            .and_then(Item::as_str)
                            .map(|c| parse_our_command(agent, c).is_some())
                            .unwrap_or(false);
                        if ours
                            && (h.get("command_windows").is_some()
                                || h.get("commandWindows").is_none())
                        {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

pub fn preview(agent: AgentId, op: Op) -> Result<ChangePreview, String> {
    let def = agent.def();
    let st = status(agent);

    if !supported(agent) {
        return Ok(ChangePreview {
            agent: agent.id().to_string(),
            name: def.name.to_string(),
            mode: IntegrationMode::Process,
            supported: false,
            file: None,
            format: "none".into(),
            creates_file: false,
            summary: "Process detection only".into(),
            note: def.integration_note().to_string(),
            backup: None,
            diff: Vec::new(),
            installed: false,
            needs_repair: false,
            can_install: false,
            can_remove: false,
            can_repair: false,
        });
    }

    let rendered = render(agent, op)?;
    let diff = diff_lines(&rendered.before, &rendered.after);
    let backup = if rendered.creates || agent == AgentId::Cline {
        None
    } else {
        Some(backup_path(&rendered.path).display().to_string())
    };

    let verb = match op {
        Op::Install => "Add Ember hooks to",
        Op::Remove => "Remove Ember hooks from",
    };

    Ok(ChangePreview {
        agent: agent.id().to_string(),
        name: def.name.to_string(),
        mode: def.primary_mode(),
        supported: true,
        file: Some(rendered.path.display().to_string()),
        format: rendered.format.to_string(),
        creates_file: rendered.creates,
        summary: format!("{verb} {}", rendered.path.display()),
        note: def.integration_note().to_string(),
        backup,
        diff,
        installed: st.installed,
        needs_repair: st.needs_repair,
        can_install: op == Op::Install,
        can_remove: op == Op::Remove && st.installed,
        can_repair: st.needs_repair,
    })
}

pub fn apply(agent: AgentId, op: Op) -> Result<InstallResult, String> {
    if agent == AgentId::Cline {
        return cline::apply(op, &current_exe());
    }
    if !supported(agent) {
        return Err("This integration has no installable hook surface".into());
    }
    let exe = current_exe();
    let rendered = render(agent, op)?;
    // Codex hook commands reference the space-free launcher; keep it in sync
    // with the running executable (see `codex_launcher_path`).
    if agent == AgentId::Codex {
        apply_codex_launcher(&exe, op)?;
    }
    let file = rendered.path.display().to_string();
    let backup = write_backup(&rendered.path, &rendered.before)?;
    let diff = diff_lines(&rendered.before, &rendered.after);

    if rendered.after.trim().is_empty() {
        // Removal of a file we own: delete it.
        match std::fs::remove_file(&rendered.path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("could not remove {}: {e}", rendered.path.display())),
        }
    } else {
        paths::ensure_parent(&rendered.path);
        crate::settings::write_atomic(&rendered.path, rendered.after.as_bytes())?;
    }

    // Update the ledger.
    let mut manifest = load_manifest();
    match op {
        Op::Install => {
            manifest.agents.insert(
                agent.id().to_string(),
                InstalledRecord {
                    file: file.clone(),
                    format: rendered.format.to_string(),
                    entries: rendered.entries.clone(),
                    installed_unix: timestamp(),
                },
            );
        }
        Op::Remove => {
            manifest.agents.remove(agent.id());
        }
    }
    save_manifest(&manifest)?;

    let st = status(agent);
    let message = match op {
        Op::Install => format!("Installed Ember hooks for {}", agent.def().name),
        Op::Remove => format!("Removed Ember hooks for {}", agent.def().name),
    };
    Ok(InstallResult {
        ok: true,
        message,
        file: Some(file),
        backup,
        diff,
        installed: st.installed,
        needs_repair: st.needs_repair,
    })
}

/// Keep `%USERPROFILE%\.ember\hooks\ember-hook.cmd` in sync with the running
/// executable so Codex hook commands never need quoted (spacey) paths.
fn apply_codex_launcher(exe: &Path, op: Op) -> Result<(), String> {
    let path = match codex_launcher_path() {
        Ok(p) => p,
        // No space-free path available: hook commands use the quoted shape.
        Err(_) => return Ok(()),
    };
    match op {
        Op::Install => {
            paths::ensure_parent(&path);
            crate::settings::write_atomic(&path, launcher_contents(exe).as_bytes())
                .map_err(|e| format!("could not write launcher {}: {e}", path.display()))
        }
        Op::Remove => match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("could not remove launcher {}: {e}", path.display())),
        },
    }
}

// ---------------------------------------------------------------------------
// Line diff (small LCS; config files are tiny)
// ---------------------------------------------------------------------------

const MAX_DIFF_LINES: usize = 400;

pub fn diff_lines(before: &str, after: &str) -> Vec<DiffLine> {
    if before == after {
        return Vec::new();
    }
    let a: Vec<&str> = if before.is_empty() {
        Vec::new()
    } else {
        before.lines().collect()
    };
    let b: Vec<&str> = if after.is_empty() {
        Vec::new()
    } else {
        after.lines().collect()
    };

    // LCS table
    let n = a.len();
    let m = b.len();
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut out: Vec<DiffLine> = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            out.push(DiffLine {
                kind: DiffKind::Context,
                text: a[i].to_string(),
            });
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push(DiffLine {
                kind: DiffKind::Remove,
                text: a[i].to_string(),
            });
            i += 1;
        } else {
            out.push(DiffLine {
                kind: DiffKind::Add,
                text: b[j].to_string(),
            });
            j += 1;
        }
        if out.len() > MAX_DIFF_LINES {
            out.push(DiffLine {
                kind: DiffKind::Context,
                text: "… diff truncated …".to_string(),
            });
            return out;
        }
    }
    while i < n {
        out.push(DiffLine {
            kind: DiffKind::Remove,
            text: a[i].to_string(),
        });
        i += 1;
    }
    while j < m {
        out.push(DiffLine {
            kind: DiffKind::Add,
            text: b[j].to_string(),
        });
        j += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe() -> PathBuf {
        PathBuf::from(r"C:\Program Files\Ember\Ember.exe")
    }

    #[test]
    fn codex_repair_checks_interrupt_binding_even_with_duplicate_other_hooks() {
        let (rendered, _) = render_codex_toml("", AgentId::Codex, &exe(), Op::Install).unwrap();
        assert!(!codex_missing_bindings(&rendered));
        let mut doc = rendered.parse::<DocumentMut>().unwrap();
        doc["hooks"].as_table_mut().unwrap().remove("Interrupt");
        assert!(codex_missing_bindings(&doc.to_string()));
        let (repaired, _) =
            render_codex_toml(&doc.to_string(), AgentId::Codex, &exe(), Op::Install).unwrap();
        assert!(!codex_missing_bindings(&repaired));
    }

    #[test]
    fn generated_plugins_execute_lifecycle_contracts() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        for agent in [AgentId::Pi, AgentId::OhMyPi, AgentId::OpenCode] {
            let (source, _) = render_plugin_file("", agent, &exe(), Op::Install).unwrap();
            let mut child = Command::new("node")
                .arg("-e")
                .arg(include_str!(
                    "../../../scripts/check-generated-integrations.cjs"
                ))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("Node is required for generated integration contract tests");
            let request = serde_json::json!({"agent":agent.id(),"source":source}).to_string();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(request.as_bytes())
                .unwrap();
            let result = child.wait_with_output().unwrap();
            assert!(
                result.status.success(),
                "{}: {}",
                agent.id(),
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }

    #[test]
    fn hook_command_shape_matches_spec() {
        let cmd = hook_command(&exe(), AgentId::ClaudeCode, HookEvent::Working);
        assert_eq!(
            cmd,
            r#""C:\Program Files\Ember\Ember.exe" hook claude-code working"#
        );
    }

    #[test]
    fn our_commands_are_recognised_and_foreign_ones_are_not() {
        let cmd = hook_command(&exe(), AgentId::Codex, HookEvent::Idle);
        assert_eq!(
            parse_our_command(AgentId::Codex, &cmd),
            Some(HookEvent::Idle)
        );
        assert_eq!(parse_our_command(AgentId::ClaudeCode, &cmd), None);
        assert_eq!(
            parse_our_command(AgentId::Codex, "prettier --write ."),
            None
        );
        assert_eq!(
            parse_our_command(AgentId::Codex, r#""C:\other\Thing.exe" hook codex idle"#),
            None
        );
    }

    #[test]
    fn codex_launcher_commands_are_recognised() {
        let cmd = r"C:\Users\me\.ember\hooks\ember-hook.cmd codex idle";
        assert_eq!(
            parse_our_command(AgentId::Codex, cmd),
            Some(HookEvent::Idle)
        );
        assert_eq!(parse_our_command(AgentId::ClaudeCode, cmd), None);
        assert_eq!(
            parse_our_command(
                AgentId::Codex,
                r"C:\Users\me\.ember\hooks\other.cmd codex idle"
            ),
            None
        );
    }

    #[test]
    fn claude_json_merge_is_idempotent_and_preserves_user_hooks() {
        let before = r#"{
  "model": "opus",
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [ { "type": "command", "command": "my-guard.sh" } ] }
    ]
  }
}"#;
        let (once, entries) =
            render_claude_json(before, AgentId::ClaudeCode, &exe(), Op::Install).unwrap();
        let (twice, _) =
            render_claude_json(&once, AgentId::ClaudeCode, &exe(), Op::Install).unwrap();
        assert_eq!(once, twice, "install must be idempotent");
        assert!(once.contains("my-guard.sh"), "user hook preserved");
        assert!(once.contains("opus"));
        for binding in bindings(AgentId::ClaudeCode) {
            assert!(once.contains(binding.config_event));
        }
        assert_eq!(entries.len(), bindings(AgentId::ClaudeCode).len());
    }

    #[test]
    fn claude_json_remove_restores_original() {
        let before = r#"{
  "model": "opus",
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [ { "type": "command", "command": "my-guard.sh" } ] }
    ]
  }
}"#;
        let (installed, _) =
            render_claude_json(before, AgentId::ClaudeCode, &exe(), Op::Install).unwrap();
        let (removed, _) =
            render_claude_json(&installed, AgentId::ClaudeCode, &exe(), Op::Remove).unwrap();
        let orig: serde_json::Value = serde_json::from_str(before).unwrap();
        let back: serde_json::Value = serde_json::from_str(&removed).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn claude_json_rejects_broken_input() {
        assert!(
            render_claude_json("{ not json", AgentId::ClaudeCode, &exe(), Op::Install).is_err()
        );
        assert!(render_claude_json("[]", AgentId::ClaudeCode, &exe(), Op::Install).is_err());
        assert!(
            render_claude_json(
                r#"{"hooks": "nope"}"#,
                AgentId::ClaudeCode,
                &exe(),
                Op::Install
            )
            .is_err()
        );
    }

    #[test]
    fn codex_toml_edit_preserves_comments_and_is_idempotent() {
        let before = "# my codex config\nmodel = \"gpt-5-codex\" # favourite\n\n[history]\npersistence = \"save-all\"\n";
        let (once, _) = render_codex_toml(before, AgentId::Codex, &exe(), Op::Install).unwrap();
        let (twice, _) = render_codex_toml(&once, AgentId::Codex, &exe(), Op::Install).unwrap();
        assert_eq!(once, twice, "install must be idempotent");
        assert!(once.contains("# my codex config"), "comment preserved");
        assert!(once.contains("# favourite"), "inline comment preserved");
        assert!(once.contains("[history]"));
        assert!(once.contains("[[hooks.SessionStart]]"));
        assert!(once.contains("commandWindows"));
        // still parses as TOML
        once.parse::<DocumentMut>().expect("valid toml");
    }

    #[test]
    fn codex_install_repairs_stale_and_old_shape_commands() {
        let before = r#"# cfg

[[hooks.SessionStart]]
[[hooks.SessionStart.hooks]]
type = "command"
command = '"C:\Old\Place\ember.exe" hook codex start'
command_windows = '"C:\Old\Place\ember.exe" hook codex start'
"#;
        let (once, _) = render_codex_toml(before, AgentId::Codex, &exe(), Op::Install).unwrap();
        assert!(!once.contains("Old"), "stale commands must be refreshed");
        assert!(
            !once.contains("command_windows"),
            "legacy snake_case key removed"
        );
        assert!(once.contains("commandWindows"), "camelCase key written");
        let (twice, _) = render_codex_toml(&once, AgentId::Codex, &exe(), Op::Install).unwrap();
        assert_eq!(once, twice, "repair must be idempotent");
    }

    #[test]
    fn codex_toml_remove_restores_original_semantics() {
        let before = "# keep me\nmodel = \"gpt-5-codex\"\n";
        let (installed, _) =
            render_codex_toml(before, AgentId::Codex, &exe(), Op::Install).unwrap();
        let (removed, _) =
            render_codex_toml(&installed, AgentId::Codex, &exe(), Op::Remove).unwrap();
        assert!(removed.contains("# keep me"));
        assert!(removed.contains("model = \"gpt-5-codex\""));
        assert!(!removed.contains("hooks"));
    }

    #[test]
    fn codex_toml_rejects_broken_input() {
        assert!(render_codex_toml("not = = toml", AgentId::Codex, &exe(), Op::Install).is_err());
    }

    #[test]
    fn antigravity_uses_named_hook_key_and_removes_cleanly() {
        let before = r#"{ "mine": { "Stop": [ { "type": "command", "command": "./mine.sh" } ] } }"#;
        let (installed, _) =
            render_antigravity_json(before, AgentId::Antigravity, &exe(), Op::Install).unwrap();
        assert!(installed.contains("\"ember\""));
        assert!(installed.contains("./mine.sh"), "other hooks preserved");
        let (removed, _) =
            render_antigravity_json(&installed, AgentId::Antigravity, &exe(), Op::Remove).unwrap();
        let orig: serde_json::Value = serde_json::from_str(before).unwrap();
        let back: serde_json::Value = serde_json::from_str(&removed).unwrap();
        assert_eq!(orig, back);
    }

    #[test]
    fn plugin_files_are_written_removed_and_refuse_foreign_files() {
        for agent in [AgentId::OpenCode, AgentId::Pi, AgentId::OhMyPi] {
            let (written, entries) = render_plugin_file("", agent, &exe(), Op::Install).unwrap();
            assert!(written.starts_with(MANAGED_HEADER), "{}", agent.id());
            assert!(written.contains("hook"), "{}", agent.id());
            assert_eq!(entries.len(), bindings(agent).len());

            // idempotent rewrite
            let (again, _) = render_plugin_file(&written, agent, &exe(), Op::Install).unwrap();
            assert_eq!(written, again);

            // removal yields empty text => file deleted
            let (gone, _) = render_plugin_file(&written, agent, &exe(), Op::Remove).unwrap();
            assert!(gone.is_empty());

            // a foreign file is never clobbered
            assert!(
                render_plugin_file("// someone else's plugin\n", agent, &exe(), Op::Install)
                    .is_err()
            );
        }
    }

    #[test]
    fn plugin_embeds_the_exe_path_escaped() {
        let (written, _) = render_plugin_file("", AgentId::OpenCode, &exe(), Op::Install).unwrap();
        assert!(written.contains(r"C:\\Program Files\\Ember\\Ember.exe"));
    }

    #[test]
    fn opencode_plugin_maps_v2_run_lifecycle_and_survives_long_turns() {
        let (js, _) = render_plugin_file("", AgentId::OpenCode, &exe(), Op::Install).unwrap();

        // A finished tool must not idle the row; only the run lifecycle ends it.
        assert!(js.contains("tool.execute.after")); // only explicit question completion resumes a wait
        assert!(!js.contains("session.error\") fire"));

        // The v2 run lifecycle is mapped: started = Working, terminal = Idle.
        assert!(js.contains("session.execution.started"));
        assert!(js.contains("session.execution.succeeded"));
        assert!(js.contains("session.execution.failed"));
        assert!(js.contains("session.execution.interrupted"));

        // v2 hosts instantiate the default export's setup(), not the v1 factory;
        // the factory stays as a compatibility shim.
        assert!(js.contains("export default"));
        assert!(js.contains("async function setup(ctx)"));
        assert!(js.contains("EmberHooks"));

        // Long silent turns are kept fresh by a heartbeat, dedupe keeps event
        // storms cheap, and state survives the host's plugin hot reload.
        assert!(js.contains("heartbeat"));
        assert!(js.contains("globalThis"));
        assert!(js.contains("20000"));

        // Folder attribution comes from the created event's location.
        assert!(js.contains("location"));

        // Dead v1-only names must not linger.
        assert!(!js.contains("session.next."));

        // Mid-run idle-ish events are intentionally unmapped: only the
        // terminal execution events may end a run.
        assert!(
            js.contains(r#"case "session.idle": if(v1&&!store.sessions.get(id)?.executionOwned)"#)
        );
        assert!(!js.contains("else if (st === \"idle\")"));
    }

    #[test]
    fn ohmy_pi_extension_uses_real_omp_events_and_two_arg_handlers() {
        let (ts, entries) = render_plugin_file("", AgentId::OhMyPi, &exe(), Op::Install).unwrap();

        // Real omp lifecycle events from orca-agent-status.ts
        for ev in [
            "session_start",
            "before_agent_start",
            "agent_start",
            "tool_call",
            "tool_execution_start",
            "agent_end",
            "session_shutdown",
        ] {
            assert!(ts.contains(&format!("\"{ev}\"")), "missing {ev}");
            assert!(entries.contains_key(ev), "missing binding {ev}");
        }

        // Two-argument handler signature: (event, ctx) => ...
        assert!(ts.contains("(event, ctx) =>"));
        assert!(ts.contains("sessionManager?.getSessionId"));
        assert!(ts.contains("event?.willContinue === true"));

        assert!(!ts.contains("agent_settled"));

        // Stale event names must not appear
        assert!(!ts.contains("session_stop"));
        assert!(!ts.contains("turn_start"));
    }

    #[test]
    fn pi_extension_uses_two_arg_handlers_and_covers_modern_and_legacy_events() {
        let (ts, entries) = render_plugin_file("", AgentId::Pi, &exe(), Op::Install).unwrap();
        for ev in [
            "session_start",
            "before_agent_start",
            "agent_start",
            "turn_start",
            "tool_call",
            "tool_execution_start",
            "agent_settled",
            "session_shutdown",
        ] {
            assert!(ts.contains(&format!("\"{ev}\"")), "missing {ev}");
            assert!(entries.contains_key(ev), "missing binding {ev}");
        }
        assert!(ts.contains("(event, ctx) =>"));
        assert!(ts.contains("sessionManager?.getSessionId"));
        assert!(!ts.contains("on(\"agent_end\""));
    }

    #[test]
    fn claude_install_repairs_stale_commands_and_adds_idle_prompt_matcher() {
        let before = r#"{
  "hooks": {
    "Notification": [
      { "hooks": [ { "type": "command", "command": "\"C:\\Old\\Ember.exe\" hook claude-code idle", "timeout": 10 } ] }
    ]
  }
}"#;
        assert!(claude_needs_schema_repair(before, AgentId::ClaudeCode));
        let (repaired, _) =
            render_claude_json(before, AgentId::ClaudeCode, &exe(), Op::Install).unwrap();
        assert!(!repaired.contains("C:\\\\Old\\\\Ember.exe"));
        assert!(repaired.contains("\"matcher\": \"idle_prompt\""));
        assert!(!claude_needs_schema_repair(&repaired, AgentId::ClaudeCode));
    }

    #[test]
    fn antigravity_uses_non_blocking_invocation_and_grouped_posttool_hooks() {
        let (installed, entries) =
            render_antigravity_json("{}", AgentId::Antigravity, &exe(), Op::Install).unwrap();
        assert!(
            !installed.contains("PreToolUse"),
            "PreToolUse blocks tools without a stdout decision"
        );
        assert!(!entries.contains_key("PreToolUse"));
        assert!(entries.contains_key("PreInvocation"));
        assert!(entries.contains_key("PostInvocation"));
        assert!(entries.contains_key("PostToolUse"));
        assert!(entries.contains_key("SessionStart"));
        assert!(entries.contains_key("Stop"));
        assert!(!antigravity_needs_schema_repair(&installed));

        // Legacy schema (with PreToolUse and ungrouped PostToolUse) triggers repair
        let legacy = r#"{
  "ember": {
    "PreToolUse": [{ "matcher": "*", "hooks": [{ "type": "command", "command": "x" }] }],
    "PostToolUse": [{ "type": "command", "command": "x" }]
  }
}"#;
        assert!(antigravity_needs_schema_repair(legacy));
    }

    #[test]
    fn cline_exposes_the_current_windows_hook_contract() {
        assert!(supported(AgentId::Cline));
        assert!(
            bindings(AgentId::Cline)
                .iter()
                .any(|b| b.config_event == "TaskCancel" && b.ember_event == HookEvent::Idle)
        );
        assert!(
            target_path(AgentId::Cline)
                .unwrap()
                .ends_with("UserPromptSubmit.ps1")
        );
    }

    #[test]
    fn diff_lines_marks_adds_removes_and_context() {
        let d = diff_lines("a\nb\nc", "a\nB\nc");
        assert!(
            d.iter()
                .any(|l| matches!(l.kind, DiffKind::Remove) && l.text == "b")
        );
        assert!(
            d.iter()
                .any(|l| matches!(l.kind, DiffKind::Add) && l.text == "B")
        );
        assert!(
            d.iter()
                .any(|l| matches!(l.kind, DiffKind::Context) && l.text == "a")
        );
        assert!(diff_lines("same", "same").is_empty());
        assert!(!diff_lines("", "x").is_empty());
    }

    #[test]
    fn every_supported_agent_has_bindings() {
        for agent in AgentId::ALL {
            if supported(agent) {
                assert!(!bindings(agent).is_empty(), "{}", agent.id());
            }
        }
    }

    #[test]
    fn all_bindings_are_canonical_events() {
        for agent in AgentId::ALL {
            for binding in bindings(agent) {
                assert_eq!(
                    HookEvent::parse(binding.ember_event.as_str()),
                    Some(binding.ember_event)
                );
            }
        }
    }
}
