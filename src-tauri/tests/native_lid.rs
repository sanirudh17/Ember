//! Opt-in release proof: briefly writes lid policy, then restores the originals.
//! Never closes the lid, suspends the machine, or requests a shutdown.
#![cfg(windows)]
use ember::lid::{self, LidHold, PowerApi, RealPowerApi};
use std::os::windows::process::CommandExt;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
struct Restore {
    hold: LidHold,
    path: PathBuf,
}
impl Drop for Restore {
    fn drop(&mut self) {
        lid::restore_at(&RealPowerApi, &self.hold, &self.path)
            .expect("Release verification must restore original lid policy");
    }
}

#[test]
#[ignore = "Opt-in release test briefly changes the current user's lid policy"]
fn real_lid_policy_and_detached_watchdog_restore_the_original_values() {
    let api = RealPowerApi;
    let scheme = api.active_scheme().unwrap();
    let ac = api.read_ac(&scheme).unwrap();
    let dc = api.read_dc(&scheme).unwrap();
    let path = std::env::temp_dir().join(format!("ember-release-lid-{}.json", std::process::id()));
    let hold = lid::engage_at(&api, &path).unwrap();
    let restore = Restore {
        hold: hold.clone(),
        path: path.clone(),
    };
    assert_eq!(
        api.read_ac(&scheme).unwrap(),
        0,
        "AC lid action must be Do nothing"
    );
    assert_eq!(
        api.read_dc(&scheme).unwrap(),
        0,
        "DC lid action must be Do nothing"
    );
    lid::restore_at(&api, &hold, &path).unwrap();
    assert!(!path.exists());
    assert_eq!(
        (api.read_ac(&scheme).unwrap(), api.read_dc(&scheme).unwrap()),
        (ac, dc)
    );
    let hold = lid::engage_at(&api, &path).unwrap();
    let mut owner = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Sleep -Seconds 1",
        ])
        .creation_flags(0x08000000)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut state = hold.to_state();
    state.owner_pid = owner.id();
    lid::write_state_at(&path, &state).unwrap();
    let mut watchdog = Command::new(env!("CARGO_BIN_EXE_ember"))
        .arg("--restore-watchdog")
        .arg(owner.id().to_string())
        .arg(&path)
        .creation_flags(0x08000000)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    owner.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while path.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !path.exists(),
        "Watchdog must remove its undo record after restoring"
    );
    assert!(watchdog.wait().unwrap().success());
    assert_eq!(
        (api.read_ac(&scheme).unwrap(), api.read_dc(&scheme).unwrap()),
        (ac, dc)
    );
    println!(
        "Real AC/DC lid policy restored by normal path and watchdog; originals AC={ac}, DC={dc}"
    );
    drop(restore);
}
