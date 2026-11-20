# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Windows developers and creators running independent coding agents (or long-running local tasks like renders) who need their machine to stay awake reliably without manually toggling Windows power plans.

## Product Purpose

Ember is a Windows desktop utility (Tauri v2 + React + Rust) whose primary job is to prevent system sleep during confirmed coding-agent work or an explicit manual hold, while giving quick, trustworthy visibility into why the PC is being held awake and for how long.

## Positioning

Unlike generic keep-awake toggles, Ember pairs native Windows power requests (`SystemRequired`, `ExecutionRequired` for Modern Standby, and `DisplayRequired`) with direct hook/plugin integrations across seven coding agents, backed by reversible closed-lid overrides, battery-floor guards, and Windows Battery Saver deference.

## Operating Context

- Runs continuously on Windows 11/10 desktop and Modern Standby (S0ix) laptops, either open in a frameless 1100×700 workspace window (remembering normal geometry and maximization) or minimized to the system tray.
- Monitors seven coding tools: Claude Code, Codex, OpenCode, Pi, Oh My Pi, Antigravity, and Cline.
- Hooks provide authoritative session state; process CPU activity is an estimate, and stale working sessions are bounded against missed shutdown hooks.
- Manual holds keep the display lit to protect unattended renders and active sessions; agent-triggered holds follow the separate display setting.

## Capabilities and Constraints

- **Three-screen navigation:** `Awake` (live hold status, manual hold controls, 4-metric device readout, automatic hold policies, and sessions), `Agents` (hook installation/repair with diff previews and live sessions), and `Settings` (General, Power & display, Updates, and collapsible Recent activity log).
- **Safety invariants:** Battery Saver always takes priority; battery floor releases holds when unplugged below threshold; closed-lid overrides are capped and automatically restored; closing the window may keep Ember in the tray.
- **Honest telemetry:** Status, hold durations, agent rows, battery state, and lid state always come from live backend snapshots—never fabricated activity.

## Brand Commitments

- Name: **Ember**.
- Retain the supplied folded ember artwork (`app-icon.png` and derived icons) throughout.
- Support System, Light, and Dark themes with strong outer-chrome vs. inner-workspace contrast and Ember's signature warm amber accent (no violet/lavender).
- Preserve Ember's character: framed rectangular containers for Manual hold and the 4-metric dashboard readout, tactile Solid Glass materiality, and scannable grouped Settings without stripping away helpful context.

## Evidence on Hand

- Live Rust backend (`src-tauri/src/`) providing real snapshots, power requests, hook installers, battery/lid telemetry, and engine activity logs.
- Supplied source artwork in `app-icon.png` and monochrome interface/agent icons in `src/assets/interface/` (with third-party notices in `THIRD_PARTY_NOTICES.md`).
- Bundled display typeface `Archivo-400.woff2` in `src/assets/fonts/`.
- No invented testimonials, user metrics, or placeholder agent activity.

## Product Principles

1. **Instant legibility of hold truth:** A single glance answers whether the PC is held awake, why, and for how long.
2. **Tactile precision over flat austerity:** Controls, framed instrument panels, and preference groups feel physical, high-contrast, and deliberate without visual clutter.
3. **Fail-safe power stewardship:** Power requests, lid-close overrides, and stale hook caches always err on the side of protecting battery life and hardware safety.
4. **Transparent automation:** Every hook install or removal previews the exact file path and diff before applying.

## Accessibility & Inclusion

- High-contrast light and dark themes targeting WCAG AA/AAA text legibility across all surfaces.
- Full keyboard navigation for switches, sliding segmented radio groups (Arrow/Home/End keys), and modal dialogs (Escape to dismiss), plus `prefers-reduced-motion` compliance.
