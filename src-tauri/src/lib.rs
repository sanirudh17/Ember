//! Ember — a Windows keep-awake companion for coding agents.
//!
//! # Process roles
//!
//! The same executable has three jobs, decided from `argv` before anything else
//! runs (see `main.rs`):
//!
//! 1. `Ember.exe` — the app. A Tauri window plus the state loop.
//! 2. `Ember.exe hook <agent> <event>` — a ~300 ms fire-and-forget hook that
//!    posts one JSON line to the app's named pipe. Never touches Tauri.
//! 3. `Ember.exe --restore-watchdog <pid> <state-file>` — a detached watchdog
//!    that puts the lid-close action back if the app dies unexpectedly.
//!
//! # Lid-proof crash recovery, in three layers
//!
//! 1. The original lid action is written to `session-state.json` *before* it is
//!    changed, and restored at launch if that file survived.
//! 2. A detached watchdog (role 3) restores it the moment the app's PID exits.
//! 3. Every normal exit path restores it explicitly.

pub mod agents;
pub mod backdrop;
pub mod battery;
pub mod commands;
pub mod engine;
pub mod hookclient;
pub mod lid;
pub mod log;
pub mod paths;
pub mod power;
pub mod settings;
pub mod shell;
pub mod sound;
pub mod tray;
pub mod watchdog;

use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use tauri::{Emitter, Manager, RunEvent, WindowEvent};

use crate::engine::Engine;
use crate::lid::RealPowerApi;

/// Everything the Tauri commands and the window loop share.
pub struct Ember {
    pub engine: Mutex<Engine>,
    /// Set when the remembered window geometry needs persisting.
    pub window_dirty: AtomicBool,
    /// Set once the user has confirmed they really want to quit.
    pub closing: AtomicBool,
}

/// Build and run the app. Never returns for the hook/watchdog roles — those are
/// handled in `main.rs` before this is reached.
pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin registered so a second launch is caught
        // before any window is created.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window(shell::MAIN_WINDOW) {
                shell::force_show(&window);
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::update_settings,
            commands::set_watching,
            commands::start_manual,
            commands::start_manual_until,
            commands::start_manual_unlimited,
            commands::stop_manual,
            commands::extend_manual,
            commands::set_paused,
            commands::pause_for,
            commands::release_lid_now,
            commands::clear_lid_cap,
            commands::preview_install,
            commands::apply_install,
            commands::refresh_agents,
            commands::test_chime,
            commands::test_notification,
            commands::hide_window,
            commands::show_window,
            commands::quit_app,
            commands::mark_closing,
            commands::reset_window,
            commands::focus_window,
            commands::window_control,
            commands::window_resize,
            commands::engine_is_armed,
            commands::get_recent_log,
            commands::app_version,
            commands::check_update,
            commands::install_update,
        ])
        .setup(|app| {
            log::install_panic_hook();
            let settings = settings::Settings::load();

            // Layer 1 of lid-proof recovery: if a state file survived a crash,
            // put the user's lid action back before doing anything else.
            if let Ok(restored) = lid::restore_from_state_file(&RealPowerApi)
                && restored
            {
                eprintln!("ember: restored the lid-close action left over from a previous run");
            }

            let engine = Engine::new();
            app.manage(Ember {
                engine: Mutex::new(engine),
                window_dirty: AtomicBool::new(false),
                closing: AtomicBool::new(false),
            });

            // Background services.
            battery::spawn_listener();
            sound::warm_up();

            // Hook transport: pipe server -> registry.
            let (tx, rx) = std::sync::mpsc::channel();
            agents::hookserver::spawn(tx);
            let hook_app = app.handle().clone();
            std::thread::Builder::new()
                .name("ember-hook-forward".into())
                .spawn(move || {
                    for message in rx {
                        if let Some(state) = hook_app.try_state::<Ember>() {
                            // Recover from a poisoned lock like every other
                            // consumer: dropping hooks after a panic elsewhere
                            // would silently disable hook-driven holds.
                            let mut engine = state
                                .engine
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            let who = format!(
                                "{} pid={}",
                                message.agent.id(),
                                message
                                    .pid
                                    .map(|p| p.to_string())
                                    .unwrap_or_else(|| "none".into())
                            );
                            let event = format!("{:?}", message.event);
                            let effect = engine.registry.on_hook(message, Instant::now());
                            match effect {
                                crate::agents::registry::HookEffect::None => {}
                                crate::agents::registry::HookEffect::Created(state) => {
                                    crate::log::append(
                                        "HOOK",
                                        &format!("{who} created -> {state:?} ({event})"),
                                    );
                                }
                                crate::agents::registry::HookEffect::Changed { from, to } => {
                                    crate::log::append(
                                        "HOOK",
                                        &format!("{who} {from:?} -> {to:?} ({event})"),
                                    );
                                }
                                crate::agents::registry::HookEffect::Removed => {
                                    crate::log::append(
                                        "HOOK",
                                        &format!("{who} removed ({event})"),
                                    );
                                }
                            }
                            engine.dirty = true;
                        }
                    }
                })?;

            let window = app
                .get_webview_window(shell::MAIN_WINDOW)
                .ok_or("the main window is missing from tauri.conf.json")?;

            // Window shell.
            shell::place_window(&window, &settings);
            backdrop::apply_solid(&window);

            // Keep the remembered geometry up to date.
            let event_window = window.clone();
            window.on_window_event(move |event| match event {
                WindowEvent::CloseRequested { api, .. } => {
                    let handle = event_window.app_handle();
                    let Some(state) = handle.try_state::<Ember>() else {
                        return;
                    };
                    if shell::should_confirm_close(&state) {
                        // The frontend renders an inline confirm instead of a
                        // modal dialog.
                        api.prevent_close();
                        let _ = event_window.emit("close-requested", ());
                        return;
                    }
                    shell::remember_window(&event_window, &state);
                    if let Ok(engine) = state.engine.lock() {
                        let _ = engine.settings.save();
                    }
                }
                WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                    if let Some(state) = event_window.app_handle().try_state::<Ember>() {
                        shell::remember_window(&event_window, &state);
                    }
                }
                _ => {}
            });

            shell::spawn_loop(app.handle().clone(), window.clone());

            // The tray is a convenience surface, never a requirement: a machine
            // that refuses it must still start normally.
            match tray::install(app.handle()) {
                Ok(()) => crate::log::append("TRAY", "tray icon installed"),
                Err(e) => {
                    crate::log::append("TRAY", &format!("tray icon failed: {e}"));
                    eprintln!("ember: could not install the tray icon: {e}");
                }
            }

            shell::force_show(&window);

            // Safety net: ensure the window is shown, unminimized, and focused even if delayed.
            let show_window = window.clone();
            std::thread::Builder::new()
                .name("ember-show".into())
                .spawn(move || {
                    std::thread::sleep(Duration::from_millis(400));
                    shell::force_show(&show_window);
                })?;

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Ember could not start")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // Layer 3 of lid-proof recovery: never leave a changed power
                // plan behind on a normal exit.
                if let Some(state) = app.try_state::<Ember>()
                    && let Ok(mut engine) = state.engine.lock()
                {
                    engine.release_lid_now();
                    engine.settings.save().ok();
                }
            }
        });
}
