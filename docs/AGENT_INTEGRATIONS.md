# Agent integrations

Ember learns whether a coding agent is working in one of two ways:

1. **Hooks** — the agent is configured to run `Ember.exe hook <agent> <event>`
   at documented lifecycle points. This is authoritative: Ember knows the exact
   working/idle transition.
2. **Process detection** — Ember watches the process tree and infers state from
   CPU usage with hysteresis. This is approximate and always available, and is
   the *corroborating* signal that keeps detection working when a hook name
   drifts between agent releases.

Both run at once. A hook-managed session always wins over a process-detected one
for the same PID (see `agents/registry.rs`).

## The one command shape

Every hook Ember installs is the same command:

```
"<Ember.exe>" hook <agent-id> <event>
```

with `agent-id` ∈ `claude-code, codex, opencode, pi, oh-my-pi, antigravity, cline`
and `event` ∈ `start, working, idle, end, heartbeat`.

**Codex exception.** Codex runs hook commands as `cmd /C "<command>"`, and
cmd's `/C` quote stripping silently destroys any command that itself starts
with a quoted path (e.g. `"C:\Users\me\Windows Tool\...\ember.exe" ...`). For
Codex, Ember therefore installs a space-free launcher instead —
`%USERPROFILE%\.ember\hooks\ember-hook.cmd`, which forwards argv and stdin to
the real executable — and the hook command becomes:

```
C:\Users\<user>\.ember\hooks\ember-hook.cmd codex <event>
```

The launcher is rewritten on every install/repair and its path stays constant,
so moving or rebuilding `ember.exe` never invalidates Codex's stored hook
trust (`[hooks.state]` in `~/.codex/config.toml`).

The hook process:

- reads the agent's JSON payload from **stdin** (non-blocking, 200 ms budget,
  1 MiB cap) where the agent provides one;
- walks its own parent PIDs to find the owning agent process;
- writes **one JSON line** to `\\.\pipe\ember-hooks`;
- rejects remote (non-local) pipe clients;
- authenticates with a per-install random token;
- **always exits 0**, in roughly 300 ms, writing nothing to stdout or stderr.

Ember is never allowed to break the host agent: a missing pipe, a malformed
payload or a timeout is silently ignored.

## Two integration styles

| Style | Agents | How it reports |
| --- | --- | --- |
| **stdin-JSON shell hooks** | Claude Code, Codex, Antigravity CLI, Cline (files) | the agent runs the command; Ember reads JSON from stdin |
| **in-process extension** | OpenCode, Pi, Oh My Pi | Ember installs a small JS/TS file the agent loads; it calls `spawnSync` to run the helper, passing a JSON payload on stdin |

Everything Ember writes is:

- **previewed first** — the UI shows the exact file and the exact diff;
- **backed up** — `<file>.ember-backup-<timestamp>` before any write;
- **content-preserving** — a merge, not a replace (`toml_edit` for TOML so
  comments and formatting survive);
- **idempotent** — installing twice is a no-op;
- **cleanly removable** — Ember only ever removes the entries it added (or the
  file it owns end-to-end), and records what it did in
  `%APPDATA%\Ember\installs.json`;
- **marked** — plugin/extension files start with
  `// ember-hook-managed — installed by Ember.`

---

## Claude Code

- **File:** `%USERPROFILE%\.claude\settings.json` (JSON, `hooks` object)
- **Mechanism:** shell hooks. Windows is supported (PowerShell commands work).
- **Payload (stdin):** `session_id`, `cwd`, `transcript_path`, `hook_event_name`.

| Agent event | Ember event |
| --- | --- |
| `SessionStart` | `start` |
| `UserPromptSubmit` | `working` |
| `PreToolUse` | `working` |
| `PostToolUse` | `working` |
| `Notification` (`matcher: "idle_prompt"`) | `idle` |
| `Stop` | `idle` |
| `SessionEnd` | `end` |

**Windows pitfall:** the `SessionStart` hook has had regressions on Windows
(e.g. not firing in v2.1.141). Ember therefore also corroborates with process
detection (`claude.exe`, or `node` with `claude` on its command line).

---

## Codex CLI

- **Files:** `%USERPROFILE%\.codex\config.toml` (primary) and
  `%USERPROFILE%\.codex\hooks.json` (also supported upstream)
- **Mechanism:** inline `[[hooks.<Event>]]` tables; each handler carries a
  portable `command` **and** a Windows override so the quoted `.exe` path is
  used verbatim.

| Agent event | Ember event |
| --- | --- |
| `SessionStart` | `start` |
| `UserPromptSubmit` | `working` |
| `PreToolUse` | `working` |
| `PostToolUse` | `working` |
| `Stop` | `idle` |
| `SessionEnd` | `end` |
| legacy `notify` (agent-turn-complete) | `idle` |

**Payload (stdin):** `session_id`, `cwd`, `hook_event_name`, `model`.

**Windows pitfalls:**

- Codex does **not** honour `PATHEXT` when launching hooks, so Ember always
  writes the real `Ember.exe` path and never a `.cmd` shim.
- The legacy `notify` program may not fire on native Windows, which is why the
  `[[hooks]]` table is the primary mechanism.
- Newly added hooks may need to be trusted before Codex runs them.

**Verified against Codex 0.163.0 (`codex.exe`):** the `HookHandlerConfig::Command`
serde field is camelCase `commandWindows` (even inside `config.toml`), and
`legacy_notify.rs` remains active alongside `[[hooks]]`.
---

## OpenCode

- **File:** `%USERPROFILE%\.config\opencode\plugins\ember-hooks.js`
  (project-local `.opencode\plugins\` is also supported upstream)
- **Mechanism:** an in-process plugin. There is **no stdin**; the plugin spawns
  the helper itself.

| Agent event | Ember event |
| --- | --- |
| `session.created` (carries `location.directory`) | `start` |
| `session.prompt` via `ctx.session.hook("prompt")` | `working` |
| `session.execution.started` | `working` |
| `session.status` = `busy` / `retry` | `working` |
| `session.execution.succeeded` / `.failed` / `.interrupted` | `idle` |
| `session.deleted` | `end` |

**Notes** (verified live against OpenCode 2.0.29)

- The installed file targets the **V2 plugin contract**: the host instantiates
  `default.setup(ctx)` (the exported `server` module); the v1 factory export is
  kept only as a compatibility shim. State lives on `globalThis` so plugin hot
  reloads and duplicate instances share one session map.
- A **run** spans the whole turn — prompt to final message, reasoning, thinking
  and text streaming included. `session.execution.started` opens it and the
  terminal `session.execution.*` events close it; there are no per-step
  execution events, so a finished tool or a long silent reasoning phase can
  never idle a live run.
- `session.idle` and `session.status` = `idle` are **deliberately not mapped**:
  they can fire mid-run, and the terminal execution events are the single
  reliable end-of-run signal. Only a working signal may refresh a run.
- While a run is working and silent, a background heartbeat (~30s cadence,
  checked on a 60s timer; worst-case gap ~60s) refreshes the session so the
  registry's stale window can never demote a long reasoning phase. Heartbeats
  never resurrect an idled session. The same cadence re-announces a mid-run
  session within about a minute of an Ember restart.
- The plugin spawns the helper with a hard timeout; the helper retries the
  hook pipe until its budget expires, and a dropped hook is recorded in
  `ember.log` (`HOOK … dropped`), so "the agent worked but Ember stayed idle"
  is always diagnosable.
- **WSL:** an OpenCode install inside WSL has a different home
  (`\\wsl$\<distro>\home\<user>\...`); Ember resolves the Windows home only.

---

## Pi

- **File:** `%USERPROFILE%\.pi\agent\extensions\ember-hooks.ts`
- **Mechanism:** an in-process TypeScript extension using `pi.on(name, (event, ctx) => ...)`.

| Agent event | Ember event |
| --- | --- |
| `session_start` | `start` |
| `before_agent_start` | `working` |
| `agent_start` | `working` |
| `turn_start` | `working` |
| `tool_call` | `working` |
| `tool_execution_start` | `working` |
| `agent_settled` | `idle` |
| `agent_end` (`willContinue !== true`) | `idle` |
| `session_shutdown` | `end` |

**Notes** (verified against `~/.omp/agent/extensions/orca-agent-status.ts`)

- Pi and Oh My Pi share the same extension API (`pi.on(event, (event, ctx) => ...)`).
  Modern Pi emits `agent_settled` as the terminal turn signal; legacy Pi emits
  `agent_end` (`willContinue !== true`). Both are mapped so modern and older
  releases transition cleanly to `idle`.
- The npm scope moved from `@mariozechner/pi` to `@earendil-works/pi`; Ember's
  detection lists both.
- Ember also matches `pi.exe` and a `bun`/`node` host with a Pi path on its
  command line.
---

## Oh My Pi

- **File:** `%USERPROFILE%\.omp\agent\extensions\ember-hooks.ts`
- **Mechanism:** an in-process TypeScript extension using `pi.on(name, (event, ctx) => ...)`.
  The first handler argument is the event payload and the second is the extension context
  (`ctx.sessionManager.getSessionId()` provides the session ID).

| Agent event | Ember event |
| --- | --- |
| `session_start` | `start` |
| `before_agent_start` | `working` |
| `agent_start` | `working` |
| `tool_call` | `working` |
| `tool_execution_start` | `working` |
| `agent_settled` | `idle` |
| `agent_end` (`willContinue !== true`) | `idle` |
| `session_shutdown` | `end` |

**Notes** (verified against `~/.omp/agent/extensions/orca-agent-status.ts`)

- Non-terminal `agent_end` events carry `willContinue: true` and are ignored so mid-turn
  continuations do not prematurely mark the session idle.

---

## Antigravity CLI

- **File:** `%USERPROFILE%\.gemini\config\hooks.json` (global). A workspace copy
  can live at `<repo>\.agents\hooks.json`, and `agy` also reads
  `...\antigravity-cli\settings.json`.
- **Mechanism:** shell hooks keyed by hook **name** — Ember installs the single
  key `"ember"`, which is also the removal handle.
- **Payload (stdin):** camelCase, e.g. `conversationId`, `workspacePaths[]`,
  `modelName`.

| Agent event | Ember event |
| --- | --- |
| `SessionStart` | `start` |
| `PreInvocation` | `working` |
| `PostInvocation` | `working` |
| `PostToolUse` (`matcher: "*"`) | `working` |
| `Stop` | `idle` |

**Notes** (verified against `agy.EXE` 1.4.0 and `agy-customizations/docs/hooks.md`)

- `PreInvocation` and `PostInvocation` are flat handler arrays that fire before
  and after each LLM invocation; `PostToolUse` is a matcher-grouped array
  (`[{"matcher": "*", "hooks": [...]}]`) that fires after every tool step.
- `PreToolUse` is **deliberately not bound**: Antigravity requires `PreToolUse`
  hooks to print a JSON `{"decision": "allow"|"ask"|...}` object on stdout and
  blocks tool calls if stdout is empty, whereas Ember's hook helper is strictly
  silent on stdout.
- There is **no** `SessionEnd`; `Stop` is the termination signal, so Ember maps
  it to `idle` and relies on the process tree (or a later `start`) for teardown.
- The CLI ships as `agy`. `gemini.exe` is a *different* CLI and `Antigravity.exe`
  is the Electron desktop/IDE app, so neither is listed as an Antigravity CLI
  executable.
---

## Cline

- **Status:** **no global Windows hook surface.**
- Upstream's file hooks (`%USERPROFILE%\Documents\Cline\Hooks\`, no extension)
  and workspace `.clinerules\hooks\` are described as macOS/Linux only, and the
  `.ps1` launcher has known bugs. The SDK plugin hooks are a different API.
- Ember therefore uses **process detection as the primary mechanism** for Cline.
  It matches the `cline` CLI (or a `bun`/`node` host with `cline` / `cline-core`
  on its command line).
- The VS Code / Cursor extension host (`Code.exe`) is **not** unique to Cline,
  so Ember never keys on it — doing so would report Cline on any machine with VS
  Code installed.

The Agents page shows Cline with a disabled **Install hooks** button and the
reason, exactly as above.

---

## Cross-cutting notes

- **WSL homes differ.** A tool installed inside WSL keeps its config under
  `\\wsl$\<distro>\...`, which Ember does not scan. Detection routes through the
  Windows home only.
- **Never a `.cmd` shim.** Ember always writes the quoted absolute `.exe` path,
  because some agents do not resolve `PATHEXT`.
- **Repair.** If the installed command no longer points at the running
  `Ember.exe` (an update moved it), the Agents page offers **Repair**, which
  rewrites the hook in place after a fresh preview.
- **Removal is exact.** When Ember removes a hook it removes only groups whose
  command it recognises as its own (`parse_our_command`), leaving any user hooks
  untouched; for plugin/extension files it owns end to end, removal deletes the
  file.

## Build-time verification summary

Verified against the installed agent binaries and configs on Windows:

- **Codex (`codex.exe` 0.163.0):** `HookHandlerConfig::Command` uses camelCase
  `commandWindows` in both `hooks.json` and `config.toml`; `hooks\src\legacy_notify.rs`
  still supports legacy `notify`.
- **Pi & Oh My Pi (`omp.exe` + `orca-agent-status.ts`):** extensions load from
  `~/.pi/agent/extensions/` and `~/.omp/agent/extensions/`, pass `(event, ctx)` to
  `pi.on(...)`, expose `ctx.sessionManager.getSessionId()`, and signal turn
  completion via `agent_settled` and `agent_end` (`willContinue !== true`).
- **OpenCode (`opencode.exe` 2.0.29):** V2 plugins export `default.setup(ctx)`
  with `ctx.event.subscribe` (`session.created`, `session.execution.*`,
  `session.status`, `session.deleted`) and `ctx.session.hook("prompt", ...)`;
  the V1 factory export is retained for 1.x hosts.
- **Antigravity CLI (`agy.EXE` 1.4.0):** loads `~/.gemini/config/hooks.json`;
  `PreInvocation`, `PostInvocation`, `PostToolUse` (matcher-grouped), and `Stop`
  track the full turn lifecycle without requiring stdout decisions.
- **Claude Code (`claude.exe` 2.1.214):** `Notification` is scoped to
  `matcher: "idle_prompt"` so mid-turn permission prompts do not mark the
  session idle; process detection corroborates `SessionStart` on Windows.
