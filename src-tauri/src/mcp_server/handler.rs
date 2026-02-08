use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::daemon_client::DaemonClient;
use crate::persistence::AutoSaveManager;
use crate::state::AppState;

use godly_protocol::{McpRequest, McpResponse, McpTerminalInfo, McpWorkspaceInfo};

/// Handle an MCP request by delegating to AppState and DaemonClient.
pub fn handle_mcp_request(
    request: &McpRequest,
    app_state: &Arc<AppState>,
    daemon: &Arc<DaemonClient>,
    auto_save: &Arc<AutoSaveManager>,
    app_handle: &AppHandle,
) -> McpResponse {
    match request {
        McpRequest::Ping => McpResponse::Pong,

        McpRequest::ListTerminals => {
            let terminals = app_state.terminals.read();
            let list: Vec<McpTerminalInfo> = terminals
                .values()
                .map(|t| McpTerminalInfo {
                    id: t.id.clone(),
                    workspace_id: t.workspace_id.clone(),
                    name: t.name.clone(),
                    process_name: t.process_name.clone(),
                })
                .collect();
            McpResponse::TerminalList { terminals: list }
        }

        McpRequest::GetTerminal { terminal_id } => {
            let terminals = app_state.terminals.read();
            match terminals.get(terminal_id) {
                Some(t) => McpResponse::TerminalInfo {
                    terminal: McpTerminalInfo {
                        id: t.id.clone(),
                        workspace_id: t.workspace_id.clone(),
                        name: t.name.clone(),
                        process_name: t.process_name.clone(),
                    },
                },
                None => McpResponse::Error {
                    message: format!("Terminal {} not found", terminal_id),
                },
            }
        }

        McpRequest::GetCurrentSession { session_id } => {
            let terminals = app_state.terminals.read();
            match terminals.get(session_id) {
                Some(t) => McpResponse::TerminalInfo {
                    terminal: McpTerminalInfo {
                        id: t.id.clone(),
                        workspace_id: t.workspace_id.clone(),
                        name: t.name.clone(),
                        process_name: t.process_name.clone(),
                    },
                },
                None => McpResponse::Error {
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        McpRequest::CreateTerminal {
            workspace_id,
            shell_type,
            cwd,
        } => {
            use std::collections::HashMap;
            use uuid::Uuid;

            let terminal_id = Uuid::new_v4().to_string();

            // Determine working dir
            let working_dir = if let Some(dir) = cwd {
                Some(dir.clone())
            } else {
                app_state
                    .get_workspace(workspace_id)
                    .map(|ws| ws.folder_path)
            };

            // Determine shell type
            let shell = shell_type
                .clone()
                .or_else(|| {
                    app_state
                        .get_workspace(workspace_id)
                        .map(|ws| to_protocol_shell_type(&ws.shell_type))
                })
                .unwrap_or(godly_protocol::ShellType::Windows);

            let process_name = match &shell {
                godly_protocol::ShellType::Windows => String::from("powershell"),
                godly_protocol::ShellType::Wsl { distribution } => {
                    distribution.clone().unwrap_or_else(|| String::from("wsl"))
                }
            };

            // Build env vars
            let mut env_vars = HashMap::new();
            env_vars.insert("GODLY_SESSION_ID".to_string(), terminal_id.clone());
            env_vars.insert("GODLY_WORKSPACE_ID".to_string(), workspace_id.clone());

            let request = godly_protocol::Request::CreateSession {
                id: terminal_id.clone(),
                shell_type: shell.clone(),
                cwd: working_dir.clone(),
                rows: 24,
                cols: 80,
                env: Some(env_vars),
            };

            match daemon.send_request(&request) {
                Ok(godly_protocol::Response::SessionCreated { .. }) => {}
                Ok(godly_protocol::Response::Error { message }) => {
                    return McpResponse::Error { message };
                }
                Ok(other) => {
                    return McpResponse::Error {
                        message: format!("Unexpected response: {:?}", other),
                    };
                }
                Err(e) => return McpResponse::Error { message: e },
            }

            // Attach
            let attach_req = godly_protocol::Request::Attach {
                session_id: terminal_id.clone(),
            };
            match daemon.send_request(&attach_req) {
                Ok(godly_protocol::Response::Ok | godly_protocol::Response::Buffer { .. }) => {}
                Ok(godly_protocol::Response::Error { message }) => {
                    return McpResponse::Error {
                        message: format!("Failed to attach: {}", message),
                    };
                }
                _ => {}
            }

            // Store metadata
            let app_shell = from_protocol_shell_type(&shell);
            app_state.add_session_metadata(
                terminal_id.clone(),
                crate::state::SessionMetadata {
                    shell_type: app_shell,
                    cwd: working_dir,
                    worktree_path: None,
                    worktree_branch: None,
                },
            );

            app_state.add_terminal(crate::state::Terminal {
                id: terminal_id.clone(),
                workspace_id: workspace_id.clone(),
                name: String::from("Terminal"),
                process_name,
            });

            auto_save.mark_dirty();

            // Notify frontend
            let _ = app_handle.emit("mcp-terminal-created", &terminal_id);

            McpResponse::Created {
                id: terminal_id,
            }
        }

        McpRequest::CloseTerminal { terminal_id } => {
            let request = godly_protocol::Request::CloseSession {
                session_id: terminal_id.clone(),
            };
            match daemon.send_request(&request) {
                Ok(godly_protocol::Response::Ok) => {}
                Ok(godly_protocol::Response::Error { message }) => {
                    eprintln!("[mcp] Warning: close session error: {}", message);
                }
                _ => {}
            }

            app_state.remove_session_metadata(terminal_id);
            app_state.remove_terminal(terminal_id);
            auto_save.mark_dirty();

            let _ = app_handle.emit("mcp-terminal-closed", terminal_id);

            McpResponse::Ok
        }

        McpRequest::RenameTerminal { terminal_id, name } => {
            app_state.update_terminal_name(terminal_id, name.clone());
            auto_save.mark_dirty();

            #[derive(serde::Serialize, Clone)]
            struct RenamePayload {
                terminal_id: String,
                name: String,
            }

            let _ = app_handle.emit(
                "terminal-renamed",
                RenamePayload {
                    terminal_id: terminal_id.clone(),
                    name: name.clone(),
                },
            );

            McpResponse::Ok
        }

        McpRequest::FocusTerminal { terminal_id } => {
            let _ = app_handle.emit("focus-terminal", terminal_id);
            McpResponse::Ok
        }

        McpRequest::ListWorkspaces => {
            let workspaces = app_state.get_all_workspaces();
            let list: Vec<McpWorkspaceInfo> = workspaces
                .iter()
                .map(|w| McpWorkspaceInfo {
                    id: w.id.clone(),
                    name: w.name.clone(),
                    folder_path: w.folder_path.clone(),
                })
                .collect();
            McpResponse::WorkspaceList { workspaces: list }
        }

        McpRequest::CreateWorkspace { name, folder_path } => {
            let workspace_id = uuid::Uuid::new_v4().to_string();

            let workspace = crate::state::Workspace {
                id: workspace_id.clone(),
                name: name.clone(),
                folder_path: folder_path.clone(),
                tab_order: Vec::new(),
                shell_type: crate::state::ShellType::default(),
                worktree_mode: false,
                claude_code_mode: false,
            };

            app_state.add_workspace(workspace);
            auto_save.mark_dirty();

            McpResponse::Created { id: workspace_id }
        }

        McpRequest::SwitchWorkspace { workspace_id } => {
            let _ = app_handle.emit("switch-workspace", workspace_id);
            McpResponse::Ok
        }

        McpRequest::MoveTerminalToWorkspace {
            terminal_id,
            workspace_id,
        } => {
            app_state.update_terminal_workspace(terminal_id, workspace_id.clone());
            auto_save.mark_dirty();

            #[derive(serde::Serialize, Clone)]
            struct MovePayload {
                terminal_id: String,
                workspace_id: String,
            }

            let _ = app_handle.emit(
                "mcp-terminal-moved",
                MovePayload {
                    terminal_id: terminal_id.clone(),
                    workspace_id: workspace_id.clone(),
                },
            );

            McpResponse::Ok
        }

        McpRequest::WriteToTerminal { terminal_id, data } => {
            let request = godly_protocol::Request::Write {
                session_id: terminal_id.clone(),
                data: data.as_bytes().to_vec(),
            };
            match daemon.send_request(&request) {
                Ok(godly_protocol::Response::Ok) => McpResponse::Ok,
                Ok(godly_protocol::Response::Error { message }) => {
                    McpResponse::Error { message }
                }
                Ok(other) => McpResponse::Error {
                    message: format!("Unexpected response: {:?}", other),
                },
                Err(e) => McpResponse::Error { message: e },
            }
        }

        McpRequest::Notify {
            terminal_id,
            message,
        } => {
            #[derive(serde::Serialize, Clone)]
            struct NotifyPayload {
                terminal_id: String,
                message: Option<String>,
            }

            let _ = app_handle.emit(
                "mcp-notify",
                NotifyPayload {
                    terminal_id: terminal_id.clone(),
                    message: message.clone(),
                },
            );

            McpResponse::Ok
        }

        McpRequest::SetNotificationEnabled {
            terminal_id,
            workspace_id,
            enabled,
        } => {
            if let Some(tid) = terminal_id {
                app_state.set_notification_enabled_terminal(tid, *enabled);
            }
            if let Some(wid) = workspace_id {
                app_state.set_notification_enabled_workspace(wid, *enabled);
            }

            #[derive(serde::Serialize, Clone)]
            struct SettingsPayload {
                terminal_id: Option<String>,
                workspace_id: Option<String>,
                enabled: bool,
            }

            let _ = app_handle.emit(
                "notification-settings-changed",
                SettingsPayload {
                    terminal_id: terminal_id.clone(),
                    workspace_id: workspace_id.clone(),
                    enabled: *enabled,
                },
            );

            McpResponse::Ok
        }

        McpRequest::GetNotificationStatus {
            terminal_id,
            workspace_id,
        } => {
            let (enabled, source) = app_state.is_notification_enabled(
                terminal_id.as_deref(),
                workspace_id.as_deref(),
            );
            McpResponse::NotificationStatus {
                enabled,
                source: source.to_string(),
            }
        }
    }
}

/// Convert app ShellType to protocol ShellType
fn to_protocol_shell_type(st: &crate::state::ShellType) -> godly_protocol::ShellType {
    match st {
        crate::state::ShellType::Windows => godly_protocol::ShellType::Windows,
        crate::state::ShellType::Wsl { distribution } => godly_protocol::ShellType::Wsl {
            distribution: distribution.clone(),
        },
    }
}

/// Convert protocol ShellType to app ShellType
fn from_protocol_shell_type(st: &godly_protocol::ShellType) -> crate::state::ShellType {
    match st {
        godly_protocol::ShellType::Windows => crate::state::ShellType::Windows,
        godly_protocol::ShellType::Wsl { distribution } => crate::state::ShellType::Wsl {
            distribution: distribution.clone(),
        },
    }
}
