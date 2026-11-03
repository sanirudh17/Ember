# Desktop and detection fixes

## Changes
- Recognize exact agy_acp_server executable names without matching arbitrary ACP servers.
- Start/Idle/End hooks no longer earn a 90-second live-process grace. Explicit stop releases after the engine's 3-second quiet floor; the overlay follows the same snapshot. A real OS hold still begins before the 1-second visible-state gate.
- Inferred CPU activity uses an 8-unit energy budget (4-unit sample cap), a 4-second quiet hold, and at most 4 seconds of extra inferred grace. Process polling is every 2 seconds. New processes still settle for 10 seconds. This is heuristic, not an instant or exact turn signal.
- Same-agent PID reuse resets classifier history. Replayed stale End events are validated before they can remove a live session.
- Codex Interrupt maps to idle. Existing installations need Repair hooks, and changed Codex hook definitions may require trust review by Codex.
- Toast delivery follows every visible state transition independently of the 60-second sound debounce.
- Failed taskbar overlay updates retry instead of caching failure.
- Failed lid rollback preserves the original-values recovery record for retry.
- Opaque startup, hidden until first themed paint, 920x640 default, preserved maximize/geometry restore. Native Windows manages maximize/restore animations.
- Compact framed desktop layout, themed thin scrollbars, centered button labels, no redundant footer, supplied artwork throughout.

## Verification
188 Rust tests passed, including new ACP, session-start, stale-End and failed-rollback regressions. Existing tests cover manual timer/until/unlimited, battery vetoes, pause/resume, power request continuity, lid cap and restoration, hook installers and protocol validation. Frontend TypeScript/Vite production build passed. Native debug build passed. Browser review: 18 captures, three routes, three desktop sizes, both themes; no page errors or horizontal document overflow. Impeccable detector returned no findings. Taste inspection reported no failures and one system-font fallback warning (Segoe UI is the intentional Windows fallback); source lint reported three style warnings.

## Limits
No physical lid-close, suspend/resume, toast delivery, taskbar repaint, or native minimize/maximize animation test was performed. Unit power tests use a fake API and do not establish firmware behavior. Live turn replay has not been performed against every installed agent or ACP GUI host. CPU-only detection can miss network-only thinking and mistake sustained input activity for work. Hooks are preferred for exact lifecycle state. No installer or public release was produced.

## Orca reference
Reviewed https://github.com/stablyai/orca/blob/main/src/main/agent-awake-service.ts . Orca derives automatic holds from eligible running-session statuses via a freshness lease and reconciles the underlying blocker. Ember retains its independent Windows power requests and separate display setting. See also official Codex lifecycle event documentation: https://developers.openai.com/de-DE/docs/hooks .
