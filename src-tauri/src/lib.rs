mod commands;
mod persistence;
mod pty;
mod state;
mod utils;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager};

use crate::persistence::{save_on_exit, AutoSaveManager};
use crate::pty::ProcessMonitor;
use crate::state::AppState;

/// Flag to signal that scrollback save is complete
static SCROLLBACK_SAVED: AtomicBool = AtomicBool::new(false);

#[tauri::command]
fn scrollback_save_complete() {
    eprintln!("[lib] Scrollback save complete signal received");
    SCROLLBACK_SAVED.store(true, Ordering::SeqCst);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = Arc::new(AppState::new());
    let process_monitor = ProcessMonitor::new();
    let auto_save = Arc::new(AutoSaveManager::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state.clone())
        .manage(auto_save.clone())
        .invoke_handler(tauri::generate_handler![
            commands::create_terminal,
            commands::close_terminal,
            commands::write_to_terminal,
            commands::resize_terminal,
            commands::rename_terminal,
            commands::create_workspace,
            commands::delete_workspace,
            commands::get_workspaces,
            commands::move_tab_to_workspace,
            commands::reorder_tabs,
            commands::get_wsl_distributions,
            commands::is_wsl_available,
            persistence::save_layout,
            persistence::load_layout,
            persistence::save_scrollback,
            persistence::load_scrollback,
            persistence::delete_scrollback,
            scrollback_save_complete,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let state_clone = app_state.clone();

            // Start process monitor
            process_monitor.start(app_handle.clone(), state_clone.clone());

            // Start auto-save manager
            auto_save.start(app_handle.clone(), state_clone.clone());

            // Save layout on window close
            let main_window = app.get_webview_window("main").unwrap();
            let state_for_close = state_clone.clone();
            let handle_for_close = app_handle.clone();
            let window_for_close = main_window.clone();

            main_window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    // Prevent immediate close
                    api.prevent_close();

                    // Reset the flag
                    SCROLLBACK_SAVED.store(false, Ordering::SeqCst);

                    // Request frontend to save scrollbacks
                    eprintln!("[lib] Requesting frontend to save scrollbacks...");
                    let _ = window_for_close.emit("request-scrollback-save", ());

                    // Wait for scrollback save to complete (max 3 seconds)
                    let start = std::time::Instant::now();
                    while !SCROLLBACK_SAVED.load(Ordering::SeqCst) {
                        if start.elapsed() > Duration::from_secs(3) {
                            eprintln!("[lib] Scrollback save timeout, proceeding with close");
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }

                    // Now save layout and close
                    save_on_exit(&handle_for_close, &state_for_close);
                    eprintln!("[lib] Destroying window...");
                    let _ = window_for_close.destroy();
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
