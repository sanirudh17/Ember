# Ember interface

## Direction

Taste skill only for the current revision. Achromatic ledger, seed 989623370. Preserve the three-screen sidebar format, the Ember artwork and the light/dark theme choice. The utility answers whether the computer is held awake, why, and for how long. Status, hold durations, agent rows, battery guard and lid hold come from backend snapshots; do not invent activity.

## Shell

One continuous neutral frame holds both the 46px title bar and the 174px sidebar. Their background is identical in each theme: dark #090a0c, light #ededee. The main sheet is inset 8px at right/bottom with a 1px edge and 7px corners. Sidebar selection is neutral, without an accent underline. Chrome carries no decorative color; the existing artwork retains its own color.

## Content

Dark canvas #131416, foreground #ededee, secondary #a6a6ac. Light canvas #fafafa, foreground #202124, secondary #62636a. Use the bundled Archivo regular face, correctly declared at weight 400. Page/status headings are 28px; controls and rows 12-13px. Button text has a centered 20px line box inside a 30px control. Corners are 5px on controls. Reuse neutral theme tokens for borders, hover, scrollbars and switch tracks.

Settings uses a section-label column beside flat preference rows, with hairline separators rather than large rounded filled cards. The layout stacks section labels at minimum width. Long descriptions use available width instead of an arbitrary character cap. General, Power & display, Updates and a collapsed activity disclosure retain their existing functions. Awake uses a ruled horizontal status readout and flat control rows; Agents keeps the aligned list. No glass panels or glow effects.

## Verification

TypeScript/Vite build and browser control smoke checks passed, including sidebar routing, manual hold start/end, settings changes, no active-nav shadow and label-box centering. Browser captures cover all screens in light/dark at 760, 920 and 1600px. OpenCode's generated integration contract test passed again: v1 factory busy/retry/idle/delete and v2 setup execution-start/succeeded/interrupted/delete; v2 idle step does not release work. These are simulated contract tests, not authenticated live model sessions.

Source artwork is preserved in app-icon.png. Run npm run icon to regenerate derivatives. No backend behavior was changed in this visual revision.


## Reference refinement

The current refinement combines Impeccable polish with Taste's existing utility composition. References reviewed: hardbeat920/monocode (navigation and provider glyph sources), anomalyco/hex (bespoke sidebar icons and native control grouping), and krishhgg/Insomnia (compact, focused keep-awake controls). The user's attached API-tool screenshot guides toolbar density rather than being treated as a screenshot of the linked Insomnia utility.

MonoCode-derived navigation: lightning, robot, gear. Agent glyphs use a consistent theme-aware foreground and 20px row sizing; four glyphs come from MonoCode's assets. Antigravity's monochrome silhouette is derived from the existing SVG mask, removing filter effects that did not render reliably as an external mask. License texts are retained under src/assets/interface and referenced in THIRD_PARTY_NOTICES.md.

Ember's own identity returns through the original blue-charcoal ink family, Archivo regular display type (32px page titles, 36px status), amber selected-navigation glyphs and enabled switches, and warm primary actions. Title bar and sidebar still share one frame, with flat settings rows and no underline. Body/control sizes remain 13-14px for laptop readability. Wording remains Ember-specific rather than copying unrelated product text.


## Compact workspace remap

The default normal window is now 860 by 620 logical pixels and opens 40 logical pixels above its centered position, clamped below the monitor's top edge. Saved geometry/maximized behavior remains intact. Reset uses the same default constants. The current user's window settings were backed up before selecting the compact placement for review.

Awake is a hold console: current status and elapsed/remaining time share a single surface with manual start/extend/end controls. Automatic watching and lid behavior are separate compact rows. The session region shows working sessions only, with a View agents action for the complete list, including idle sessions. Battery and lid status sit on a quiet bottom line. A grace hold can still correctly read Finishing up; idle rows no longer contradict an Active sessions heading.

Agents retains its integration controls and gets 38px separation plus a divider before Sessions. Settings now stacks General and Power & display headings above their rows at every window size; the previous left-label column is removed. Appearance, battery, display, notifications, startup, updater and activity functions are retained. Updated workspace rules live in src/styles/workspace.css.

Verification: 193 Rust tests, TypeScript/Vite build, native debug build and browser interaction smoke check passed. Captures cover three routes in both themes at 860x620, 760x520 and 1600x900, plus working/manual/battery-blocked/off Awake states. No browser page errors or horizontal document overflow were reported. Inspected empty, active and manual layouts. Impeccable detector returned no findings. Physical lid testing and live model-host verification are outside this UI revision.


## Responsive and divider refinement

Removed the obsolete wide-window dashboard rules that left shrink-to-content alignment on Awake's sections. Awake now stretches in compact mode, and at 1140px or wider becomes a centered two-column workspace (up to 1180px): hold/automation left, active sessions right, device status across the bottom. The sidebar and title bar retain their shared frame.

Settings major groups use a 42px visual break, stronger section headings and normal row rhythm. There are no per-setting rules or header rules. Awake removes toolbar, automation and session-row rules; its main surface boundary and the device-status separator remain. Agents uses table-header rules only, with a 42px break between integrations and sessions. Added the missing page--agents scope so these rules apply to the actual page.

Frontend build and browser interaction smoke checks passed. Review covers normal, minimum and 1920x1080 maximized sizes in both themes, plus working/manual/blocked/off Awake states. No detection, power or window-state behavior changed in this refinement.


## Current Awake direction

The latest reference is Ember's original open composition. Awake has no enclosing card: status and manual controls occupy the left of the overview, four device readings occupy a separate quiet surface on the right, automatic controls form their own group, and sessions use the remaining content width. Compact windows stack the overview and wrap the readings and controls. Agents and Settings retain their accepted stacked, bounded layouts.

Removed the redundant Compact window title-bar button. Standard minimize and maximize/restore remain, including native state queries and the saved-geometry safeguards. Recent activity uses a 15px disclosure label, 14px explanation, and 13px log text. Content and logs use 5px transparent-track scrollbars with a theme-derived thumb and a stronger hover state.

Validation: TypeScript/Vite and standalone Tauri debug builds; centered-button/manual-hold/navigation smoke checks; all three pages at 760x520, 860x620, and 1920x1080 in both themes, plus working/manual/blocked/off Awake states. Browser review reported no page errors or horizontal document overflow. Taste lint and Impeccable detector returned no findings.

## Text-led Awake revision

Replaced the panel-based Awake layout with a continuous full-width ledger. Status and manual actions share the first band; four plain readouts share the next; watching and lid controls use aligned text rows; sessions show both working and idle entries. Only major band boundaries use rules. All routes now use adaptive 24-48px side insets instead of centered fixed-width caps. Compact windows stack controls and readings as needed.

Per user instruction, no build was run. Verified the Vite development preview at 1920x1080, 860x620, and 760x520, including both themes, with no horizontal overflow or page errors. TypeScript checking, desktop interaction smoke checks, Taste lint, and Impeccable detection passed. The packaged executable remains unchanged by this revision.

### Awake typography follow-up

Matched the Awake status heading to the other routes at 28px. Removed both readout-strip rules, leaving the session boundary only. Manual controls and readout labels use 13px text, and compact readout values use 16px. Browser-only preview now defaults to idle and identifies sample data in the title bar; explicit state=default remains available for working-state review. The former Claude/Codex message was a browser fixture, not a live engine report. Verified type checking, controls and 860/1920px layouts without building.

### Automatic-policy affordances and Settings rhythm

Updates now has 56px spacing after the power safety note. Awake automatic policies use plain outline icons, explicit On/Off text and aligned switches with no enclosing cards. Wide windows place the two policies side by side; compact windows stack them. Sessions retains the remaining available space and full-width rows, growing and scrolling with actual entries. Type checking, control smoke checks, 760/860/1920px browser checks and design source checks passed without building.

### Edge-spaced dashboard and native development review

Readouts use content-sized columns with space-between, aligning the first and last reading to the content edges. Compact two-column mode remains. Increased spacing beneath the status band and added a single restrained border around Manual hold. Native review confirmed both edge insets effectively zero and no horizontal overflow. Controls smoke check passed. No build ran.

Launched the existing debug desktop executable with a local WebView2 debugging port and loaded Vite HTML through a temporary CDP document response at the app's local origin. Vite script and React refresh imports point to localhost:1420, retaining the native IPC origin without expanding remote capabilities. Confirmed get_snapshot returns seven real watched agents and the preview marker is absent. Source modules and HMR come from Vite; a full document reload or app restart returns to the embedded document unless the temporary response is applied again. This is a review session, not a rebuilt development binary.

### Native font parity and quieter readings

The native Vite review now injects a development base URL, so CSS font URLs resolve against localhost rather than the embedded app. Confirmed Archivo loaded as a custom font in the native webview. Added scripts/open-native-dev.mjs to keep this document response available across reloads; it launches only the existing executable and performs no build. Start npm run dev first, then node scripts/open-native-dev.mjs.

The four readings now share one subdued surface with 20px/24px internal spacing. Scrollbars are 3px with 14-percent muted thumb opacity, rising to 32 percent on hover. Normal window defaults increase from 860 to 960 logical pixels wide; height remains 620, and saved placement is respected. Updated source defaults and their test expectation without compiling. Frontend type checking and control smoke checks passed. Native verification confirmed live snapshots, loaded Archivo, and no horizontal overflow.

## Shipped v1.1.0 finish

Default normal geometry is 1100x700 logical pixels, matching the approved window orientation; saved user geometry/maximization remains. The light theme now distinguishes frame, content and summary surfaces more clearly. Dark grounds are slightly deeper while preserving their blue identity. Manual hold has a softened border. Sessions states the open and working counts together. The complete release and actual v1.0.0 upgrade evidence is recorded in RELEASE_1.1.0_VALIDATION.md.
