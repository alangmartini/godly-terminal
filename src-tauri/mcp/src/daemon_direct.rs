use std::collections::HashMap;
use std::io::Write;

use godly_protocol::ansi::{strip_ansi, truncate_output};
use godly_protocol::{
    DaemonMessage, McpRequest, McpResponse, McpTerminalInfo, Request, Response, ShellType,
};

use crate::backend::Backend;
use crate::log::mcp_log;

/// Backend that talks directly to the daemon, bypassing the Tauri app.
/// Only supports daemon-routable tools; returns clear errors for app-only tools.
pub struct DaemonDirectBackend {
    pipe: std::fs::File,
}

impl DaemonDirectBackend {
    /// Connect to the daemon's named pipe.
    #[cfg(windows)]
    pub fn connect() -> Result<Self, String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use winapi::um::errhandlingapi::GetLastError;
        use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
        use winapi::um::handleapi::INVALID_HANDLE_VALUE;
        use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

        let pipe_name_str = godly_protocol::pipe_name();
        mcp_log!("daemon_direct: connecting to daemon pipe: {}", pipe_name_str);

        let pipe_name: Vec<u16> = OsStr::new(&pipe_name_str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe {
            CreateFileW(
                pipe_name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            let err = unsafe { GetLastError() };
            mcp_log!("daemon_direct: CreateFileW FAILED — error code {}", err);
            return Err(format!(
                "Cannot connect to daemon pipe (error: {}). Is the daemon running?",
                err
            ));
        }

        mcp_log!("daemon_direct: connected to daemon pipe");
        use std::os::windows::io::FromRawHandle;
        let pipe = unsafe { std::fs::File::from_raw_handle(handle as _) };
        Ok(Self { pipe })
    }

    #[cfg(not(windows))]
    pub fn connect() -> Result<Self, String> {
        Err("Daemon direct connection is only supported on Windows".to_string())
    }

    /// Send a daemon Request and read the Response, discarding async Events.
    fn daemon_request(&mut self, request: &Request) -> Result<Response, String> {
        godly_protocol::write_message(&mut self.pipe, request)
            .map_err(|e| format!("Daemon write error: {}", e))?;
        self.pipe.flush().ok();

        // Read messages, discarding Events until we get a Response
        loop {
            let msg: DaemonMessage = godly_protocol::read_message(&mut self.pipe)
                .map_err(|e| format!("Daemon read error: {}", e))?
                .ok_or_else(|| "Daemon pipe closed".to_string())?;

            match msg {
                DaemonMessage::Response(resp) => return Ok(resp),
                DaemonMessage::Event(_) => {
                    // Discard async events (output, process-changed, etc.)
                    continue;
                }
            }
        }
    }

    /// Look up a single session by ID via ListSessions.
    fn get_session_by_id(&mut self, session_id: &str) -> Result<McpResponse, String> {
        let resp = self.daemon_request(&Request::ListSessions)?;
        match resp {
            Response::SessionList { sessions } => {
                match sessions.iter().find(|s| s.id == session_id) {
                    Some(s) => Ok(McpResponse::TerminalInfo {
                        terminal: McpTerminalInfo {
                            id: s.id.clone(),
                            workspace_id: String::new(),
                            name: format!("Session {}", &s.id[..8.min(s.id.len())]),
                            process_name: s.shell_type.display_name(),
                        },
                    }),
                    None => Ok(McpResponse::Error {
                        message: format!("Session {} not found", session_id),
                    }),
                }
            }
            Response::Error { message } => Ok(McpResponse::Error { message }),
            other => Ok(McpResponse::Error {
                message: format!("Unexpected daemon response: {:?}", other),
            }),
        }
    }

    /// Return an error for tools that require the Tauri app.
    fn app_only_error(tool: &str) -> McpResponse {
        McpResponse::Error {
            message: format!(
                "{} requires the Godly Terminal app (running in daemon-direct fallback mode)",
                tool
            ),
        }
    }
}

impl Backend for DaemonDirectBackend {
    fn send_request(&mut self, request: &McpRequest) -> Result<McpResponse, String> {
        match request {
            McpRequest::Ping => {
                let resp = self.daemon_request(&Request::Ping)?;
                match resp {
                    Response::Pong => Ok(McpResponse::Pong),
                    Response::Error { message } => Ok(McpResponse::Error { message }),
                    other => Ok(McpResponse::Error {
                        message: format!("Unexpected daemon response: {:?}", other),
                    }),
                }
            }

            McpRequest::ListTerminals => {
                let resp = self.daemon_request(&Request::ListSessions)?;
                match resp {
                    Response::SessionList { sessions } => {
                        let terminals: Vec<McpTerminalInfo> = sessions
                            .iter()
                            .map(|s| McpTerminalInfo {
                                id: s.id.clone(),
                                workspace_id: String::new(), // unknown in direct mode
                                name: format!("Session {}", &s.id[..8.min(s.id.len())]),
                                process_name: s.shell_type.display_name(),
                            })
                            .collect();
                        Ok(McpResponse::TerminalList { terminals })
                    }
                    Response::Error { message } => Ok(McpResponse::Error { message }),
                    other => Ok(McpResponse::Error {
                        message: format!("Unexpected daemon response: {:?}", other),
                    }),
                }
            }

            McpRequest::GetTerminal { terminal_id } => {
                self.get_session_by_id(terminal_id)
            }

            McpRequest::GetCurrentSession { session_id } => {
                self.get_session_by_id(session_id)
            }

            McpRequest::CreateTerminal {
                workspace_id: _,
                shell_type,
                cwd,
                worktree_name,
                worktree,
                command,
            } => {
                // Worktree support requires git operations from the Tauri app
                if worktree.unwrap_or(false) || worktree_name.is_some() {
                    return Ok(McpResponse::Error {
                        message: "Worktree creation requires the Godly Terminal app (running in daemon-direct fallback mode)".to_string(),
                    });
                }

                let terminal_id = uuid::Uuid::new_v4().to_string();
                let shell = shell_type.clone().unwrap_or(ShellType::Windows);

                let mut env_vars = HashMap::new();
                env_vars.insert("GODLY_SESSION_ID".to_string(), terminal_id.clone());

                let create_req = Request::CreateSession {
                    id: terminal_id.clone(),
                    shell_type: shell,
                    cwd: cwd.clone(),
                    rows: 24,
                    cols: 80,
                    env: Some(env_vars),
                };

                match self.daemon_request(&create_req)? {
                    Response::SessionCreated { .. } => {}
                    Response::Error { message } => return Ok(McpResponse::Error { message }),
                    other => {
                        return Ok(McpResponse::Error {
                            message: format!("Unexpected response: {:?}", other),
                        })
                    }
                }

                // Attach to receive output
                let attach_req = Request::Attach {
                    session_id: terminal_id.clone(),
                };
                match self.daemon_request(&attach_req) {
                    Ok(Response::Ok | Response::Buffer { .. }) => {}
                    Ok(Response::Error { message }) => {
                        mcp_log!("daemon_direct: attach warning: {}", message);
                    }
                    _ => {}
                }

                // Run command if specified
                if let Some(cmd) = command {
                    let write_req = Request::Write {
                        session_id: terminal_id.clone(),
                        data: format!("{}\r", cmd).into_bytes(),
                    };
                    let _ = self.daemon_request(&write_req);
                }

                Ok(McpResponse::Created {
                    id: terminal_id,
                    worktree_path: None,
                    worktree_branch: None,
                })
            }

            McpRequest::CloseTerminal { terminal_id } => {
                let resp = self.daemon_request(&Request::CloseSession {
                    session_id: terminal_id.clone(),
                })?;
                match resp {
                    Response::Ok => Ok(McpResponse::Ok),
                    Response::Error { message } => Ok(McpResponse::Error { message }),
                    other => Ok(McpResponse::Error {
                        message: format!("Unexpected response: {:?}", other),
                    }),
                }
            }

            McpRequest::WriteToTerminal { terminal_id, data } => {
                // Convert \n → \r for PTY (same as handler.rs)
                let converted = data.replace("\r\n", "\r").replace('\n', "\r");
                let resp = self.daemon_request(&Request::Write {
                    session_id: terminal_id.clone(),
                    data: converted.as_bytes().to_vec(),
                })?;
                match resp {
                    Response::Ok => Ok(McpResponse::Ok),
                    Response::Error { message } => Ok(McpResponse::Error { message }),
                    other => Ok(McpResponse::Error {
                        message: format!("Unexpected response: {:?}", other),
                    }),
                }
            }

            McpRequest::ReadTerminal {
                terminal_id,
                mode,
                lines,
                strip_ansi: do_strip,
            } => {
                let resp = self.daemon_request(&Request::ReadBuffer {
                    session_id: terminal_id.clone(),
                })?;
                match resp {
                    Response::Buffer { data, .. } => {
                        let text = String::from_utf8_lossy(&data).into_owned();
                        let mut content = truncate_output(&text, mode.as_deref(), *lines);
                        if do_strip.unwrap_or(false) {
                            content = strip_ansi(&content);
                        }
                        Ok(McpResponse::TerminalOutput { content })
                    }
                    Response::Error { message } => Ok(McpResponse::Error { message }),
                    other => Ok(McpResponse::Error {
                        message: format!("Unexpected response: {:?}", other),
                    }),
                }
            }

            McpRequest::ReadGrid { terminal_id } => {
                let resp = self.daemon_request(&Request::ReadGrid {
                    session_id: terminal_id.clone(),
                })?;
                match resp {
                    Response::Grid { grid } => Ok(McpResponse::GridSnapshot {
                        rows: grid.rows,
                        cursor_row: grid.cursor_row,
                        cursor_col: grid.cursor_col,
                        cols: grid.cols,
                        num_rows: grid.num_rows,
                        alternate_screen: grid.alternate_screen,
                    }),
                    Response::Error { message } => Ok(McpResponse::Error { message }),
                    other => Ok(McpResponse::Error {
                        message: format!("Unexpected response: {:?}", other),
                    }),
                }
            }

            McpRequest::ResizeTerminal {
                terminal_id,
                rows,
                cols,
            } => {
                let resp = self.daemon_request(&Request::Resize {
                    session_id: terminal_id.clone(),
                    rows: *rows,
                    cols: *cols,
                })?;
                match resp {
                    Response::Ok => Ok(McpResponse::Ok),
                    Response::Error { message } => Ok(McpResponse::Error { message }),
                    other => Ok(McpResponse::Error {
                        message: format!("Unexpected response: {:?}", other),
                    }),
                }
            }

            McpRequest::WaitForIdle {
                terminal_id,
                idle_ms,
                timeout_ms,
            } => {
                let deadline =
                    std::time::Instant::now() + std::time::Duration::from_millis(*timeout_ms);
                let poll_ms = (*idle_ms / 4).min(500).max(50);

                loop {
                    let resp = self.daemon_request(&Request::GetLastOutputTime {
                        session_id: terminal_id.clone(),
                    })?;

                    match resp {
                        Response::LastOutputTime { epoch_ms, running } => {
                            let now_ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64;
                            let ago = now_ms.saturating_sub(epoch_ms);

                            if ago >= *idle_ms || !running {
                                return Ok(McpResponse::WaitResult {
                                    completed: true,
                                    last_output_ago_ms: ago,
                                });
                            }

                            if std::time::Instant::now() >= deadline {
                                return Ok(McpResponse::WaitResult {
                                    completed: false,
                                    last_output_ago_ms: ago,
                                });
                            }
                        }
                        Response::Error { message } => {
                            return Ok(McpResponse::Error { message });
                        }
                        _ => {
                            return Ok(McpResponse::Error {
                                message: "Unexpected daemon response".to_string(),
                            });
                        }
                    }

                    std::thread::sleep(std::time::Duration::from_millis(poll_ms));
                }
            }

            McpRequest::WaitForText {
                terminal_id,
                text,
                timeout_ms,
            } => {
                let deadline =
                    std::time::Instant::now() + std::time::Duration::from_millis(*timeout_ms);
                let poll_ms = 200u64;

                loop {
                    let resp = self.daemon_request(&Request::SearchBuffer {
                        session_id: terminal_id.clone(),
                        text: text.clone(),
                        strip_ansi: true,
                    })?;

                    match resp {
                        Response::SearchResult { found, running } => {
                            if found {
                                let now_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64;
                                let ago = match self.daemon_request(
                                    &Request::GetLastOutputTime {
                                        session_id: terminal_id.clone(),
                                    },
                                ) {
                                    Ok(Response::LastOutputTime { epoch_ms, .. }) => {
                                        now_ms.saturating_sub(epoch_ms)
                                    }
                                    _ => 0,
                                };
                                return Ok(McpResponse::WaitResult {
                                    completed: true,
                                    last_output_ago_ms: ago,
                                });
                            }

                            if !running || std::time::Instant::now() >= deadline {
                                return Ok(McpResponse::WaitResult {
                                    completed: false,
                                    last_output_ago_ms: 0,
                                });
                            }
                        }
                        Response::Error { message } => {
                            return Ok(McpResponse::Error { message });
                        }
                        _ => {
                            return Ok(McpResponse::Error {
                                message: "Unexpected daemon response".to_string(),
                            });
                        }
                    }

                    std::thread::sleep(std::time::Duration::from_millis(poll_ms));
                }
            }

            // App-only tools that require Tauri
            McpRequest::ListWorkspaces => Ok(Self::app_only_error("list_workspaces")),
            McpRequest::CreateWorkspace { .. } => Ok(Self::app_only_error("create_workspace")),
            McpRequest::DeleteWorkspace { .. } => Ok(Self::app_only_error("delete_workspace")),
            McpRequest::SwitchWorkspace { .. } => Ok(Self::app_only_error("switch_workspace")),
            McpRequest::GetActiveWorkspace => Ok(Self::app_only_error("get_active_workspace")),
            McpRequest::GetActiveTerminal => Ok(Self::app_only_error("get_active_terminal")),
            McpRequest::FocusTerminal { .. } => Ok(Self::app_only_error("focus_terminal")),
            McpRequest::RenameTerminal { .. } => Ok(Self::app_only_error("rename_terminal")),
            McpRequest::MoveTerminalToWorkspace { .. } => {
                Ok(Self::app_only_error("move_terminal_to_workspace"))
            }
            McpRequest::RemoveWorktree { .. } => Ok(Self::app_only_error("remove_worktree")),
            McpRequest::Notify { .. } => Ok(Self::app_only_error("notify")),
            McpRequest::SetNotificationEnabled { .. } => {
                Ok(Self::app_only_error("set_notification_enabled"))
            }
            McpRequest::GetNotificationStatus { .. } => {
                Ok(Self::app_only_error("get_notification_status"))
            }
            McpRequest::QuickClaude { .. } => Ok(Self::app_only_error("quick_claude")),
        }
    }

    fn label(&self) -> &'static str {
        "daemon-direct"
    }
}
