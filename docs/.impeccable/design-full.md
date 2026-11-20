# Design System: Ember

## Overview

**Creative North Star: "Quiet Graphite & Amber Instrument"**

Ember preserves its original stable release visual identity: neutral warm graphite monochrome in Dark mode (`#171717` chrome, `#111213` canvas, `#1b1c1e` readout surface, `#efefed` ink) and crisp cool-blue/white in Light mode (`#e5e9f0` chrome, `#fbfcfe` canvas, `#eaf0f6` readout surface), accented solely by Ember's warm orange-amber (`#f59e0b`). No glass effects or extra accent hues.

## Colors

- **Primary:** Ember Amber (`#f59e0b`) — active navigation icon, enabled switches, primary CTA buttons, and subtle working-session row wash.
- **Dark Neutral:** Chrome `#171717`, Canvas `#111213`, Surface `#1b1c1e`, Border `#303133`, Ink `#efefed`, Muted `#adaeaf`.
- **Light Neutral:** Chrome `#e5e9f0`, Canvas `#fbfcfe`, Surface `#eaf0f6`, Border `#ccd5e2`, Ink `#202124`.

## Typography

- **Display:** Bundled `Archivo-400.woff2` (`28px` page titles and Awake status).
- **Body / UI:** `Segoe UI Variable Text` / `Segoe UI` (`13-14px`).
- **Control labels:** Cap-height centered via `text-box-trim: trim-both; text-box-edge: cap alphabetic;`.

## Components

- **Segmented Control:** Subpixel `getBoundingClientRect` sliding thumb (`210ms cubic-bezier(0.22, 1, 0.36, 1)`) with cap-centered labels and atomic theme switching.
- **Switch:** `34×20px` borderless track with `14×14px` circular thumb (`3px` uniform ring on all sides).
- **Scrollbars:** Whisper-faint `4px` transparent-track thumb with no native arrow buttons.
- **Route Panes:** All three views stay mounted (`hidden` when inactive) for instant zero-delay tab switching.
