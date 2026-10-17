//! Ember's single executable, with three roles decided from `argv` *before*
//! anything heavy runs.
//!
//! ```text
//! Ember.exe                                     -> the app (Tauri + WebView2)
//! Ember.exe hook <agent-id> <event>             -> the ~300 ms hook CLI
//! Ember.exe --restore-watchdog <pid> <state>    -> the detached lid watchdog
//! ```
//!
//! Roles 2 and 3 must never touch Tauri: a hook that booted a WebView would be
//! slow, and a watchdog that joined the single-instance group would be useless.

// Release builds are GUI-only so no console window ever flashes for a hook.
// Debug builds keep the console, which makes `cargo run` and the tests usable.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    // Role 2: `Ember.exe hook claude-code working`.
    if let Some((agent, event)) = ember::hookclient::parse_args(&args) {
        let agent = agent.to_string();
        let event = event.to_string();
        return ExitCode::from(ember::hookclient::run(&agent, &event) as u8);
    }

    // Role 3: `Ember.exe --restore-watchdog <parent-pid> <state-file>`.
    if let Some((parent_pid, state_file)) = ember::watchdog::parse_args(&args) {
        return ExitCode::from(ember::watchdog::run(parent_pid, &state_file) as u8);
    }

    // Role 1: the app itself.
    ember::run();
    ExitCode::SUCCESS
}
