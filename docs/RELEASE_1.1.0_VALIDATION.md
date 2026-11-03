# Ember v1.1.0 validation

- Frontend TypeScript/Vite build passed.
- Rust library: 200 passed, zero failed. Includes actual generated integrations in a Node VM, per-agent matching, shared cancellation/wait/resume, concurrent sessions, process death, battery/manual/power/lid/watchdog logic.
- Release-mode Tauri build produced the signed NSIS installer and signature.
- Native packaged runtime confirmed v1.1.0, Archivo loaded, no browser mock, live engine, and 1100x700 normal geometry. Maximize/restore and minimize/show passed.
- Native helper/pipe/registry lifecycle sequences passed for all seven agent IDs, including waiting/resume and late callbacks after cancellation/end.
- Native manual holds passed: 30m, 1h, 2h, until-time, unlimited, and stop. Assertions observe committed snapshots rather than synchronous command returns.
- Browser interaction smoke passed centered labels, sidebar routing, settings toggle and manual start/end. Three routes in both themes at 760x520, 860x620, and 1920x1080 plus active/manual/blocked/off states reported no horizontal overflow or page errors.
- Signature verification uses the exact public key shipped in v1.0.0; installer signature and trusted comment pass, altered bytes fail. Update endpoint is unchanged. latest.json is BOM-free with windows-x86_64 and an HTTPS versioned installer URL; SHA256SUMS.txt records payload/manifest/signature hashes.
- Taste lint and Impeccable detection: no findings on changed UI source.

The agent-specific coverage and remaining upstream limits are in AGENT_DETECTION_AUDIT.md. Physical firmware sleep/lid behavior and seven authenticated model-host sessions were not exercised. Public download and v1.0.0 discovery verification is recorded after publication.
