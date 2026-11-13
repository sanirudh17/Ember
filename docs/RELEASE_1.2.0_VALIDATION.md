# Ember v1.2.0 validation

## Verified before publication

- 214 Rust unit tests passed, including actual generated OpenCode/Pi/OMP JavaScript, Cline payload handling, executed PowerShell output, bundle rollback, hook persistence, process identities, concurrent sessions, battery/manual/lid/watchdog guards and window placement.
- Clippy passed for all targets with warnings denied. TypeScript/Vite and the signed release-mode NSIS build passed.
- A production-library Windows integration test successfully used real SystemRequired and DisplayRequired requests, retained the request through 60 reason changes, queried SystemExecutionState and released/reacquired its owned request. Unit tests do not substitute mocks for this integration test.
- The opt-in native lid test passed separately: real AC/DC lid settings were written/read back as Do nothing, restored normally, then restored by the actual watchdog after its owner process exited. The originals on this device were AC=0/DC=0 and remained unchanged. No physical lid closure, sleep or shutdown was triggered.
- Packaged v1.2.0 exposed hook integration support for all seven agents. Real helper -> Windows pipe -> registry work/wait/resume/idle/end and late-callback rejection passed for all seven IDs.
- The installed Cline .ps1 files passed real PowerShell -> helper -> pipe sequences for task start, tools, user questions, answer, notification wait, cancellation, resume, completion and shutdown. Each hook returned non-blocking cancel:false JSON. Existing foreign policies are refused; simulated bundle write failure rolled back preceding files.
- Packaged manual holds passed 30 minutes, one hour, two hours, until-time and unlimited modes, including real power engagement and stop.
- Packaged close behavior passed Keep running, Escape, hidden-window preservation and confirmed Quit. On battery, Require charger prevented the lid override; the controlled battery-allowed test engaged it. Quit restored the real lid policy, removed the undo record and terminated Ember. User preferences were restored after testing.
- The packaged app loaded Archivo and had no horizontal document overflow. The approved graphite theme and user-adjusted normal geometry were retained.
- Local installer signature, trusted comment and tamper rejection passed. Updater keys/endpoints match both v1.0.0 and v1.1.0. The manifest is UTF-8 without BOM and SHA256SUMS.txt covers the installer, signature and manifest.

These checks total 216 passing Rust tests when the opt-in native lid test is included, plus the packaged application scripts. The native lid test is intentionally ignored in unattended default test runs because it briefly changes the user's lid policy.

## Scope and limits

The source review includes Orca's status ownership/leases, T3 and GOAT's private Antigravity profiles and local projections, OpenCode's execution/stream contracts, and current Cline Windows/SDK hook discovery. Links and the per-agent recovery matrix are in [RELIABILITY_FOLLOWUP.md](RELIABILITY_FOLLOWUP.md).

All seven now offer lifecycle-hook paths on compatible current hosts. Hooks must be enabled, loaded and trusted by their host; older or disabled Cline hosts retain approximate CPU fallback. Exactness is limited by events the host actually emits. These tests are not seven authenticated live model sessions or a physical firmware/lid certification. Windows can expire battery-powered Modern Standby requests and can honor explicit sleep/shutdown and safety actions; Ember does not bypass those policies.

## Public release verification

The public v1.2.0 tag points to `0372b317796fc2169a2a824c68ff1e097df75c9a`. All four anonymously downloaded release assets matched the local artifacts byte-for-byte, and the latest updater endpoint returned v1.2.0.

| Asset | SHA-256 |
| --- | --- |
| Ember_1.2.0_x64-setup.exe | `0290ca613f0345be6653139af34d78c9c62b778be81b8894406325aa486f1a58` |
| Ember_1.2.0_x64-setup.exe.sig | `b606ac72e7c5e7ea22c95b49dd15a7f9d84b4c4ad07055f1a2a7b95ae0cad3bd` |
| latest.json | `38759eea9877adbdd18c57b52ec2d8d08ff1712deb4c347074e2bfcf7e69db17` |
| SHA256SUMS.txt | `56dbd64cd30d2a9bd98687bf701ac4ca0d543b1b0c38449e8ec28b9fe459a1ec` |

The tag-triggered CI packaging run was cancelled so it cannot replace the locally tested, signed artifacts.

## Installed upgrade and configuration verification

The preserved public v1.1.0 executable discovered v1.2.0 through its existing endpoint and completed its own signed update. The old runtime exited, the installed v1.2.0 relaunched, the bundled frontend loaded without Vite, and user preferences matched before/after excluding window geometry.

The installed executable SHA-256 is `179d05de8e42057076359f7d8b2eff3daeb7a1336fa1b63b64725b9736e54217`. Tauri temporarily patches the standalone executable's three-byte bundle marker from `UNK` to `NSS` while creating NSIS and then restores it. Comparison against that exact patched payload passed; those three bytes are the only difference from the tested standalone executable.

Using the installed application's normal backup-and-merge installer, Claude Code, Codex, OpenCode, Oh My Pi, Antigravity and Cline configurations were installed/repaired and read back without a repair warning. Their helper paths now use the permanent installed runtime. Pi is absent on this device; its integration is available and covered by the generated-extension and shared lifecycle tests, not a live installed-host check. Hosts must reload/trust their changed integrations; Cline's normal Hooks enablement remains host-controlled.
