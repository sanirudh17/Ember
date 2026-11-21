<div align="center">

<img src="app-icon.png" width="112" alt="Ember icon" />

# Ember

**A quiet, local-first keep-awake companion for Windows that watches your coding agents.**

Ember notices when Claude Code, Codex, OpenCode, Pi, Oh My Pi, Antigravity or Cline is working — or thinking — and holds your PC awake exactly until the work is done. No accounts, no telemetry, no cloud.

[![Latest release](https://img.shields.io/github/v/release/sanirudh17/Ember?label=download&color=5e6ad2)](https://github.com/sanirudh17/Ember/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-5e6ad2.svg)](LICENSE)
[![Platform: Windows](https://img.shields.io/badge/platform-Windows%2010%2F11-0078d6.svg)](#download--install)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%20v2-24c8db.svg)](https://v2.tauri.app/)

</div>

---

## Detection and window behavior

Ember runs its first engine tick immediately on launch so already-working agents engage the OS hold right away. It also recognizes Antigravity's `agy_acp_server.exe` when launched by GUI clients; shared ACP transport CPU and background helper processes (`codex app-server`, `codex-code-mode-host`, `codex-computer-use`, `opencode serve`, `opencode-pty`) never earn a hold on their own. Session hooks or supported T3/GOAT local session projections report the actual work, including private Antigravity profiles and GOAT stable/nightly homes. Hooks are authoritative: opening a session does not engage a hold, an explicit idle/end event releases immediately, abrupt process termination or PID reuse clears the session cleanly, and silent or cached working turns are bounded by a 45-minute safety cap.

Current Cline supports global Windows PowerShell hooks; enable Hooks in Cline after installing the integration. Its fallback for older or disabled hosts remains an estimate, using sustained CPU activity and a short quiet window; it cannot guarantee immediate turn detection during network waits or distinguish every typing burst. Prefer hooks where supported. Toasts follow visible state changes; repeated sounds are throttled separately.

The default window is 1146 by 715 logical pixels with DPI-aware taskbar clearance, and it remembers its geometry and maximized state. Startup waits for the themed frontend to paint. The supplied Ember artwork is used in the app, tray, taskbar, and installer; run `npm run icon` to regenerate sizes without replacing the source artwork.

## Overview

Ember is a keep-awake companion for Windows aimed at people who leave long agent runs unattended. It watches the coding agents you actually use, and holds the machine awake exactly while any of them is working. When the last one finishes, Ember lets go.

The usual keep-awake tool is a toggle you forget: you turn it on and your laptop never sleeps again, or you forget it and a multi-hour agent run gets suspended halfway. Ember replaces the toggle with detection. Busy machines stay awake, quiet machines sleep like normal, and you never think about it.

> **New here?** Install the single `Ember_1.3.0_x64-setup.exe` from [Releases](https://github.com/sanirudh17/Ember/releases/latest) and run it. Ember lives in the tray and updates itself from then on.

---

## Contents

- [Features](#features)
- [How It Works](#how-it-works)
- [Supported Agents](#supported-agents)
- [Settings Reference](#settings-reference)
- [Download & Install](#download--install)
- [Updating](#updating)
- [Building from Source](#building-from-source)
- [Tech Stack](#tech-stack)
- [Project Structure](#project-structure)
- [Configuration & Data](#configuration--data)
- [Privacy](#privacy)
- [Troubleshooting](#troubleshooting)
- [License](#license)

---

## Features

**A hold that matches the work**

- **Exactly while agents work.** Ember takes the hold when a coding agent starts working and releases it when the last one stops — no timers to set, no toggles to remember.
- **Long sessions are safe.** Hook-managed runs retain work through reasoning and ordinary tool completion. Plugin integrations refresh long runs; explicit waiting, completion, interruption, or process exit releases the session.
- **Two signals, one truth.** Hooks are authoritative; process detection corroborates. If an agent renames an event between releases, detection degrades gracefully instead of breaking. A hook-managed session always wins over a process-detected one for the same PID.

**Closed-lid support**

- **Work with the lid shut.** While a hold is active, closing the lid no longer sleeps the machine. The override is capped, can require the charger, flushes immediately back into Windows kernel power policy when restored, and backs off safely if restricted by system policy.
- **Crash-proof restore.** The original lid action is recorded before it is changed, a detached watchdog process reverts it even if Ember is killed mid-hold, and in-place updates release any active lid override before restarting.
- **Battery floor and Battery Saver release.** Ember lets go below your chosen battery percentage (10–30 %) and whenever Windows Battery Saver turns on, clearly marking any active manual hold as paused until power conditions recover.

**Control and comfort**

- **Manual hold.** Take the hold yourself with quick presets (`30m`, `1h`, `2h`), a custom duration or `Until HH:MM` time picker, or `Until released`. Manual holds keep both the system and display awake so Windows Modern Standby does not drop unattended builds or renders when the screen blanks, and starting a manual hold automatically resumes Ember if paused.
- **Keep the screen on, or hands off.** Optionally keep the display lit while holding for agents; otherwise your screen follows the Windows power plan while the PC stays awake via `SystemRequired` and best-effort `ExecutionRequired` power requests, automatically re-asserted across resume, AC/DC, and lid-close events.
- **Tray companion.** Launch at login, timed or indefinite pause from the tray (with in-window `Resume`), a toast and chime when Ember takes hold and lets go (mutable), single instance, and a window position reset.
- **Quiet surface.** Smoked-obsidian dark mode and crisp studio light mode with warm amber accents, unified Agents status panel, instant tab switching, auto-hiding scrollbars, and an **Awake** workspace that narrates every decision the engine makes.

**Signed, local-first updates**

- **Self-updating.** Ember checks GitHub once at launch and every few hours. A newer build appears as a small Update pill while it waits; Settings → Updates has the same check on demand, with plain-language status.
- **Nothing runs unverified.** Every download is minisign-verified against a public key baked into the app before it is installed in place, and nothing installs without your click.

---

## How It Works

Everything between "agent got a prompt" and "PC stays awake" runs as one decision pipeline. A hook-managed session owns its process state. Only unhooked Cline uses CPU-derived working estimates. The other six require lifecycle signals or supported harness metadata; exact open processes appear immediately.

```
  any agent moving?
        │
        ▼
  hooks ────────────────►  Ember.exe hook <agent> <event>
        │                  stdin JSON · ~300 ms · always exits 0
        ▼
  process scan ─────────►  process tree + CPU hysteresis
        │
        ▼
  registry ─────────────►  one session per agent run · working/idle transitions timestamped
        │
        ▼
  the decision ─────────►  "3 coding agents working" → take the hold · last session ends → let go
        │
        ├─ power requests: SystemRequired + best-effort ExecutionRequired (DisplayRequired on manual holds or when enabled), re-asserted on resume/AC-DC/lid-close
        └─ lid override: closing the lid stops sleeping — capped, charger-gated, kernel-flushed on restore, watchdog-restored
```

**Why two signals.** Hooks report lifecycle transitions when the host emits them. Process discovery confirms which applications are open, validates process creation times so abruptly killed processes and reused PIDs clear immediately, and isolates CLI agents from IDE host processes, while persisted hook state and supported T3/GOAT session metadata provide restart recovery. CPU is used only for unhooked Cline's approximate working estimate; opening or typing in a hook-supported agent cannot earn a hold from CPU alone. Missing hooks must be installed or repaired rather than hidden behind an unreliable activity guess.

**Ember is never allowed to break an agent.** Hook commands read their payload from stdin with a bounded budget, write one JSON line to a local named pipe, authenticate with a per-install token, and always exit 0 in roughly 300 ms. A missing pipe, a malformed payload or a timeout is silently ignored.

**Why the lid is treated carefully.** Changing the lid-close action survives Ember only if it is reverted — so Ember writes the original state to disk *before* changing anything, a detached watchdog holds the responsibility for reverting it, and the override has both a safety cap and an optional charger requirement. Killing Ember mid-hold puts the machine back.

---

## Supported Agents

Ember knows seven agents. Current supported hosts can all report lifecycle events through installed hooks; all seven are also found from the process tree. Hook enablement and host event coverage determine exactness.

| Agent | Integration | Where it lives |
| --- | --- | --- |
| **Claude Code** | shell hooks + process detection | `~/.claude/settings.json` |
| **Codex** | hooks via a stable, space-free launcher | `~/.codex/config.toml` (+ `hooks.json`) |
| **OpenCode** | in-process plugin (V2 API) | `~/.config/opencode/plugins/ember-hooks.js` |
| **Pi** | in-process extension | `~/.pi/agent/extensions/ember-hooks.ts` |
| **Oh My Pi** | in-process extension | `~/.omp/agent/extensions/ember-hooks.ts` |
| **Antigravity CLI** | named shell hooks | `~/.gemini/config/hooks.json` |
| **Cline** | global Windows PowerShell hooks | Documents/Cline/Hooks; enable Hooks in the host; approximate fallback for older or disabled hosts |

Install, repair or remove any agent's hook from the **Agents** tab. Every write is previewed as a diff first, backed up to `<file>.ember-backup-<timestamp>`, merged rather than replaced, idempotent, and removable exactly — Ember only ever removes what it added. The full per-agent matrix lives in [`docs/AGENT_INTEGRATIONS.md`](docs/AGENT_INTEGRATIONS.md).

> **Extensions load at session start.** After installing a hook for an agent that is already open, restart that agent's session once — a fresh process picks the extension up.

---

## Settings Reference

**General** — `Launch at login` · `Appearance` (System / Light / Dark) · `Notifications` (a toast and a chime when Ember takes hold and lets go).

**Power** — `Battery floor` (10 / 15 / 20 / 30 %) · `Closed-lid hold requires the charger` · `Keep the screen on`.

**Always on** (applied without asking) — the battery floor, the Battery Saver release, the lid safety cap, and how the display behaves while holding.

**Updates** — your version, a plain-language status line, and `Check now` / `Install vX.Y.Z`. Every download is signature-verified before it runs.

**Recent activity** — the last decisions from the engine, straight from the log. The footer carries `Reset window position` and `Quit Ember`.

---

## Download & Install

Download the latest installer from the [**Releases**](https://github.com/sanirudh17/Ember/releases/latest) page and run it:

| Installer | Notes |
| --- | --- |
| **`Ember_1.3.0_x64-setup.exe`** | NSIS installer — single executable, no extra dependencies. Recommended. |

The installer is currently unsigned, so Windows SmartScreen may show *"Windows protected your PC"* → **More info → Run anyway**.

**System requirements:** Windows 10 or 11 (64-bit). WebView2 is preinstalled on Windows 11 and installed automatically on Windows 10 if missing.

---

## Updating

From 1.0.0 onward Ember updates itself. At launch — and every few hours after that — it quietly asks GitHub whether a newer release exists. If one does, a small **Update** pill appears in the top bar; installing verifies the build's signature, swaps it in place and restarts Ember. Your settings, hook installs and history are left alone.

You can also check on demand from **Settings → Updates → Check now**. A check that cannot reach GitHub says so in plain language and changes nothing; when there is nothing newer, Ember says *"You're on the latest version."*

---

## Building from Source

**Prerequisites**

- [Node.js](https://nodejs.org/) 18 or newer, with npm
- [Rust](https://rustup.rs/) (stable toolchain)
- The [Tauri v2 Windows prerequisites](https://v2.tauri.app/start/prerequisites/) — Microsoft C++ Build Tools and WebView2

**Clone and install**

```bash
git clone https://github.com/sanirudh17/Ember.git
cd ember
npm install
```

**Run in development**

```bash
npm run tauri dev
```

**Build the installer**

```bash
npm run tauri build
# → src-tauri/target/release/bundle/nsis/Ember_1.3.0_x64-setup.exe
```

**Run the test suite**

```bash
cd src-tauri && cargo test                                        # Rust unit + real Windows Power API tests
cd src-tauri && cargo test --test native_lid -- --ignored         # opt-in live Windows lid-policy + watchdog restore test
cd src-tauri && cargo clippy --all-targets -- -D warnings         # lint check
npx tsc --noEmit                                                  # frontend typecheck
```

**Cutting a release (updater signing)**

Releases are built and signed with the updater key; the workflow drafts the GitHub release with the installer, its `.sig` and `latest.json`. See [`docs/RELEASING.md`](docs/RELEASING.md) for the one-time repository setup and the tag flow, and [`CHANGELOG.md`](CHANGELOG.md) for the release notes that accompany each version.

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1   # local signed release build
git tag v1.3.0 && git push origin v1.3.0                              # CI builds, signs, and drafts the release
```

---

## Tech Stack

| Layer | Technology |
| --- | --- |
| App shell | [Tauri v2](https://v2.tauri.app/) — Rust core + WebView2 |
| Backend | Rust — the [`windows`](https://crates.io/crates/windows) crate for power requests, lid power-scheme control, named pipes and process inspection |
| Frontend | React 19, TypeScript, Vite |
| Process scanning | [`sysinfo`](https://crates.io/crates/sysinfo) |
| Tray & notifications | `tauri-plugin-notification`, tray-icon support |
| Autostart / single instance | `tauri-plugin-autostart`, `tauri-plugin-single-instance` |
| Updates | `tauri-plugin-updater` (minisign-verified) |
| Sound | [`rodio`](https://crates.io/crates/rodio) |
| Window material | [`window-vibrancy`](https://crates.io/crates/window-vibrancy) |

---

## Project Structure

```
src/                     React + TypeScript frontend (Vite)
  pages/                 Home (Awake workspace), Agents, Settings
  components/            TopBar, CustomHoldDialog, CloseDialog, shared UI, agent icons
  lib/                   store, appearance, typed command bindings, formatting
src-tauri/
  src/
    agents/              the seven agents: registry, process scan, activity cache, harness reader, hook installers, hook pipe server
    engine/              decisions, sessions, manual holds, DST-aware until resolver, snapshots
    power.rs  lid.rs     the hold: Modern Standby power requests and the lid-close override
    battery.rs           battery level, AC/DC, Battery Saver, and power-event re-assertion monitor
    watchdog.rs          detached lid-restore watchdog (survives Ember being killed)
    hookclient.rs        the ~300 ms `ember.exe hook …` client agents invoke
    tray.rs  sound.rs  log.rs  settings.rs  shell.rs  paths.rs
  icons/                 generated from app-icon.png
docs/                    AGENT_INTEGRATIONS.md · AGENT_DETECTION_AUDIT.md · RELEASING.md
scripts/                 build-release.ps1 · gen-icon.mjs
```

---

## Configuration & Data

Everything Ember writes stays on your machine:

```
%APPDATA%\Ember\
├── settings.json       # your choices from the Settings tab
├── installs.json       # exactly what Ember installed, and where (the hook ledger)
├── hook-token          # per-install random token that authenticates hook writes
└── ember.log           # rolling log — also what Settings → Recent activity shows

%LOCALAPPDATA%\Ember\
└── session-state.json  # lid state, written before it changes and reverted on start
```

Hook writes arrive over the local named pipe `\\.\pipe\ember-hooks`, authenticated with the token above; non-local clients are rejected.

---

## Privacy

Ember is built to be **local-first**. It has no accounts, no telemetry and no analytics. The only network request it ever makes is the update check against this repository's releases — and only when it checks for updates. Agent hooks never send your prompts, transcripts or code anywhere: they deliver a session id, a working folder and a timestamp to a local pipe, and nothing else.

---

## Troubleshooting

**An agent never shows as Working.**
Open the **Agents** tab and install or repair hooks for that agent if it says they are missing or need repair. Then restart that agent's session once — extensions and plugins load at session start, so a chat that was already running before the install will not pick it up. Hook-supported agents require lifecycle hooks (or supported T3/GOAT session metadata) rather than CPU heuristics so opening or typing in an editor never causes false holds.

**The PC still sleeps.**
Check the battery — Ember lets go below your battery floor and whenever Battery Saver turns on. If the lid is closed on battery and *"Closed-lid hold requires the charger"* is on, the lid override is not applied by design.

**Windows SmartScreen warns about the installer.**
The installer is not code-signed yet. Click **More info → Run anyway**; the in-app updater artifacts are signed and verified independently of SmartScreen.

**The update check says it cannot reach GitHub.**
That is the one thing Ember needs the network for. Check your connection — and note that a repository with no published release simply reports *"You're on the latest version."* Nothing else is affected.

---

## License

Released under the [MIT License](LICENSE). © 2026 Sanirudh ([sanirudh17](https://github.com/sanirudh17)).

---

## Acknowledgements

Built with [Tauri](https://v2.tauri.app/). Agent integrations follow each tool's own documented hook or extension surface — the full per-agent matrix, including the pitfalls each one had on Windows, lives in [`docs/AGENT_INTEGRATIONS.md`](docs/AGENT_INTEGRATIONS.md).

### Detection coverage in v1.3.0

Reasoning and tool execution belong to the enclosing turn; finishing one tool is not finishing the turn. Explicit user-question/UI waits suspend a hold when the host exposes those events. Cancellation, session closure, and abrupt process termination cannot be undone by late tool callbacks or reused PIDs, and silent or cached working sessions are bounded by a 45-minute safety cap. See [the agent audit](docs/AGENT_DETECTION_AUDIT.md) for the tested sequences and host-specific limits.

After updating, use **Agents > Repair** for any integration flagged for repair and restart/reload its host process so updated bindings and exit cleanup handlers take effect. Codex may request normal trust review for changed hooks. Current Cline supports global Windows PowerShell hooks; older or disabled hosts retain approximate CPU fallback. Hosts that do not load lifecycle hooks cannot provide exact turn boundaries. Modern Standby power requests are strengthened with `ExecutionRequired` (best-effort), `DisplayRequired` during manual holds, and automatic re-assertion on resume, AC/DC, and lid-close events; on battery-powered Modern Standby devices, Windows may still enforce OEM/OS limits, so plug in for extended unattended runs. Ember does not block explicit shutdown or Windows safety actions.
