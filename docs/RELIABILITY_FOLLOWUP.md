# Detection and desktop follow-up — 2026-11-12

## Changes

- First-run normal size is 1060 × 680 logical pixels. Placement uses the selected monitor's Windows work area, not its full bounds, with a 24 logical pixel clearance and a slightly raised center. Normal user geometry is restored when it fits; disconnected monitors and oversized saved rectangles are clamped. Maximization remains a separate saved state.
- Following the user's latest palette correction, the dark theme uses neutral graphite: chrome #171717, canvas #111213, surface #1b1c1e, warm white text and neutral secondary text. Amber controls remain. The accepted layout, Archivo font, light theme, manual controls and session rows remain intact.
- Process discovery no longer has two ten-second settling gates. Exact open processes appear on the first sample; subsequent process sampling is once per second.
- The first IPC snapshot is populated during engine initialization, before the background loop's first tick, so the opening view does not briefly show an empty default registry.
- CPU never starts a turn for the six hook-supported agents. Opening a GUI, typing, indexing, and resident app-server/ACP activity do not prove work. Unhooked Cline retains an explicitly approximate process fallback. Current Cline Windows hooks were added during the final source audit.
- Hook helpers persist only the last normalized phase, agent/session identity, PID creation time and folder. On reopening, Ember restores records only if that exact process still exists. An old working record cannot attach to a reused PID. Idle/end and guarded resume/heartbeat semantics apply equally while Ember is closed.
- Explicit idle/end now releases the power request on the next 250 ms engine tick, without an additional three-second release floor. The one-second visible-engagement filter and notification debounce remain.
- OpenCode execution ownership also applies when v2 events arrive through the v1-compatible factory. Intermediate idle/error-looking step events cannot end a v2 execution. Reasoning, retries, tool calls and background waits remain part of the enclosing turn. Questions pause it, and late replies cannot undo interruption.
- OpenCode's v2 event subscriber reconnects after stream closure. A transport failure no longer ends every session. Reconciliation against host busy/idle timestamps catches missed terminal events; it does not infer completion from quiet output.
- T3 Code / GOAT integration reads local session projections using SQLite read-only connections. Their isolated Antigravity profiles bypass normal user CLI hooks. The adapter supports the legacy session/runtime tables and newer orchestration-v2 projections. It validates the live server PID and creation time, excludes records from a previous server lifetime, distinguishes background waits from unresolved user requests, and retires closed/dead sessions. It queries no message text, tool arguments, credentials or tokens.

## Upstream source review

The implementation follows lifecycle ownership, rather than adopting CPU activity as an equivalent signal. Orca can use its own runtime's agent status because it owns that runtime; an external utility cannot reproduce unavailable events by watching CPU.

- [T3 Antigravity driver](https://github.com/pingdotgg/t3code/blob/main/apps/server/src/provider/Drivers/AntigravityDriver.ts), [Antigravity ACP support](https://github.com/pingdotgg/t3code/blob/main/apps/server/src/provider/Drivers/AntigravityAcpSupport.ts), [server runtime identity](https://github.com/pingdotgg/t3code/blob/main/apps/server/src/serverRuntimeState.ts), and [orchestration contracts](https://github.com/pingdotgg/t3code/blob/main/packages/contracts/src/orchestrationV2.ts).
- [GOAT source](https://github.com/goat-si/goat/tree/dev): Antigravity driver/profile paths, private GEMINI_HOME, server config and session metadata were compared with T3.
- [OpenCode execution lifecycle](https://github.com/anomalyco/opencode/blob/dev/packages/core/src/session/execution.ts), [plugin host](https://github.com/anomalyco/opencode/blob/dev/packages/core/src/plugin/host.ts), [session timestamps](https://github.com/anomalyco/opencode/blob/dev/packages/schema/src/session.ts), and [session protocol](https://github.com/anomalyco/opencode/blob/dev/packages/protocol/src/groups/session.ts).
- [Orca source](https://github.com/stablyai/orca): agent-awake service/status leases and OpenCode-family runtime events were reviewed.

## Verification boundaries

Unit tests exercise both harness schemas, active/background/user-wait/terminal transitions, runtime PID identity, stale cache rejection, restart recovery and taskbar-aware placement. Actual generated OpenCode/Pi/OMP JavaScript runs in a Node VM. The OpenCode fixture includes an hour of virtual silent reasoning, v2 lifecycle events through both adapters, stream failure without release, reconnect, missed-terminal reconciliation and late-callback rejection. Virtual time is not a live hour-long model run.

The user's reported 38-minute failure was not reproduced with a live model session. Stream teardown and mixed lifecycle handling are concrete defects addressed here; the available rotated logs do not establish which caused that exact occurrence.

Initially neither harness was installed. GOAT Nightly subsequently became available during this task. Its actual local OpenCode session metadata was read successfully using the adapter query. This also exposed its channel-specific `.goat/desktop-nightly/userdata` location; packaged stable uses `.goat/desktop/userdata`. Both are now discovered, along with custom GOAT_HOME paths. Native provider-thread references merge with hook session IDs, and separate threads sharing one provider server retain separate states.

No live Antigravity ACP model session was run. Antigravity lifecycle coverage is source/fixture verified, not an authenticated harness compatibility certification. Unsupported schema changes fail closed rather than falling back to transport CPU. Unhooked Cline and hosts that omit cancellation/user-wait events still cannot provide exact reasoning/Escape classification.

Changed integration files need Repair and host reload/restart. Normal host trust checks remain in force. No hook trust database is rewritten. Battery guards and reversible lid protection remain unchanged.

## Final local results

- Final release test counts and native closure/lid results are recorded in RELEASE_1.2.0_VALIDATION.md.
- TypeScript/Vite and standalone native debug builds passed.
- Browser captures covered all three routes in both themes at 760 × 520, 1060 × 680 and 1920 × 1080, plus active/manual/blocked/off states: no page errors or horizontal document overflow.
- Actual native hook helper → Windows pipe → registry checks passed working/waiting/resume/idle/end and late-callback rejection for all seven agent IDs. These synthetic messages establish Ember's transport behavior, not every host's event emission.
- Native reset/maximize/restore returned 1060 × 680 logical pixels. The latest palette was checked in the native view at the user's adjusted 1106 × 680 size: Archivo, dark chrome #171717 and canvas #111213, with no horizontal overflow. User-adjusted geometry is retained.
- Actual closed-app hook persistence and restart passed: the first IPC snapshot contained the recovered work without another host event. Idle was observed in 190 ms including launching the helper in this run; this is an observed sample, not a timing guarantee.
- GOAT Nightly's real local OpenCode thread was detected and later appeared idle as its metadata changed. Hook/harness session keys had no duplicates in the final native snapshot. No user task was started or interrupted for this check.
- Existing installed integrations needing repair were backed up and repaired through Ember's own commands. Host reload/restart and normal trust review still apply.
- The development checkpoint left the native app open against Vite; packaged release validation follows.

## Recovery coverage across all seven integrations

| Integration | Recovery mechanism | Limit |
| --- | --- | --- |
| Claude Code | Each hook invocation retries pipe delivery; normalized state survives Ember restart with live PID creation-time validation | Depends on the host emitting its hooks; no invented universal Escape hook |
| Codex | Same shared hook recovery; Stop/Interrupt/End and guarded late callbacks | Host configuration/trust must allow the hooks |
| OpenCode | Shared persistence plus reconnecting v2 subscription, busy/idle timestamp reconciliation and execution ownership | Reconnect applies to its event-stream adapter; v1 uses direct callbacks |
| Pi | In-process extension callbacks, active-turn heartbeat and shared persistence | Requires the extension to be loaded |
| Oh My Pi | In-process extension callbacks, active-turn heartbeat and shared persistence | Requires the extension to be loaded |
| Antigravity | Shared hook recovery; supported T3/GOAT projections provide an additional local ACP state source | Live Antigravity ACP turn replay remains untested |
| Cline | Global Windows .ps1 hooks for task, prompt, tool, cancellation, completion, user-wait and shutdown; shared persistence | Current host hook support and enablement required; older or disabled hosts retain approximate CPU fallback |

An additional executed JavaScript regression covers pending user input being followed by tool-start callbacks in Pi/OMP. Tool startup and unrelated callbacks now leave the user wait intact. A shared on-disk restart/wait/resume/cancel/end regression runs for all seven agent IDs; The final audit then added current Cline global Windows hook support and its own executed PowerShell contract tests.


## Cline source revalidation during release audit

The earlier process-only assumption was superseded by current upstream Windows support. Cline now discovers explicit `<HookName>.ps1` scripts and global hook directories. Ember offers that integration, using Windows Known Folder Documents for VS Code and an SDK-compatible directory when Documents is redirected. It refuses foreign policy scripts, previews the managed bundle, backs up originals and rolls back already-written files on a bundle failure. Hooks always return non-blocking JSON. Task IDs and workspace roots are normalized; ordinary tool results cannot clear a pending question; explicit cancellation/completion wins over late callbacks. Enable Hooks in Cline settings where required and reload the host.

Sources rechecked: [Windows hook factory](https://github.com/cline/cline/blob/main/apps/vscode/src/core/hooks/hook-factory.ts), [Windows Documents resolution](https://github.com/cline/cline/blob/main/apps/vscode/src/core/storage/documents-path.ts), [SDK hook filenames](https://github.com/cline/cline/blob/main/sdk/packages/core/src/hooks/hook-file-config.ts), [SDK search paths](https://github.com/cline/cline/blob/main/sdk/packages/shared/src/storage/paths.ts), and [CLI lifecycle dispatch](https://github.com/cline/cline/blob/main/apps/cli/src/utils/hooks.ts).

Windows power limits: [PowerSetRequest](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-powersetrequest) documents automatic-idle sleep prevention, user-initiated sleep and the Modern Standby DC timeout. Settings now explains the battery limitation. [CallNtPowerInformation](https://learn.microsoft.com/en-us/windows/win32/api/powerbase/nf-powerbase-callntpowerinformation) is used by the native integration proof to read system-required activity. No physical lid closure, shutdown or forced sleep is performed.
