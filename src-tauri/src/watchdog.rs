//! Crash-recovery watchdog.
//!
//! Layer 2 of the lid-proof recovery story: when Ember changes the lid-close
//! action it also spawns
//!
//! ```text
//! Ember.exe --restore-watchdog <ember_pid> <session-state.json>
//! ```
//!
//! detached. That process blocks on the Ember PID and, however Ember dies —
//! clean exit, crash, Task Manager kill — the moment the PID disappears it puts
//! the user's original lid action back and deletes the state file.
//!
//! Layer 1 is the state file being written *before* the change; layer 3 is the
//! restore performed on every normal exit path inside the app.
//!
//! Two rules keep the layers from fighting each other:
//!
//!   * the watchdog only touches a state file whose `owner_pid` is the Ember
//!     instance it was spawned for — after a crash-and-restart the file
//!     belongs to the new instance, which has its own watchdog;
//!   * a failed restore keeps the file (see [`crate::lid::restore_at`]) so
//!     the next launch can retry instead of silently losing the user's lid
//!     action.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};

use crate::lid::{self, LidHold, PowerApi, RealPowerApi};

pub const WATCHDOG_FLAG: &str = "--restore-watchdog";

const INFINITE: u32 = 0xFFFF_FFFF;
const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `argv[1] == --restore-watchdog`, then the parent pid and the state file.
pub fn parse_args(args: &[String]) -> Option<(u32, String)> {
    if args.len() != 4 || args[1] != WATCHDOG_FLAG {
        return None;
    }
    let pid = args[2].parse::<u32>().ok()?;
    Some((pid, args[3].clone()))
}

/// The watchdog body. Returns the process exit code (always 0).
pub fn run(parent_pid: u32, state_file: &str) -> i32 {
    wait_for_exit(parent_pid);
    handle_exit(&RealPowerApi, parent_pid, state_file);
    0
}

/// Restore the lid action the dying instance had overridden — but only when
/// the state file still belongs to that instance. A fast crash-restart means
/// a newer Ember already wrote its own record (and spawned its own watchdog);
/// undoing *their* hold would change the lid action mid-session and delete
/// the record that guard still needs. A failed restore keeps the file so the
/// next launch can retry. Either outcome is written to the log, because a
/// silently dropped restore is exactly the failure this layer exists to rule
/// out.
fn handle_exit(api: &dyn PowerApi, parent_pid: u32, state_file: &str) {
    let path = Path::new(state_file);
    let Some(state) = lid::read_state_at(path) else {
        return; // a clean shutdown already restored and deleted it
    };
    if state.owner_pid != parent_pid {
        // The record belongs to a newer instance that is still running; its
        // own watchdog (or its exit path) owns it.
        return;
    }
    let hold = LidHold::from_state(&state);
    match lid::restore_at(api, &hold, path) {
        Ok(()) => crate::log::append("LID", "watchdog put the lid action back after Ember died"),
        Err(e) => crate::log::append(
            "LID",
            &format!("watchdog could not restore the lid action ({e}); the record stays for the next launch"),
        ),
    }
}

/// Block until the parent process is gone.
///
/// OpenProcess failing with ERROR_INVALID_PARAMETER is the reliable "the PID
/// no longer exists" signal. Anything else (a transient loader lock, an
/// access window) is retried before giving up: bailing out on the first
/// failure could restore the lid action while the parent is still alive and
/// holding it.
fn wait_for_exit(parent_pid: u32) {
    // Win32 error 87, surfaced as HRESULT 0x80070057.
    const ERROR_INVALID_PARAMETER: u32 = 0x8007_0057;
    const TRANSIENT_RETRIES: u32 = 25; // ~5s of retries

    for attempt in 0..=TRANSIENT_RETRIES {
        match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, parent_pid) } {
            Ok(handle) => {
                let _ = unsafe { WaitForSingleObject(handle, INFINITE) };
                let _ = unsafe { CloseHandle(handle) };
                return;
            }
            Err(e) if e.code().0 as u32 == ERROR_INVALID_PARAMETER => return,
            Err(_) if attempt < TRANSIENT_RETRIES => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(_) => return,
        }
    }
}

/// Spawn the watchdog fully detached so it survives Ember and owns no console.
/// Returns `true` when the process was actually launched.
pub fn spawn_detached(state_file: &Path) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let pid = std::process::id().to_string();
    let state = state_file.display().to_string();

    use std::os::windows::process::CommandExt;
    Command::new(exe)
        .args([WATCHDOG_FLAG, pid.as_str(), state.as_str()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
        .spawn()
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn watchdog_arguments_parse() {
        let parsed = parse_args(&args(&[
            "Ember.exe",
            "--restore-watchdog",
            "4242",
            r"C:\Users\me\AppData\Local\Ember\session-state.json",
        ]))
        .expect("parses");
        assert_eq!(parsed.0, 4242);
        assert!(parsed.1.ends_with("session-state.json"));
    }

    #[test]
    fn unrelated_arguments_are_rejected() {
        assert!(parse_args(&args(&["Ember.exe"])).is_none());
        assert!(parse_args(&args(&["Ember.exe", "hook", "codex", "idle"])).is_none());
        assert!(parse_args(&args(&["Ember.exe", "--restore-watchdog", "abc", "x"])).is_none());
        assert!(parse_args(&args(&["Ember.exe", "--restore-watchdog", "1"])).is_none());
    }

    #[test]
    fn a_missing_state_file_is_not_an_error() {
        // The parent PID does not exist, so the wait returns immediately.
        let missing = std::env::temp_dir().join("ember-definitely-not-here.json");
        let _ = std::fs::remove_file(&missing);
        assert_eq!(run(u32::MAX, &missing.display().to_string()), 0);
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("ember-watchdog-{name}.json"));
        let _ = std::fs::remove_file(&path);
        path
    }

    fn state_for(owner_pid: u32, ac: u32, dc: u32) -> lid::SessionState {
        lid::SessionState {
            v: lid::STATE_VERSION,
            scheme: lid::GuidData::from_guid(&windows::core::GUID::from_u128(0x7777)),
            ac,
            dc,
            owner_pid,
            started_unix: 0,
        }
    }

    struct RecordingPower {
        ac: std::sync::atomic::AtomicU32,
        dc: std::sync::atomic::AtomicU32,
        writes: std::sync::Mutex<Vec<String>>,
    }

    impl RecordingPower {
        fn new(ac: u32, dc: u32) -> Self {
            RecordingPower {
                ac: std::sync::atomic::AtomicU32::new(ac),
                dc: std::sync::atomic::AtomicU32::new(dc),
                writes: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    impl PowerApi for RecordingPower {
        fn active_scheme(&self) -> Result<windows::core::GUID, String> {
            Ok(windows::core::GUID::from_u128(0x7777))
        }
        fn read_ac(&self, _scheme: &windows::core::GUID) -> Result<u32, String> {
            Ok(self.ac.load(std::sync::atomic::Ordering::SeqCst))
        }
        fn read_dc(&self, _scheme: &windows::core::GUID) -> Result<u32, String> {
            Ok(self.dc.load(std::sync::atomic::Ordering::SeqCst))
        }
        fn write_ac(&self, _scheme: &windows::core::GUID, value: u32) -> Result<(), String> {
            self.ac.store(value, std::sync::atomic::Ordering::SeqCst);
            self.writes.lock().unwrap().push(format!("ac={value}"));
            Ok(())
        }
        fn write_dc(&self, _scheme: &windows::core::GUID, value: u32) -> Result<(), String> {
            self.dc.store(value, std::sync::atomic::Ordering::SeqCst);
            self.writes.lock().unwrap().push(format!("dc={value}"));
            Ok(())
        }
        fn set_active(&self, _scheme: &windows::core::GUID) -> Result<(), String> {
            self.writes.lock().unwrap().push("active".into());
            Ok(())
        }
    }

    #[test]
    fn a_foreign_state_file_is_left_for_its_own_instance() {
        let api = RecordingPower::new(0, 0);
        let path = temp_path("foreign");
        lid::write_state_at(&path, &state_for(4242, 3, 3)).unwrap();

        handle_exit(&api, 9999, &path.display().to_string());

        assert!(
            lid::read_state_at(&path).is_some(),
            "the record belongs to a newer instance and must survive"
        );
        assert!(
            api.writes.lock().unwrap().is_empty(),
            "no writes for a foreign state file"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_matching_owner_restores_and_removes_the_record() {
        let api = RecordingPower::new(0, 0);
        let path = temp_path("owned");
        lid::write_state_at(&path, &state_for(4242, 6, 5)).unwrap();

        handle_exit(&api, 4242, &path.display().to_string());

        assert_eq!(api.writes.lock().unwrap().as_slice(), ["ac=6", "dc=5"]);
        assert!(
            lid::read_state_at(&path).is_none(),
            "a successful restore removes the undo record"
        );
        let _ = std::fs::remove_file(&path);
    }
}
