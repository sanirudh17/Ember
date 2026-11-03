Ember v1.1.0 refines the desktop workspace and strengthens agent lifecycle detection.

- Responsive Awake dashboard, clearer settings, open/working session counts, and a 1100x700 default normal window.
- Supplied Ember artwork across the app, tray, taskbar and installer; consistent fonts and agent icons; clearer light surfaces and deeper blue-black dark mode.
- Improved reasoning, tool, waiting and cancellation handling, including OpenCode v1/v2 and Pi-family plugins.
- Prevented late callbacks, exited processes and shared server CPU from creating phantom holds.
- Preserved manual holds, battery guardrails, display options and reversible lid protection.

### Install or update

Download Ember_1.1.0_x64-setup.exe below, or use Settings > Updates > Check now in v1.0.0. The signed updater payload uses the same key and endpoint as the previous release. The installer is not Authenticode-signed, so a fresh install may still show SmartScreen.

After updating, open Agents and repair existing hooks, then restart/reload their agent hosts. Review changed Codex hooks through its normal trust flow. Cline and hosts without active hooks use approximate process inference; exact reasoning, approval and Escape timing require host signals. See the [agent audit](https://github.com/sanirudh17/Ember/blob/main/docs/AGENT_DETECTION_AUDIT.md) and [validation](https://github.com/sanirudh17/Ember/blob/main/docs/RELEASE_1.1.0_VALIDATION.md) for coverage and limits.

### Verification

200 Rust tests, frontend build and browser checks, and native lifecycle transport checks for all seven agent IDs passed. The actual published v1.0.0 executable discovered and installed v1.1.0 through its normal updater; user settings were preserved. Public downloads match the tested local artifacts. Physical lid/sleep testing and authenticated live turns for every upstream host remain outside this verification.
