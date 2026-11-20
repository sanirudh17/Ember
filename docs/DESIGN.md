---
name: Ember
description: Solid Glass desktop keep-awake utility for Windows coding agents
colors:
  ember-amber: "#f59e0b"
  ember-amber-light: "#c95804"
  obsidian-chrome: "#07080a"
  smoky-canvas: "#121419"
  smoky-pane: "rgba(26, 30, 38, 0.84)"
  slate-chrome-light: "#dde2ec"
  alabaster-canvas-light: "#f4f6fa"
  porcelain-pane-light: "rgba(255, 255, 255, 0.9)"
typography:
  display:
    fontFamily: "Archivo, Segoe UI Variable Display, system-ui, sans-serif"
    fontSize: "28px"
    fontWeight: 400
  body:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "13.5px"
    fontWeight: 400
rounded:
  control: "7px"
  panel: "10px"
  dialog: "12px"
components:
  button-primary:
    backgroundColor: "{colors.ember-amber}"
    rounded: "{rounded.control}"
  glass-pane:
    backgroundColor: "{colors.smoky-pane}"
    rounded: "{rounded.panel}"
---

# Design System: Ember

<!-- impeccable:design-map -->
North Star: "The Solid Glass Instrument" · Fonts: Archivo / Segoe UI Variable Text
Rules: Edge-only finish (top rim + hairline + shadow); warm amber for active holds only; no violet/lavender; never nest cards inside cards.
Do: 3-tier chrome/sheet/pane contrast, cap-centered control labels, framed Manual hold & 4-metric readout, helpful Settings hints.
Full: .impeccable/design-full.md
