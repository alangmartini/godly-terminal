//! Window lifecycle management: close handler with scrollback save coordination
//! and graceful session detachment.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::Emitter;

use crate::daemon_client::DaemonClient;
use crate::persistence::save_on_exit;
use crate::state::AppState;

/// Flag to signal that scrollback save is complete.
static SCROLLBACK_SAVED: AtomicBool = AtomicBool::new(false);

/// Called by the frontend after it finishes persisting scrollback data.
#[tauri::command]
pub(crate) fn scrollback_save_complete() {
    eprintln!("[window_lifecycle] Scrollback save complete signal received");
    SCROLLBACK_SAVED.store(true, Ordering::SeqCst);
}

/// Register the window close handler on the main window.
///
/// On close: requests frontend scrollback save, waits (up to 3s), detaches
/// all daemon sessions so they survive the app exit, saves layout, then
/// destroys the window.
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
            // Prevent immediate close
            api.prevent_close();

            // Reset the flag
            SCROLLBACK_SAVED.store(false, Ordering::SeqCst);

            // Request frontend to save scrollbacks
            eprintln!("[window_lifecycle] Requesting frontend to save scrollbacks...");
            let _ = window_for_close.emit("request-scrollback-save", ());

            // Move the blocking wait to a background thread so the main
            // thread stays free to process the frontend's IPC callback
            // (scrollback_save_complete). Previously this busy-waited on
            // the main thread, deadlocking because the callback could
            // never be dispatched.
            let state = state_for_close.clone();
            let daemon = daemon_for_close.clone();
            let handle = handle_for_close.clone();
            let window = window_for_close.clone();
            std::thread::spawn(move || {
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

                // Save layout and close
                save_on_exit(&handle, &state);
                eprintln!("[window_lifecycle] Destroying window...");
                let _ = window.destroy();
            });
        }
    });
}
