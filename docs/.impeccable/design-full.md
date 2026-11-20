---
name: Ember
description: Solid Glass desktop keep-awake utility for Windows coding agents
colors:
  obsidian-chrome: "#07080a"
  smoky-canvas: "#121419"
  smoky-pane: "rgba(26, 30, 38, 0.84)"
  raised-control-dark: "rgba(41, 47, 59, 0.92)"
  milled-well-dark: "#090b0f"
  ink-dark: "#f5f7fa"
  ink-muted-dark: "#8b95a8"
  slate-chrome-light: "#dde2ec"
  alabaster-canvas-light: "#f4f6fa"
  porcelain-pane-light: "rgba(255, 255, 255, 0.9)"
  milled-well-light: "#e3e7f0"
  ink-light: "#111418"
  ink-muted-light: "#5b6578"
  ember-amber: "#f59e0b"
  ember-amber-light: "#c95804"
  status-ok: "#34d399"
  status-danger: "#f87171"
typography:
  display:
    fontFamily: "Archivo, Segoe UI Variable Display, Segoe UI, system-ui, sans-serif"
    fontSize: "28px"
    fontWeight: 400
    lineHeight: 1.15
    letterSpacing: "-0.03em"
  title:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 600
    lineHeight: 1.3
  body:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "13.5px"
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "12.5px"
    fontWeight: 500
    lineHeight: 1
rounded:
  thumb: "6px"
  control: "7px"
  panel: "10px"
  dialog: "12px"
  pill: "999px"
spacing:
  xs: "6px"
  sm: "10px"
  md: "16px"
  lg: "24px"
components:
  button-primary:
    backgroundColor: "{colors.ember-amber}"
    textColor: "{colors.obsidian-chrome}"
    rounded: "{rounded.control}"
    height: "32px"
  button-quiet:
    backgroundColor: "{colors.raised-control-dark}"
    textColor: "{colors.ink-dark}"
    rounded: "{rounded.control}"
    height: "32px"
  glass-pane:
    backgroundColor: "{colors.smoky-pane}"
    textColor: "{colors.ink-dark}"
    rounded: "{rounded.panel}"
    padding: "15px 18px"
  segmented-thumb:
    backgroundColor: "{colors.raised-control-dark}"
    textColor: "{colors.ink-dark}"
    rounded: "{rounded.thumb}"
    height: "28px"
---

# Design System: Ember

## Overview

**Creative North Star: "The Solid Glass Instrument"**

Ember is a tactile, edge-lit Windows keep-awake instrument. Near-opaque smoked dark panes and porcelain light panes read solid from afar and reveal crisp top specular rims, fine hairlines, and grounded shadows up close. A deep outer shell wraps an inset workspace sheet for unmistakable contrast in both themes; warm amber signals active holds and primary actions.

**Key Characteristics:**
- 3-tier tonal contrast: outer chrome, wrapped workspace sheet, Solid Glass panes.
- Edge-only finish (top specular rim + hairline + shadow); flat, crisp faces.
- Recessed milled grooves for switch/segmented tracks paired with raised glass thumbs.
- Optical cap-height centering across all buttons and segmented controls via `text-box-trim`.

## Colors

### Primary
- **Ember Amber** (`#f59e0b` dark / `#c95804` light): Active hold state, primary CTAs, enabled switches, active nav icon, and working-row tint.

### Neutral
- **Outer Chrome**: Obsidian (`#07080a`) dark / Slate (`#dde2ec`) light.
- **Wrapped Sheet**: Smoky (`#121419`) dark / Alabaster (`#f4f6fa`) light.
- **Glass Panes**: Smoked (`rgba(26,30,38,0.84)`) dark / Porcelain (`rgba(255,255,255,0.9)`) light.
- **Milled Wells**: `#090b0f` dark / `#e3e7f0` light for segmented tracks, off switches, and diffs.

**The Edge-Only Finish Rule.** Glass materiality lives at the rim, hairline, and shadow—never muddy blur or gradients across text faces.
**The Amber Restraint Rule.** Keep chrome neutral; never introduce violet or lavender.

## Typography

**Display:** Archivo (`Archivo-400.woff2`) · **UI:** Segoe UI Variable Text · **Mono:** Cascadia Mono

- **Display** (400, 28px, 1.15): Page titles and primary Awake state headline.
- **Title** (600, 14px, 1.3): Section headers.
- **Body** (400/500, 13.5px, 1.5): Row labels, status lines, and concise descriptions.
- **Label** (500, 12.5px, 1): Controls and readout keys, cap-centered via `text-box-trim: trim-both; text-box-edge: cap alphabetic;`.

## Layout

1100×700 default window (`172px` sidebar, `46px` title bar). `.content` sits inset (`margin: 0 8px 8px 0`, `10px` radius). Awake pairs a status band + framed Manual hold pane, full-width framed 4-column readout pane, open 2-column policies, and Sessions ledger. Agents and Settings group rows inside `.glass-pane` rectangles.

## Elevation & Depth

1. **Outer Chrome** base frame -> 2. **Wrapped Sheet** (1px edge + top rim) -> 3. **Solid Glass Panes** (top rim + inner hairline + contact/diffuse shadow) -> 4. **Dialogs** (popover glass over 6px scrim) -> 5. **Milled Grooves** (inverted inset top shade + bottom rim).

## Shapes

10px panes & wrapped sheet; 7px buttons & inputs; 6px segmented thumbs inside a 9px groove; 12px dialogs; 999px switches & status dots.

## Components

- **Buttons:** 32px (`29px` sm), cap-centered `.control-label`, amber Solid Glass primary / raised neutral quiet / transparent ghost.
- **Segmented:** Recessed groove + subpixel `getBoundingClientRect` sliding Solid Glass thumb (`220ms cubic-bezier(0.22,1,0.36,1)`).
- **Switches:** 40×23px recessed track (off) or amber glass (on) with top-lit 16px circular knob.
- **Panes & Nav:** Single-level `.glass-pane` containers divided by hairlines; 172px sidebar with Solid Glass active pill.

## Do's and Don'ts

### Do:
- **Do** preserve 3-tier outer/sheet/pane contrast in both themes and cap-center all control labels.
- **Do** frame Manual hold and the 4-metric readout in `.glass-pane` rectangles and keep helpful Settings descriptions.

### Don't:
- **Don't** nest bordered cards inside cards, use liquid-glass blur over text, or add violet/lavender accents.
