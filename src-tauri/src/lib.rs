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
use std::time::{Duration, Instant};
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

/// Delete `.old` binaries left in the resource directory from previous upgrades.
/// During builds and installs, locked executables are renamed to `.old` so new
/// binaries can be written. This cleans them up once the old processes have exited.
fn cleanup_old_binaries(app_handle: &tauri::AppHandle) {
    let resource_dir = match app_handle.path().resource_dir() {
        Ok(dir) => dir,
        Err(_) => return,
    };

    for name in &[
        "godly-daemon.exe.old",
        "godly-mcp.exe.old",
        "godly-notify.exe.old",
    ] {
        let path = resource_dir.join(name);
        if path.exists() {
            match std::fs::remove_file(&path) {
                Ok(_) => eprintln!("[lib] Cleaned up {}", name),
                Err(e) => eprintln!("[lib] Could not clean up {} (still locked?): {}", name, e),
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(feature = "leak-check")]
    let _profiler = dhat::Profiler::new_heap();

    // Set Windows timer resolution to 1ms. Without this, thread::sleep(1ms)
    // actually sleeps ~15ms due to the default 15.625ms timer resolution.
    // This is critical for input responsiveness: both the bridge I/O thread
    // and daemon I/O thread use adaptive polling that falls back to sleep(1ms).
    // Arrow keys pressed after a pause hit the sleep penalty on every transition,
    // adding ~30ms of pure sleep overhead to the keystroke round-trip.
    #[cfg(windows)]
    unsafe {
        winapi::um::timeapi::timeBeginPeriod(1);
    }

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
            commands::sync_active_terminal,
            commands::detach_all_sessions,
            commands::create_workspace,
            commands::delete_workspace,
            commands::get_workspaces,
            commands::move_tab_to_workspace,
            commands::reorder_tabs,
            commands::set_split_view,
            commands::clear_split_view,
            commands::get_mcp_state,
            commands::get_wsl_distributions,
            commands::is_wsl_available,
            commands::is_pwsh_available,
            commands::toggle_worktree_mode,
            commands::toggle_claude_code_mode,
            commands::is_git_repo,
            commands::list_worktrees,
            commands::remove_worktree,
            commands::cleanup_all_worktrees,
            commands::read_file,
            commands::write_file,
            commands::get_user_claude_md_path,
            commands::get_sounds_dir,
            commands::list_custom_sounds,
            commands::read_sound_file,
            commands::get_grid_snapshot,
            commands::get_grid_snapshot_diff,
            commands::get_grid_dimensions,
            commands::get_grid_text,
            commands::set_scrollback,
            commands::write_frontend_log,
            commands::get_log_dir,
            commands::read_frontend_log,
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

            // Get the non-blocking event emitter for the process monitor
            let emitter = daemon_client.event_emitter();

            // Start process monitor (queries daemon for PIDs, resolves process names locally)
            process_monitor.start(app_handle.clone(), emitter, state_clone.clone(), daemon_client.clone());

            // Start auto-save manager
            auto_save.start(app_handle.clone(), state_clone.clone());

            // Clean up .old binaries left from previous upgrades.
            // During builds/installs, locked .exe files are renamed to .old so
            // new binaries can be written. Delete them now if the old processes
            // have exited.
            cleanup_old_binaries(&app_handle);

            // Copy bundled sounds to user's sounds directory (first run)
            commands::install_bundled_sounds(&app_handle);

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
                                eprintln!("[lib] Scrollback save timeout, proceeding with close");
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(50));
                        }

                        // Detach all sessions (they keep running in the daemon)
                        eprintln!("[lib] Detaching all sessions...");
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
                        eprintln!("[lib] Destroying window...");
                        let _ = window.destroy();
                    });
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
