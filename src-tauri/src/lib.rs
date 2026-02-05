mod commands;
mod persistence;
mod pty;
mod state;

use std::sync::Arc;
use tauri::Manager;

use crate::persistence::save_on_exit;
use crate::pty::ProcessMonitor;
use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = Arc::new(AppState::new());
    let process_monitor = ProcessMonitor::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state.clone())
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
            persistence::save_layout,
            persistence::load_layout,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let state_clone = app_state.clone();

            // Start process monitor
            process_monitor.start(app_handle.clone(), state_clone.clone());

            // Save layout on window close
            let main_window = app.get_webview_window("main").unwrap();
            let state_for_close = state_clone.clone();
            let handle_for_close = app_handle.clone();

            main_window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    save_on_exit(&handle_for_close, &state_for_close);
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
