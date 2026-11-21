# Changelog - Ember

All notable changes to [Ember](https://github.com/sanirudh17/Ember) are documented here.

## [v1.3.0] - 2026-11-21

### Power, Sleep, and Lid-Proof Reliability

- Strengthened Modern Standby (S0 Low Power Idle) power holds by acquiring `PowerRequestSystemRequired` together with best-effort `PowerRequestExecutionRequired`, and acquiring `PowerRequestDisplayRequired` during manual holds so unattended timers and builds do not drop into standby when `VIDEOIDLE` expires.
- Added monotonic power-request generation tracking and automatic re-assertion on Windows resume, AC/DC transitions, and lid-close events so active holds survive power state changes cleanly.
- Fixed lid-close policy restoration in `lid::restore_at()` to call `PowerSetActiveScheme` when the user is still on the modified power plan, flushing the restored AC/DC lid action immediately into the Windows kernel power policy.
- Added a 30-second retry backoff (`lid_retry_after`) if applying a lid override fails, preventing repeated Win32 power-API thrashing on restricted policies.
- Cleared stale lid errors, retry backoffs, and lid safety-cap flags cleanly when toggling lid settings or releasing the lid override from Settings.
- Releasing the lid override from Settings now also disables `lid_proof` and saves settings so the next engine tick does not immediately re-engage it.
- Released any active lid-close override inside the updater `before_exit` callback so in-place upgrades restore the Windows lid policy before the installer replaces and restarts Ember.

### Agent Detection, Startup Speed, and Hooks

- Ran the first engine tick immediately on startup before sleeping for the loop interval, eliminating the 250 ms startup delay so already-working coding agents engage the OS power hold, window title, overlay indicator, and system tray status at launch.
- Eliminated phantom holds from abruptly terminated agent processes by tracking process liveness (`pid_seen_alive`), validating process creation timestamps (`pid_birth`) to detect PID reuse, pruning dead and PID-less orphan sessions immediately, and clearing both explicit and PID-synthesized activity cache entries on removal.
- Prevented CLI coding agents from falling back to shared IDE editor processes (`code.exe`, `electron.exe`, and similar hosts) by separating runtime hosts from IDE hosts during parent-process attribution.
- Excluded background helper and server processes (`codex app-server`, `codex-code-mode-host`, `codex-computer-use`, `opencode serve`, `opencode-pty`) from false-positive process detection.
- Bounded cached and silent working turns to a 45-minute safety cap (`MAX_CACHED_WORK_AGE_SECS` / `LIVE_PID_SILENCE_CAP`) so stale state cannot hold a PC awake indefinitely after an unclean crash.
- Fixed an OpenCode plugin idle-reconciliation loop where repeated `markIdle()` calls could trigger continuous status spam, and added `SIGHUP` / `SIGBREAK` process-exit cleanup handlers to OpenCode, Pi, and Oh My Pi extensions.
- Guarded `process_start_time()` against zero-FILETIME subtraction panics and hardened Claude Code event-mapping and Codex `commandWindows` hook repair checks.

### Manual Holds, Pause Controls, and Guardrails

- Starting any manual hold (30m, 1h, 2h, Custom, or Until released) while Ember is paused now automatically clears the pause state and engages the hold immediately.
- Added an in-window Resume action on the Awake screen when Ember is paused from the system tray, and updated the tray status text to distinguish timed pauses ("Paused - resumes in ...") from indefinite pauses ("Paused - until you resume").
- Fixed conflicting UI state when a manual hold is active alongside Windows Battery Saver or the battery floor guardrail, clearly displaying "Manual hold - paused", "Cancel hold", and a "Paused" status readout instead of conflicting active-hold controls.
- Replaced the native browser time input in the Custom hold dialog with a custom Ember `HH:MM` input and two-column Hour/Minute picker that matches both light and dark themes.

### User Interface, Themes, and Window Behavior

- Refined dark mode into a balanced smoked-obsidian palette with clear sidebar-to-workspace tonal separation, subtle espresso-charcoal warmth, high-contrast foreground typography, and warm amber accents while keeping light mode crisp and legible.
- Unified the Agents watching list into a single elevated studio panel while keeping the lower Sessions list clean and lightweight, and added subtle tactile 3D surface bevels and an encapsulated outer bezel around the main workspace shell.
- Added smooth sliding segmented controls, centered switch geometry, instant zero-remount tab switching across Awake, Agents, and Settings, atomic theme transitions without flash, and slim 4px auto-hiding scrollbars that fade out after 5 seconds of idle time.
- Adjusted the default first-launch window geometry to 1146 by 715 logical pixels with DPI-aware vertical taskbar clearance so the window opens centered and fully visible on standard and high-DPI displays.
- Updated the Watch for agents off-state hint copy to remain clear when watching is disabled.

### Verification

- Verified 227 Rust unit tests, real Windows Power Request API integration test (`tests/native_power.rs`), and real Windows lid-close policy and detached crash-recovery watchdog integration test (`tests/native_lid.rs`), along with `cargo clippy --all-targets -- -D warnings` and TypeScript/Vite production builds.

### Upgrade notes

- After updating, open Agents and repair any hooks flagged as needing repair, then restart or reload those agent sessions so updated exit handlers and bindings take effect.
- The updater public key and GitHub release endpoint are unchanged from v1.0.0 through v1.2.0.

## [v1.2.0] - 2026-11-13

### Added

- Global Cline Windows PowerShell hooks, including task cancellation/completion, user questions, redirected Documents folders and SDK discovery. Existing user policy scripts are preserved and bundle failures roll back.
- Read-only T3 Code and GOAT session metadata support, including private Antigravity ACP profiles and GOAT stable/nightly homes. Native thread identities avoid duplicate hook/harness rows.
- Persistent last-hook state with live process creation-time validation, allowing active sessions to be recovered when Ember reopens.
- Real Windows power API integration coverage and an expanded detection/recovery verification report.

### Fixed

- Open processes appear immediately at startup, and the first snapshot contains discovered sessions. CPU cannot classify hook-supported agents as working merely because a GUI starts, indexes files or receives typing.
- OpenCode reconnects its v2 event stream without ending ongoing reasoning or tools. Recovery recognizes native date objects, and old metadata replies cannot override newer executions. V2 execution ownership also applies through its compatible v1 adapter.
- Pi and Oh My Pi tool-start callbacks no longer override pending user input. Cancellation, guarded resume and late-callback safeguards remain shared across agent integrations.
- Explicit idle/end events release on the next engine tick without an additional release floor. Short cache-file contention no longer silently drops an idle update.
- First-run windows are compact and placed inside the Windows work area with taskbar clearance. Valid user-adjusted geometry and maximization are preserved.

### Changed

- Dark mode uses neutral graphite surfaces, warm white text, subtle borders and amber controls. The approved layout and light theme remain intact.
- Settings explains Windows' battery-powered Modern Standby limitation. Plug in for extended runs on these devices.

### Upgrade notes

- Repair existing integrations in Agents, then restart or reload their hosts. Changed Codex hooks retain the normal host trust review.
- Current Cline builds can use the new Windows hooks; enable Hooks in Cline and reload. Older or disabled Cline hosts retain approximate CPU fallback. Exact boundaries require host lifecycle events, and universal perfect detection is not claimed.
- Ember prevents automatic idle sleep during accepted power requests, not explicit shutdown or Windows safety actions. Modern Standby can expire battery-powered requests.
- The updater public key and endpoint are unchanged from v1.0.0 and v1.1.0.

## [v1.1.0] - 2026-11-03

### Changed

- Redesigned the desktop workspace with a unified sidebar, responsive Awake overview, manual hold group, open/working session counts, and clearer settings sections.
- Applied the supplied Ember icon across the app, tray, taskbar, installer, and README. Refined agent icons, font loading, light-theme contrast, blue-black dark surfaces, and subtle 3px scrollbars.
- Normal windows default to 1100 by 700 logical pixels and retain user size, placement, and maximization. First display waits for themed content; maximization transition frames no longer overwrite normal geometry.

### Fixed

- Shared ACP server background CPU no longer creates a working hold. Codex app-server and OpenCode serve processes rely on their session hooks rather than shared-server CPU.
- Explicit user-question waits and guarded resume transitions cannot revive a cancelled or closed session. Late tool callbacks cannot create phantom open sessions, and a dead process no longer earns an invisible recovery hold.
- OpenCode v1/v2 retain work through reasoning, retries, and ordinary tool completion; pending permission, question, and form requests suspend the hold until valid replies or terminal completion.
- Pi uses final agent_settled (including Escape) rather than mid-retry agent_end; UI prompt waits suspend activity. Oh My Pi preserves nonterminal continuations and handles explicit question-tool waits.
- Codex Interrupt, Claude StopFailure, compacting session starts, and post-tool callbacks have dedicated lifecycle handling. Hook ownership prevents CPU estimates from reactivating a completed turn.
- Preserved battery, display, manual timer, and reversible lid-close safeguards.

### Upgrade notes

- In Agents, repair existing hooks and restart/reload each agent host to load new bindings. Review changed Codex hooks in its normal trust flow.
- Cline and CLI instances without working hooks use approximate CPU fallback. Exact reasoning, approval, or Escape boundaries require events from the host; universal instantaneous detection is not claimed.
- This release retains the v1.0.0 update endpoint and signing key for in-place updates.

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
