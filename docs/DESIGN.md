---
name: Ember
description: Quiet monochrome-and-amber Windows keep-awake utility for coding agents
colors:
  ember-amber: "#f59e0b"
  dark-chrome: "#171717"
  dark-bg: "#111213"
  dark-surface: "#1b1c1e"
  dark-ink: "#efefed"
  light-chrome: "#e5e9f0"
  light-bg: "#fbfcfe"
  light-surface: "#eaf0f6"
  light-ink: "#202124"
typography:
  display:
    fontFamily: "Archivo, Segoe UI Variable Display, system-ui, sans-serif"
    fontSize: "28px"
    fontWeight: 400
  body:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
rounded:
  control: "5px"
  panel: "7px"
  pill: "999px"
components:
  button-primary:
    backgroundColor: "{colors.ember-amber}"
    rounded: "{rounded.control}"
  summary-panel:
    backgroundColor: "{colors.dark-surface}"
    rounded: "{rounded.panel}"
---

# Design System: Ember

<!-- impeccable:design-map -->
North Star: "Quiet Graphite & Amber Instrument" · Fonts: Archivo / Segoe UI Variable Text
Rules: Monochrome neutral graphite dark / cool-blue light with warm orange-amber accents only; no glass effects, no extra colors; cap-centered control labels; smooth sliding segmented thumb; instant route switching.
Full: docs/.impeccable/design-full.md
