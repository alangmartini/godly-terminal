//! Window lifecycle management: close handler with scrollback save coordination
//! and graceful session detachment.
//!
//! When the user closes the window, the frontend is first asked whether to show
//! a confirmation dialog (based on active session count and user settings).
//! If confirmed (or skipped), scrollback is saved, sessions are detached, and
//! the window is destroyed.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::Emitter;

use crate::daemon_client::DaemonClient;
use crate::persistence::save_on_exit;
use crate::state::{AppState, WindowState};

/// Flag to signal that scrollback save is complete.
static SCROLLBACK_SAVED: AtomicBool = AtomicBool::new(false);

/// Flag to signal that the user confirmed the quit (or the frontend skipped the dialog).
static QUIT_CONFIRMED: AtomicBool = AtomicBool::new(false);

/// Flag to signal that the user cancelled the quit.
static QUIT_CANCELLED: AtomicBool = AtomicBool::new(false);

/// Flag to prevent re-entrant close handling while a confirm dialog is showing.
static CLOSE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Payload sent with the `confirm-quit` event so the frontend knows how many
/// active sessions would be lost.
#[derive(Clone, Serialize)]
struct ConfirmQuitPayload {
    active_session_count: usize,
}

/// Called by the frontend after it finishes persisting scrollback data.
#[tauri::command]
pub(crate) fn scrollback_save_complete() {
    eprintln!("[window_lifecycle] Scrollback save complete signal received");
    SCROLLBACK_SAVED.store(true, Ordering::SeqCst);
}

/// Called by the frontend when the user confirms the quit (or the setting is
/// disabled / no active sessions).
#[tauri::command]
pub(crate) fn confirm_quit() {
    eprintln!("[window_lifecycle] Quit confirmed by frontend");
    QUIT_CONFIRMED.store(true, Ordering::SeqCst);
}

/// Called by the frontend when the user cancels the quit dialog.
#[tauri::command]
pub(crate) fn cancel_quit() {
    eprintln!("[window_lifecycle] Quit cancelled by frontend");
    QUIT_CANCELLED.store(true, Ordering::SeqCst);
}

/// Register the window close handler on the main window.
///
/// Flow:
/// 1. `CloseRequested` → prevent close, emit `confirm-quit` to frontend
/// 2. Frontend checks settings/session count → calls `confirm_quit` or `cancel_quit`
/// 3. If confirmed: emit `request-scrollback-save`, wait, detach, save, destroy
/// 4. If cancelled: reset state, window stays open
pub(crate) fn setup_window_close_handler(
    window: &tauri::WebviewWindow,
    state: Arc<AppState>,
    daemon: Arc<DaemonClient>,
    app_handle: tauri::AppHandle,
) {
    let state_for_close = state;
    let handle_for_close = app_handle;
    let window_for_close = window.clone();
    let daemon_for_close = daemon;

    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            // Prevent re-entrant close while a dialog is already showing
            if CLOSE_IN_PROGRESS.swap(true, Ordering::SeqCst) {
                api.prevent_close();
                return;
            }

            // Prevent immediate close — we need to ask the frontend first
            api.prevent_close();

            // Reset all flags
            SCROLLBACK_SAVED.store(false, Ordering::SeqCst);
            QUIT_CONFIRMED.store(false, Ordering::SeqCst);
            QUIT_CANCELLED.store(false, Ordering::SeqCst);

            // Count active sessions
            let active_session_count = state_for_close.terminals.read().len();

            // Emit confirm-quit event to frontend with session count
            eprintln!(
                "[window_lifecycle] Requesting quit confirmation ({} active sessions)...",
                active_session_count
            );
            let _ = window_for_close.emit(
                "confirm-quit",
                ConfirmQuitPayload {
                    active_session_count,
                },
            );

            // Spawn background thread to wait for frontend decision
            let state = state_for_close.clone();
            let daemon = daemon_for_close.clone();
            let handle = handle_for_close.clone();
            let window = window_for_close.clone();
            std::thread::spawn(move || {
                // Wait for confirm or cancel (max 30s — generous timeout for user interaction)
                let start = Instant::now();
                loop {
                    if QUIT_CONFIRMED.load(Ordering::SeqCst) {
                        break;
                    }
                    if QUIT_CANCELLED.load(Ordering::SeqCst) {
                        eprintln!("[window_lifecycle] Quit cancelled, window stays open");
                        CLOSE_IN_PROGRESS.store(false, Ordering::SeqCst);
                        return;
                    }
                    if start.elapsed() > Duration::from_secs(30) {
                        eprintln!("[window_lifecycle] Quit confirmation timeout, proceeding with close");
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }

                // --- Confirmed: proceed with graceful shutdown ---

                // Request frontend to save scrollbacks
                eprintln!("[window_lifecycle] Requesting frontend to save scrollbacks...");
                let _ = window.emit("request-scrollback-save", ());

                // Wait for scrollback save to complete (max 3 seconds)
                let start = Instant::now();
                while !SCROLLBACK_SAVED.load(Ordering::SeqCst) {
                    if start.elapsed() > Duration::from_secs(3) {
                        eprintln!("[window_lifecycle] Scrollback save timeout, proceeding with close");
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }

                // Detach all sessions (they keep running in the daemon)
                eprintln!("[window_lifecycle] Detaching all sessions...");
                let terminals = state.terminals.read();
                for terminal_id in terminals.keys() {
                    let request = godly_protocol::Request::Detach {
                        session_id: terminal_id.clone(),
                    };
                    let _ = daemon.send_request(&request);
                }
                drop(terminals);

                // Capture window geometry and current monitor before saving
                capture_window_state(&window, &state);

                // Save layout and close
                save_on_exit(&handle, &state);
                CLOSE_IN_PROGRESS.store(false, Ordering::SeqCst);
                eprintln!("[window_lifecycle] Destroying window...");
                let _ = window.destroy();
            });
        }
    });
}

/// Snapshot the window's position, size, maximized flag, and current monitor name
/// into `AppState::window_state` so it can be persisted on exit.
fn capture_window_state(window: &tauri::WebviewWindow, state: &AppState) {
    let position = match window.outer_position() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[window_lifecycle] Failed to get window position: {}", e);
            return;
        }
    };
    let size = match window.outer_size() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[window_lifecycle] Failed to get window size: {}", e);
            return;
        }
    };
    let maximized = window.is_maximized().unwrap_or(false);
    let monitor_name = window
        .current_monitor()
        .ok()
        .flatten()
        .and_then(|m| m.name().map(|n| n.to_string()));

    let ws = WindowState {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        maximized,
        monitor_name,
    };

    eprintln!(
        "[window_lifecycle] Captured window state: {}x{} at ({},{}) maximized={} monitor={:?}",
        ws.width, ws.height, ws.x, ws.y, ws.maximized, ws.monitor_name
    );

    *state.window_state.write() = Some(ws);
}
