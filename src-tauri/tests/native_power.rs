//! Real Windows power API proof. No sleep, shutdown, power-plan or lid changes.
#![cfg(windows)]
use ember::power::SleepBlocker;
use windows::Win32::System::Power::{
    CallNtPowerInformation, ES_SYSTEM_REQUIRED, SystemExecutionState,
};

#[test]
fn windows_accepts_real_system_and_display_requests_through_reason_changes() {
    // Integration tests link the production library, so PowerSetRequest is real.
    let mut request = SleepBlocker::new();
    request
        .engage("Ember release verification", false)
        .expect("Windows must accept SystemRequired");
    for n in 1..=60 {
        request
            .engage(&format!("Ember verification: {n} active agents"), false)
            .unwrap();
        assert!(request.is_engaged());
    }
    let mut state = 0u32;
    let status = unsafe {
        CallNtPowerInformation(
            SystemExecutionState,
            None,
            0,
            Some(&mut state as *mut _ as _),
            4,
        )
    };
    assert!(status.is_ok(), "Execution-state query failed: {status:?}");
    assert_ne!(
        state & ES_SYSTEM_REQUIRED.0,
        0,
        "Windows must report system-required work"
    );
    println!("Windows execution state during real hold: {state:#x}");
    request
        .engage("Ember display verification", true)
        .expect("Windows must accept DisplayRequired");
    assert!(request.keeps_display());
    request.release();
    assert!(!request.is_engaged());
    request
        .engage("Ember final reacquire verification", false)
        .unwrap();
    // Drop closes the process-owned request, including during unwinding.
}
