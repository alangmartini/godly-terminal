mod commands;
mod daemon_client;
mod mcp_server;
mod persistence;
mod pty;
mod state;
mod utils;
mod worktree;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager};

use crate::daemon_client::DaemonClient;
use crate::persistence::{save_on_exit, AutoSaveManager};
use crate::pty::ProcessMonitor;
use crate::state::AppState;

#[cfg(feature = "leak-check")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// Flag to signal that scrollback save is complete
static SCROLLBACK_SAVED: AtomicBool = AtomicBool::new(false);

#[tauri::command]
fn scrollback_save_complete() {
    eprintln!("[lib] Scrollback save complete signal received");
    SCROLLBACK_SAVED.store(true, Ordering::SeqCst);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(feature = "leak-check")]
    let _profiler = dhat::Profiler::new_heap();

    let app_state = Arc::new(AppState::new());
    let auto_save = Arc::new(AutoSaveManager::new());
    let process_monitor = ProcessMonitor::new();

    // Connect to daemon (or launch one)
    let daemon_client = Arc::new(
        DaemonClient::connect_or_launch()
            .expect("Failed to connect to daemon. Run 'npm run build:daemon' first."),
    );
    eprintln!("[lib] Connected to daemon");

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(app_state.clone())
        .manage(auto_save.clone())
        .manage(daemon_client.clone())
        .invoke_handler(tauri::generate_handler![
            commands::create_terminal,
            commands::close_terminal,
            commands::write_to_terminal,
            commands::resize_terminal,
            commands::rename_terminal,
            commands::reconnect_sessions,
            commands::attach_session,
            commands::detach_all_sessions,
            commands::create_workspace,
            commands::delete_workspace,
            commands::get_workspaces,
            commands::move_tab_to_workspace,
            commands::reorder_tabs,
            commands::get_wsl_distributions,
            commands::is_wsl_available,
            commands::toggle_worktree_mode,
            commands::toggle_claude_code_mode,
            commands::is_git_repo,
            commands::list_worktrees,
            commands::remove_worktree,
            commands::cleanup_all_worktrees,
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

            // Start the daemon event bridge (forwards daemon events -> Tauri events)
            // The bridge owns both reader AND writer, doing all pipe I/O from a
            // single thread to avoid Windows named pipe file-object serialization.
            daemon_client
                .setup_bridge(app_handle.clone())
                .expect("Failed to setup daemon bridge");

            // Start keepalive thread — periodically pings the daemon to detect
            // broken connections early (e.g. after system sleep/wake) and trigger
            // reconnection + session re-attachment before the user notices.
            DaemonClient::start_keepalive(daemon_client.clone());

            // Start process monitor (queries daemon for PIDs, resolves process names locally)
            process_monitor.start(app_handle.clone(), state_clone.clone(), daemon_client.clone());

            // Start auto-save manager
            auto_save.start(app_handle.clone(), state_clone.clone());

            // Start MCP pipe server for Claude Code integration
            mcp_server::start_mcp_server(
                app_handle.clone(),
                state_clone.clone(),
                daemon_client.clone(),
                auto_save.clone(),
            );

            // Handle window close: detach sessions (don't kill them) and save layout
            let main_window = app.get_webview_window("main").unwrap();
            let state_for_close = state_clone.clone();
            let handle_for_close = app_handle.clone();
            let window_for_close = main_window.clone();
            let daemon_for_close = daemon_client.clone();

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

                    // Detach all sessions (they keep running in the daemon)
                    eprintln!("[lib] Detaching all sessions...");
                    let terminals = state_for_close.terminals.read();
                    for terminal_id in terminals.keys() {
                        let request = godly_protocol::Request::Detach {
                            session_id: terminal_id.clone(),
                        };
                        let _ = daemon_for_close.send_request(&request);
                    }
                    drop(terminals);

                    // Save layout and close
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
