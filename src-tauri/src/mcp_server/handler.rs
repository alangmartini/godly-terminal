use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::daemon_client::DaemonClient;
use crate::llm_state::LlmState;
use crate::persistence::AutoSaveManager;
use crate::state::AppState;

use godly_protocol::{McpRequest, McpResponse, McpTerminalInfo, McpWorkspaceInfo};

/// Ensure the MCP "Agent" workspace exists, creating it on first call.
/// Returns the Agent workspace ID.
///
/// MCP terminals are displayed in the main window's Agent workspace.
/// No second WebView window is created — this avoids the broadcast event
/// storm that caused WebView2 crashes under heavy output (issue #204).
fn ensure_mcp_workspace(
    app_state: &Arc<AppState>,
) -> String {
    // Fast path: workspace already created
    if let Some(id) = app_state.mcp_workspace_id.read().clone() {
        return id;
    }

    // Slow path: create workspace
    let workspace_id = uuid::Uuid::new_v4().to_string();

    // Use the first workspace's folder_path as a fallback, or home dir
    let folder_path = app_state
        .get_all_workspaces()
        .first()
        .map(|ws| ws.folder_path.clone())
        .unwrap_or_else(|| {
            std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_else(|_| "C:\\".to_string())
        });

    let workspace = crate::state::Workspace {
        id: workspace_id.clone(),
        name: "Agent".to_string(),
        folder_path,
        tab_order: Vec::new(),
        shell_type: crate::state::ShellType::default(),
        worktree_mode: false,
        claude_code_mode: false,
    };

    app_state.add_workspace(workspace);
    *app_state.mcp_workspace_id.write() = Some(workspace_id.clone());

    workspace_id
}

/// Handle an MCP request by delegating to AppState and DaemonClient.
pub fn handle_mcp_request(
    request: &McpRequest,
    app_state: &Arc<AppState>,
    daemon: &Arc<DaemonClient>,
    auto_save: &Arc<AutoSaveManager>,
    app_handle: &AppHandle,
    llm_state: &Arc<LlmState>,
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
            workspace_id: _original_workspace_id,
            shell_type,
            cwd,
            worktree_name,
            worktree,
            command,
        } => {
            use std::collections::HashMap;
            use uuid::Uuid;

            // MCP terminals always go into the Agent workspace (separate window)
            let workspace_id = &ensure_mcp_workspace(app_state);

            let want_worktree = worktree.unwrap_or(false) || worktree_name.is_some();

            // Validate mutual exclusivity
            if cwd.is_some() && want_worktree {
                return McpResponse::Error {
                    message: "Cannot specify both cwd and worktree/worktree_name".to_string(),
                };
            }

            let terminal_id = Uuid::new_v4().to_string();

            // Determine working dir and optional worktree info
            let mut worktree_path_result: Option<String> = None;
            let mut worktree_branch_result: Option<String> = None;

            let working_dir = if want_worktree {
                // Worktree mode: workspace must exist and be a git repo
                let ws = match app_state.get_workspace(workspace_id) {
                    Some(ws) => ws,
                    None => {
                        return McpResponse::Error {
                            message: format!("Workspace {} not found", workspace_id),
                        };
                    }
                };

                if !crate::worktree::is_git_repo(&ws.folder_path, None) {
                    return McpResponse::Error {
                        message: format!(
                            "Workspace folder is not a git repo: {}",
                            ws.folder_path
                        ),
                    };
                }

                let repo_root = match crate::worktree::get_repo_root(&ws.folder_path, None) {
                    Ok(r) => r,
                    Err(e) => {
                        return McpResponse::Error {
                            message: format!("Failed to get repo root: {}", e),
                        };
                    }
                };

                match crate::worktree::create_worktree(&repo_root, &terminal_id, worktree_name.as_deref(), None) {
                    Ok(wt_result) => {
                        eprintln!(
                            "[mcp] Created worktree at: {} (branch: {})",
                            wt_result.path, wt_result.branch
                        );
                        worktree_path_result = Some(wt_result.path.clone());
                        worktree_branch_result = Some(wt_result.branch);
                        Some(wt_result.path)
                    }
                    Err(e) => {
                        return McpResponse::Error {
                            message: format!("Failed to create worktree: {}", e),
                        };
                    }
                }
            } else if let Some(dir) = cwd {
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

            let process_name = shell.display_name();

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

            // Run command if specified
            if let Some(ref cmd) = command {
                let write_req = godly_protocol::Request::Write {
                    session_id: terminal_id.clone(),
                    data: format!("{}\r", cmd).into_bytes(),
                };
                let _ = daemon.send_request(&write_req);
            }

            // Store metadata
            let app_shell = from_protocol_shell_type(&shell);
            app_state.add_session_metadata(
                terminal_id.clone(),
                crate::state::SessionMetadata {
                    shell_type: app_shell,
                    cwd: working_dir,
                    worktree_path: worktree_path_result.clone(),
                    worktree_branch: worktree_branch_result.clone(),
                },
            );

            app_state.add_terminal(crate::state::Terminal {
                id: terminal_id.clone(),
                workspace_id: workspace_id.clone(),
                name: String::from("Terminal"),
                process_name,
            });

            auto_save.mark_dirty();

            // Notify frontend (include workspace_id so the MCP window adds it correctly)
            #[derive(serde::Serialize, Clone)]
            struct McpTerminalCreatedPayload {
                terminal_id: String,
                workspace_id: String,
            }
            let _ = app_handle.emit(
                "mcp-terminal-created",
                McpTerminalCreatedPayload {
                    terminal_id: terminal_id.clone(),
                    workspace_id: workspace_id.clone(),
                },
            );

            McpResponse::Created {
                id: terminal_id,
                worktree_path: worktree_path_result,
                worktree_branch: worktree_branch_result,
            }
        }

        McpRequest::QuickClaude {
            workspace_id: _original_workspace_id,
            prompt,
            branch_name,
            skip_fetch,
        } => {
            use std::collections::HashMap;
            use uuid::Uuid;

            // Auto-generate branch name from prompt if not provided
            let branch_name = branch_name.clone().or_else(|| {
                llm_state.try_generate_branch_name(prompt)
            });

            // MCP terminals always go into the Agent workspace (separate window)
            let workspace_id = &ensure_mcp_workspace(app_state);

            let terminal_id = Uuid::new_v4().to_string();

            // Determine working dir via worktree (always create worktree for Quick Claude)
            let mut worktree_path_result: Option<String> = None;
            let mut worktree_branch_result: Option<String> = None;

            let working_dir = {
                let ws = match app_state.get_workspace(workspace_id) {
                    Some(ws) => ws,
                    None => {
                        return McpResponse::Error {
                            message: format!("Workspace {} not found", workspace_id),
                        };
                    }
                };

                let wsl = crate::worktree::WslConfig::from_path(&ws.folder_path);
                match crate::worktree::get_repo_root(&ws.folder_path, wsl.as_ref()) {
                    Ok(repo_root) => {
                        let should_skip = skip_fetch.unwrap_or(true);
                        match crate::worktree::create_worktree_with_options(
                            &repo_root,
                            &terminal_id,
                            branch_name.as_deref(),
                            wsl.as_ref(),
                            should_skip,
                        ) {
                            Ok(wt_result) => {
                                eprintln!(
                                    "[mcp] Quick Claude worktree: {} (branch: {})",
                                    wt_result.path, wt_result.branch
                                );
                                worktree_path_result = Some(wt_result.path.clone());
                                worktree_branch_result = Some(wt_result.branch);
                                Some(wt_result.path)
                            }
                            Err(e) => {
                                return McpResponse::Error {
                                    message: format!("Failed to create worktree: {}", e),
                                };
                            }
                        }
                    }
                    Err(_) => {
                        // Not a git repo — fall back to workspace directory
                        Some(ws.folder_path.clone())
                    }
                }
            };

            // Determine shell type
            let shell = app_state
                .get_workspace(workspace_id)
                .map(|ws| to_protocol_shell_type(&ws.shell_type))
                .unwrap_or(godly_protocol::ShellType::Windows);

            let process_name = shell.display_name();

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
                    worktree_path: worktree_path_result.clone(),
                    worktree_branch: worktree_branch_result.clone(),
                },
            );

            let terminal_name = worktree_branch_result
                .clone()
                .unwrap_or_else(|| "Quick Claude".to_string());

            app_state.add_terminal(crate::state::Terminal {
                id: terminal_id.clone(),
                workspace_id: workspace_id.clone(),
                name: terminal_name.clone(),
                process_name,
            });

            auto_save.mark_dirty();

            // Notify frontend
            #[derive(serde::Serialize, Clone)]
            struct McpTerminalCreatedPayload {
                terminal_id: String,
                workspace_id: String,
            }
            let _ = app_handle.emit(
                "mcp-terminal-created",
                McpTerminalCreatedPayload {
                    terminal_id: terminal_id.clone(),
                    workspace_id: workspace_id.clone(),
                },
            );

            // Spawn background thread: start dclaude → wait → write prompt
            let daemon_bg = daemon.clone();
            let app_handle_bg = app_handle.clone();
            let terminal_id_bg = terminal_id.clone();
            let prompt_bg = prompt.clone();
            std::thread::spawn(move || {
                crate::commands::terminal::quick_claude_background(
                    daemon_bg,
                    app_handle_bg,
                    terminal_id_bg,
                    prompt_bg,
                    terminal_name,
                );
            });

            McpResponse::Created {
                id: terminal_id,
                worktree_path: worktree_path_result,
                worktree_branch: worktree_branch_result,
            }
        }

        McpRequest::CloseTerminal { terminal_id } => {
            // Validate terminal exists
            if !app_state.terminals.read().contains_key(terminal_id) {
                return McpResponse::Error {
                    message: format!("Terminal {} not found", terminal_id),
                };
            }

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
            // Validate terminal exists
            if !app_state.terminals.read().contains_key(terminal_id) {
                return McpResponse::Error {
                    message: format!("Terminal {} not found", terminal_id),
                };
            }

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
            // Validate terminal exists
            if !app_state.terminals.read().contains_key(terminal_id) {
                return McpResponse::Error {
                    message: format!("Terminal {} not found", terminal_id),
                };
            }

            app_state.set_active_terminal_id(Some(terminal_id.clone()));
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

            McpResponse::Created {
                id: workspace_id,
                worktree_path: None,
                worktree_branch: None,
            }
        }

        McpRequest::DeleteWorkspace { workspace_id } => {
            // Validate workspace exists
            if !app_state.workspaces.read().contains_key(workspace_id) {
                return McpResponse::Error {
                    message: format!("Workspace {} not found", workspace_id),
                };
            }

            // Close all terminals in the workspace
            let terminals_to_close: Vec<String> = app_state
                .get_workspace_terminals(workspace_id)
                .iter()
                .map(|t| t.id.clone())
                .collect();

            for tid in &terminals_to_close {
                let request = godly_protocol::Request::CloseSession {
                    session_id: tid.clone(),
                };
                match daemon.send_request(&request) {
                    Ok(godly_protocol::Response::Ok) => {}
                    Ok(godly_protocol::Response::Error { message }) => {
                        eprintln!("[mcp] Warning: close session error for {}: {}", tid, message);
                    }
                    _ => {}
                }
                app_state.remove_session_metadata(tid);
                app_state.remove_terminal(tid);
            }

            // Remove the workspace
            app_state.remove_workspace(workspace_id);
            auto_save.mark_dirty();

            let _ = app_handle.emit("mcp-workspace-deleted", workspace_id);

            McpResponse::Ok
        }

        McpRequest::GetActiveWorkspace => {
            let active_id = app_state.active_workspace_id.read().clone();
            let workspace = active_id
                .as_deref()
                .and_then(|id| app_state.get_workspace(id))
                .map(|w| McpWorkspaceInfo {
                    id: w.id,
                    name: w.name,
                    folder_path: w.folder_path,
                });
            McpResponse::ActiveWorkspace { workspace }
        }

        McpRequest::GetActiveTerminal => {
            let active_id = app_state.get_active_terminal_id();
            let terminal = active_id
                .as_deref()
                .and_then(|id| {
                    let terminals = app_state.terminals.read();
                    terminals.get(id).map(|t| McpTerminalInfo {
                        id: t.id.clone(),
                        workspace_id: t.workspace_id.clone(),
                        name: t.name.clone(),
                        process_name: t.process_name.clone(),
                    })
                });
            McpResponse::ActiveTerminal { terminal }
        }

        McpRequest::SwitchWorkspace { workspace_id } => {
            // Validate workspace exists
            if !app_state.workspaces.read().contains_key(workspace_id) {
                return McpResponse::Error {
                    message: format!("Workspace {} not found", workspace_id),
                };
            }

            let _ = app_handle.emit("switch-workspace", workspace_id);
            McpResponse::Ok
        }

        McpRequest::MoveTerminalToWorkspace {
            terminal_id,
            workspace_id,
        } => {
            // Validate terminal exists
            if !app_state.terminals.read().contains_key(terminal_id) {
                return McpResponse::Error {
                    message: format!("Terminal {} not found", terminal_id),
                };
            }
            // Validate target workspace exists
            if !app_state.workspaces.read().contains_key(workspace_id) {
                return McpResponse::Error {
                    message: format!("Workspace {} not found", workspace_id),
                };
            }

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

        McpRequest::ResizeTerminal {
            terminal_id,
            rows,
            cols,
        } => {
            // Validate terminal exists
            if !app_state.terminals.read().contains_key(terminal_id) {
                return McpResponse::Error {
                    message: format!("Terminal {} not found", terminal_id),
                };
            }

            let request = godly_protocol::Request::Resize {
                session_id: terminal_id.clone(),
                rows: *rows,
                cols: *cols,
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

        McpRequest::RemoveWorktree { worktree_path } => {
            // Find a workspace whose folder_path is a git repo containing this worktree
            let workspaces = app_state.get_all_workspaces();
            let mut repo_root_found: Option<String> = None;
            for ws in &workspaces {
                if crate::worktree::is_git_repo(&ws.folder_path, None) {
                    if let Ok(root) = crate::worktree::get_repo_root(&ws.folder_path, None) {
                        repo_root_found = Some(root);
                        break;
                    }
                }
            }
            match repo_root_found {
                Some(repo_root) => {
                    match crate::worktree::remove_worktree(&repo_root, worktree_path, true, None) {
                        Ok(()) => McpResponse::Ok,
                        Err(e) => McpResponse::Error {
                            message: format!("Failed to remove worktree: {}", e),
                        },
                    }
                }
                None => McpResponse::Error {
                    message: "No workspace with a git repo found to resolve worktree".to_string(),
                },
            }
        }

        McpRequest::ReadGrid { terminal_id } => {
            let request = godly_protocol::Request::ReadGrid {
                session_id: terminal_id.clone(),
            };
            match daemon.send_request(&request) {
                Ok(godly_protocol::Response::Grid { grid }) => McpResponse::GridSnapshot {
                    rows: grid.rows,
                    cursor_row: grid.cursor_row,
                    cursor_col: grid.cursor_col,
                    cols: grid.cols,
                    num_rows: grid.num_rows,
                    alternate_screen: grid.alternate_screen,
                },
                Ok(godly_protocol::Response::Error { message }) => {
                    McpResponse::Error { message }
                }
                Ok(other) => McpResponse::Error {
                    message: format!("Unexpected response: {:?}", other),
                },
                Err(e) => McpResponse::Error { message: e },
            }
        }

        McpRequest::ReadTerminal {
            terminal_id,
            mode,
            lines,
            strip_ansi: do_strip,
        } => {
            let request = godly_protocol::Request::ReadBuffer {
                session_id: terminal_id.clone(),
            };
            match daemon.send_request(&request) {
                Ok(godly_protocol::Response::Buffer { data, .. }) => {
                    let text = String::from_utf8_lossy(&data).into_owned();
                    let mut content = truncate_output(&text, mode.as_deref(), *lines);
                    if do_strip.unwrap_or(false) {
                        content = strip_ansi(&content);
                    }
                    McpResponse::TerminalOutput { content }
                }
                Ok(godly_protocol::Response::Error { message }) => {
                    McpResponse::Error { message }
                }
                Ok(other) => McpResponse::Error {
                    message: format!("Unexpected response: {:?}", other),
                },
                Err(e) => McpResponse::Error { message: e },
            }
        }

        McpRequest::WriteToTerminal { terminal_id, data } => {
            // Convert \n → \r for PTY: terminals expect CR (Enter), not LF.
            // Normalize \r\n → \r first to avoid double CR.
            // Only affects MCP path — frontend xterm.js already sends correct bytes.
            let converted = data.replace("\r\n", "\r").replace('\n', "\r");
            let request = godly_protocol::Request::Write {
                session_id: terminal_id.clone(),
                data: converted.as_bytes().to_vec(),
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

        McpRequest::WaitForIdle {
            terminal_id,
            idle_ms,
            timeout_ms,
        } => {
            // Validate terminal exists in app state
            if !app_state.terminals.read().contains_key(terminal_id) {
                return McpResponse::Error {
                    message: format!("Terminal {} not found", terminal_id),
                };
            }

            let deadline = std::time::Instant::now()
                + std::time::Duration::from_millis(*timeout_ms);
            // Poll interval: min(idle_ms/4, 500ms), clamped to min 50ms
            let poll_ms = (*idle_ms / 4).min(500).max(50);

            loop {
                let req = godly_protocol::Request::GetLastOutputTime {
                    session_id: terminal_id.clone(),
                };
                match daemon.send_request(&req) {
                    Ok(godly_protocol::Response::LastOutputTime { epoch_ms, running }) => {
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        let ago = now_ms.saturating_sub(epoch_ms);

                        if ago >= *idle_ms {
                            return McpResponse::WaitResult {
                                completed: true,
                                last_output_ago_ms: ago,
                            };
                        }

                        if !running {
                            // Process exited — it's idle by definition
                            return McpResponse::WaitResult {
                                completed: true,
                                last_output_ago_ms: ago,
                            };
                        }

                        if std::time::Instant::now() >= deadline {
                            return McpResponse::WaitResult {
                                completed: false,
                                last_output_ago_ms: ago,
                            };
                        }
                    }
                    Ok(godly_protocol::Response::Error { message }) => {
                        return McpResponse::Error { message };
                    }
                    Ok(_) => {
                        return McpResponse::Error {
                            message: "Unexpected daemon response".to_string(),
                        };
                    }
                    Err(e) => {
                        return McpResponse::Error { message: e };
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
            // Validate terminal exists in app state
            if !app_state.terminals.read().contains_key(terminal_id) {
                return McpResponse::Error {
                    message: format!("Terminal {} not found", terminal_id),
                };
            }

            let deadline = std::time::Instant::now()
                + std::time::Duration::from_millis(*timeout_ms);
            let poll_ms = 200u64;

            loop {
                let req = godly_protocol::Request::SearchBuffer {
                    session_id: terminal_id.clone(),
                    text: text.clone(),
                    strip_ansi: true, // always strip ANSI for matching
                };
                match daemon.send_request(&req) {
                    Ok(godly_protocol::Response::SearchResult { found, running }) => {
                        if found {
                            let now_ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64;
                            // Get actual last output time for the response
                            let ago = match daemon.send_request(
                                &godly_protocol::Request::GetLastOutputTime {
                                    session_id: terminal_id.clone(),
                                },
                            ) {
                                Ok(godly_protocol::Response::LastOutputTime {
                                    epoch_ms, ..
                                }) => now_ms.saturating_sub(epoch_ms),
                                _ => 0,
                            };
                            return McpResponse::WaitResult {
                                completed: true,
                                last_output_ago_ms: ago,
                            };
                        }

                        if !running {
                            // Process exited and text wasn't found
                            return McpResponse::WaitResult {
                                completed: false,
                                last_output_ago_ms: 0,
                            };
                        }

                        if std::time::Instant::now() >= deadline {
                            return McpResponse::WaitResult {
                                completed: false,
                                last_output_ago_ms: 0,
                            };
                        }
                    }
                    Ok(godly_protocol::Response::Error { message }) => {
                        return McpResponse::Error { message };
                    }
                    Ok(_) => {
                        return McpResponse::Error {
                            message: "Unexpected daemon response".to_string(),
                        };
                    }
                    Err(e) => {
                        return McpResponse::Error { message: e };
                    }
                }

                std::thread::sleep(std::time::Duration::from_millis(poll_ms));
            }
        }
    }
}

/// Convert app ShellType to protocol ShellType
fn to_protocol_shell_type(st: &crate::state::ShellType) -> godly_protocol::ShellType {
    match st {
        crate::state::ShellType::Windows => godly_protocol::ShellType::Windows,
        crate::state::ShellType::Pwsh => godly_protocol::ShellType::Pwsh,
        crate::state::ShellType::Cmd => godly_protocol::ShellType::Cmd,
        crate::state::ShellType::Wsl { distribution } => godly_protocol::ShellType::Wsl {
            distribution: distribution.clone(),
        },
        crate::state::ShellType::Custom { program, args } => godly_protocol::ShellType::Custom {
            program: program.clone(),
            args: args.clone(),
        },
    }
}

/// Re-export truncate_output from the shared protocol crate.
fn truncate_output(text: &str, mode: Option<&str>, lines: Option<usize>) -> String {
    godly_protocol::ansi::truncate_output(text, mode, lines)
}

/// Convert protocol ShellType to app ShellType
fn from_protocol_shell_type(st: &godly_protocol::ShellType) -> crate::state::ShellType {
    match st {
        godly_protocol::ShellType::Windows => crate::state::ShellType::Windows,
        godly_protocol::ShellType::Pwsh => crate::state::ShellType::Pwsh,
        godly_protocol::ShellType::Cmd => crate::state::ShellType::Cmd,
        godly_protocol::ShellType::Wsl { distribution } => crate::state::ShellType::Wsl {
            distribution: distribution.clone(),
        },
        godly_protocol::ShellType::Custom { program, args } => crate::state::ShellType::Custom {
            program: program.clone(),
            args: args.clone(),
        },
    }
}

/// Re-export strip_ansi from the shared protocol crate.
fn strip_ansi(input: &str) -> String {
    godly_protocol::ansi::strip_ansi(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_output_tail_default() {
        let text = (1..=200).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        let result = truncate_output(&text, None, None);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 100);
        assert_eq!(lines[0], "line 101");
        assert_eq!(lines[99], "line 200");
    }

    #[test]
    fn test_truncate_output_head() {
        let text = (1..=200).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        let result = truncate_output(&text, Some("head"), Some(5));
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "line 1");
        assert_eq!(lines[4], "line 5");
    }

    #[test]
    fn test_truncate_output_tail_custom_lines() {
        let text = (1..=50).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        let result = truncate_output(&text, Some("tail"), Some(10));
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0], "line 41");
        assert_eq!(lines[9], "line 50");
    }

    #[test]
    fn test_truncate_output_full() {
        let text = "line 1\nline 2\nline 3";
        let result = truncate_output(text, Some("full"), None);
        assert_eq!(result, text);
    }

    #[test]
    fn test_truncate_output_fewer_lines_than_requested() {
        let text = "line 1\nline 2\nline 3";
        let result = truncate_output(text, Some("tail"), Some(100));
        assert_eq!(result, text);
    }

    #[test]
    fn test_truncate_output_empty() {
        let result = truncate_output("", None, None);
        assert_eq!(result, "");
    }

    #[test]
    fn test_strip_ansi_csi_sequences() {
        // SGR (color) sequences
        assert_eq!(strip_ansi("\x1b[31mhello\x1b[0m"), "hello");
        // Cursor movement
        assert_eq!(strip_ansi("\x1b[2Jscreen"), "screen");
        // Multiple params
        assert_eq!(strip_ansi("\x1b[1;32mbold green\x1b[0m"), "bold green");
    }

    #[test]
    fn test_strip_ansi_osc_with_bel() {
        // OSC title set terminated by BEL
        assert_eq!(strip_ansi("\x1b]0;My Title\x07prompt$"), "prompt$");
    }

    #[test]
    fn test_strip_ansi_osc_with_st() {
        // OSC sequence terminated by ST (\x1b\\)
        assert_eq!(strip_ansi("\x1b]0;Title\x1b\\text"), "text");
    }

    #[test]
    fn test_strip_ansi_no_escapes() {
        assert_eq!(strip_ansi("plain text"), "plain text");
    }

    #[test]
    fn test_strip_ansi_empty() {
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn test_strip_ansi_mixed() {
        let input = "\x1b[32mPS C:\\>\x1b[0m echo \x1b]0;powershell\x07hello";
        assert_eq!(strip_ansi(input), "PS C:\\> echo hello");
    }

    #[test]
    fn test_strip_ansi_two_byte_sequence() {
        // e.g. \x1b= (set alternate keypad mode)
        assert_eq!(strip_ansi("\x1b=text"), "text");
    }
}
