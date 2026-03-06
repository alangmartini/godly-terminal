use futures_channel::mpsc;
use godly_protocol::{McpRequest, McpResponse};

/// MCP events forwarded from the pipe server to the Iced app for state mutations.
#[derive(Debug, Clone)]
pub enum McpEvent {
    FocusTerminal { terminal_id: String },
    SwitchWorkspace { workspace_id: String },
    RenameTerminal { terminal_id: String, name: String },
    CreateTerminal {
        workspace_id: String,
        shell_type: Option<godly_protocol::types::ShellType>,
        cwd: Option<String>,
    },
    CloseTerminal { terminal_id: String },
    MoveTerminal { terminal_id: String, workspace_id: String },
    Notify { terminal_id: String, message: Option<String> },
    SplitTerminal {
        workspace_id: String,
        target_terminal_id: String,
        new_terminal_id: String,
        direction: String,
        ratio: f64,
    },
    UnsplitTerminal { workspace_id: String, terminal_id: String },
    SwapPanes {
        workspace_id: String,
        terminal_id_a: String,
        terminal_id_b: String,
    },
    ZoomPane {
        workspace_id: String,
        terminal_id: Option<String>,
    },
}

/// Start the MCP named pipe server in a background thread.
///
/// Events are sent through `event_tx` to the Iced app's update loop.
/// Spawns a thread that listens for `godly-mcp` connections on the MCP pipe.
pub fn start_mcp_server(event_tx: mpsc::UnboundedSender<McpEvent>) {
    std::thread::spawn(move || {
        run_mcp_server(event_tx);
    });
}

fn run_mcp_server(event_tx: mpsc::UnboundedSender<McpEvent>) {
    log::info!("[mcp-pipe] Starting MCP pipe server for native shell");

    loop {
        match accept_mcp_connection() {
            Ok(pipe) => {
                log::info!("[mcp-pipe] MCP client connected");
                let tx = event_tx.clone();
                std::thread::spawn(move || {
                    handle_mcp_client(pipe, tx);
                });
            }
            Err(e) => {
                log::error!("[mcp-pipe] Accept error: {}", e);
                std::thread::sleep(std::time::Duration::from_millis(100));
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
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
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
            unsafe { winapi::um::handleapi::CloseHandle(handle) };
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
///
/// Reads McpRequest messages, converts mutation requests to McpEvent,
/// sends them through the channel, and writes McpResponse back.
fn handle_mcp_client(
    mut pipe: std::fs::File,
    event_tx: mpsc::UnboundedSender<McpEvent>,
) {
    loop {
        match godly_protocol::read_message::<_, McpRequest>(&mut pipe) {
            Ok(Some(request)) => {
                log::debug!("[mcp-pipe] Received: {:?}", request);
                let response = dispatch_request(&request, &event_tx);
                if godly_protocol::write_message(&mut pipe, &response).is_err() {
                    log::warn!("[mcp-pipe] Write error, client disconnected");
                    break;
                }
            }
            Ok(None) => {
                log::info!("[mcp-pipe] MCP client disconnected (EOF)");
                break;
            }
            Err(e) => {
                log::error!("[mcp-pipe] Read error: {}", e);
                break;
            }
        }
    }
}

/// Dispatch a single MCP request.
///
/// Mutation requests are forwarded as McpEvents and return `McpResponse::Ok`.
/// Query requests return `McpResponse::Error` (not yet implemented in native shell).
/// Ping returns Pong directly.
fn dispatch_request(
    request: &McpRequest,
    event_tx: &mpsc::UnboundedSender<McpEvent>,
) -> McpResponse {
    match request {
        McpRequest::Ping => McpResponse::Pong,

        // --- J1: Focus Terminal ---
        McpRequest::FocusTerminal { terminal_id } => {
            send_event(event_tx, McpEvent::FocusTerminal {
                terminal_id: terminal_id.clone(),
            })
        }

        // --- J2: Switch Workspace ---
        McpRequest::SwitchWorkspace { workspace_id } => {
            send_event(event_tx, McpEvent::SwitchWorkspace {
                workspace_id: workspace_id.clone(),
            })
        }

        // --- J3: Rename Terminal ---
        McpRequest::RenameTerminal { terminal_id, name } => {
            send_event(event_tx, McpEvent::RenameTerminal {
                terminal_id: terminal_id.clone(),
                name: name.clone(),
            })
        }

        // --- J4: Create Terminal ---
        McpRequest::CreateTerminal {
            workspace_id,
            shell_type,
            cwd,
            ..
        } => {
            send_event(event_tx, McpEvent::CreateTerminal {
                workspace_id: workspace_id.clone(),
                shell_type: shell_type.clone(),
                cwd: cwd.clone(),
            })
        }

        // --- J5: Close Terminal ---
        McpRequest::CloseTerminal { terminal_id } => {
            send_event(event_tx, McpEvent::CloseTerminal {
                terminal_id: terminal_id.clone(),
            })
        }

        // --- J6: Move Terminal ---
        McpRequest::MoveTerminalToWorkspace {
            terminal_id,
            workspace_id,
        } => {
            send_event(event_tx, McpEvent::MoveTerminal {
                terminal_id: terminal_id.clone(),
                workspace_id: workspace_id.clone(),
            })
        }

        // --- J7: Notify ---
        McpRequest::Notify {
            terminal_id,
            message,
        } => {
            send_event(event_tx, McpEvent::Notify {
                terminal_id: terminal_id.clone(),
                message: message.clone(),
            })
        }

        // --- J8: Split Terminal ---
        McpRequest::SplitTerminal {
            workspace_id,
            target_terminal_id,
            new_terminal_id,
            direction,
            ratio,
        } => {
            send_event(event_tx, McpEvent::SplitTerminal {
                workspace_id: workspace_id.clone(),
                target_terminal_id: target_terminal_id.clone(),
                new_terminal_id: new_terminal_id.clone(),
                direction: direction.clone(),
                ratio: *ratio,
            })
        }

        McpRequest::UnsplitTerminal {
            workspace_id,
            terminal_id,
        } => {
            send_event(event_tx, McpEvent::UnsplitTerminal {
                workspace_id: workspace_id.clone(),
                terminal_id: terminal_id.clone(),
            })
        }

        // --- J9: Swap Panes / Zoom Pane ---
        McpRequest::SwapPanes {
            workspace_id,
            terminal_id_a,
            terminal_id_b,
        } => {
            send_event(event_tx, McpEvent::SwapPanes {
                workspace_id: workspace_id.clone(),
                terminal_id_a: terminal_id_a.clone(),
                terminal_id_b: terminal_id_b.clone(),
            })
        }

        McpRequest::ZoomPane {
            workspace_id,
            terminal_id,
        } => {
            send_event(event_tx, McpEvent::ZoomPane {
                workspace_id: workspace_id.clone(),
                terminal_id: terminal_id.clone(),
            })
        }

        // --- All other requests: not yet implemented in native shell ---
        _ => McpResponse::Error {
            message: "Not yet implemented in native shell".to_string(),
        },
    }
}

/// Send an event through the channel and return Ok response.
fn send_event(
    event_tx: &mpsc::UnboundedSender<McpEvent>,
    event: McpEvent,
) -> McpResponse {
    if event_tx.unbounded_send(event).is_err() {
        McpResponse::Error {
            message: "App event channel closed".to_string(),
        }
    } else {
        McpResponse::Ok
    }
}
