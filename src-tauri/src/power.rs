//! Sleep/display blocking via `PowerCreateRequest` + `PowerSetRequest`.
//!
//! The request is owned by this process, so it disappears automatically if
//! Ember dies — Windows never needs a cleanup pass.

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Power::{
    PowerClearRequest, PowerRequestDisplayRequired, PowerRequestSystemRequired,
};
#[cfg(not(test))]
use windows::Win32::System::Power::{PowerCreateRequest, PowerSetRequest};
#[cfg(not(test))]
use windows::Win32::System::Threading::{
    POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0,
};
#[cfg(not(test))]
use windows::core::PWSTR;

pub struct SleepBlocker {
    handle: Option<SendHandle>,
    /// Backing storage for the wide reason string; must outlive the handle.
    reason: Vec<u16>,
    reason_text: String,
    keep_display: bool,
}

/// `HANDLE` is a raw pointer, so it is not `Send` by default. The power request
/// is owned by this process and is only ever reached through a `Mutex`, so it is
/// safe to move the owning value between threads.
struct SendHandle(HANDLE);

unsafe impl Send for SendHandle {}

impl Default for SleepBlocker {
    fn default() -> Self {
        SleepBlocker::new()
    }
}

impl SleepBlocker {
    pub fn new() -> Self {
        SleepBlocker {
            handle: None,
            reason: Vec::new(),
            reason_text: String::new(),
            keep_display: false,
        }
    }

    pub fn is_engaged(&self) -> bool {
        self.handle.is_some()
    }

    pub fn keeps_display(&self) -> bool {
        self.keep_display
    }

    pub fn reason(&self) -> &str {
        &self.reason_text
    }

    /// Engage (or re-engage) the request. Idempotent for identical parameters.
    ///
    /// A long hold must survive cosmetic updates: when only the human-facing
    /// reason changed, the existing OS request is kept and its string is
    /// rewritten in place (the OS reads it from our buffer, which stays alive
    /// for the lifetime of the handle). A replacement request is created
    /// *before* the old one is cleared, so a transient failure can never leave
    /// the PC unprotected.
    pub fn engage(&mut self, reason: &str, keep_display: bool) -> Result<(), String> {
        if self.handle.is_some() && self.reason_text == reason && self.keep_display == keep_display
        {
            return Ok(());
        }

        if self.handle.is_some() && self.keep_display == keep_display {
            let needed = reason.encode_utf16().count() + 1;
            if self.reason.capacity() >= needed {
                self.reason.clear();
                self.reason.extend(reason.encode_utf16());
                self.reason.push(0);
                self.reason_text = reason.to_string();
                return Ok(());
            }
            // The buffer cannot grow in place; fall through to a fresh request.
        }

        let (handle, wide) = create_request(reason, keep_display)?;

        // Swap: the new request is live; drop the old one with its own flags.
        let old_display = self.keep_display;
        if let Some(SendHandle(old)) = self.handle.take() {
            release_handle(old, old_display);
        }
        self.handle = Some(SendHandle(handle));
        // The heap buffer stays alive for as long as the handle does.
        self.reason = wide;
        self.reason_text = reason.to_string();
        self.keep_display = keep_display;
        Ok(())
    }

    pub fn release(&mut self) {
        if let Some(SendHandle(handle)) = self.handle.take() {
            release_handle(handle, self.keep_display);
        }
        self.keep_display = false;
        self.reason_text.clear();
        self.reason.clear();
    }
}

/// Build the wide, NUL-terminated reason buffer with spare capacity so later
/// cosmetic updates can be rewritten in place (the OS keeps the pointer).
fn make_wide(reason: &str) -> Vec<u16> {
    let mut wide: Vec<u16> = Vec::with_capacity(reason.encode_utf16().count() + 65);
    wide.extend(reason.encode_utf16());
    wide.push(0);
    wide
}

/// Clear and close one request. `handle` is null only under unit tests, where
/// no real OS request was ever created.
fn release_handle(handle: HANDLE, keep_display: bool) {
    if handle.is_invalid() {
        return;
    }
    unsafe {
        let _ = PowerClearRequest(handle, PowerRequestSystemRequired);
        if keep_display {
            let _ = PowerClearRequest(handle, PowerRequestDisplayRequired);
        }
        let _ = CloseHandle(handle);
    }
}

#[cfg(not(test))]
fn create_request(reason: &str, keep_display: bool) -> Result<(HANDLE, Vec<u16>), String> {
    let mut wide = make_wide(reason);
    let ctx = REASON_CONTEXT {
        Version: 0,
        Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
        Reason: REASON_CONTEXT_0 {
            SimpleReasonString: PWSTR(wide.as_mut_ptr()),
        },
    };
    unsafe {
        let handle = PowerCreateRequest(&ctx).map_err(|e| e.to_string())?;
        if let Err(e) = PowerSetRequest(handle, PowerRequestSystemRequired) {
            let _ = CloseHandle(handle);
            return Err(format!("PowerSetRequest(SystemRequired): {e}"));
        }
        if keep_display && let Err(e) = PowerSetRequest(handle, PowerRequestDisplayRequired) {
            let _ = PowerClearRequest(handle, PowerRequestSystemRequired);
            let _ = CloseHandle(handle);
            return Err(format!("PowerSetRequest(DisplayRequired): {e}"));
        }
        Ok((handle, wide))
    }
}

/// Unit tests must never touch the machine's real power-request table: they
/// get a null handle so every engage/release code path stays identical.
#[cfg(test)]
fn create_request(reason: &str, keep_display: bool) -> Result<(HANDLE, Vec<u16>), String> {
    let _ = keep_display;
    Ok((HANDLE(std::ptr::null_mut()), make_wide(reason)))
}

impl Drop for SleepBlocker {
    fn drop(&mut self) {
        self.release();
    }
}

/// Human-facing reason string shown in Reliability and used in the OS request.
pub fn reason_string(manual: bool, agents_working: usize) -> String {
    match (manual, agents_working) {
        (true, 0) => "Ember: manual session".to_string(),
        (false, 1) => "Ember: 1 coding agent working".to_string(),
        (false, n) => format!("Ember: {n} coding agents working"),
        (true, n) => format!("Ember: manual session + {n} agents working"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_strings_read_naturally() {
        assert_eq!(reason_string(false, 0), "Ember: 0 coding agents working");
        assert_eq!(reason_string(false, 1), "Ember: 1 coding agent working");
        assert_eq!(reason_string(false, 3), "Ember: 3 coding agents working");
        assert_eq!(reason_string(true, 0), "Ember: manual session");
        assert_eq!(
            reason_string(true, 2),
            "Ember: manual session + 2 agents working"
        );
    }

    #[test]
    fn a_cosmetic_reason_change_keeps_the_request_engaged() {
        let mut blocker = SleepBlocker::new();
        blocker
            .engage("Ember: 1 coding agent working", false)
            .unwrap();
        assert!(blocker.is_engaged());
        assert_eq!(blocker.reason(), "Ember: 1 coding agent working");

        // The agent count changes on almost every tick; the hold must not be
        // torn down and re-created just because the label grew.
        blocker
            .engage("Ember: 2 coding agents working", false)
            .unwrap();
        assert!(blocker.is_engaged());
        assert_eq!(blocker.reason(), "Ember: 2 coding agents working");

        // Display keeps its own flag and survives a reason update.
        blocker
            .engage("Ember: manual session + 2 agents working", true)
            .unwrap();
        assert!(blocker.keeps_display());

        blocker.release();
        assert!(!blocker.is_engaged());
        assert!(!blocker.keeps_display());
    }
}
