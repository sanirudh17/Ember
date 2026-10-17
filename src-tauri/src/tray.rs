//! The system tray icon and its context menu.
//!
//! The tray is a second, always-available control surface: left click raises
//! the window, right click opens a menu that mirrors the manual-session
//! controls. [`refresh`] is driven from the state loop every tick so the status
//! line and the pause/resume items stay in sync with the engine.

use std::sync::Mutex;

use tauri::menu::{MenuBuilder, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::Ember;
use crate::engine::{Engine, Snapshot};

/// The tray icon id, shared by `install` and `refresh`.
const TRAY_ID: &str = "ember-tray";

/// Handles to the menu items [`refresh`] mutates. `TrayIcon` has no menu getter,
/// so the shell keeps its own handles alive for the lifetime of the app.
///
/// The `TrayIcon` handle itself is kept here too, because the type is
/// reference-counted and the OS icon is removed the moment the last instance
/// is dropped — letting `install` return would otherwise make the icon vanish.
struct TrayMenu {
    _tray: tauri::tray::TrayIcon,
    status: MenuItem<tauri::Wry>,
    pause30: MenuItem<tauri::Wry>,
    pause60: MenuItem<tauri::Wry>,
    resume: MenuItem<tauri::Wry>,
    /// Last paused state pushed to the OS; menu edits are only sent on change.
    last_paused: Option<bool>,
}

/// The one installed tray menu. `None` until [`install`] succeeds; keeping the
/// items here is also what keeps them alive, since a menu edit needs a handle.
static MENU: Mutex<Option<TrayMenu>> = Mutex::new(None);

/// Lock the engine exactly like the state loop does: recover from a poisoned
/// mutex rather than refusing to act on a tray click.
fn lock_engine(ember: &Ember) -> std::sync::MutexGuard<'_, Engine> {
    match ember.engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Build and install the tray icon. Called once from `setup`; a failure is
/// reported to the caller and must never stop the app from starting.
pub fn install(app: &AppHandle) -> Result<(), String> {
    // The tray shows the bare glyph (no tile) so it stays legible at 16px;
    // if the asset ever fails to decode, the tiled window icon still fits.
    let icon = match tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png")) {
        Ok(icon) => icon,
        Err(_) => app
            .default_window_icon()
            .cloned()
            .ok_or_else(|| "the app has no window icon to use for the tray".to_string())?,
    };

    let status = MenuItem::with_id(app, "status", "Starting…", false, None::<&str>)
        .map_err(|e| e.to_string())?;
    let open = MenuItem::with_id(app, "open", "Open Ember", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let pause30 = MenuItem::with_id(app, "pause30", "Pause for 30 minutes", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let pause60 = MenuItem::with_id(app, "pause60", "Pause for 1 hour", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let resume = MenuItem::with_id(app, "resume", "Resume now", false, None::<&str>)
        .map_err(|e| e.to_string())?;
    let endhold = MenuItem::with_id(app, "endhold", "End hold now", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let quit = MenuItem::with_id(app, "quit", "Quit Ember", true, None::<&str>)
        .map_err(|e| e.to_string())?;

    let sep1 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let sep2 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let sep3 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;

    let menu = MenuBuilder::new(app)
        .item(&status)
        .item(&sep1)
        .item(&open)
        .item(&pause30)
        .item(&pause60)
        .item(&resume)
        .item(&sep2)
        .item(&endhold)
        .item(&sep3)
        .item(&quit)
        .build()
        .map_err(|e| e.to_string())?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Ember")
        .menu(&menu)
        // Left click raises the window; the menu opens on right click only.
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
                && let Some(window) = tray
                    .app_handle()
                    .get_webview_window(crate::shell::MAIN_WINDOW)
            {
                crate::shell::force_show(&window);
            }
        })
        .build(app)
        .map_err(|e| e.to_string())?;

    *MENU.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(TrayMenu {
        _tray: tray,
        status,
        pause30,
        pause60,
        resume,
        last_paused: None,
    });
    Ok(())
}

/// The tray menu's click handler.
fn handle_menu(app: &AppHandle, event: MenuEvent) {
    let Some(state) = app.try_state::<Ember>() else {
        return;
    };
    match event.id().as_ref() {
        "open" => {
            if let Some(window) = app.get_webview_window(crate::shell::MAIN_WINDOW) {
                crate::shell::force_show(&window);
            }
        }
        "pause30" => lock_engine(&state).pause_for(1800),
        "pause60" => lock_engine(&state).pause_for(3600),
        "resume" => lock_engine(&state).set_paused(false),
        // An indefinite pause: releases the manual session and gates further
        // holds. Agent-driven holds also end, because the tick releases
        // everything within a quarter-second of `paused` becoming true.
        "endhold" => lock_engine(&state).set_paused(true),
        // Restore the lid action before leaving; `RunEvent::Exit` in lib.rs
        // finishes the remaining shutdown work.
        "quit" => {
            lock_engine(&state).release_lid_now();
            app.exit(0);
        }
        _ => {}
    }
}

/// Push the latest state into the tray. Cheap and idempotent: safe to call on
/// every state-loop tick.
pub fn refresh(_app: &AppHandle, snapshot: &Snapshot) {
    let mut guard = MENU.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(menu) = guard.as_mut() else {
        return;
    };

    let text = status_text(snapshot);
    // Menu edits cross a thread boundary to the OS shell, so only touch the
    // status line when it actually changed.
    if menu.status.text().ok().as_deref() != Some(text.as_str()) {
        let _ = menu.status.set_text(&text);
    }

    // set_enabled crosses into the OS shell, so only touch the pause/resume
    // items when the paused state actually flipped.
    if menu.last_paused != Some(snapshot.paused) {
        let _ = menu.resume.set_enabled(snapshot.paused);
        let _ = menu.pause30.set_enabled(!snapshot.paused);
        let _ = menu.pause60.set_enabled(!snapshot.paused);
        menu.last_paused = Some(snapshot.paused);
    }
}

/// The one-line status the tray shows for a snapshot. Split out so it can be
/// tested without a live Tauri app.
fn status_text(snapshot: &Snapshot) -> String {
    if snapshot.paused {
        format!(
            "Paused — resumes in {}",
            snapshot
                .paused_countdown
                .clone()
                .unwrap_or_else(|| "a moment".into())
        )
    } else if snapshot.engaged {
        "Holding — this PC stays awake".to_string()
    } else if !snapshot.watching {
        "Idle — watching is off".to_string()
    } else if let Some(label) = snapshot.blocker_label.as_deref() {
        format!("Idle — {}", label.to_lowercase())
    } else {
        "Idle — watching for agents".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_line_reads_naturally_in_every_state() {
        let idle = Snapshot {
            watching: true,
            ..Snapshot::default()
        };
        assert_eq!(status_text(&idle), "Idle — watching for agents");

        let off = Snapshot {
            watching: false,
            ..Snapshot::default()
        };
        assert_eq!(status_text(&off), "Idle — watching is off");

        let engaged = Snapshot {
            watching: true,
            engaged: true,
            ..Snapshot::default()
        };
        assert_eq!(status_text(&engaged), "Holding — this PC stays awake");

        let blocked = Snapshot {
            watching: true,
            blocker_label: Some("Battery too low".into()),
            ..Snapshot::default()
        };
        assert_eq!(status_text(&blocked), "Idle — battery too low");
    }

    #[test]
    fn a_paused_status_shows_the_countdown_or_a_placeholder() {
        let counting = Snapshot {
            paused: true,
            paused_countdown: Some("30:00".into()),
            ..Snapshot::default()
        };
        assert_eq!(status_text(&counting), "Paused — resumes in 30:00");

        let no_timer = Snapshot {
            paused: true,
            paused_countdown: None,
            ..Snapshot::default()
        };
        assert_eq!(status_text(&no_timer), "Paused — resumes in a moment");
    }
}
