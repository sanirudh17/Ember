//! Current Cline Windows hooks: explicit .ps1 files, valid nonblocking JSON.
//! Refuse foreign hook files; never replace a user's policy script.
use super::*;
const HEADER: &str = "# Ember managed Cline hook. Remove or repair through Ember.";
pub(super) const BINDINGS: &[Binding] = &[
    b("TaskStart", HookEvent::Working, None),
    b("TaskResume", HookEvent::Working, None),
    b("UserPromptSubmit", HookEvent::Working, None),
    b("PreToolUse", HookEvent::Working, None),
    b("PostToolUse", HookEvent::Resume, None),
    b("TaskCancel", HookEvent::Idle, None),
    b("TaskComplete", HookEvent::Idle, None),
    b("TaskError", HookEvent::Idle, None),
    b("SessionShutdown", HookEvent::End, None),
    b("Notification", HookEvent::Heartbeat, None),
    b("PreCompact", HookEvent::Heartbeat, None),
];
pub(super) fn directory() -> Result<PathBuf, String> {
    Ok(dirs::document_dir()
        .unwrap_or(home()?.join("Documents"))
        .join("Cline")
        .join("Hooks"))
}
fn directories() -> Result<Vec<PathBuf>, String> {
    let documents = directory()?;
    let mut dirs = vec![documents.clone()];
    // SDK/CLI searches HOME/Documents and ~/.cline/hooks. VS Code resolves the
    // real Known Folder, which may be redirected to OneDrive.
    if documents.to_string_lossy().to_lowercase()
        != home()?
            .join("Documents/Cline/Hooks")
            .to_string_lossy()
            .to_lowercase()
    {
        dirs.push(home()?.join(".cline/hooks"));
    }
    Ok(dirs)
}
fn script(exe: &Path, event: HookEvent) -> String {
    let executable = exe.to_string_lossy().replace('\'', "''");
    format!(
        r#"{HEADER}
$ErrorActionPreference = 'Stop'
$helper = $null
try {{
  $raw = [Console]::In.ReadToEnd()
  $data = $raw | ConvertFrom-Json
  if (-not $data.cwd) {{ $data | Add-Member -NotePropertyName cwd -NotePropertyValue (Get-Location).Path -Force }}
  $info = New-Object System.Diagnostics.ProcessStartInfo
  $info.FileName = '{executable}'
  $info.Arguments = 'hook cline {event}'
  $info.UseShellExecute = $false
  $info.CreateNoWindow = $true
  $info.RedirectStandardInput = $true
  $info.RedirectStandardOutput = $true
  $info.RedirectStandardError = $true
  $helper = [System.Diagnostics.Process]::Start($info)
  $helper.StandardInput.Write(($data | ConvertTo-Json -Depth 32 -Compress))
  $helper.StandardInput.Close()
  if (-not $helper.WaitForExit(900)) {{ $helper.Kill() }}
}} catch {{ }} finally {{ if ($helper) {{ $helper.Dispose() }} }}
[Console]::Out.Write('{{"cancel":false}}')
exit 0
"#,
        event = event.as_str()
    )
}
fn files_at(dirs: &[PathBuf], exe: &Path, op: Op) -> Result<Vec<Rendered>, String> {
    let mut files = Vec::new();
    for dir in dirs {
        for binding in BINDINGS {
            let path = dir.join(format!("{}.ps1", binding.config_event));
            let (before, creates) = read_or_empty(&path)?;
            if !before.trim().is_empty() && !before.starts_with(HEADER) {
                return Err(format!(
                    "{} is a user-owned Cline hook; Ember will not replace it. Keep that policy and compose the integration manually.",
                    path.display()
                ));
            }
            let after = if op == Op::Remove {
                String::new()
            } else {
                script(exe, binding.ember_event)
            };
            files.push(Rendered {
                path,
                format: "powershell",
                before,
                after,
                entries: BTreeMap::new(),
                creates,
            });
        }
    }
    Ok(files)
}
fn aggregate(files: &[Rendered]) -> Rendered {
    let combine = |after: bool| {
        files
            .iter()
            .map(|f| {
                format!(
                    "# File: {}\n{}",
                    f.path.display(),
                    if after { &f.after } else { &f.before }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    Rendered {
        path: files[0].path.parent().unwrap().to_path_buf(),
        format: "powershell bundle",
        before: combine(false),
        after: combine(true),
        entries: BINDINGS
            .iter()
            .map(|b| (b.config_event.into(), b.ember_event.as_str().into()))
            .collect(),
        creates: files.iter().all(|f| f.creates),
    }
}
pub(super) fn render(op: Op, exe: &Path) -> Result<Rendered, String> {
    Ok(aggregate(&files_at(&directories()?, exe, op)?))
}
pub(super) fn status(exe: &Path) -> InstallStatus {
    let dirs = match directories() {
        Ok(d) => d,
        Err(e) => {
            return InstallStatus {
                installed: false,
                needs_repair: false,
                file: None,
                detail: e,
                last_event_unix: None,
            };
        }
    };
    let paths: Vec<_> = dirs
        .iter()
        .flat_map(|d| {
            BINDINGS
                .iter()
                .map(move |b| (d.join(format!("{}.ps1", b.config_event)), b.ember_event))
        })
        .collect();
    let installed = paths
        .iter()
        .any(|(p, _)| std::fs::read_to_string(p).is_ok_and(|s| s.starts_with(HEADER)));
    let stale = installed
        && paths.iter().any(|(p, e)| {
            std::fs::read_to_string(p).map_or(true, |s| s.replace("\r\n", "\n") != script(exe, *e))
        });
    InstallStatus{installed,needs_repair:stale,file:Some(dirs[0].display().to_string()),detail:if installed{"Windows hooks installed; enable Hooks in Cline and reload. Older or disabled hosts remain approximate."}else{"Global Windows PowerShell hooks available. Enable Hooks in Cline after installation."}.into(),last_event_unix:None}
}
fn write_files(files: &[Rendered]) -> Result<Option<String>, String> {
    // Validate every target before writing any file, then back up every original.
    let mut backup = None;
    for f in files {
        let saved = write_backup(&f.path, &f.before)?;
        if backup.is_none() {
            backup = saved;
        }
    }
    for (index, f) in files.iter().enumerate() {
        let result = if f.after.is_empty() {
            match std::fs::remove_file(&f.path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        } else {
            paths::ensure_parent(&f.path);
            crate::settings::write_atomic(&f.path, f.after.as_bytes())
        };
        if let Err(error) = result {
            let mut rollback_errors = Vec::new();
            for prior in files[..index].iter().rev() {
                // Do not undo an external edit that raced the installation.
                let current = std::fs::read_to_string(&prior.path).unwrap_or_default();
                if current != prior.after {
                    rollback_errors.push(format!("{} changed externally", prior.path.display()));
                    continue;
                }
                let restored = if prior.creates {
                    std::fs::remove_file(&prior.path).map_err(|e| e.to_string())
                } else {
                    crate::settings::write_atomic(&prior.path, prior.before.as_bytes())
                };
                if let Err(e) = restored {
                    rollback_errors.push(e);
                }
            }
            return Err(format!(
                "Cline hook bundle could not be written: {error}. Rollback issues: {}",
                if rollback_errors.is_empty() {
                    "none".into()
                } else {
                    rollback_errors.join("; ")
                }
            ));
        }
    }
    Ok(backup)
}
pub(super) fn apply(op: Op, exe: &Path) -> Result<InstallResult, String> {
    let files = files_at(&directories()?, exe, op)?;
    let rendered = aggregate(&files);
    let backup = write_files(&files)?;
    let mut manifest = load_manifest();
    if op == Op::Install {
        manifest.agents.insert(
            "cline".into(),
            InstalledRecord {
                file: rendered.path.display().to_string(),
                format: rendered.format.into(),
                entries: rendered.entries.clone(),
                installed_unix: timestamp(),
            },
        );
    } else {
        manifest.agents.remove("cline");
    }
    save_manifest(&manifest)?;
    let state = status(exe);
    Ok(InstallResult {
        ok: true,
        message: if op == Op::Install {
            "Installed Cline Windows hooks. Enable Hooks in Cline settings and reload."
        } else {
            "Removed Ember's Cline hooks."
        }
        .into(),
        file: state.file,
        backup,
        diff: diff_lines(&rendered.before, &rendered.after),
        installed: state.installed,
        needs_repair: state.needs_repair,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_bundle_write_failure_rolls_back_already_written_hooks() {
        let dir = std::env::temp_dir().join(format!("ember-cline-rollback-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("blocked.ps1")).unwrap();
        let item = |name: &str| Rendered {
            path: dir.join(name),
            format: "powershell",
            before: String::new(),
            after: HEADER.into(),
            entries: BTreeMap::new(),
            creates: true,
        };
        assert!(write_files(&[item("TaskStart.ps1"), item("blocked.ps1")]).is_err());
        assert!(!dir.join("TaskStart.ps1").exists());
        assert!(dir.join("blocked.ps1").is_dir());
        assert!(dir.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn windows_bundle_is_idempotent_reversible_and_never_replaces_foreign_policies() {
        let dir = std::env::temp_dir().join(format!("ember-cline-bundle-{}", std::process::id()));
        let roots = vec![dir.clone()];
        let exe = Path::new("C:/Ember/ember.exe");
        let files = files_at(&roots, exe, Op::Install).unwrap();
        assert_eq!(files.len(), 11);
        write_files(&files).unwrap();
        assert!(
            files_at(&roots, exe, Op::Install)
                .unwrap()
                .iter()
                .all(|f| f.before == f.after)
        );
        let removal = files_at(&roots, exe, Op::Remove).unwrap();
        write_files(&removal).unwrap();
        std::fs::write(dir.join("PreToolUse.ps1"), "# user security policy").unwrap();
        assert!(files_at(&roots, exe, Op::Install).is_err());
        assert!(files_at(&roots, exe, Op::Remove).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.join("PreToolUse.ps1")).unwrap(),
            "# user security policy"
        );
        assert!(dir.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn powershell_wrapper_always_returns_clines_nonblocking_json_contract() {
        let path =
            std::env::temp_dir().join(format!("ember-cline-script-{}.ps1", std::process::id()));
        std::fs::write(
            &path,
            script(Path::new("C:/missing-ember-helper.exe"), HookEvent::Working),
        )
        .unwrap();
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(&path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(br#"{"taskId":"contract","workspaceRoots":["C:/project"]}"#)
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(result.status.success());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap(),
            serde_json::json!({"cancel":false})
        );
        std::fs::remove_file(path).unwrap();
    }
}
