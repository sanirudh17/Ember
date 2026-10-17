//! Tauri commands — the entire surface the React frontend can call.
//!
//! Commands are deliberately thin: they mutate engine state, mark it dirty and
//! return. All window work (title, overlay icon, backdrop, theme,
//! always-on-top) happens on the window shell's own loop, because those calls
//! need the main thread and would otherwise have to be marshalled twice.

use std::sync::MutexGuard;
use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager, State, WebviewWindow};

use crate::Ember;
use crate::agents::AgentId;
use crate::agents::installers::{self, ChangePreview, InstallResult, Op};
use crate::engine::{Engine, Snapshot, until::UntilTarget};
use crate::settings::{Chime, Settings};
use crate::sound;

fn engine_of<'a>(state: &'a State<'_, Ember>) -> Result<MutexGuard<'a, Engine>, String> {
    state
        .engine
        .lock()
        .map_err(|_| "Ember's engine is unavailable".to_string())
}

fn parse_agent(id: &str) -> Result<AgentId, String> {
    AgentId::parse(id).ok_or_else(|| format!("unknown agent {id:?}"))
}

fn parse_op(op: &str) -> Result<Op, String> {
    match op {
        "install" => Ok(Op::Install),
        "remove" => Ok(Op::Remove),
        other => Err(format!("unknown operation {other:?}")),
    }
}

fn commit(engine: &Engine) -> Result<Settings, String> {
    engine.settings.save()?;
    Ok(engine.settings.clone())
}

// ---------------------------------------------------------------------------
// Reading state
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_snapshot(state: State<'_, Ember>) -> Result<Snapshot, String> {
    Ok(engine_of(&state)?.current_snapshot())
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn update_settings(
    state: State<'_, Ember>,
    mut settings: Settings,
) -> Result<Settings, String> {
    let mut engine = engine_of(&state)?;
    settings.clamp();
    // Window geometry is engine-owned (it tracks moves and resizes directly)
    // and must not be clobbered by a stale copy in the incoming settings doc.
    settings.window = engine.settings.window.clone();
    engine.settings = settings;
    engine.dirty = true;
    commit(&engine)
}

#[tauri::command]
pub fn set_watching(state: State<'_, Ember>, enabled: bool) -> Result<Settings, String> {
    let mut engine = engine_of(&state)?;
    engine.settings.watching = enabled;
    engine.dirty = true;
    commit(&engine)
}

// ---------------------------------------------------------------------------
// The manual session
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn start_manual(state: State<'_, Ember>, seconds: u64) -> Result<(), String> {
    engine_of(&state)?.start_manual_secs(seconds);
    Ok(())
}

#[tauri::command]
pub fn start_manual_until(
    state: State<'_, Ember>,
    hour: u32,
    minute: u32,
) -> Result<UntilTarget, String> {
    Ok(engine_of(&state)?.start_manual_until(hour, minute))
}

#[tauri::command]
pub fn start_manual_unlimited(state: State<'_, Ember>) -> Result<(), String> {
    engine_of(&state)?.start_manual_unlimited();
    Ok(())
}

#[tauri::command]
pub fn stop_manual(state: State<'_, Ember>) -> Result<(), String> {
    engine_of(&state)?.stop_manual();
    Ok(())
}

#[tauri::command]
pub fn extend_manual(state: State<'_, Ember>, seconds: u64) -> Result<(), String> {
    engine_of(&state)?.extend_manual(seconds);
    Ok(())
}

#[tauri::command]
pub fn set_paused(state: State<'_, Ember>, paused: bool) -> Result<(), String> {
    engine_of(&state)?.set_paused(paused);
    Ok(())
}

#[tauri::command]
pub fn pause_for(state: State<'_, Ember>, seconds: u64) -> Result<(), String> {
    engine_of(&state)?.pause_for(seconds);
    Ok(())
}

// ---------------------------------------------------------------------------
// Lid / recovery
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn release_lid_now(state: State<'_, Ember>) -> Result<(), String> {
    engine_of(&state)?.release_lid_now();
    Ok(())
}

#[tauri::command]
pub fn clear_lid_cap(state: State<'_, Ember>) -> Result<(), String> {
    engine_of(&state)?.clear_lid_cap();
    Ok(())
}

// ---------------------------------------------------------------------------
// Agent installers
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn preview_install(agent: String, op: String) -> Result<ChangePreview, String> {
    let agent = parse_agent(&agent)?;
    installers::preview(agent, parse_op(&op)?)
}

#[tauri::command]
pub fn apply_install(
    state: State<'_, Ember>,
    agent: String,
    op: String,
) -> Result<InstallResult, String> {
    let agent = parse_agent(&agent)?;
    let result = installers::apply(agent, parse_op(&op)?)?;
    engine_of(&state)?.force_agent_refresh();
    Ok(result)
}

#[tauri::command]
pub fn refresh_agents(state: State<'_, Ember>) -> Result<(), String> {
    engine_of(&state)?.force_agent_refresh();
    Ok(())
}

// ---------------------------------------------------------------------------
// Notifications
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn test_chime(state: State<'_, Ember>, chime: Chime) -> Result<(), String> {
    let engine = engine_of(&state)?;
    if engine.settings.notifications.mute {
        return Err("Chimes are muted".to_string());
    }
    sound::play(chime, crate::settings::NOTIFY_VOLUME);
    Ok(())
}

#[tauri::command]
pub fn test_notification(
    app: AppHandle,
    state: State<'_, Ember>,
    kind: String,
) -> Result<(), String> {
    let engine = engine_of(&state)?;
    let notifications = engine.settings.notifications.clone();
    drop(engine);
    let notify = match kind.as_str() {
        "engage" => crate::engine::Notify::Engaged,
        "finish" => crate::engine::Notify::Finished,
        "battery" => crate::engine::Notify::Battery,
        other => return Err(format!("unknown notification {other:?}")),
    };
    crate::shell::preview(&app, &notifications, notify)
}

// ---------------------------------------------------------------------------
// Window
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn hide_window(window: WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn show_window(window: WebviewWindow) -> Result<(), String> {
    window.show().map_err(|e| e.to_string())?;
    let _ = window.unminimize();
    let _ = window.set_focus();
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: AppHandle, state: State<'_, Ember>) -> Result<(), String> {
    // Restore the lid action before we go, so a normal quit never leaves the
    // machine with a changed power plan.
    if let Ok(mut engine) = state.engine.lock() {
        engine.release_lid_now();
    }
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn mark_closing(state: State<'_, Ember>) {
    state.closing.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub fn reset_window(window: WebviewWindow, state: State<'_, Ember>) -> Result<(), String> {
    if window.is_maximized().unwrap_or(false) {
        let _ = window.unmaximize();
    }
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
    let settings = {
        let mut engine = engine_of(&state)?;
        engine.settings.window = crate::settings::WindowState {
            x: None,
            y: None,
            placed: false,
            width: 920,
            height: 640,
            maximized: false,
        };
        engine.settings.save()?;
        engine.dirty = true;
        engine.settings.clone()
    };
    crate::shell::place_window(&window, &settings);
    Ok(())
}

#[tauri::command]
pub fn get_recent_log(lines: Option<usize>) -> String {
    crate::log::read_recent(lines.unwrap_or(200))
}

/// Ask the shell to re-check whether the window should be visible.
#[tauri::command]
pub fn focus_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window(crate::shell::MAIN_WINDOW) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// The custom title bar's buttons. Doing this in Rust keeps the frontend's
/// capability surface to a single drag-region permission.
#[tauri::command]
pub fn window_control(window: WebviewWindow, action: String) -> Result<bool, String> {
    match action.as_str() {
        "minimize" => window.minimize().map_err(|e| e.to_string())?,
        "toggle_maximize" => {
            if window.is_maximized().unwrap_or(false) {
                window.unmaximize().map_err(|e| e.to_string())?;
            } else {
                window.maximize().map_err(|e| e.to_string())?;
            }
        }
        // Closing funnels through `CloseRequested`, so the "Keep running or
        // quit?" inline confirm still applies.
        "close" => window.close().map_err(|e| e.to_string())?,
        other => return Err(format!("unknown window action {other:?}")),
    }
    Ok(window.is_maximized().unwrap_or(false))
}

/// Edge and corner resize dragging for the frameless window via Win32 non-client hit-testing.
#[tauri::command]
pub fn window_resize(window: WebviewWindow, direction: String) -> Result<(), String> {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT,
        PostMessageW, WM_NCLBUTTONDOWN,
    };

    let ht = match direction.as_str() {
        "n" => HTTOP,
        "s" => HTBOTTOM,
        "e" => HTRIGHT,
        "w" => HTLEFT,
        "ne" => HTTOPRIGHT,
        "nw" => HTTOPLEFT,
        "se" => HTBOTTOMRIGHT,
        "sw" => HTBOTTOMLEFT,
        other => return Err(format!("unknown resize direction {other:?}")),
    };
    let handle = window.hwnd().map_err(|e| e.to_string())?;
    let hwnd = HWND(handle.0 as isize as *mut core::ffi::c_void);
    unsafe {
        PostMessageW(Some(hwnd), WM_NCLBUTTONDOWN, WPARAM(ht as usize), LPARAM(0))
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn engine_is_armed(state: State<'_, Ember>) -> Result<bool, String> {
    let engine = engine_of(&state)?;
    Ok(engine.manual.active || engine.sleep_blocker().is_engaged())
}

// ---------------------------------------------------------------------------
// Updates
// ---------------------------------------------------------------------------

/// The newer signed release GitHub is offering, in the shape the UI needs.
#[derive(serde::Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
}

/// Download progress relayed to the UI while an installer is being fetched.
#[derive(serde::Serialize, Clone)]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

/// The version this copy of Ember was built as.
#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Ask GitHub whether a newer signed release exists. `Ok(None)` means this
/// copy is the newest one; it never installs anything on its own. Failures
/// the user cannot act on — a repository with no published release yet —
/// read as "up to date"; the rest arrive as a plain sentence, with the
/// technical detail kept in the log.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    use tauri_plugin_updater::UpdaterExt;

    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(found) => Ok(found.map(|update| UpdateInfo {
            version: update.version.clone(),
            current_version: update.current_version.clone(),
            notes: update.body.clone(),
        })),
        Err(err) => match update_check_note(&err) {
            Some(note) => Err(note),
            None => Ok(None),
        },
    }
}

/// Turn a failed update check into what the user should read.
///
/// A missing release manifest — a repository with no published release yet
/// answers with a 404 that the updater reports as `ReleaseNotFound` — is not
/// worth showing: nothing newer exists. `None` means "treat this as up to
/// date"; `Some(note)` is a plain-English line for the UI. Either way the
/// raw error goes to the log for troubleshooting.
fn update_check_note(err: &tauri_plugin_updater::Error) -> Option<String> {
    use tauri_plugin_updater::Error as UpdaterError;

    if matches!(err, UpdaterError::ReleaseNotFound) {
        crate::log::append("UPDATE", "no release manifest at the endpoint (nothing published yet)");
        return None;
    }

    crate::log::append("UPDATE", &format!("update check failed: {err}"));
    Some(match err {
        UpdaterError::Reqwest(e) if e.is_connect() || e.is_timeout() || e.is_request() => {
            "Couldn't reach GitHub. Check your connection and try again.".to_string()
        }
        _ => "The update check didn't finish. Ember will try again on its own.".to_string(),
    })
}

/// The plain-English line for a failed install; the raw error goes to the log.
fn update_install_note(err: &tauri_plugin_updater::Error) -> String {
    use tauri_plugin_updater::Error as UpdaterError;

    crate::log::append("UPDATE", &format!("update install failed: {err}"));
    match err {
        UpdaterError::Reqwest(e) if e.is_connect() || e.is_timeout() || e.is_request() => {
            "The download didn't finish. Check your connection and try again."
        }
        UpdaterError::Network(_) => "The download didn't finish. Try again in a few minutes.",
        UpdaterError::Minisign(_)
        | UpdaterError::Base64(_)
        | UpdaterError::SignatureUtf8(_)
        | UpdaterError::SignedVersionMismatch { .. }
        | UpdaterError::MissingSignedVersion => {
            "The update could not be verified, so Ember did not install it."
        }
        _ => "The update didn't install. Ember is still running the current version.",
    }
    .to_string()
}

/// Download the newest release, verify its signature, run the installer and
/// restart into it. The UI receives `update-progress` events meanwhile.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    use tauri::Emitter;
    use tauri_plugin_updater::UpdaterExt;

    let updater = app.updater().map_err(|e| e.to_string())?;
    let found = match updater.check().await {
        Ok(found) => found,
        Err(err) => {
            crate::log::append("UPDATE", &format!("update install aborted at check: {err}"));
            return Err("The update couldn't be fetched. Ember will try again later.".into());
        }
    };

    if let Some(update) = found {
        let progress_app = app.clone();
        update
            .download_and_install(
                move |chunk, total| {
                    let _ = progress_app.emit(
                        "update-progress",
                        UpdateProgress {
                            downloaded: chunk as u64,
                            total,
                        },
                    );
                },
                || {},
            )
            .await
            .map_err(|e| update_install_note(&e))?;
        // The NSIS installer replaces Ember and relaunches it; nothing after
        // this call is reached.
        app.restart();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_updater::Error as UpdaterError;

    #[test]
    fn a_missing_release_is_not_shown_as_an_error() {
        assert_eq!(update_check_note(&UpdaterError::ReleaseNotFound), None);
    }

    #[test]
    fn other_check_failures_read_plainly() {
        let note = update_check_note(&UpdaterError::EmptyEndpoints)
            .expect("a broken check should still say something");
        assert_eq!(
            note,
            "The update check didn't finish. Ember will try again on its own."
        );
    }

    #[test]
    fn install_failures_read_plainly() {
        assert_eq!(
            update_install_note(&UpdaterError::EmptyEndpoints),
            "The update didn't install. Ember is still running the current version."
        );
    }
}
