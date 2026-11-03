# Agent detection audit, 2026-11-02

## Evidence and scope

193 Rust tests passed. Generated integration code is evaluated in a Node VM with a stub hook helper and event sources, so these tests exercise real emitted JavaScript rather than checking only strings. Each supported agent also has executable matching and engine lifecycle coverage. The power API is mocked in unit tests: this establishes decision and request ownership behavior, not physical sleep prevention on every Windows firmware.

| Agent | Reviewed integration | Verification and changes | Remaining limit |
| --- | --- | --- | --- |
| Claude Code | SessionStart, UserPromptSubmit, Pre/PostToolUse, idle notification, Stop, SessionEnd | Existing schema/merge/removal and payload tests; shared engine start/work/idle/release test | No authenticated live turn run; Stop may be followed by another extension's continuation |
| Codex | TOML lifecycle hooks, including Interrupt | Existing commandWindows, launcher, trust/schema and protocol tests; shared engine lifecycle | Existing hooks need Repair; Codex may require trust review; no authenticated live turn run |
| OpenCode | Separate v1 factory and v2 setup contracts | Executed v1 busy/retry/idle/delete and v2 execution start/succeed/interrupt/delete; v2 idle step does not release; running state re-announced after restart | No live v2 session replay in this run |
| Pi | agent_settled is final; agent_end is not | Removed premature agent_end idle mapping; executed start/work/settled/end; added 30-second running-state refresh; idle never refreshes | Older runtimes lacking agent_settled are not verified; Pi not found on PATH |
| Oh My Pi | agent_end with willContinue guard | Removed unsupported agent_settled binding; executed continuation and final completion; added running-state refresh | No authenticated live turn run |
| Antigravity / ACP | Named hooks, exact agy and agy_acp_server executables | Existing schema/payload tests; ACP matching regression; shared engine lifecycle | GUI host must actually load user hooks for exact timing; CPU fallback remains approximate |
| Cline | Existing process fallback | Exact executable/runtime entrypoint tests and shared engine request lifecycle | Cline not found on PATH; extension inside shared Code.exe is intentionally not classified; no new hook support claimed |

## Cross-agent fixes

Runtime matching inspects the program argument, not arbitrary later prompt text. A script receiving an argument such as codex.js cannot become a Codex session. PID payload conversion rejects values outside u32. Same-process CPU estimates cannot resurrect a session after an authoritative hook End. Process folder enrichment still works for hook sessions. Existing startup settling, hysteresis, PID reuse checks, battery guards, and manual holds remain in place.

Process classification uses CPU, not a protocol turn boundary. Sustained typing, updates, and background tools can look busy; network-only thinking can look idle. A perfect CPU-only classifier is not claimed. Hooks/plugin lifecycle signals are the preferred source.

## Sources checked

- Claude: https://code.claude.com/docs/en/hooks
- Pi lifecycle: https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/extensions.md
- OMP typed events: https://github.com/can1357/oh-my-pi/blob/main/packages/coding-agent/src/extensibility/shared-events.ts
- OpenCode plugin docs: https://opencode.ai/docs/plugins/
- Antigravity hook schema: https://antigravity.google/docs/hooks
- Cline documentation entry: https://docs.cline.bot/customization/hooks (content unavailable to this audit; no new compatibility claim)

## UI verification

18 captures across Awake, Agents and Settings in dark/light at 760x520, 920x640 and 1600x900: no page errors or horizontal document overflow. Additional browser smoke test passed sidebar routing, settings toggle, theme choice, manual start/end, label-box vertical centering within 1px, and absent active-tab shadow/underline. Native debug and TypeScript/Vite builds passed. Impeccable detector returned no findings; Taste source lint reported style warnings, including shadow:none declarations.

No physical lid-close or unattended suspend/resume test, full security audit, installer build, or public release was performed. Existing hook/plugin installations need Repair and their agent host restarted/reloaded to receive these changed bindings.


## Cancellation follow-up

The installed Codex configuration lacked Interrupt. It now contains Interrupt mapped to idle, with its normal backup retained; the launcher points at the current local binary. New/changed Codex hooks must be trusted by Codex and existing agent hosts must reload/restart to consume changed configuration. Hook trust records were not rewritten by this work.

Tool/invocation completion is now a heartbeat for Claude Code, Codex and Antigravity so a late completion cannot resurrect an explicitly idled turn. Codex repair checks required event bindings rather than only counting commands. Claude StopFailure handles terminal API errors. OpenCode v1 handles MessageAbortedError in addition to idle events; v2 execution.interrupted remains terminal. Pi aborted agent_settled and OMP terminal agent_end remain covered by executed generated-plugin contracts.

196 Rust tests passed. The native standalone app also passed synthetic hook-helper -> named-pipe -> registry work/idle/late-heartbeat checks for all seven agent IDs. This tests Ember's real transport and registry, not the upstream hosts actually emitting Escape events. Claude's documented hooks do not provide a universal Escape event, and process-only Cline cannot infer an exact Escape boundary; no global keyboard interception or universal instantaneous cancellation guarantee is claimed. Physical lid/sleep and authenticated model-host turn replay remain untested.

References: https://developers.openai.com/de-DE/docs/hooks and https://code.claude.com/docs/en/hooks .

## v1.1.0 release audit - 2026-11-03

200 Rust tests pass, including executed generated JavaScript. The release executable passed real hook CLI -> named pipe -> registry work/wait/resume/idle/end sequences for all seven supported IDs. Late heartbeat/resume events cannot recreate ended sessions, a cancelled wait cannot resume, and a second session keeps working when its sibling waits or closes. These are native transport tests and synthetic host sequences, not seven authenticated live model runs.

| Integration | Active work and final boundary | Waiting, cancellation, and limits |
| --- | --- | --- |
| Claude Code | Prompt and tool entry hold; ordinary tool completion is guarded resume; compact SessionStart preserves a run; Stop/StopFailure/SessionEnd release | AskUserQuestion/ExitPlanMode are waiting tools; MCP elicitation has waiting/resume hooks. No universal documented Escape hook or precise post-approval tool-entry signal is fabricated. |
| Codex | Prompt and tool entry hold; PostToolUse resumes explicit waits only; Stop/Interrupt/SessionEnd release | Explicit question-tool payloads suspend work when host hooks expose them. Changed bindings need normal trust review. |
| OpenCode v1 | Busy/retry and tool entry; session.idle/status idle/error are terminal | Permission/question requests suppress busy/timers until matching replies; ordinary tool completion never idles. |
| OpenCode v2 | execution.started to execution.succeeded/failed/interrupted; reasoning/tool progress and 30-second refresh retain the run | Forms (including nested form.sessionID), permissions and questions track pending IDs. One reply cannot clear another pending request. Mid-step idle cannot terminate a run; late progress/replies cannot undo terminal completion. |
| Pi | agent_settled is final, including aborted turns; agent_end remains nonterminal | ui_prompt_start/end suspend/resume only active runs; late UI closure cannot undo agent_settled. |
| Oh My Pi | agent_end respects willContinue; working-state refresh protects silent reasoning | Known question tools suspend until their own tool result, not an unrelated result. Generic third-party extension UI without lifecycle events remains a host limitation. |
| Antigravity | PreInvocation through Stop covers model/tool loop; PostInvocation/PostToolUse only refresh existing work | agy_acp_server remains discoverable but background transport CPU does not hold awake. GUI clients must load user hooks for exact turns. No unsupported permission-decision hooks are installed. |
| Cline | Exact executable and runtime entrypoint matching with bounded CPU hysteresis | Process-only estimate. Exact reasoning, user questions, and Escape require host lifecycle support; no perfect CPU-only detection is claimed. |

Shared fixes: ignore heartbeat/resume for unknown sessions; suppress CPU after hook ownership; no invisible hold after dead-PID cleanup; raise the CPU noise floor to 3; exclude resident Codex app-server and OpenCode serve from CPU-derived work; require session events for ACP transport activity. Hooks remain authoritative, with a three-second engine release floor and short process sampling/grace windows.

References reviewed (original implementation, no third-party source copied):
- Orca status normalization: https://github.com/stablyai/orca/blob/main/src/shared/agent-hook-listener/providers/opencode-family-events.ts
- Orca power service: https://github.com/stablyai/orca/blob/main/src/main/power/agent-awake-service.ts
- Claude Code Agent Monitor lifecycle model: https://github.com/hoangsonww/Claude-Code-Agent-Monitor/blob/master/docs/HOOKS.md
- OpenCode v2 event schemas: https://dev.opencode.ai/v2/openapi.json
- OpenCode v1 plugins: https://opencode.ai/docs/plugins/
- Pi UI spans and final settlement: https://pi.dev/docs/latest/extensions
- OMP shared events: https://github.com/can1357/oh-my-pi/blob/main/packages/coding-agent/src/extensibility/shared-events.ts
- Claude hooks: https://code.claude.com/docs/en/hooks
- Codex hooks: https://developers.openai.com/codex/hooks
- Antigravity hooks: https://antigravity.google/docs/hooks

Physical lid-close/suspend/resume and every authenticated upstream host are not tested. Battery/lid/watchdog/request ownership have unit coverage, including restoration safeguards. Existing integrations need Repair and host restart/reload; Codex trust is not bypassed.
