# Changelog - Ember

All notable changes to [Ember](https://github.com/sanirudh17/Ember) are documented here.

## [v1.0.0] - 2026-10-17

### Added

- **Agent detection for seven coding agents** — Claude Code, Codex, OpenCode, Pi, Oh My Pi, Antigravity CLI and Cline are found from the process tree, and six of the seven report exact working/idle transitions through hooks Ember installs for them. A hook-managed session always beats a process-detected one for the same PID.
- **Hook management with previews** — every install, repair and removal is previewed as a diff, backed up to `<file>.ember-backup-<timestamp>`, merged instead of replaced (comments and formatting survive in TOML), idempotent, and exactly reversible. The Agents tab shows per-agent status and offers Install / Repair / Remove.
  - Claude Code: `~/.claude/settings.json` shell hooks.
  - Codex: `~/.codex/config.toml` (plus `hooks.json`) through a stable, space-free launcher (`%USERPROFILE%\.ember\hooks\ember-hook.cmd`) so the quoted exe path can never be mangled by `cmd /C`.
  - OpenCode: an in-process V2 plugin (`~/.config/opencode/plugins/ember-hooks.js`) whose runs span the whole turn — prompt to final message — with a heartbeat that keeps long, silent reasoning phases working.
  - Pi: an extension (`~/.pi/agent/extensions/ember-hooks.ts`).
  - Oh My Pi: an extension (`~/.omp/agent/extensions/ember-hooks.ts`) using the verified omp event set — `before_agent_start` / `agent_start` / `tool_call` / `tool_execution_start` open a run, `agent_settled` and terminal `agent_end` (non-terminal events carry `willContinue: true` and are ignored) close it.
  - Antigravity CLI: `~/.gemini/config/hooks.json` keyed by hook name.
  - Cline: process detection only — upstream has no global Windows hook surface.
- **A hold that matches the work** — the PC (and optionally the screen) stays awake exactly while any agent is working or thinking. Long silent reasoning phases, tool runs and multi-hour sessions never release early; when the last agent finishes, Ember lets go within seconds.
- **Closed-lid support** — while a hold is active, closing the lid no longer sleeps the machine. The override is capped, can require the charger, and is always put back.
- **Crash-proof restore** — the lid action is recorded before it is changed and reverted by a detached watchdog process, so even killing Ember mid-hold puts the machine back correctly.
- **Battery floor and Battery Saver release** — Ember lets go below a chosen battery percentage (10–30 %) and whenever Windows Battery Saver turns on.
- **Manual hold** — take the hold yourself for a chosen duration when Ember is not watching an agent (a long build, an upload, a download).
- **Signed, local-first auto-updates** — Ember checks GitHub once at launch and every few hours. A newer, minisign-verified build appears as a small Update pill in the top bar and installs in place on click; Settings → Updates has the same check on demand with plain-language status.
- **Tray companion** — tray icon, launch at login, toast + chime on take/let-go (mutable), single instance, window position reset, dark / light / system appearance.
- **A quiet surface** — every agent gets a generated monogram tile (never a third-party logo), and the Home tab narrates what the engine is doing instead of hiding it behind toggles.

### Quiet, trustworthy detection

- **Think-gaps never release a live run.** A session backed by a live process stays trusted through 90 seconds of silence, and a settled session's grace survives its row being removed — so long reasoning phases, hook dropouts and step-settle events cannot hand the PC back mid-turn.
- **One chime per episode.** Sound and toast fire once per work episode (60 seconds); a burst of short holds still flips the orb and tray, silently, while a long hold chimes again when it finishes.
- **Sustained, not spiky.** CPU corroboration uses a whole-tree energy budget — modest streaming or reasoning load engages; isolated spikes never do.
- **Every hook transition is logged.** `ember.log` records each accepted working / idle / removal transition, so "why did it take or let go?" is answerable straight from Settings → Recent activity.

### Notes

- **Installing:** first release — download `Ember_1.0.0_x64-setup.exe` from Releases and run it. The installer is not code-signed, so Windows SmartScreen may warn — **More info → Run anyway**.
- **Updating:** from 1.0.0 onward Ember updates itself from GitHub Releases; no manual downloads for future versions.
- **Privacy:** the only network traffic Ember ever makes is the update check against this repository.
