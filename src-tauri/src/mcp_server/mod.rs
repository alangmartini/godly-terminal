mod handler;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::AppHandle;

use crate::daemon_client::DaemonClient;
use crate::persistence::AutoSaveManager;
use crate::state::AppState;

use self::handler::handle_mcp_request;

/// Start the MCP named pipe server in a background thread.
/// Accepts connections from godly-mcp.exe instances and handles their requests.
pub fn start_mcp_server(
    app_handle: AppHandle,
    app_state: Arc<AppState>,
    daemon: Arc<DaemonClient>,
    auto_save: Arc<AutoSaveManager>,
) {
    std::thread::spawn(move || {
        run_mcp_server(app_handle, app_state, daemon, auto_save);
    });
}

fn run_mcp_server(
    app_handle: AppHandle,
    app_state: Arc<AppState>,
    daemon: Arc<DaemonClient>,
    auto_save: Arc<AutoSaveManager>,
) {
    let running = Arc::new(AtomicBool::new(true));

    eprintln!("[mcp-server] Starting MCP pipe server");

    loop {
        if !running.load(Ordering::Relaxed) {
            break;
        }

        match accept_mcp_connection() {
            Ok(pipe) => {
                eprintln!("[mcp-server] MCP client connected");
                let state = app_state.clone();
                let daemon = daemon.clone();
                let auto_save = auto_save.clone();
                let handle = app_handle.clone();

                std::thread::spawn(move || {
                    handle_mcp_client(pipe, state, daemon, auto_save, handle);
                });
            }
            Err(e) => {
                eprintln!("[mcp-server] Accept error: {}", e);
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Accept a single MCP pipe connection (Windows implementation).
#[cfg(windows)]
fn accept_mcp_connection() -> Result<std::fs::File, String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use winapi::shared::winerror::ERROR_PIPE_CONNECTED;
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::namedpipeapi::{ConnectNamedPipe, CreateNamedPipeW};
    use winapi::um::winbase::{
        PIPE_ACCESS_DUPLEX, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES,
        PIPE_WAIT,
    };

    let pipe_name_str = godly_protocol::mcp_pipe_name();
    let pipe_name: Vec<u16> = OsStr::new(&pipe_name_str)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle = unsafe {
        CreateNamedPipeW(
            pipe_name.as_ptr(),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            4096,
            4096,
            0,
            std::ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        return Err(format!(
            "CreateNamedPipe failed: {}",
            unsafe { GetLastError() }
        ));
    }

    let connected = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) };
    if connected == 0 {
        let err = unsafe { GetLastError() };
        if err != ERROR_PIPE_CONNECTED {
            unsafe { CloseHandle(handle) };
            return Err(format!("ConnectNamedPipe failed: {}", err));
        }
    }

    use std::os::windows::io::FromRawHandle;
    let pipe = unsafe { std::fs::File::from_raw_handle(handle as _) };

    Ok(pipe)
}

#[cfg(not(windows))]
fn accept_mcp_connection() -> Result<std::fs::File, String> {
    Err("MCP named pipes are only supported on Windows".to_string())
}

/// Handle a single MCP client connection.
/// Reads requests, processes them, and sends responses using the same
/// length-prefixed JSON framing as the daemon protocol.
fn handle_mcp_client(
    mut pipe: std::fs::File,
    app_state: Arc<AppState>,
    daemon: Arc<DaemonClient>,
    auto_save: Arc<AutoSaveManager>,
    app_handle: AppHandle,
) {
    use godly_protocol::McpRequest;

    loop {
        // Read request
        match godly_protocol::read_message::<_, McpRequest>(&mut pipe) {
            Ok(Some(request)) => {
                eprintln!("[mcp-server] Received: {:?}", request);

                let response =
                    handle_mcp_request(&request, &app_state, &daemon, &auto_save, &app_handle);

                if godly_protocol::write_message(&mut pipe, &response).is_err() {
                    eprintln!("[mcp-server] Write error, client disconnected");
                    break;
                }
            }
            Ok(None) => {
                eprintln!("[mcp-server] MCP client disconnected (EOF)");
                break;
            }
            Err(e) => {
                eprintln!("[mcp-server] Read error: {}", e);
                break;
            }
        }
    }
}
