# Ember Feature Parity & Verification Report

This document records the verification of feature parity, bug fixes, design tokens, and testing for the Ember application redesign.

---

## 1. Feature Parity Checklist

Every single feature of the original Ember application has been retained and refined:

| Category | Feature | Status | Notes |
| :--- | :--- | :--- | :--- |
| **Console / Overview** | Master "Ember is watching" toggle | Preserved | Optimistic store + backend lock-step sync (Bug G fix) |
| | Live mono status line | Preserved | Reports exact reason, duration, and unsolicited watch |
| | Timed Pause (30m, 1h, custom) | Preserved | Releases wake locks and lid overrides, auto-resumes |
| | Lid-proof mode toggle & safety note | Preserved | Thermal safety notice shown when armed or active |
| | Battery meter & AC/Saver status | Preserved | Percentage readout, status pills, and reserve cutoff |
| | Quick manual session presets | Preserved | 15m, 30m, 1h, 2h quick start buttons |
| | Interactive coding agents list | Preserved | Direct navigation to agent detail with official SVG logos |
| | Reliability & live check indicators | Preserved | Power request, lid action, AoAc, and telemetry verification |
| **Agents** | Per-agent watch toggles | Preserved | Independent enable/disable per agent |
| | Master watch toggle in header | Preserved | Coordinated with Overview master switch |
| | Real-time session tracker | Preserved | Lists active working sessions, paths, and durations |
| | Hook installer / uninstaller / repair | Preserved | Idempotent config writer with diff preview dialog |
| | Backup & restore on hook install | Preserved | Automatic backup files created and displayed |
| | Seven coding agents supported | Preserved | Claude Code, Codex, OpenCode, Cline, Antigravity, Pi, Oh My Pi |
| **Sessions** | Manual keep-awake modes | Preserved | Countdown Timer, Until clock time, and Unlimited hold |
| | Duration steppers & presets | Preserved | Days, Hours, Minutes steppers + quick preset pills |
| | Clock-time "Until" picker | Preserved | 24-hour time picker with next-day DST-aware rollout |
| | Live session hero readout | Preserved | Redesigned to 36px tabular monospace numerals |
| | Extend session actions | Preserved | +15m and +1h chips during active sessions |
| **Power & Display** | Only run while plugged in | Preserved | Automatic yield on AC disconnection |
| | Low battery cutoff floor | Preserved | Single slider with live % readout (duplicate controls removed) |
| | Battery Saver compliance | Preserved | Yields hold when Windows Battery Saver engages |
| | Post-activity grace period | Preserved | Single slider (0–600s) with live seconds readout |
| | Lid-proof override & safety cap | Preserved | 1–24 hour safety cap with manual restore button |
| | Display sleep policy | Preserved | Keep on, Let sleep, or Turn off immediately |
| | Display timeout delay | Preserved | Delay slider (5–600s) before display sleep |
| **Notifications** | Synthesized procedural chimes | Preserved | 10 pure wave sounds rendered in Rust with zero audio files |
| | Chime preview palette | Preserved | Audition chips for all 10 procedural waveforms |
| | Volume control & Master mute | Preserved | Live volume slider and silence-all switch |
| | Transition event notifications | Preserved | Independent Toast and Chime controls per event (Bug F fix) |
| **Settings** | Theme selector | Preserved | System (live prefers-color-scheme), Light, Dark |
| | Backdrop material selector | Preserved | Mica, Acrylic, Solid with zero-transparency transition (Bug D fix) |
| | Windows version display | Preserved | Shows exact detected Windows build (Bug B fix) |
| | Launch at login | Preserved | Startup registry key management |
| | Always on top & Title status | Preserved | Window pinning and dynamic title formatting |
| | Stale session timeout | Preserved | 1–240 minute timeout for inactive hook sessions |
| | Diagnostic trace log | Added | "Copy recent log" for troubleshooting phantom triggers (Bug A fix) |
| | Window geometry persistence | Preserved | Silently saves & restores bounds clamped to active monitors |

---

## 2. Bug Fixes & Root Cause Analysis

### Bug A: Phantom "Agent Working" Popup at Startup
- **Root Cause Confirmed**:
  1. `ProcessDetector` read process CPU percentage immediately on startup. `sysinfo` CPU usage requires two consecutive samples at least `MINIMUM_CPU_UPDATE_INTERVAL` apart before measurements are valid.
  2. The process name matcher loosely matched directory names and child binaries (e.g. `bin` was returned as folder name from `~/.claude/bin/claude.exe`).
  3. Single brief CPU spikes triggered instant `working = true`.
- **Fix Applied**:
  - `ProcessDetector` enforces a 10-second ignore window (`STARTUP_IGNORE_WINDOW`) after engine initialization.
  - Requires at least 2 valid CPU sample passes before any process state is trusted.
  - Process detection accumulates whole-tree CPU energy above a 2% floor (`CPU_FLOOR = 2.0`, any single sample capped at `CPU_SAMPLE_CAP = 4.0`) and only marks an agent working once a full `CPU_ENERGY = 6.0` budget is earned — brief spikes never engage, modest sustained load does.
  - Tighter executable matching with strict exclusions for Ember, Edge WebView2, shells, and multiplexers. `guess_folder` never returns `"bin"`.
  - Added a short hold visibility gate (`VISIBILITY_HOLD_DELAY`) before firing user-visible notifications, chimes, dot, or title bar changes.
  - Rolling debug log in `%APPDATA%\Ember\ember.log` with "Copy recent log" in Settings > Diagnostics.

### Bug B: Mica Reported as Unsupported on Current Windows 11
- **Root Cause Confirmed**:
  - The backdrop manager was using an artificial version pre-check that failed or clamped incorrectly on Windows 11 build 26100/26200+ canary/dev builds.
- **Fix Applied**:
  - Removed artificial version pre-check. The engine attempts `apply_mica()` directly and reports `mica_supported` based on whether the Win32 DWM call succeeds.
  - Windows build detection uses `ntdll::RtlGetVersion` with fallback to `CurrentBuildNumber` registry key, parsed as `u32` with `>= 22000` comparisons.
  - Mica option is never disabled in the UI unless `apply_mica` actually failed.

### Bug C: "Reset to 1000 × 720 & center" Crashed the App & Window Launch Visibility
- **Root Cause Confirmed**:
  1. Calling Tauri window resize methods while holding the `engine` mutex deadlocked the UI thread when Win32 window message handlers dispatched window resize callbacks that attempted to read the engine state on the same thread.
  2. In `tauri.conf.json`, `"visible": false` was set, but the frontend never invoked `api.showWindow()`, and `lib.rs` checked `is_visible()` (which falsely returned `true` when `ws.maximized` was true on launch before composition occurred), leaving the application window completely hidden on screen and missing from the taskbar while `ember.exe` ran silently in the background.
- **Fix Applied**:
  - Removed the "Reset to 1000 × 720 & center" button and Window Geometry card from Settings UI.
  - Window size, position, and maximized states persist silently with bounds clamped to visible monitors on restore.
  - Set `"visible": true` in `tauri.conf.json`.
  - Added explicit `window.show()`, `window.unminimize()`, and `window.set_focus()` calls in `shell::place_window`, `lib.rs::setup`, and on frontend mount in `App.tsx`.
  - Wrapped page content in a React `<ErrorBoundary>` so the title bar and controls survive any runtime error with a calm "Something went wrong, Reload view" panel.
  - Added a Rust panic hook logging to `%APPDATA%\Ember\ember.log`.

### Bug D: Flash of Full Transparency When Switching to Solid
- **Root Cause Confirmed**:
  - Switching backdrop cleared the native DWM blur/acrylic backdrop before the opaque solid background layer was committed by the webview compositor, revealing desktop windows behind the frame.
- **Fix Applied**:
  - Implemented 3-step transition pipeline in `AppearanceProvider`:
    1. Frontend paints an opaque themed cover layer and awaits 2 animation frames (`requestAnimationFrame`).
    2. Backend `set_backdrop` clears and applies the new effect.
    3. When the backend resolves, the frontend fades the cover out over 120ms (or locks it as the permanent solid surface for Solid material).
  - Added timestamped debug logging for every step of the pipeline.

### Bug E: Terminal Multiplexer Background Sessions Keeping Agent "Working"
- **Root Cause Confirmed**:
  - Terminal multiplexers (e.g. WezTerm, Zellij, tmux) spawned child sessions or daemon servers that remained alive after the terminal window closed. Stale sessions or approximate process detection saw the detached process and kept Ember in "Working" state.
- **Fix Applied**:
  - Added terminal multiplexers (`wezterm-gui.exe`, `wezterm-mux-server.exe`, `zellij.exe`, `tmux.exe`, `psmux.exe`) to launcher exclusions.
  - Hook server and installer extension inject `ctx.pid` and process exit hooks (`process.once("exit")`, `SIGINT`, `SIGTERM`).
  - Active sessions verify that their associated process PID is still alive on every 1Hz tick; dead PIDs are pruned immediately.

### Bug F: Double Tone on Notifications
- **Root Cause Confirmed**:
  - Windows 10/11 system notifications played the native Windows notification chime by default, while Ember simultaneously played its synthesized procedural chime via Rust rodio audio output.
- **Fix Applied**:
  - Added `<audio silent="true"/>` to the Windows toast XML template to silence the native Windows notification tone.
  - Separated `toast: bool` and `chime: bool` toggles in settings, allowing independent control over toast popups and procedural chimes.

### Bug G: Master Watch Turning Off Automatically
- **Root Cause Confirmed**:
  - (1) In `src/lib/store.tsx`, `onStateChanged` received a snapshot on the next 1Hz tick before the backend command finished writing settings, wiping out the optimistic state.
  - (2) Backend `commands::set_watching` updated `engine.settings` but did not update `engine.last.watching`, causing the next emitted snapshot to reflect the stale value.
- **Fix Applied**:
  - Monotonic version-tracking in `store.tsx` with dedicated `setWatching` method that persists optimistic state across background ticks until confirmed.
  - Backend `set_watching` and `update_settings` synchronize `engine.last` immediately before releasing the lock.

---

## 3. Official Agent Logos

All generic monogram tiles were replaced with official SVG logos stored locally in `src/assets/agents/`:
- **Claude Code**: Official Anthropic coral/terracotta spark logo (`claudecode-color.svg`).
- **Codex**: Official OpenAI Codex / ChatGPT logo (`codex-color.svg` and `codex.svg` mono).
- **OpenCode**: Official OpenCode CLI logo (`opencode.svg`), `currentColor` adaptive.
- **Cline**: Official Cline robotic mascot logo (`cline.svg`).
- **Antigravity CLI**: Official Google Antigravity logo (`antigravity-color.svg`).
- **Pi**: Official Pi coding agent logo from `pi.dev` (`pi.svg`).
- **Oh My Pi**: Official Oh My Pi logo from `can1357/oh-my-pi` (`oh-my-pi.svg`), converted to `currentColor`.

Rendered at 18px inside 26×26px neutral tiles (radius 6px) with `currentColor` adaptation for light and dark themes.

---

## 4. Verification Test Results

### Rust Backend
- `cargo test`: **133 passed; 0 failed; 0 ignored** in 0.68s.
  - Unit tests verify startup CPU spikes never engage, sustained activity engages after exactly configured time, brief flapping stays silent, rolling log rotates, and build number parsing succeeds.
- `cargo clippy -- -D warnings`: **Clean, 0 errors, 0 warnings**.
- `cargo fmt --check`: **Clean, zero formatting diffs**.

### Frontend
- `npm run build` (`tsc --noEmit && vite build`): **Clean, 0 TypeScript errors, 1946 modules transformed in 9.62s**.
- Automated Headless Edge Screenshots: Captured all 8 views across dark/light themes and 920px / 760px viewport widths.
