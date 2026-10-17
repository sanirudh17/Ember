# Ember Design System & UI Specification

This document defines the visual tokens, typography, layout rules, surface patterns, and interaction principles applied across Ember. It translates the principles from our design skills (`impeccable`, `design-taste-frontend`, `ui-ux-pro-max`, `minimalist-ui`, `gpt-taste`, `high-end-visual-design`, and `redesign-existing-projects`) into concrete rules for Ember's compact Windows utility experience.

---

## 1. Aesthetic Direction & References

- **Density Reference**: macOS Finder list view — crisp 13px typography, 32–36px interactive rows, 1px hairline separators, and near-zero decorative chrome.
- **Restraint Reference**: Linear, Raycast, and Vercel — quiet, engineered surfaces where status and hierarchy come from alignment, contrast, and spacing rather than gradients, drop shadows, or badges.
- **Anti-Patterns Strictly Forbidden**:
  - No purple or multi-stop decorative gradients.
  - No neon glow effects or pulsing status animations.
  - No oversized rounded cards (`>12px` radius) or nested cards-inside-cards.
  - No emojis or decorative section header icons.
  - No duplicate controls for the same underlying setting (e.g., never pair both a stepper and a range slider for the same value).
  - No internal jargon (`AoAc`, `DWM`, `named pipe`, `rgba tint`, `SetThreadExecutionState`) in primary user-facing copy; technical identifiers belong exclusively in **Settings > Advanced > Diagnostics**.

---

## 2. Typography Tokens

- **Primary Font Stack (`--font-sans`)**: `'Segoe UI Variable Text', 'Segoe UI Variable Display', 'Segoe UI', -apple-system, BlinkMacSystemFont, sans-serif`
- **Monospace Font Stack (`--font-mono`)**: `'Cascadia Mono', 'JetBrains Mono', 'Consolas', monospace` (used strictly for numeric readouts, filesystem paths, timestamps, and compact status tags; always paired with `font-variant-numeric: tabular-nums`).

| Token Role | Size / Line Height | Weight | Tracking / Case | Usage |
| :--- | :--- | :--- | :--- | :--- |
| **Countdown Display** | `36px / 40px` | `600` (Semibold) | `-0.03em`, `tabular-nums` | Timer page main countdown readout |
| **Hero Status** | `22px / 28px` | `600` (Semibold) | `-0.02em` | Primary status headline |
| **Page Title** | `18px / 24px` | `600` (Semibold) | `-0.015em` | Top-level view header |
| **Body / Row Label** | `13px / 18px` | `400` / `500` | `0` | Primary list rows, controls, buttons |
| **Secondary / Helper** | `12px / 16px` | `400` | `0` | Single-line descriptions, metadata |
| **Mono Label** | `11px / 14px` | `600` | `+0.05em`, `uppercase` | Section headers, badges, status pills |

---

## 3. Sizing, Spacing & Geometry Tokens

- **Window Dimensions**:
  - Default Window Size: `920 × 640`
  - Minimum Window Size: `760 × 520` (resizable across all monitors)
  - Overview and Timer pages must fit cleanly at `920 × 640` without vertical scrolling.
- **Chrome & Navigation**:
  - Title Bar Height: `32px`
  - Sidebar Width: `188px` default; collapses automatically to a `56px` icon rail below `760px` viewport width.
  - Sidebar Navigation Row Height: `30px`
  - Main Content Max-Width: `960px` centered, with `24px` page padding (`16px` compact under `760px`).
- **Component Sizing**:
  - Standard Controls (Buttons, Selects, Inputs, Segmented Controls): `28px` height
  - Icon Buttons: `28 × 28px`
  - Switch Toggles: `32 × 18px` (`14px` thumb)
  - Quick-Action Chips / Pills: `24px` height
  - List / Setting Rows: `32–36px` minimum height (compact single-line or label + 1-line helper)
  - Agent Logo Tile: `26 × 26px` neutral container (`border-radius: 6px`) rendering an `18 × 18px` official SVG icon.
- **Spacing Scale**: `4px`, `8px`, `12px`, `16px`, `24px`
  - Section / Card Padding: `12px–16px`
  - Section Stack Gap: `12px`
- **Border Radii**:
  - Primary Section Surfaces: `12px`
  - Controls, Buttons, Inputs, Menu Popovers: `8px`
  - Agent Icon Tiles: `6px`

---

## 4. Color & Surface Architecture

Ember supports **Dark** and **Light** themes over **Mica**, **Acrylic**, and **Solid** backdrops while guaranteeing WCAG AA contrast over any desktop wallpaper.

### Accent & Status Semantics
- **Ember Accent (`#E8672E` dark / `#D4551C` light)**: Reserved strictly for primary call-to-action buttons, active switch toggles, and active slider fills.
- **Status Dot**: Calm, static `8px` circle — no pulsing rings or box-shadow blurs:
  - **Awake / Working**: `#E8672E` (Ember orange)
  - **Manual / Timed Hold**: `#3B82F6` (Calm blue)
  - **Attention / Paused / Battery Guard**: `#F59E0B` (Amber)
  - **Idle / Normal Sleep**: `#6B7280` (Neutral slate)
  - **Verified / Healthy**: `#10B981` (Emerald)

### Surface Hierarchy
- **One Surface Per Section**: Group related settings into a single flat surface (`border-radius: 12px`, `1px solid` subtle border, quiet translucent fill on Mica/Acrylic or opaque surface on Solid) divided internally by `1px` hairline separators (`rgba(255, 255, 255, 0.06)` in dark mode, `rgba(15, 23, 42, 0.07)` in light mode).
- **Row Layout**: Label and concise one-line helper on the left; a single canonical control on the right.

---

## 5. Material & Theme Transition Pipeline (Zero Transparency Flash)

Switching between **Mica**, **Acrylic**, and **Solid** (or toggling **Dark** / **Light** theme) uses a single deterministic pipeline so an unstyled transparent frame never flashes on screen:

1. **Cover Paint**: Frontend renders an opaque, theme-matched cover layer (`#111317` in dark, `#F6F7F9` in light) over the viewport and awaits **two `requestAnimationFrame` ticks** so the compositor has committed the opaque frame.
2. **Backend Apply**: Frontend invokes `set_backdrop` (or theme sync), which applies the new DWM backdrop before clearing the previous effect where supported, and logs timestamped steps to `ember.log`.
3. **Cover Resolve**:
   - When the target material is **Solid**, the opaque background remains permanently active as the base surface.
   - When the target material is **Mica** or **Acrylic**, the cover layer fades out smoothly over `120ms` (`ease-out`) to reveal the newly applied native backdrop.
