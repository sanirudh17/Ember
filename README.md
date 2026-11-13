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

Ember also recognizes Antigravity's `agy_acp_server.exe` when launched by GUI clients. Shared ACP transport CPU alone does not earn a hold; session hooks or supported T3/GOAT local session projections report the actual work. Private Antigravity profiles and GOAT stable/nightly homes are supported. Hooks are authoritative: opening a session does not engage a hold, and an explicit idle/end event does not inherit a long live-process grace. Codex interruption is handled as idle (repair existing hooks to install this binding).

Current Cline supports global Windows PowerShell hooks; enable Hooks in Cline after installing the integration. Its fallback for older or disabled hosts remains an estimate, using sustained CPU activity and a short quiet window; it cannot guarantee immediate turn detection during network waits or distinguish every typing burst. Prefer hooks where supported. Toasts follow visible state changes; repeated sounds are throttled separately.

The default window is 1060 by 680 and remembers its geometry and maximized state. Startup waits for the themed frontend to paint. The supplied Ember artwork is used in the app, tray, taskbar, and installer; run `npm run icon` to regenerate sizes without replacing the source artwork.

## Overview

Ember is a keep-awake companion for Windows aimed at people who leave long agent runs unattended. It watches the coding agents you actually use, and holds the machine awake exactly while any of them is working. When the last one finishes, Ember lets go.

The usual keep-awake tool is a toggle you forget: you turn it on and your laptop never sleeps again, or you forget it and a multi-hour agent run gets suspended halfway. Ember replaces the toggle with detection. Busy machines stay awake, quiet machines sleep like normal, and you never think about it.

> **New here?** Install the single `Ember_1.2.0_x64-setup.exe` from [Releases](https://github.com/sanirudh17/Ember/releases/latest) and run it. Ember lives in the tray and updates itself from then on.

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

- **Work with the lid shut.** While a hold is active, closing the lid no longer sleeps the machine. The override is capped, can require the charger, and is always put back.
- **Crash-proof restore.** The original lid action is recorded before it is changed, and a detached watchdog process reverts it even if Ember is killed mid-hold.
- **Battery floor and Battery Saver release.** Ember lets go below your chosen battery percentage (10–30 %) and whenever Windows Battery Saver turns on.

**Control and comfort**

- **Manual hold.** Take the hold yourself for a chosen duration when Ember is not watching an agent — a long build, an upload, a download.
- **Keep the screen on, or hands off.** Optionally keep the display lit while holding; otherwise your screen follows the Windows power plan while the PC stays awake.
- **Tray companion.** Launch at login, a toast and chime when Ember takes hold and lets go (mutable), single instance, and a window position reset.
- **Quiet surface.** Every agent gets a generated monogram tile — never a third-party logo — and the Home tab narrates every decision the engine makes instead of hiding them behind toggles.

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
        ├─ power requests: system sleep blocked (display optional)
        └─ lid override: closing the lid stops sleeping — capped, charger-gated, watchdog-restored
```

**Why two signals.** Hooks report lifecycle transitions when the host emits them. Process discovery confirms which applications are open, while persisted hook state and supported T3/GOAT session metadata provide restart recovery. CPU is used only for unhooked Cline's approximate working estimate; opening or typing in a hook-supported agent cannot earn a hold from CPU alone. Missing hooks must be installed or repaired rather than hidden behind an unreliable activity guess.

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
| **`Ember_1.2.0_x64-setup.exe`** | NSIS installer — single executable, no extra dependencies. Recommended. |

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
# → src-tauri/target/release/bundle/nsis/Ember_1.2.0_x64-setup.exe
```

**Run the test suite**

```bash
cd src-tauri && cargo test   # Rust unit tests
npx tsc --noEmit             # frontend typecheck
```

**Cutting a release (updater signing)**

Releases are built and signed with the updater key; the workflow drafts the GitHub release with the installer, its `.sig` and `latest.json`. See [`docs/RELEASING.md`](docs/RELEASING.md) for the one-time repository setup and the tag flow, and [`CHANGELOG.md`](CHANGELOG.md) for the release notes that accompany each version.

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1   # local dry run
git tag v1.0.0 && git push origin v1.0.0                              # CI drafts the release
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
  pages/                 Home (hold status), Agents, Settings
  components/            TopBar, dialogs, shared UI, generated agent monograms
  lib/                   store, typed command bindings, formatting
src-tauri/
  src/
    agents/              the seven agents: registry, process scan, hook installers, hook pipe server
    engine/              decisions, sessions, manual holds, snapshots
    power.rs  lid.rs     the hold: power requests and the lid-close override
    watchdog.rs          detached lid-restore watchdog (survives Ember being killed)
    hookclient.rs        the ~300 ms `ember.exe hook …` client agents invoke
    tray.rs  sound.rs  log.rs  settings.rs  paths.rs
  icons/                 generated from app-icon.png
docs/                    AGENT_INTEGRATIONS.md · RELEASING.md
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
Open the **Agents** tab and install hooks for that agent if it says they are missing. Then restart that agent's session once — extensions and plugins load at session start, so a chat that was already running before the install will not pick it up. Process detection keeps the machine awake in the meantime.

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

### Detection coverage in v1.2.0

Reasoning and tool execution belong to the enclosing turn; finishing one tool is not finishing the turn. Explicit user-question/UI waits suspend a hold when the host exposes those events. Cancellation and session closure cannot be undone by late tool callbacks. See [the agent audit](docs/AGENT_DETECTION_AUDIT.md) for the tested sequences and host-specific limits.

After updating, use **Agents > Repair hooks** for existing integrations and restart/reload their host processes. Codex may request normal trust review for changed hooks. Current Cline can use the new global Windows hooks; older or disabled hosts retain approximate fallback. Hosts that do not load lifecycle hooks cannot provide exact turn boundaries. CPU cannot reliably distinguish network-only reasoning, approvals, and Escape. See [the v1.2.0 verification report](docs/RELIABILITY_FOLLOWUP.md) for coverage and native power API proof. On battery-powered Modern Standby devices, Windows may expire power requests; plug in for extended runs. Ember does not block explicit shutdown or Windows safety actions.
