//! The window shell: sizing, the 250 ms state loop, the overlay icon, backdrop
//! and notifications.
//!
//! Everything that needs the Win32 main thread lives here and is driven from a
//! single background loop, so the Tauri commands stay pure state mutations.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewWindow,
    image::Image,
};
use tauri_plugin_autostart::ManagerExt;

use crate::Ember;
use crate::agents::AgentId;
use crate::backdrop;
use crate::engine::Notify;
use crate::settings::{NOTIFY_VOLUME, Notifications, Settings, Theme};
use crate::sound;

/// The single window's label, shared by every lookup.
pub const MAIN_WINDOW: &str = "main";

/// The state loop's period. The engine throttles snapshots to 1/s itself.
const LOOP_INTERVAL: Duration = Duration::from_millis(250);

/// Ember's default window size, in logical pixels. Wide enough to open in
/// the dashboard form; the layout folds to one column below 1040px.
pub const DEFAULT_WIDTH: u32 = 1160;
pub const DEFAULT_HEIGHT: u32 = 740;

/// Restore size, position and maximised state, clamping onto a real monitor.
pub fn place_window(window: &WebviewWindow, settings: &Settings) {
    let ws = &settings.window;
    let is_max = window.is_maximized().unwrap_or(false);
    let is_min = window.is_minimized().unwrap_or(false);
    if !is_max && !is_min {
        if ws.placed && ws.x.is_some() && ws.y.is_some() {
            let monitors = monitor_rects(window);
            let (x, y) = clamp_rect(
                ws.x.unwrap_or(0),
                ws.y.unwrap_or(0),
                ws.width.max(760),
                ws.height.max(520),
                &monitors,
            );
            let _ = window.set_size(PhysicalSize::new(ws.width.max(760), ws.height.max(520)));
            let _ = window.set_position(PhysicalPosition::new(x, y));
        } else {
            let _ = window.set_size(LogicalSize::new(
                f64::from(DEFAULT_WIDTH),
                f64::from(DEFAULT_HEIGHT),
            ));
            let _ = window.center();
        }
    }
    if ws.maximized && !is_max {
        let _ = window.maximize();
    }
    force_show(window);
}

/// Bring the window to the foreground robustly. `show`/`unminimize`/`set_focus`
/// alone are not enough in practice: a window that is shown right after startup
/// (or during the single-instance handoff) often never actually reaches the
/// foreground. Flipping always-on-top on and off again forces Windows to raise
/// it, and `SetForegroundWindow` finishes the job.
pub fn force_show(window: &WebviewWindow) {
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_always_on_top(true);
    let _ = window.set_focus();
    if let Ok(handle) = window.hwnd() {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
        let hwnd = HWND(handle.0 as isize as *mut core::ffi::c_void);
        unsafe {
            let _ = SetForegroundWindow(hwnd);
        }
    }
    let _ = window.set_always_on_top(false);
}

/// Physical rectangles of every monitor, as `(x, y, width, height)`.
pub fn monitor_rects(window: &WebviewWindow) -> Vec<(i32, i32, u32, u32)> {
    window
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .map(|m| {
            let p = m.position();
            let s = m.size();
            (p.x, p.y, s.width, s.height)
        })
        .collect()
}

/// Keep the window on a real monitor, and centre it if the saved spot is gone.
pub fn clamp_rect(x: i32, y: i32, w: u32, h: u32, monitors: &[(i32, i32, u32, u32)]) -> (i32, i32) {
    if monitors.is_empty() {
        return (x, y);
    }
    let wi = w as i32;
    let hi = h as i32;
    let visible = monitors.iter().any(|(mx, my, mw, mh)| {
        let overlap_x = (x + wi).min(mx + *mw as i32) - x.max(*mx);
        let overlap_y = (y + hi).min(my + *mh as i32) - y.max(*my);
        // At least a usable corner (120x60 px) has to be on screen.
        overlap_x >= 120 && overlap_y >= 60
    });
    if visible {
        return (x, y);
    }
    let (mx, my, mw, mh) = monitors[0];
    (
        mx + ((mw as i32 - wi) / 2).max(0),
        my + ((mh as i32 - hi) / 2).max(0),
    )
}

/// Record the live geometry so the next launch restores it.
pub fn remember_window(window: &WebviewWindow, ember: &Ember) {
    let Ok(position) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.inner_size() else {
        return;
    };
    let maximized = window.is_maximized().unwrap_or(false);
    let minimized = window.is_minimized().unwrap_or(false);
    if minimized {
        return;
    }

    let Ok(mut engine) = ember.engine.lock() else {
        return;
    };
    let ws = &mut engine.settings.window;
    if !maximized {
        ws.x = Some(position.x);
        ws.y = Some(position.y);
        ws.width = size.width.max(760);
        ws.height = size.height.max(520);
    }
    ws.maximized = maximized;
    ws.placed = true;
    drop(engine);
    ember.window_dirty.store(true, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// Notifications
// ---------------------------------------------------------------------------

/// The chime, title and body a transition maps to.
pub fn notify_parts(notify: Notify) -> (crate::settings::Chime, &'static str, &'static str) {
    use crate::settings::Chime;
    match notify {
        Notify::Engaged => (
            Chime::Glass,
            "Ember is awake",
            "Ember is holding this PC awake while your agents work.",
        ),
        Notify::Finished => (
            Chime::Marimba,
            "Agents finished",
            "Every watched agent is idle, so Ember released the hold.",
        ),
        Notify::Battery => (
            Chime::LowTone,
            "Battery guardrail",
            "Ember released the hold because the battery rules said so.",
        ),
    }
}

/// Fire the chime and toast for a transition. A single mute switch gates both.
pub fn fire(app: &AppHandle, notifications: &Notifications, notify: Notify) {
    if notifications.mute {
        return;
    }
    let (chime, title, body) = notify_parts(notify);
    sound::play(chime, NOTIFY_VOLUME);
    toast(app, title, body);
}

/// Preview a chime + toast from the Notifications page (respects the mute switch).
pub fn preview(
    app: &AppHandle,
    notifications: &Notifications,
    notify: Notify,
) -> Result<(), String> {
    if notifications.mute {
        return Err("Notifications are muted".to_string());
    }
    let (chime, title, body) = notify_parts(notify);
    sound::play(chime, NOTIFY_VOLUME);
    toast(app, title, body);
    Ok(())
}

/// A best-effort Windows toast. Never allowed to break the app.
pub fn toast(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title(title).body(body).show();
}

/// Convenience used by commands that want a plain toast.
pub fn show_notification(app: &AppHandle, notifications: &Notifications, title: &str, body: &str) {
    if notifications.mute {
        return;
    }
    toast(app, title, body);
}

// ---------------------------------------------------------------------------
// The loop
// ---------------------------------------------------------------------------

/// Start the state loop. One loop drives the engine, the window's title,
/// overlay icon, theme and backdrop, so there is exactly one source of truth.
pub fn spawn_loop(app: AppHandle, window: WebviewWindow) {
    let _ = std::thread::Builder::new()
        .name("ember-loop".into())
        .spawn(move || {
            let ember = app.state::<Ember>();

            let mut last_title = String::new();
            let mut last_overlay: Option<bool> = None;
            let mut last_theme: Option<Theme> = None;
            let mut last_autostart: Option<bool> = None;
            let mut last_saved = Instant::now();

            loop {
                std::thread::sleep(LOOP_INTERVAL);

                let (outcome, settings) = {
                    let mut engine = match ember.engine.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    let outcome = engine.tick(Instant::now());
                    (outcome, engine.settings.clone())
                };

                // --- window theme ----------------------------------------------
                if last_theme != Some(settings.theme) {
                    let theme = match settings.theme {
                        Theme::System => None,
                        Theme::Light => Some(tauri::Theme::Light),
                        Theme::Dark => Some(tauri::Theme::Dark),
                    };
                    let _ = window.set_theme(theme);
                    last_theme = Some(settings.theme);
                }

                // --- launch at login ------------------------------------------
                // The autostart entry and the setting must agree, including on
                // first run, so the OS entry is created from the saved default.
                if last_autostart != Some(settings.launch_at_login) {
                    let autolaunch = app.autolaunch();
                    let current = autolaunch.is_enabled().unwrap_or(false);
                    if current != settings.launch_at_login {
                        let result = if settings.launch_at_login {
                            autolaunch.enable()
                        } else {
                            autolaunch.disable()
                        };
                        if let Err(e) = result {
                            eprintln!("ember: could not update the launch-at-login entry: {e}");
                        }
                    }
                    last_autostart = Some(settings.launch_at_login);
                }

                // --- title ----------------------------------------------------
                let title = outcome.snapshot.window_title();
                if title != last_title {
                    let _ = window.set_title(&title);
                    last_title = title;
                }

                // --- tray -----------------------------------------------------
                // Cheap and idempotent; it only touches the items that changed.
                crate::tray::refresh(&app, &outcome.snapshot);

                // --- overlay icon ----------------------------------------------
                if last_overlay != Some(outcome.snapshot.engaged) {
                    let icon = if outcome.snapshot.engaged {
                        Some(Image::new(&backdrop::ember_dot_rgba(), 16, 16).to_owned())
                    } else {
                        None
                    };
                    let _ = window.set_overlay_icon(icon);
                    last_overlay = Some(outcome.snapshot.engaged);
                }

                // --- notifications -------------------------------------------
                for notify in &outcome.notifications {
                    fire(&app, &settings.notifications, *notify);
                }

                // --- emit ------------------------------------------------------
                if outcome.emit {
                    let _ = app.emit("state-changed", &outcome.snapshot);
                }

                // --- periodic settings save ------------------------------------
                if ember.window_dirty.load(Ordering::Relaxed)
                    && last_saved.elapsed() >= Duration::from_secs(5)
                {
                    ember.window_dirty.store(false, Ordering::Relaxed);
                    last_saved = Instant::now();
                    if let Ok(engine) = ember.engine.lock() {
                        let _ = engine.settings.save();
                    }
                }
            }
        });
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Should closing the window be intercepted? Only while Ember is actually
/// holding the machine awake or a manual session is running.
pub fn should_confirm_close(ember: &Ember) -> bool {
    if ember.closing.load(Ordering::Relaxed) {
        return false;
    }
    let Ok(engine) = ember.engine.lock() else {
        return false;
    };
    engine.manual.active || engine.sleep_blocker().is_engaged()
}

/// Agent ids in a stable, display-ready order.
pub fn agent_order() -> Vec<&'static str> {
    AgentId::ALL.iter().map(|a| a.id()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: (i32, i32, u32, u32) = (0, 0, 1920, 1080);
    const TWO: (i32, i32, u32, u32) = (1920, 0, 2560, 1440);

    #[test]
    fn an_on_screen_position_is_left_alone() {
        assert_eq!(clamp_rect(200, 120, 1000, 720, &[ONE, TWO]), (200, 120));
        assert_eq!(clamp_rect(2100, 200, 1000, 720, &[ONE, TWO]), (2100, 200));
    }

    #[test]
    fn an_off_screen_window_is_centred_on_the_primary_monitor() {
        // Way off to the right, where no monitor exists.
        assert_eq!(clamp_rect(9000, 20, 1000, 720, &[ONE, TWO]), (460, 180));
        // Negative space with no monitor there.
        assert_eq!(clamp_rect(-5000, -4000, 1000, 720, &[ONE]), (460, 180));
    }

    #[test]
    fn a_monitor_that_vanished_still_lands_somewhere_visible() {
        // Saved on the second monitor, now unplugged.
        assert_eq!(clamp_rect(3000, 400, 1000, 720, &[ONE]), (460, 180));
    }

    #[test]
    fn a_sliver_of_overlap_is_not_enough() {
        // Only 40px of the window is on screen horizontally.
        assert_eq!(clamp_rect(1880, 100, 1000, 720, &[ONE]), (460, 180));
        // 200px across is plenty.
        assert_eq!(clamp_rect(1720, 100, 1000, 720, &[ONE]), (1720, 100));
    }

    #[test]
    fn no_monitors_means_no_clamping() {
        assert_eq!(clamp_rect(123, 456, 1000, 720, &[]), (123, 456));
    }

    #[test]
    fn defaults_are_the_documented_window_size() {
        assert_eq!((DEFAULT_WIDTH, DEFAULT_HEIGHT), (1160, 740));
    }

    #[test]
    fn agent_order_covers_every_agent() {
        let order = agent_order();
        assert_eq!(order.len(), AgentId::ALL.len());
        assert!(order.contains(&"claude-code"));
        assert!(order.contains(&"cline"));
    }
}
