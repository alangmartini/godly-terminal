mod commands;
mod custom_protocols;
mod daemon_client;
mod gpu_renderer;
mod llm_state;
mod mcp_server;
mod persistence;
mod pty;
mod sidecar;
mod state;
mod utils;
mod whisper_client;
mod whisper_state;
mod window_lifecycle;
mod worktree;

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::Mutex;
use tauri::Manager;

use crate::daemon_client::bridge::OutputStreamRegistry;
use crate::daemon_client::DaemonClient;
use crate::gpu_renderer::GpuRendererManager;
use crate::llm_state::LlmState;
use crate::persistence::AutoSaveManager;
use crate::pty::ProcessMonitor;
use crate::state::AppState;
use crate::whisper_state::WhisperState;

#[cfg(feature = "leak-check")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// Shared state for JS execution callback channels
pub struct JsCallbackState {
    pub senders: Mutex<HashMap<String, std::sync::mpsc::Sender<(Option<String>, Option<String>)>>>,
}

/// Callback from frontend JS execution — receives the result of execute_js
#[tauri::command]
fn mcp_js_result(
    id: String,
    result: Option<String>,
    error: Option<String>,
    js_state: tauri::State<'_, JsCallbackState>,
) {
    if let Some(tx) = js_state.senders.lock().remove(&id) {
        let _ = tx.send((result, error));
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
    let llm_state = Arc::new(LlmState::new());
    let whisper_state = Arc::new(WhisperState::new());

    // Connect to daemon (or launch one)
    let daemon_client = Arc::new(
        DaemonClient::connect_or_launch()
            .expect("Failed to connect to daemon. Run 'npm run build:daemon' first."),
    );
    eprintln!("[lib] Connected to daemon");

    // Create per-session output stream registry for the custom protocol.
    // Raw PTY bytes flow: daemon → bridge I/O thread → registry → stream:// fetch.
    // This bypasses app_handle.emit() JSON serialization on the output hot path.
    let output_registry = Arc::new(OutputStreamRegistry::new());
    daemon_client.set_output_registry(output_registry.clone());

    // Clone for the custom protocol closure (captured by move)
    let registry_for_protocol = output_registry.clone();

    // Dedicated worker pool for stream:// protocol responses (2 threads).
    // The WebView2 WebResourceRequested callback runs on the main thread, so
    // the synchronous register_uri_scheme_protocol variant blocks the main
    // thread until the response is built. Under load (rapid terminal creation
    // saturating IPC), stream:// fetches time out ("Failed to fetch"), blanking
    // all terminals. By using the async variant + a dedicated worker pool, the
    // handler returns immediately and a pool thread calls responder.respond().
    type StreamJob = Box<dyn FnOnce() + Send>;
    let (stream_tx, stream_rx) = {
        let (tx, rx) = std::sync::mpsc::sync_channel::<StreamJob>(256);
        let rx = Arc::new(std::sync::Mutex::new(rx));
        for i in 0..2 {
            let rx = rx.clone();
            std::thread::Builder::new()
                .name(format!("stream-proto-{}", i))
                .spawn(move || loop {
                    let job = {
                        let guard = rx.lock().unwrap();
                        guard.recv()
                    };
                    match job {
                        Ok(f) => f(),
                        Err(_) => break, // channel closed
                    }
                })
                .expect("Failed to spawn stream protocol worker");
        }
        (tx, rx)
    };
    // Keep rx alive for the lifetime of the app (workers hold Arc clones)
    let _stream_rx_keepalive = stream_rx;

    // GPU renderer manager — pre-warmed on background thread during startup.
    // wgpu device init takes ~500ms; running it here means it completes
    // concurrently with window creation and is ready before first render.
    let gpu_renderer_manager = Arc::new(GpuRendererManager::new("Cascadia Code", 14.0));
    gpu_renderer_manager.warm();

    // Clones for the gpuframe:// custom protocol closure
    let gpu_for_protocol = gpu_renderer_manager.clone();
    let daemon_for_gpuframe = daemon_client.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        // Register custom protocol for streaming terminal output as raw bytes.
        // Frontend fetches: http://stream.localhost/terminal-output/{session_id}
        // Returns accumulated raw PTY bytes since last fetch (application/octet-stream).
        .register_asynchronous_uri_scheme_protocol("stream", move |_ctx, request, responder| {
            let registry = registry_for_protocol.clone();
            let path = request.uri().path().to_string();
            let _ = stream_tx.try_send(Box::new(move || {
                // Expected path: /terminal-output/{session_id}
                let response = if let Some(session_id) = path.strip_prefix("/terminal-output/") {
                    if session_id.is_empty() {
                        tauri::http::Response::builder()
                            .status(400)
                            .header("Access-Control-Allow-Origin", "*")
                            .body(b"Missing session_id".to_vec())
                            .unwrap()
                    } else {
                        let bytes = registry.drain(session_id);
                        tauri::http::Response::builder()
                            .status(200)
                            .header("Content-Type", "application/octet-stream")
                            .header("Access-Control-Allow-Origin", "*")
                            .body(bytes)
                            .unwrap()
                    }
                } else {
                    tauri::http::Response::builder()
                        .status(404)
                        .header("Access-Control-Allow-Origin", "*")
                        .body(b"Not found. Use /terminal-output/{session_id}".to_vec())
                        .unwrap()
                };
                responder.respond(response);
            }));
        })
        // Register custom protocol for GPU-rendered terminal frames as PNG images.
        // Frontend fetches: http://gpuframe.localhost/render/{session_id}
        // Returns a PNG image of the terminal rendered by the GPU pipeline.
        // When the gpu-renderer feature is disabled, always returns 501.
        .register_asynchronous_uri_scheme_protocol("gpuframe", move |_ctx, request, responder| {
            let gpu = gpu_for_protocol.clone();
            let daemon = daemon_for_gpuframe.clone();
            // Capture full URI (path + query) so the handler can parse ?format=raw
            let uri = request.uri();
            let full_uri = if let Some(query) = uri.query() {
                format!("{}?{}", uri.path(), query)
            } else {
                uri.path().to_string()
            };

            // Offload to a background thread — GPU rendering can take a few ms
            // and we must not block the WebView2 main thread.
            std::thread::spawn(move || {
                let response = custom_protocols::handle_gpuframe_request(&full_uri, &gpu, &daemon);
                responder.respond(response);
            });
        })
        .manage(app_state.clone())
        .manage(auto_save.clone())
        .manage(daemon_client.clone())
        .manage(llm_state.clone())
        .manage(whisper_state.clone())
        .manage(gpu_renderer_manager)
        .manage(JsCallbackState {
            senders: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_terminal,
            commands::close_terminal,
            commands::write_to_terminal,
            commands::resize_terminal,
            commands::rename_terminal,
            commands::reconnect_sessions,
            commands::attach_session,
            commands::sync_active_terminal,
            commands::quick_claude,
            commands::pause_session,
            commands::resume_session,
            commands::detach_all_sessions,
            commands::create_workspace,
            commands::delete_workspace,
            commands::get_workspaces,
            commands::move_tab_to_workspace,
            commands::reorder_tabs,
            commands::set_split_view,
            commands::clear_split_view,
            commands::split_terminal,
            commands::unsplit_terminal,
            commands::get_layout_tree,
            commands::swap_panes,
            commands::set_layout_tree,
            commands::prune_stale_terminal_ids,
            commands::get_wsl_distributions,
            commands::is_wsl_available,
            commands::is_pwsh_available,
            commands::get_cmd_aliases_path,
            commands::ensure_cmd_autorun,
            commands::toggle_worktree_mode,
            commands::toggle_claude_code_mode,
            commands::is_git_repo,
            commands::list_worktrees,
            commands::remove_worktree,
            commands::cleanup_all_worktrees,
            commands::list_skills,
            commands::list_directory,
            commands::read_file,
            commands::write_file,
            commands::get_user_claude_md_path,
            commands::get_sounds_dir,
            commands::list_custom_sounds,
            commands::read_sound_file,
            commands::list_sound_packs,
            commands::list_sound_pack_files,
            commands::read_sound_pack_file,
            commands::get_sound_packs_dir,
            commands::install_sound_pack,
            commands::delete_sound_pack,
            commands::get_plugins_dir,
            commands::list_installed_plugins,
            commands::read_plugin_js,
            commands::read_plugin_icon,
            commands::install_plugin,
            commands::uninstall_plugin,
            commands::check_plugin_update,
            commands::fetch_plugin_registry,
            commands::gpu_render::gpu_renderer_available,
            commands::gpu_render::render_terminal_gpu,
            commands::gpu_render::get_gpu_cell_size,
            commands::get_grid_snapshot,
            commands::get_grid_snapshot_diff,
            commands::get_grid_dimensions,
            commands::get_grid_text,
            commands::set_scrollback,
            commands::scroll_and_get_snapshot,
            commands::write_frontend_log,
            commands::get_log_dir,
            commands::read_frontend_log,
            commands::llm_has_api_key,
            commands::llm_set_api_key,
            commands::llm_set_model,
            commands::llm_get_model,
            commands::llm_generate_branch_name,
            commands::whisper_get_status,
            commands::whisper_start_recording,
            commands::whisper_stop_recording,
            commands::whisper_load_model,
            commands::whisper_list_models,
            commands::whisper_get_config,
            commands::whisper_set_config,
            commands::whisper_start_sidecar,
            commands::whisper_restart_sidecar,
            commands::whisper_download_model,
            commands::list_gpu_devices,
            commands::whisper_list_audio_devices,
            commands::whisper_playback_recording,
            commands::whisper_get_audio_level,
            persistence::save_layout,
            persistence::load_layout,
            persistence::save_scrollback,
            persistence::load_scrollback,
            persistence::delete_scrollback,
            commands::save_clipboard_image,
            commands::write_remote_config,
            window_lifecycle::scrollback_save_complete,
            mcp_js_result,
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
            sidecar::cleanup_old_binaries(&app_handle);

            // Copy bundled sounds to user's sounds directory (first run)
            commands::install_bundled_sounds(&app_handle);

            // Copy bundled sound packs (first run)
            commands::install_bundled_sound_packs(&app_handle);

            // Initialize whisper state
            if let Ok(app_data) = app_handle.path().app_data_dir() {
                whisper_state.init(app_data);
            }

            // Start MCP pipe server for Claude Code integration
            mcp_server::start_mcp_server(
                app_handle.clone(),
                state_clone.clone(),
                daemon_client.clone(),
                auto_save.clone(),
                llm_state.clone(),
            );

            // Start MCP HTTP server for Streamable HTTP transport
            sidecar::start_mcp_http_server(&app_handle);

            // Start Remote HTTP server for phone remote control
            sidecar::start_remote_http_server(&app_handle);

            // Handle window close: detach sessions (don't kill them) and save layout
            let main_window = app.get_webview_window("main").unwrap();

            // Disable WebView2's built-in zoom control. Without this, Ctrl+scroll
            // triggers native browser zoom in addition to our font-size zoom handler,
            // causing the content to not fill the window and exposing a black border.
            #[cfg(target_os = "windows")]
            {
                let _ = main_window.with_webview(|webview| unsafe {
                    // Reset zoom to 100% in case it drifted from a previous session
                    let _ = webview.controller().SetZoomFactor(1.0);
                    // Disable all zoom controls (Ctrl+scroll, Ctrl+Plus/Minus, pinch)
                    if let Ok(core) = webview.controller().CoreWebView2() {
                        if let Ok(settings) = core.Settings() {
                            let _ = settings.SetIsZoomControlEnabled(false);
                        }
                    }
                });
            }

            window_lifecycle::setup_window_close_handler(
                &main_window,
                state_clone.clone(),
                daemon_client.clone(),
                app_handle.clone(),
            );

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
