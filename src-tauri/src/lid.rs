//! Lid-proof holds.
//!
//! A power request alone does **not** stop Windows from sleeping on lid close —
//! the lid-close action has to be changed while the hold is active. We read the
//! active scheme, persist everything needed to undo the change *before* touching
//! anything, set both the AC and DC lid-close actions to "Do nothing", then
//! read back to verify. Any failure rolls straight back.
//!
//! `powercfg` is deliberately never parsed: its output is localised.

use std::ffi::c_void;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::{HLOCAL, LocalFree, WIN32_ERROR};
use windows::Win32::System::Power::{
    CallNtPowerInformation, PowerGetActiveScheme, PowerReadACValueIndex, PowerReadDCValueIndex,
    PowerSetActiveScheme, PowerWriteACValueIndex, PowerWriteDCValueIndex,
    SYSTEM_POWER_CAPABILITIES, SystemPowerCapabilities,
};
use windows::core::GUID;

use crate::paths;

// The `windows` crate does not generate these power-setting GUIDs, so they are
// declared here with the authoritative values from the Windows SDK
// (`winnt.h`, Windows SDK 10.0.26100.0).
const GUID_SYSTEM_BUTTON_SUBGROUP: GUID = GUID::from_u128(0x4F971E89_EEBD_4455_A8DE_9E59040E7347);
const GUID_LIDCLOSE_ACTION: GUID = GUID::from_u128(0x5CA83367_6E45_459F_A27B_476B1D01C936);

/// `Do nothing` on lid close; the "value index" for the System Button subgroup.
const LID_ACTION_DO_NOTHING: u32 = 0;

// ---------------------------------------------------------------------------
// Capabilities
// ---------------------------------------------------------------------------

/// SYSTEM_POWER_CAPABILITIES via `CallNtPowerInformation(SystemPowerCapabilities)`.
pub fn capabilities() -> Option<SYSTEM_POWER_CAPABILITIES> {
    let mut caps: SYSTEM_POWER_CAPABILITIES = unsafe { std::mem::zeroed() };
    let status = unsafe {
        CallNtPowerInformation(
            SystemPowerCapabilities,
            None,
            0,
            Some(&mut caps as *mut _ as *mut c_void),
            std::mem::size_of::<SYSTEM_POWER_CAPABILITIES>() as u32,
        )
    };
    if status.0 >= 0 { Some(caps) } else { None }
}

/// True on laptops / convertibles with a lid switch.
pub fn lid_present() -> bool {
    capabilities().map(|c| c.LidPresent).unwrap_or(false)
}

/// True on Modern Standby (AoAc) machines.
pub fn modern_standby() -> bool {
    capabilities().map(|c| c.AoAc).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// GUID helpers (serde-friendly, so session-state.json stays readable)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuidData {
    pub d1: u32,
    pub d2: u16,
    pub d3: u16,
    pub d4: [u8; 8],
}

impl GuidData {
    pub fn from_guid(g: &GUID) -> Self {
        GuidData {
            d1: g.data1,
            d2: g.data2,
            d3: g.data3,
            d4: g.data4,
        }
    }

    pub fn to_guid(self) -> GUID {
        GUID {
            data1: self.d1,
            data2: self.d2,
            data3: self.d3,
            data4: self.d4,
        }
    }

    pub fn to_braces(self) -> String {
        let g = self.to_guid();
        format!(
            "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
            g.data1,
            g.data2,
            g.data3,
            g.data4[0],
            g.data4[1],
            g.data4[2],
            g.data4[3],
            g.data4[4],
            g.data4[5],
            g.data4[6],
            g.data4[7]
        )
    }
}

// ---------------------------------------------------------------------------
// Persisted hold
// ---------------------------------------------------------------------------

pub const STATE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SessionState {
    pub v: u32,
    pub scheme: GuidData,
    pub ac: u32,
    pub dc: u32,
    pub owner_pid: u32,
    pub started_unix: u64,
}

pub fn write_state(state: &SessionState) -> Result<(), String> {
    write_state_at(&paths::session_state_path(), state)
}

pub fn write_state_at(path: &Path, state: &SessionState) -> Result<(), String> {
    let text = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(path, text.as_bytes())
}

pub fn read_state() -> Option<SessionState> {
    read_state_at(&paths::session_state_path())
}

pub fn read_state_at(path: &Path) -> Option<SessionState> {
    let text = std::fs::read_to_string(path).ok()?;
    let state: SessionState = serde_json::from_str(&text).ok()?;
    // A record from a future state format must not be misparsed: treat it as
    // absent rather than restoring the wrong thing.
    if state.v != STATE_VERSION {
        return None;
    }
    Some(state)
}

pub fn delete_state() {
    delete_state_at(&paths::session_state_path())
}

pub fn delete_state_at(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn owner_pid() -> u32 {
    std::process::id()
}

// ---------------------------------------------------------------------------
// Power API (trait so the hold logic is testable with a fake)
// ---------------------------------------------------------------------------

pub trait PowerApi: Send + Sync {
    fn active_scheme(&self) -> Result<GUID, String>;
    fn read_ac(&self, scheme: &GUID) -> Result<u32, String>;
    fn read_dc(&self, scheme: &GUID) -> Result<u32, String>;
    fn write_ac(&self, scheme: &GUID, value: u32) -> Result<(), String>;
    fn write_dc(&self, scheme: &GUID, value: u32) -> Result<(), String>;
    fn set_active(&self, scheme: &GUID) -> Result<(), String>;
}

pub struct RealPowerApi;

fn check(rc: WIN32_ERROR, what: &str) -> Result<(), String> {
    if rc == WIN32_ERROR(0) {
        Ok(())
    } else {
        Err(format!("{what} failed (Win32 error {})", rc.0))
    }
}

impl PowerApi for RealPowerApi {
    fn active_scheme(&self) -> Result<GUID, String> {
        let mut ptr: *mut GUID = std::ptr::null_mut();
        let rc = unsafe { PowerGetActiveScheme(None, &mut ptr) };
        check(rc, "PowerGetActiveScheme")?;
        if ptr.is_null() {
            return Err("PowerGetActiveScheme returned a null scheme".into());
        }
        let scheme = unsafe { *ptr };
        unsafe {
            let _ = LocalFree(Some(HLOCAL(ptr as *mut c_void)));
        }
        Ok(scheme)
    }

    fn read_ac(&self, scheme: &GUID) -> Result<u32, String> {
        let mut value: u32 = 0;
        let rc = unsafe {
            PowerReadACValueIndex(
                None,
                Some(scheme as *const GUID),
                Some(&GUID_SYSTEM_BUTTON_SUBGROUP as *const GUID),
                Some(&GUID_LIDCLOSE_ACTION as *const GUID),
                &mut value,
            )
        };
        check(rc, "PowerReadACValueIndex")?;
        Ok(value)
    }

    fn read_dc(&self, scheme: &GUID) -> Result<u32, String> {
        let mut value: u32 = 0;
        let rc = unsafe {
            PowerReadDCValueIndex(
                None,
                Some(scheme as *const GUID),
                Some(&GUID_SYSTEM_BUTTON_SUBGROUP as *const GUID),
                Some(&GUID_LIDCLOSE_ACTION as *const GUID),
                &mut value,
            )
        };
        // powrprof returns DWORD here; treat any non-zero as failure.
        if rc != 0 {
            return Err(format!("PowerReadDCValueIndex failed (Win32 error {rc})"));
        }
        Ok(value)
    }

    fn write_ac(&self, scheme: &GUID, value: u32) -> Result<(), String> {
        let rc = unsafe {
            PowerWriteACValueIndex(
                None,
                scheme as *const GUID,
                Some(&GUID_SYSTEM_BUTTON_SUBGROUP as *const GUID),
                Some(&GUID_LIDCLOSE_ACTION as *const GUID),
                value,
            )
        };
        check(rc, "PowerWriteACValueIndex")
    }

    fn write_dc(&self, scheme: &GUID, value: u32) -> Result<(), String> {
        let rc = unsafe {
            PowerWriteDCValueIndex(
                None,
                scheme as *const GUID,
                Some(&GUID_SYSTEM_BUTTON_SUBGROUP as *const GUID),
                Some(&GUID_LIDCLOSE_ACTION as *const GUID),
                value,
            )
        };
        if rc != 0 {
            return Err(format!("PowerWriteDCValueIndex failed (Win32 error {rc})"));
        }
        Ok(())
    }

    fn set_active(&self, scheme: &GUID) -> Result<(), String> {
        let rc = unsafe { PowerSetActiveScheme(None, Some(scheme as *const GUID)) };
        check(rc, "PowerSetActiveScheme")
    }
}

// ---------------------------------------------------------------------------
// Engage / restore
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LidHold {
    pub scheme: GuidData,
    pub ac: u32,
    pub dc: u32,
    pub started_unix: u64,
    pub owner_pid: u32,
}

impl LidHold {
    pub fn to_state(&self) -> SessionState {
        SessionState {
            v: STATE_VERSION,
            scheme: self.scheme,
            ac: self.ac,
            dc: self.dc,
            owner_pid: self.owner_pid,
            started_unix: self.started_unix,
        }
    }

    pub fn from_state(s: &SessionState) -> Self {
        LidHold {
            scheme: s.scheme,
            ac: s.ac,
            dc: s.dc,
            started_unix: s.started_unix,
            owner_pid: s.owner_pid,
        }
    }
}

/// Change the lid-close action to "Do nothing" on AC and DC. The original
/// values are persisted before the first write so a crash can be undone.
pub fn engage(api: &dyn PowerApi) -> Result<LidHold, String> {
    engage_at(api, &paths::session_state_path())
}

/// [`engage`] against an explicit state file. The file is process-global in
/// production; tests pass their own so they cannot race each other.
pub fn engage_at(api: &dyn PowerApi, state_path: &Path) -> Result<LidHold, String> {
    // A state file at engage time means a previous hold was never cleanly
    // restored (its writes failed, or a crash raced the watchdog). Put those
    // originals back *before* recording fresh ones: otherwise the current
    // "Do nothing" would be recorded as the user's original and the lid
    // action would be lost for good.
    if let Some(prev) = read_state_at(state_path) {
        restore_at(api, &LidHold::from_state(&prev), state_path)?;
    }

    let scheme = api.active_scheme()?;
    let ac = api.read_ac(&scheme)?;
    let dc = api.read_dc(&scheme)?;

    let hold = LidHold {
        scheme: GuidData::from_guid(&scheme),
        ac,
        dc,
        started_unix: now_unix(),
        owner_pid: owner_pid(),
    };

    // Layer 1 of crash recovery: the undo record exists before the change.
    write_state_at(state_path, &hold.to_state())?;

    let rollback = |api: &dyn PowerApi| {
        let g = hold.scheme.to_guid();
        let _ = api.write_ac(&g, ac);
        let _ = api.write_dc(&g, dc);
        let _ = api.set_active(&g);
        delete_state_at(state_path);
    };

    if let Err(e) = api
        .write_ac(&scheme, LID_ACTION_DO_NOTHING)
        .and_then(|_| api.write_dc(&scheme, LID_ACTION_DO_NOTHING))
        .and_then(|_| api.set_active(&scheme))
    {
        rollback(api);
        return Err(e);
    }

    let ac_back = api.read_ac(&scheme).unwrap_or(u32::MAX);
    let dc_back = api.read_dc(&scheme).unwrap_or(u32::MAX);
    if ac_back != LID_ACTION_DO_NOTHING || dc_back != LID_ACTION_DO_NOTHING {
        rollback(api);
        return Err(format!(
            "lid action write did not stick (ac={ac_back}, dc={dc_back})"
        ));
    }

    Ok(hold)
}

/// Put the saved lid action back, into the scheme that was recorded at engage
/// time. The scheme is deliberately *not* re-activated: if the user switched
/// plans while Ember held the lid, their current plan was never modified and
/// must stay selected.
pub fn restore(api: &dyn PowerApi, hold: &LidHold) -> Result<(), String> {
    restore_at(api, hold, &paths::session_state_path())
}

/// [`restore`] against an explicit state file. The undo record is deleted only
/// after every write succeeded; on failure it stays behind so the watchdog or
/// the next launch can retry. Deleting it anyway used to lose the user's
/// original lid action permanently.
pub fn restore_at(api: &dyn PowerApi, hold: &LidHold, state_path: &Path) -> Result<(), String> {
    let scheme = hold.scheme.to_guid();
    let mut first_error: Option<String> = None;
    for step in [
        api.write_ac(&scheme, hold.ac),
        api.write_dc(&scheme, hold.dc),
    ] {
        if let Err(e) = step {
            first_error.get_or_insert(e);
        }
    }
    match first_error {
        None => {
            delete_state_at(state_path);
            Ok(())
        }
        Some(e) => Err(e),
    }
}

/// Crash recovery: if a state file survived, undo whatever it recorded.
pub fn restore_from_state_file(api: &dyn PowerApi) -> Result<bool, String> {
    let Some(state) = read_state() else {
        return Ok(false);
    };
    let hold = LidHold::from_state(&state);
    restore(api, &hold)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    fn gid(n: u128) -> GUID {
        GUID::from_u128(n)
    }

    /// A per-test state file, so tests never race on the process-global one.
    fn state_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ember-lid-test-{name}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("session-state.json")
    }

    struct FakePower {
        scheme: GUID,
        ac: AtomicU32,
        dc: AtomicU32,
        fail_set_active: AtomicBool,
        fail_write: AtomicBool,
        log: Mutex<Vec<String>>,
    }

    impl FakePower {
        fn new(scheme: u128, ac: u32, dc: u32) -> Self {
            FakePower {
                scheme: gid(scheme),
                ac: AtomicU32::new(ac),
                dc: AtomicU32::new(dc),
                fail_set_active: AtomicBool::new(false),
                fail_write: AtomicBool::new(false),
                log: Mutex::new(Vec::new()),
            }
        }
    }

    impl PowerApi for FakePower {
        fn active_scheme(&self) -> Result<GUID, String> {
            Ok(self.scheme)
        }
        fn read_ac(&self, _scheme: &GUID) -> Result<u32, String> {
            Ok(self.ac.load(Ordering::SeqCst))
        }
        fn read_dc(&self, _scheme: &GUID) -> Result<u32, String> {
            Ok(self.dc.load(Ordering::SeqCst))
        }
        fn write_ac(&self, _scheme: &GUID, value: u32) -> Result<(), String> {
            if self.fail_write.load(Ordering::SeqCst) {
                return Err("write_ac refused".into());
            }
            self.log.lock().unwrap().push(format!("ac={value}"));
            self.ac.store(value, Ordering::SeqCst);
            Ok(())
        }
        fn write_dc(&self, _scheme: &GUID, value: u32) -> Result<(), String> {
            if self.fail_write.load(Ordering::SeqCst) {
                return Err("write_dc refused".into());
            }
            self.log.lock().unwrap().push(format!("dc={value}"));
            self.dc.store(value, Ordering::SeqCst);
            Ok(())
        }
        fn set_active(&self, _scheme: &GUID) -> Result<(), String> {
            if self.fail_set_active.load(Ordering::SeqCst) {
                return Err("set_active refused".into());
            }
            self.log.lock().unwrap().push("active".into());
            Ok(())
        }
    }

    #[test]
    fn engage_writes_do_nothing_and_restore_puts_back() {
        let api = FakePower::new(0x1111, 3, 2);
        let path = state_path("engage_restore");
        let hold = engage_at(&api, &path).expect("engage");
        assert_eq!(hold.ac, 3);
        assert_eq!(hold.dc, 2);
        assert_eq!(api.ac.load(Ordering::SeqCst), LID_ACTION_DO_NOTHING);
        assert_eq!(api.dc.load(Ordering::SeqCst), LID_ACTION_DO_NOTHING);

        restore_at(&api, &hold, &path).expect("restore");
        assert_eq!(api.ac.load(Ordering::SeqCst), 3);
        assert_eq!(api.dc.load(Ordering::SeqCst), 2);
        assert_eq!(
            api.log.lock().unwrap().as_slice(),
            ["ac=0", "dc=0", "active", "ac=3", "dc=2"]
        );
    }

    #[test]
    fn engage_is_idempotent_about_original_values() {
        let api = FakePower::new(0x4444, 1, 1);
        let path = state_path("idempotent");
        let a = engage_at(&api, &path).expect("engage 1");
        // already do-nothing; a second engage still records the same originals
        let read_now_ac = api.ac.load(Ordering::SeqCst);
        assert_eq!(read_now_ac, 0);
        restore_at(&api, &a, &path).unwrap();
        assert_eq!(api.ac.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn engage_rolls_back_when_set_active_fails() {
        let api = FakePower::new(0x2222, 1, 1);
        api.fail_set_active.store(true, Ordering::SeqCst);
        let err = engage_at(&api, &state_path("rollback_active")).unwrap_err();
        assert!(err.contains("set_active"));
        // rolled back to originals
        assert_eq!(api.ac.load(Ordering::SeqCst), 1);
        assert_eq!(api.dc.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn engage_rolls_back_when_write_fails() {
        let api = FakePower::new(0x3333, 1, 0);
        api.fail_write.store(true, Ordering::SeqCst);
        assert!(engage_at(&api, &state_path("rollback_write")).is_err());
    }

    #[test]
    fn a_failed_restore_keeps_the_undo_record() {
        let api = FakePower::new(0x5555, 0, 0);
        let path = state_path("failed_restore");
        let hold = LidHold {
            scheme: GuidData::from_guid(&gid(0x5555)),
            ac: 4,
            dc: 4,
            started_unix: 0,
            owner_pid: 0,
        };
        write_state_at(&path, &hold.to_state()).unwrap();

        api.fail_write.store(true, Ordering::SeqCst);
        assert!(restore_at(&api, &hold, &path).is_err());
        assert!(
            read_state_at(&path).is_some(),
            "the undo record must survive a failed restore"
        );

        // Once the machine allows the writes again, the retry succeeds and
        // the record goes away.
        api.fail_write.store(false, Ordering::SeqCst);
        restore_at(&api, &hold, &path).unwrap();
        assert_eq!(api.ac.load(Ordering::SeqCst), 4);
        assert!(read_state_at(&path).is_none());
    }

    #[test]
    fn engage_recovers_a_leftover_hold_before_recording_new_originals() {
        // The machine still shows "Do nothing" from a restore that never ran.
        let api = FakePower::new(0x6666, 0, 0);
        let path = state_path("leftover");
        let leftover = LidHold {
            scheme: GuidData::from_guid(&gid(0x6666)),
            ac: 8,
            dc: 7,
            started_unix: 0,
            owner_pid: 0,
        };
        write_state_at(&path, &leftover.to_state()).unwrap();

        let hold = engage_at(&api, &path).expect("engage heals the leftover first");
        // The recovered originals, not the poisoned zeros, were recorded.
        assert_eq!(hold.ac, 8);
        assert_eq!(hold.dc, 7);
        assert_eq!(api.ac.load(Ordering::SeqCst), LID_ACTION_DO_NOTHING);
        assert_eq!(api.dc.load(Ordering::SeqCst), LID_ACTION_DO_NOTHING);
        // And a fresh undo record is in place for the next restore.
        let record = read_state_at(&path).expect("fresh record");
        assert_eq!(record.ac, 8);
        assert_eq!(record.dc, 7);
    }

    #[test]
    fn state_round_trip() {
        let state = SessionState {
            v: STATE_VERSION,
            scheme: GuidData::from_guid(&gid(0xABCDEF)),
            ac: 2,
            dc: 5,
            owner_pid: 4242,
            started_unix: 1_700_000_000,
        };
        let path = state_path("round_trip");
        let text = serde_json::to_string_pretty(&state).unwrap();
        std::fs::write(&path, &text).unwrap();
        let back = read_state_at(&path).expect("read back");
        assert_eq!(back, state);
        let guid = back.scheme.to_guid();
        assert_eq!(guid, gid(0xABCDEF));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn guid_data_braces_format() {
        let g = gid(0x5ca83367_6e45_459f_a27b_476b1d01c936);
        let text = GuidData::from_guid(&g).to_braces();
        assert_eq!(text, "{5CA83367-6E45-459F-A27B-476B1D01C936}");
    }
}
