use std::collections::HashMap;
use std::io::Write;
use std::sync::Mutex;

use godly_protocol::ansi::{strip_ansi, truncate_output};
use godly_protocol::{
    DaemonMessage, McpRequest, McpResponse, McpTerminalInfo, Request, Response, ShellType,
    read_daemon_message, write_request,
};

use crate::backend::Backend;
use crate::log::mcp_log;

/// Backend that talks directly to the daemon, bypassing the Tauri app.
/// Only supports daemon-routable tools; returns clear errors for app-only tools.
pub struct DaemonDirectBackend {
    pipe: Mutex<std::fs::File>,
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
        Ok(Self { pipe: Mutex::new(pipe) })
    }

    #[cfg(not(windows))]
    pub fn connect() -> Result<Self, String> {
        Err("Daemon direct connection is only supported on Windows".to_string())
    }

    /// Send a daemon Request and read the Response, discarding async Events.
    fn daemon_request(&self, request: &Request) -> Result<Response, String> {
        let mut pipe = self.pipe.lock().map_err(|e| format!("Mutex poisoned: {}", e))?;
        write_request(&mut *pipe, request)
            .map_err(|e| format!("Daemon write error: {}", e))?;
        pipe.flush().ok();

        // Read messages, discarding Events until we get a Response
        loop {
            let msg: DaemonMessage = read_daemon_message(&mut *pipe)
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
    fn get_session_by_id(&self, session_id: &str) -> Result<McpResponse, String> {
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
    fn send_request(&self, request: &McpRequest) -> Result<McpResponse, String> {
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
                        Response::LastOutputTime { epoch_ms, running, .. } => {
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

            McpRequest::SendKeys { terminal_id, keys } => {
                // Convert each key name to bytes and concatenate
                let mut all_bytes = Vec::new();
                for key in keys {
                    match godly_protocol::keys::key_to_bytes(key) {
                        Ok(bytes) => all_bytes.extend(bytes),
                        Err(e) => return Ok(McpResponse::Error { message: e }),
                    }
                }
                let resp = self.daemon_request(&Request::Write {
                    session_id: terminal_id.clone(),
                    data: all_bytes,
                })?;
                match resp {
                    Response::Ok => Ok(McpResponse::Ok),
                    Response::Error { message } => Ok(McpResponse::Error { message }),
                    other => Ok(McpResponse::Error {
                        message: format!("Unexpected response: {:?}", other),
                    }),
                }
            }

            McpRequest::EraseContent { terminal_id, count } => {
                let backspaces = vec![0x08u8; *count];
                let resp = self.daemon_request(&Request::Write {
                    session_id: terminal_id.clone(),
                    data: backspaces,
                })?;
                match resp {
                    Response::Ok => Ok(McpResponse::Ok),
                    Response::Error { message } => Ok(McpResponse::Error { message }),
                    other => Ok(McpResponse::Error {
                        message: format!("Unexpected response: {:?}", other),
                    }),
                }
            }

            McpRequest::ExecuteCommand {
                terminal_id,
                command,
                idle_ms,
                timeout_ms,
            } => {
                // 1. Snapshot buffer length before command
                let before_len = match self.daemon_request(&Request::ReadBuffer {
                    session_id: terminal_id.clone(),
                })? {
                    Response::Buffer { data, .. } => data.len(),
                    Response::Error { message } => return Ok(McpResponse::Error { message }),
                    _ => 0,
                };

                // 2. Write command + Enter
                let resp = self.daemon_request(&Request::Write {
                    session_id: terminal_id.clone(),
                    data: format!("{}\r", command).into_bytes(),
                })?;
                if let Response::Error { message } = resp {
                    return Ok(McpResponse::Error { message });
                }

                // 3. Wait for idle
                let deadline =
                    std::time::Instant::now() + std::time::Duration::from_millis(*timeout_ms);
                let poll_ms = (*idle_ms / 4).min(500).max(50);
                let mut completed = false;
                #[allow(unused_assignments)]
                let mut last_ago = 0u64;
                #[allow(unused_assignments)]
                let mut running = true;
                #[allow(unused_assignments)]
                let mut input_expected = None;

                loop {
                    let resp = self.daemon_request(&Request::GetLastOutputTime {
                        session_id: terminal_id.clone(),
                    })?;

                    match resp {
                        Response::LastOutputTime {
                            epoch_ms,
                            running: is_running,
                            input_expected: ie,
                            ..
                        } => {
                            let now_ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64;
                            last_ago = now_ms.saturating_sub(epoch_ms);
                            running = is_running;
                            input_expected = ie;

                            if !running {
                                completed = true;
                                break;
                            }

                            if last_ago >= *idle_ms {
                                if input_expected.unwrap_or(false) {
                                    // Terminal is idle but waiting for input — not completed
                                    completed = false;
                                } else {
                                    completed = true;
                                }
                                break;
                            }

                            if std::time::Instant::now() >= deadline {
                                break;
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

                // 4. Read new output (delta since before_len)
                let output = match self.daemon_request(&Request::ReadBuffer {
                    session_id: terminal_id.clone(),
                })? {
                    Response::Buffer { data, .. } => {
                        let new_data = if data.len() > before_len {
                            &data[before_len..]
                        } else {
                            &data[..]
                        };
                        let text = String::from_utf8_lossy(new_data).into_owned();
                        let stripped = strip_ansi(&text);
                        // Strip command echo: if the first line ends with the command, remove it
                        strip_command_echo(&stripped, command)
                    }
                    Response::Error { message } => return Ok(McpResponse::Error { message }),
                    _ => String::new(),
                };

                Ok(McpResponse::CommandOutput {
                    output,
                    completed,
                    last_output_ago_ms: last_ago,
                    running,
                    input_expected,
                })
            }

            McpRequest::ScrollPageUp { terminal_id } => {
                let tid = terminal_id.as_ref().ok_or(
                    "terminal_id is required in daemon-direct mode (no active terminal available)",
                )?;
                let resp = self.daemon_request(&Request::ReadRichGrid {
                    session_id: tid.clone(),
                })?;
                match resp {
                    Response::RichGrid { grid } => {
                        let viewport_rows = grid.dimensions.rows as usize;
                        let new_offset =
                            (grid.scrollback_offset + viewport_rows).min(grid.total_scrollback);
                        let set_resp = self.daemon_request(&Request::SetScrollback {
                            session_id: tid.clone(),
                            offset: new_offset,
                        })?;
                        match set_resp {
                            Response::Ok => Ok(McpResponse::Ok),
                            Response::Error { message } => Ok(McpResponse::Error { message }),
                            other => Ok(McpResponse::Error {
                                message: format!("Unexpected response: {:?}", other),
                            }),
                        }
                    }
                    Response::Error { message } => Ok(McpResponse::Error { message }),
                    other => Ok(McpResponse::Error {
                        message: format!("Unexpected response: {:?}", other),
                    }),
                }
            }

            McpRequest::ScrollPageDown { terminal_id } => {
                let tid = terminal_id.as_ref().ok_or(
                    "terminal_id is required in daemon-direct mode (no active terminal available)",
                )?;
                let resp = self.daemon_request(&Request::ReadRichGrid {
                    session_id: tid.clone(),
                })?;
                match resp {
                    Response::RichGrid { grid } => {
                        let viewport_rows = grid.dimensions.rows as usize;
                        let new_offset = grid.scrollback_offset.saturating_sub(viewport_rows);
                        let set_resp = self.daemon_request(&Request::SetScrollback {
                            session_id: tid.clone(),
                            offset: new_offset,
                        })?;
                        match set_resp {
                            Response::Ok => Ok(McpResponse::Ok),
                            Response::Error { message } => Ok(McpResponse::Error { message }),
                            other => Ok(McpResponse::Error {
                                message: format!("Unexpected response: {:?}", other),
                            }),
                        }
                    }
                    Response::Error { message } => Ok(McpResponse::Error { message }),
                    other => Ok(McpResponse::Error {
                        message: format!("Unexpected response: {:?}", other),
                    }),
                }
            }

            McpRequest::ScrollToTop { terminal_id } => {
                let tid = terminal_id.as_ref().ok_or(
                    "terminal_id is required in daemon-direct mode (no active terminal available)",
                )?;
                let resp = self.daemon_request(&Request::ReadRichGrid {
                    session_id: tid.clone(),
                })?;
                match resp {
                    Response::RichGrid { grid } => {
                        let set_resp = self.daemon_request(&Request::SetScrollback {
                            session_id: tid.clone(),
                            offset: grid.total_scrollback,
                        })?;
                        match set_resp {
                            Response::Ok => Ok(McpResponse::Ok),
                            Response::Error { message } => Ok(McpResponse::Error { message }),
                            other => Ok(McpResponse::Error {
                                message: format!("Unexpected response: {:?}", other),
                            }),
                        }
                    }
                    Response::Error { message } => Ok(McpResponse::Error { message }),
                    other => Ok(McpResponse::Error {
                        message: format!("Unexpected response: {:?}", other),
                    }),
                }
            }

            McpRequest::ScrollToBottom { terminal_id } => {
                let tid = terminal_id.as_ref().ok_or(
                    "terminal_id is required in daemon-direct mode (no active terminal available)",
                )?;
                let set_resp = self.daemon_request(&Request::SetScrollback {
                    session_id: tid.clone(),
                    offset: 0,
                })?;
                match set_resp {
                    Response::Ok => Ok(McpResponse::Ok),
                    Response::Error { message } => Ok(McpResponse::Error { message }),
                    other => Ok(McpResponse::Error {
                        message: format!("Unexpected response: {:?}", other),
                    }),
                }
            }

            McpRequest::GetScrollPosition { terminal_id } => {
                let tid = terminal_id.as_ref().ok_or(
                    "terminal_id is required in daemon-direct mode (no active terminal available)",
                )?;
                let resp = self.daemon_request(&Request::ReadRichGrid {
                    session_id: tid.clone(),
                })?;
                match resp {
                    Response::RichGrid { grid } => Ok(McpResponse::ScrollPosition {
                        offset: grid.scrollback_offset as u32,
                        total_scrollback: grid.total_scrollback as u32,
                        viewport_rows: grid.dimensions.rows as u32,
                    }),
                    Response::Error { message } => Ok(McpResponse::Error { message }),
                    other => Ok(McpResponse::Error {
                        message: format!("Unexpected response: {:?}", other),
                    }),
                }
            }

            // App-only tools that require Tauri
            McpRequest::ListWorkspaces => Ok(Self::app_only_error("list_workspaces")),
            McpRequest::CreateWorkspace { .. } => Ok(Self::app_only_error("create_workspace")),
            McpRequest::DeleteWorkspace { .. } => Ok(Self::app_only_error("delete_workspace")),
            McpRequest::SwitchWorkspace { .. } => Ok(Self::app_only_error("switch_workspace")),
            McpRequest::RenameWorkspace { .. } => Ok(Self::app_only_error("rename_workspace")),
            McpRequest::ReorderWorkspaces { .. } => {
                Ok(Self::app_only_error("reorder_workspaces"))
            }
            McpRequest::GetWorkspaceDetails { .. } => {
                Ok(Self::app_only_error("get_workspace_details"))
            }
            McpRequest::OpenInExplorer { .. } => Ok(Self::app_only_error("open_in_explorer")),
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
            McpRequest::CreateSplit { .. } => Ok(Self::app_only_error("create_split")),
            McpRequest::ClearSplit { .. } => Ok(Self::app_only_error("clear_split")),
            McpRequest::GetSplitState { .. } => Ok(Self::app_only_error("get_split_state")),
            McpRequest::SplitTerminal { .. } => Ok(Self::app_only_error("split_terminal")),
            McpRequest::SelfSplit { .. } => Ok(Self::app_only_error("self_split")),
            McpRequest::UnsplitTerminal { .. } => Ok(Self::app_only_error("unsplit_terminal")),
            McpRequest::GetLayoutTree { .. } => Ok(Self::app_only_error("get_layout_tree")),
            McpRequest::SwapPanes { .. } => Ok(Self::app_only_error("swap_panes")),
            McpRequest::ZoomPane { .. } => Ok(Self::app_only_error("zoom_pane")),
            McpRequest::ExecuteJs { .. } => Ok(Self::app_only_error("execute_js")),
            McpRequest::CaptureScreenshot { .. } => Ok(Self::app_only_error("capture_screenshot")),
            McpRequest::ExportTerminalInfo { .. } => {
                Ok(Self::app_only_error("export_terminal_info"))
            }

            McpRequest::NextTab { .. } => Ok(Self::app_only_error("next_tab")),
            McpRequest::PreviousTab { .. } => Ok(Self::app_only_error("previous_tab")),
            McpRequest::GoToTab { .. } => Ok(Self::app_only_error("go_to_tab")),
            McpRequest::ToggleWorktreeMode { .. } => {
                Ok(Self::app_only_error("toggle_worktree_mode"))
            }
            McpRequest::ToggleClaudeCodeMode { .. } => {
                Ok(Self::app_only_error("toggle_claude_code_mode"))
            }
            McpRequest::GetWorkspaceModes { .. } => {
                Ok(Self::app_only_error("get_workspace_modes"))
            }
            McpRequest::GetNotificationConfig => {
                Ok(Self::app_only_error("get_notification_config"))
            }
            McpRequest::SetNotificationSound { .. } => {
                Ok(Self::app_only_error("set_notification_sound"))
            }
            McpRequest::AddMutePattern { .. } => {
                Ok(Self::app_only_error("add_mute_pattern"))
            }
            McpRequest::RemoveMutePattern { .. } => {
                Ok(Self::app_only_error("remove_mute_pattern"))
            }
            McpRequest::ListMutePatterns => {
                Ok(Self::app_only_error("list_mute_patterns"))
            }

            McpRequest::OpenSettings { .. } => Ok(Self::app_only_error("open_settings")),
            McpRequest::SaveLayout => Ok(Self::app_only_error("save_layout")),
            McpRequest::GetAppInfo => Ok(Self::app_only_error("get_app_info")),

        }
    }

    fn label(&self) -> &'static str {
        "daemon-direct"
    }
}

/// Strip the command echo from terminal output.
///
/// Terminals echo the command back before showing output. If the first line
/// of the output ends with the command text, remove that line. Falls back to
/// returning the full text if no match is found.
fn strip_command_echo(text: &str, command: &str) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    // The first line often contains the prompt + command echo.
    // Check if it ends with the command text (or a trimmed version).
    let first = lines[0].trim_end();
    let cmd_trimmed = command.trim();
    if first.ends_with(cmd_trimmed) || first == cmd_trimmed {
        lines.remove(0);
    }

    // Trim trailing empty lines
    while lines.last().map_or(false, |l| l.trim().is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}
