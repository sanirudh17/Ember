# Third-Party Notices

## Agent Logos and Trademarks

Logos belong to their respective owners and are used only to identify the tools.

| Tool | Asset File | Source / Upstream |
| :--- | :--- | :--- |
| **Claude Code** | `src/assets/agents/claudecode-color.svg` | Anthropic (`@lobehub/icons-static-svg` — `icons/claudecode-color.svg`) |
| **Codex** | `src/assets/agents/codex.svg`, `src/assets/agents/codex-color.svg` | OpenAI (`@lobehub/icons-static-svg` — `icons/codex.svg`, `icons/codex-color.svg`) |
| **OpenCode** | `src/assets/agents/opencode.svg` | SST / OpenCode (`@lobehub/icons-static-svg` — `icons/opencode.svg`) |
| **Cline** | `src/assets/agents/cline.svg` | Cline (`@lobehub/icons-static-svg` — `icons/cline.svg`) |
| **Antigravity CLI** | `src/assets/agents/antigravity-color.svg` | Google Antigravity (`@lobehub/icons-static-svg` — `icons/antigravity-color.svg`) |
| **Pi** | `src/assets/agents/pi.svg` | Earendil Works / Pi Coding Agent (`https://pi.dev/logo-auto.svg`, `github.com/earendil-works/pi`) |
| **Oh My Pi** | `src/assets/agents/oh-my-pi.svg` | Can Bölük (`github.com/can1357/oh-my-pi` — `assets/icon.svg`, `#fafafa` adapted to `currentColor`) |

---

## Open-Source Packages

- **`@lobehub/icons-static-svg`** — MIT License, Copyright (c) LobeHub. Used at build preparation time to extract static SVG icons for Claude Code, Codex, OpenCode, Cline, and Antigravity CLI.
- **Tauri v2**, **React**, **Lucide Icons**, **window-vibrancy**, **sysinfo**, and **windows-sys** — Licensed under their respective MIT / Apache-2.0 licenses as declared in `package.json` and `src-tauri/Cargo.toml`.
- **`tauri-plugin-updater`** — MIT / Apache-2.0 (with its transitive TLS, zip and signature-verification crates as locked in `src-tauri/Cargo.lock`). Used for signed in-place updates from GitHub Releases.
