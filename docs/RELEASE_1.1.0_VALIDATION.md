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

## Published artifacts and actual update

Release: https://github.com/sanirudh17/Ember/releases/tag/v1.1.0

Release source is commit 50e6309 on main. All four public assets were downloaded over their public HTTPS URLs and matched local bytes:

| Asset | SHA-256 |
| --- | --- |
| Ember_1.1.0_x64-setup.exe | e3e1307430a0b998d35dcb0471938e0596309968f4d84f8c2dfbd85f14a1e21e |
| Ember_1.1.0_x64-setup.exe.sig | f6c13432420bb1f8b45308158173a3dd0edec5d181170ca7a2ae9cfe40cf8b64 |
| latest.json | b8c2961361e4904db906ce264e98fd770d30037cb80cd0e67334a4fe2521bec7 |
| SHA256SUMS.txt | d093230ffbdce8c0bbf355bd060abf16c16d3f30a38e190ae5ac1b2bf8955fc1 |

The public releases/latest/download/latest.json endpoint returned 1.1.0. Extracted the actual published v1.0.0 executable without installing it, launched it, and called its own check_update command: it reported current_version 1.0.0 and version 1.1.0. Then invoked its normal install_update path. The signed installer completed and reopened Ember from C:/Users/sanir/AppData/Local/Ember/ember.exe as v1.1.0. The installed executable exactly matches the payload extracted from the verified v1.1.0 installer: SHA-256 616e6aaf186ec3825fb139dacfa5e8ce8602b6ee1cc3905cf367bce05c1f2321. User settings, excluding remembered window geometry, remained identical to the pre-update backup.

GitHub's implicit tag creation also triggered the existing CI release workflow. Run 41632803656 was cancelled before asset replacement, preserving the exact locally tested/signed downloads. The release remains public, latest, non-draft, and non-prerelease with four assets.
