# Ember v1.2.0

More reliable agent activity detection, faster startup discovery, and the refined graphite desktop theme.

## Changes

- Global Cline Windows PowerShell hooks with task/question/cancellation handling, preserved user policies and rollback on partial write failure.
- Immediate open-process discovery and populated startup snapshots.
- Recovery of live hook sessions after Ember restarts, validated against process creation time.
- OpenCode stream reconnection and missed-terminal reconciliation, including native date objects and protection against delayed metadata overriding a newer turn.
- Pi and Oh My Pi user-question safeguards during tool callbacks.
- T3 Code and GOAT local session metadata support for private Antigravity ACP profiles, with stable/nightly home discovery and native session identities.
- Hook-supported agents no longer earn holds from startup or typing CPU activity. Explicit completion releases without an added three-second delay.
- Compact, taskbar-aware first-run placement and preserved user geometry.
- Neutral graphite dark mode with amber controls, plus a Modern Standby power-policy explanation in Settings.

## Installation and updating

Download `Ember_1.2.0_x64-setup.exe`, or use Settings > Updates in an installed copy. The update uses the existing signature key and endpoint. User preferences are retained.

After updating, repair existing integrations in Agents and reload their hosts. Codex may request its normal trust review. Configuration changes are backed up and merged.

## Detection and power limits

All seven integrations now offer lifecycle-hook paths on supported current hosts. Enable Hooks in Cline and reload; older or disabled Cline hosts retain approximate process fallback. Exact reasoning, question and interruption detection depends on host events. Live Antigravity ACP turn replay and physical lid/sleep testing are not claimed.

Ember prevents automatic idle sleep while Windows accepts its power request. It does not block explicit shutdown or safety actions. Windows may expire requests on battery-powered Modern Standby devices; plug in for extended work.

The installer has a verified updater signature. Windows Authenticode signing is separate.

See the [verification report](https://github.com/sanirudh17/Ember/blob/main/docs/RELIABILITY_FOLLOWUP.md) for coverage and limits.
