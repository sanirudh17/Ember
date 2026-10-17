# Releasing Ember

Ember ships as a signed NSIS installer on GitHub Releases. Installed copies
poll `https://github.com/sanirudh17/Ember/releases/latest/download/latest.json`
and update themselves in place: the app downloads the newer build, verifies
its signature against the public key baked into `tauri.conf.json`, runs the
installer and restarts.

## One-time setup

1. **Repository.** Push this tree to GitHub. The updater endpoint in
   `src-tauri/tauri.conf.json` hardcodes `sanirudh17/Ember`; if the repository
   lives elsewhere, change that one URL and rebuild.
2. **Signing secrets.** In the repository: Settings → Secrets and variables →
   Actions → New repository secret:
   - `TAURI_SIGNING_PRIVATE_KEY` — the contents of
     `%USERPROFILE%\.tauri\ember-updater.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the password from
     `%USERPROFILE%\.tauri\ember-updater.key.password.txt`
   The private key never goes into the repository; it lives on this machine
   and in those two secrets.
3. **Version.** Bump `version` in `src-tauri/tauri.conf.json`, in
   `src-tauri/Cargo.toml` and in `package.json` before tagging.

## Publishing a release

```powershell
git tag v1.0.0
git push origin v1.0.0
```

The Release workflow builds on `windows-latest`, signs the build with the
repository secret, and opens a **draft** release containing:

- `Ember_<version>_x64-setup.exe` — the installer for new users; it doubles
  as the payload installed copies download when they update
- `Ember_<version>_x64-setup.exe.sig` — the signature the updater verifies
  before running the installer
- `latest.json` — the manifest installed copies poll

Review the draft, then publish it. The moment it is published,
`releases/latest/download/latest.json` starts answering with the new version
and every installed Ember notices within four hours — or immediately through
Settings → Updates → Check now. The top bar shows a small Update pill while
one is waiting.

## Local dry run

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
```

This performs the same build locally with the key file, and verifies that the
installer and its update signature exist under
`src-tauri\target\release\bundle\nsis\`. `latest.json` is produced by CI —
local builds only need the signed installer.

## Notes

- Drafts do not serve `latest.json`: installed copies keep seeing the
  previous release until the draft is published.
- Losing the private key means installed copies can no longer verify new
  updates; keep a backup of `ember-updater.key` somewhere safe.
- Code signing (Authenticode) is separate from update signing: Windows
  SmartScreen may still warn on a fresh installer until the publisher
  certificate is set up in the bundle configuration.
