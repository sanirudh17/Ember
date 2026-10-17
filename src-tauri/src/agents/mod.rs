//! Agent registry: the seven tools Ember can watch.
//!
//! Each agent gets a generated monogram tile (rounded square, distinct muted
//! hue, one or two letters) — never a third-party logo.

pub mod hookserver;
pub mod installers;
pub mod process;
pub mod registry;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum AgentId {
    ClaudeCode,
    Codex,
    OpenCode,
    Pi,
    OhMyPi,
    Antigravity,
    Cline,
}

impl AgentId {
    pub const ALL: [AgentId; 7] = [
        AgentId::ClaudeCode,
        AgentId::Codex,
        AgentId::OpenCode,
        AgentId::Pi,
        AgentId::OhMyPi,
        AgentId::Antigravity,
        AgentId::Cline,
    ];

    /// Stable id used in settings, the hook CLI and the config files we touch.
    pub fn id(self) -> &'static str {
        match self {
            AgentId::ClaudeCode => "claude-code",
            AgentId::Codex => "codex",
            AgentId::OpenCode => "opencode",
            AgentId::Pi => "pi",
            AgentId::OhMyPi => "oh-my-pi",
            AgentId::Antigravity => "antigravity",
            AgentId::Cline => "cline",
        }
    }

    pub fn parse(id: &str) -> Option<AgentId> {
        AgentId::ALL.into_iter().find(|a| a.id() == id)
    }

    pub fn def(self) -> AgentDef {
        match self {
            AgentId::ClaudeCode => AgentDef {
                id: self,
                name: "Claude Code",
                monogram: "CC",
                hue: 18,
                executables: &["claude.exe", "claude.cmd", "claude", "claude-code.exe"],
                cmdline_matchers: &["claude", "@anthropic-ai/claude-code"],
                config_dir: || dirs::home_dir().map(|h| h.join(".claude")),
            },
            AgentId::Codex => AgentDef {
                id: self,
                name: "Codex",
                monogram: "CX",
                hue: 205,
                executables: &["codex.exe", "codex.cmd", "codex"],
                cmdline_matchers: &["codex", "@openai/codex"],
                config_dir: || dirs::home_dir().map(|h| h.join(".codex")),
            },
            AgentId::OpenCode => AgentDef {
                id: self,
                name: "OpenCode",
                monogram: "OC",
                hue: 158,
                // `node.exe` / `bun.exe` are deliberately absent: they are
                // runtime hosts shared by every JS agent, so they are treated
                // as a last-resort fallback (`hookclient::GENERIC_HOSTS`) and
                // must never make `detect()` claim the agent is installed.
                executables: &[
                    "opencode.exe",
                    "opencode.cmd",
                    "opencode",
                    "opencode2.exe",
                    "opencode2.cmd",
                    "opencode2",
                ],
                cmdline_matchers: &["opencode", "opencode2", "@opencode/cli"],
                config_dir: || dirs::home_dir().map(|h| h.join(".config").join("opencode")),
            },
            AgentId::Pi => AgentDef {
                id: self,
                name: "Pi",
                monogram: "PI",
                hue: 268,
                executables: &["pi.exe", "pi.cmd", "pi"],
                // The package scope moved from @mariozechner to @earendil-works;
                // both are listed so detection spans old and new installs.
                cmdline_matchers: &[
                    "pi-coding-agent",
                    "@mariozechner/pi",
                    "@earendil-works/pi",
                    "pi.dev",
                ],
                config_dir: || dirs::home_dir().map(|h| h.join(".pi")),
            },
            AgentId::OhMyPi => AgentDef {
                id: self,
                name: "Oh My Pi",
                monogram: "OM",
                hue: 322,
                executables: &["omp.exe", "omp.cmd", "omp"],
                cmdline_matchers: &["oh-my-pi", "@oh-my-pi/", "omp"],
                config_dir: || dirs::home_dir().map(|h| h.join(".omp")),
            },
            AgentId::Antigravity => AgentDef {
                id: self,
                name: "Antigravity CLI",
                monogram: "AG",
                hue: 285,
                // The Antigravity CLI ships as `agy`. `gemini.exe` is a
                // different, unrelated CLI, so it is deliberately not listed.
                // `Antigravity.exe` is the Electron desktop/IDE app and sidecar
                // runtime, not the CLI, so it is deliberately excluded.
                executables: &["agy.exe", "agy.cmd", "agy"],
                cmdline_matchers: &["antigravity", "agy"],
                config_dir: || dirs::home_dir().map(|h| h.join(".gemini")),
            },
            AgentId::Cline => AgentDef {
                id: self,
                name: "Cline",
                monogram: "CL",
                hue: 96,
                // `code.exe` / `cursor.exe` are shared editor hosts and would
                // make Cline look installed on any machine with VS Code, so they
                // are intentionally excluded. Cline is matched by its own CLI or
                // by the `cline-core` helper on a command line.
                executables: &["cline.exe", "cline.cmd", "cline"],
                cmdline_matchers: &[
                    "cline",
                    "cline-core",
                    "@cline/",
                    "claude-dev",
                    "saoudrizwan.claude-dev",
                ],
                config_dir: || dirs::home_dir().map(|h| h.join(".cline")),
            },
        }
    }
}

/// How Ember talks to a given agent.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationMode {
    /// A documented hook/plugin/extension mechanism we install.
    Hooks,
    /// Nothing documented for Windows; we watch the process tree instead.
    Process,
}

pub struct AgentDef {
    pub id: AgentId,
    pub name: &'static str,
    pub monogram: &'static str,
    pub hue: u16,
    pub executables: &'static [&'static str],
    pub cmdline_matchers: &'static [&'static str],
    pub config_dir: fn() -> Option<PathBuf>,
}

impl AgentDef {
    pub fn config_dir(&self) -> Option<PathBuf> {
        (self.config_dir)()
    }

    /// The integration mode Ember uses by default for this agent.
    pub fn primary_mode(&self) -> IntegrationMode {
        match self.id {
            // Cline's only hook surface is a per-project `.clinerules/hooks/`
            // directory; there is no documented global Windows mechanism, so
            // Ember watches the process tree (approximate, always available).
            AgentId::Cline => IntegrationMode::Process,
            _ => IntegrationMode::Hooks,
        }
    }

    pub fn integration_note(&self) -> &'static str {
        match self.id {
            AgentId::ClaudeCode => {
                "Hooks in ~/.claude/settings.json (SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Notification [idle_prompt], Stop, SessionEnd)."
            }
            AgentId::Codex => {
                "Lifecycle hooks in ~/.codex/config.toml (SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop, SessionEnd) with a Windows command override (commandWindows); the older notify program is also understood."
            }
            AgentId::OpenCode => {
                "Global plugin in ~/.config/opencode/plugins/ subscribing to session.created, session.prompt, session.execution.started, session.execution.* (succeeded/failed/interrupted), session.status (busy/retry), and session.deleted."
            }
            AgentId::Pi => {
                "TypeScript extension in ~/.pi/agent/extensions/ using the pi.on(...) API (session_start, before_agent_start, agent_start, turn_start, tool_call, tool_execution_start, agent_settled, agent_end, session_shutdown)."
            }
            AgentId::OhMyPi => {
                "TypeScript extension in ~/.omp/agent/extensions/ using the pi.on(event, (event, ctx) => ...) API (session_start, before_agent_start, agent_start, tool_call, tool_execution_start, agent_settled, agent_end, session_shutdown)."
            }
            AgentId::Antigravity => {
                "Hooks in ~/.gemini/config/hooks.json, keyed by hook name (SessionStart, PreInvocation, PostInvocation, PostToolUse, Stop)."
            }
            AgentId::Cline => {
                "Process detection only: Cline exposes hooks per-project via .clinerules, not globally on Windows."
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Session context carried by a hook invocation
// ---------------------------------------------------------------------------

/// What we can learn from an agent's hook payload on stdin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionContext {
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub pid: Option<u32>,
}

/// Parse one agent's stdin payload into session id + working directory.
///
/// Every agent differs: Claude Code/Codex send snake_case `session_id` + `cwd`,
/// Antigravity sends camelCase `conversationId` + `workspacePaths[]`.
pub fn parse_hook_payload(agent: AgentId, raw: &str) -> SessionContext {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return SessionContext::default();
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return SessionContext::default();
    };

    let get_str = |keys: &[&str]| -> Option<String> {
        for key in keys {
            if let Some(s) = value.get(*key).and_then(|v| v.as_str())
                && !s.is_empty()
            {
                return Some(s.to_string());
            }
        }
        None
    };

    let pid = value
        .get("pid")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .or_else(|| {
            value
                .get("pid")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u32>().ok())
        });

    let mut ctx = match agent {
        AgentId::ClaudeCode | AgentId::Codex => SessionContext {
            session_id: get_str(&["session_id", "sessionId", "thread_id"]),
            cwd: get_str(&["cwd", "workspace", "project_dir"]),
            pid,
        },
        AgentId::Antigravity => {
            let cwd = get_str(&["workspacePaths", "cwd"]).or_else(|| {
                value
                    .get("workspacePaths")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });
            SessionContext {
                session_id: get_str(&["conversationId", "conversation_id", "session_id"]),
                cwd,
                pid,
            }
        }
        // OpenCode / Pi / Oh My Pi plugins are ours, so they send snake_case.
        AgentId::OpenCode | AgentId::Pi | AgentId::OhMyPi | AgentId::Cline => SessionContext {
            session_id: get_str(&["session_id", "sessionId", "id"]),
            cwd: get_str(&["cwd", "directory", "worktree"]),
            pid,
        },
    };
    if ctx.pid.is_none() {
        ctx.pid = pid;
    }
    ctx
}

/// The last path component, used as the session "folder name" in the UI.
pub fn folder_name(cwd: &str) -> String {
    let trimmed = cwd.trim_end_matches(['\\', '/']);
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    PathBuf::from(trimmed)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| trimmed.to_string())
}

/// Is the agent installed? Binary on PATH, or its config folder exists.
pub fn detect(agent: AgentId) -> (bool, String) {
    let def = agent.def();
    for name in def.executables {
        if which(name).is_some() {
            return (true, format!("{name} on PATH"));
        }
    }
    if let Some(dir) = def.config_dir()
        && dir.is_dir()
    {
        return (true, format!("config folder {}", dir.display()));
    }
    (false, "not found".to_string())
}

/// Minimal PATH lookup (avoids another dependency).
pub fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = std::env::var("PATHEXT")
        .unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".to_string())
        .split(';')
        .map(|s| s.to_lowercase())
        .collect();

    let has_ext = PathBuf::from(program)
        .extension()
        .map(|_| true)
        .unwrap_or(false);

    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(program);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !has_ext {
            for ext in &exts {
                let with_ext = dir.join(format!("{program}{ext}"));
                if with_ext.is_file() {
                    return Some(with_ext);
                }
            }
        }
    }
    None
}

/// Path to the running Ember executable — baked into every hook command.
pub fn current_exe() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from(format!("{}.exe", paths::APP_NAME)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_ids_are_unique_and_parse() {
        let mut ids: Vec<&str> = AgentId::ALL.iter().map(|a| a.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 7);
        for agent in AgentId::ALL {
            assert_eq!(AgentId::parse(agent.id()), Some(agent));
        }
        assert_eq!(AgentId::parse("nope"), None);
    }

    #[test]
    fn monograms_and_hues_are_distinct() {
        let mut mono: Vec<&str> = AgentId::ALL.iter().map(|a| a.def().monogram).collect();
        mono.sort_unstable();
        mono.dedup();
        assert_eq!(mono.len(), 7);

        let mut hues: Vec<u16> = AgentId::ALL.iter().map(|a| a.def().hue).collect();
        hues.sort_unstable();
        hues.dedup();
        assert_eq!(hues.len(), 7);
    }

    #[test]
    fn claude_code_payload_parses() {
        let raw = r#"{"session_id":"abc123","transcript_path":"C:\\x.jsonl","cwd":"C:\\work\\my-proj","hook_event_name":"PreToolUse","tool_name":"Bash"}"#;
        let ctx = parse_hook_payload(AgentId::ClaudeCode, raw);
        assert_eq!(ctx.session_id.as_deref(), Some("abc123"));
        assert_eq!(ctx.cwd.as_deref(), Some("C:\\work\\my-proj"));
        assert_eq!(folder_name(ctx.cwd.as_deref().unwrap()), "my-proj");
    }

    #[test]
    fn codex_payload_parses() {
        let raw = r#"{"session_id":"thr_123","cwd":"/workspace","hook_event_name":"Stop","turn_id":"t1"}"#;
        let ctx = parse_hook_payload(AgentId::Codex, raw);
        assert_eq!(ctx.session_id.as_deref(), Some("thr_123"));
        assert_eq!(ctx.cwd.as_deref(), Some("/workspace"));
    }

    #[test]
    fn antigravity_payload_parses_camel_case() {
        let raw = r#"{"conversationId":"ec33ebf9","workspacePaths":["C:\\repo\\app"],"modelName":"gemini-3.6-flash-medium"}"#;
        let ctx = parse_hook_payload(AgentId::Antigravity, raw);
        assert_eq!(ctx.session_id.as_deref(), Some("ec33ebf9"));
        assert_eq!(ctx.cwd.as_deref(), Some("C:\\repo\\app"));
    }

    #[test]
    fn opencode_and_pi_payloads_parse() {
        for agent in [
            AgentId::OpenCode,
            AgentId::Pi,
            AgentId::OhMyPi,
            AgentId::Cline,
        ] {
            let raw =
                r#"{"session_id":"ses_9","cwd":"C:\\dev\\thing","worktree":"C:\\dev\\thing"}"#;
            let ctx = parse_hook_payload(agent, raw);
            assert_eq!(ctx.session_id.as_deref(), Some("ses_9"), "{}", agent.id());
            assert_eq!(ctx.cwd.as_deref(), Some("C:\\dev\\thing"), "{}", agent.id());
        }
    }

    #[test]
    fn malformed_and_empty_payloads_are_safe() {
        for raw in ["", "   ", "not json", "[]", "null", "{}"] {
            let ctx = parse_hook_payload(AgentId::ClaudeCode, raw);
            assert_eq!(ctx, SessionContext::default());
        }
    }

    #[test]
    fn folder_name_handles_odd_paths() {
        assert_eq!(folder_name("C:\\"), "C:");
        assert_eq!(folder_name("/"), "unknown");
        assert_eq!(folder_name(""), "unknown");
        assert_eq!(folder_name("C:\\a\\b\\"), "b");
    }
}
