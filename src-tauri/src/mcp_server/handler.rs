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

/// Auto-focus a terminal in the UI so the user sees the terminal being acted on.
/// Also switches workspace if the terminal is in a different workspace than the current view.
fn auto_focus_terminal(
    terminal_id: &str,
    app_state: &Arc<AppState>,
    app_handle: &AppHandle,
) {
    // Look up the terminal's workspace
    let terminal_workspace = app_state
        .terminals
        .read()
        .get(terminal_id)
        .map(|t| t.workspace_id.clone());

    // Switch workspace if needed
    if let Some(ref ws_id) = terminal_workspace {
        let _ = app_handle.emit("switch-workspace", ws_id);
    }

    app_state.set_active_terminal_id(Some(terminal_id.to_string()));
    let _ = app_handle.emit("focus-terminal", terminal_id.to_string());
}

/// Handle an MCP request by delegating to AppState and DaemonClient.
#[allow(deprecated)]
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

            // Auto-focus the new terminal so the user sees it
            auto_focus_terminal(&terminal_id, app_state, app_handle);

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
            no_worktree,
        } => {
            use std::collections::HashMap;
            use uuid::Uuid;

            let use_worktree = !no_worktree.unwrap_or(false);

            // Auto-generate branch name from prompt if not provided
            let branch_name = if use_worktree && branch_name.is_none() {
                if let Some(api_key) = llm_state.get_api_key() {
                    let model = llm_state.get_model();
                    tokio::runtime::Runtime::new()
                        .ok()
                        .and_then(|rt| {
                            rt.block_on(godly_llm::generate_branch_name_gemini(&api_key, prompt, &model))
                                .ok()
                        })
                        .filter(|name| godly_llm::is_quality_branch_name(name))
                } else {
                    None
                }
            } else {
                branch_name.clone()
            };

            // MCP terminals always go into the Agent workspace (separate window)
            let workspace_id = &ensure_mcp_workspace(app_state);

            let terminal_id = Uuid::new_v4().to_string();

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

                if use_worktree {
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
                } else {
                    // No worktree — open in workspace directory (main branch)
                    Some(ws.folder_path.clone())
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
            // Auto-focus so the user sees the terminal being typed into
            auto_focus_terminal(terminal_id, app_state, app_handle);

            // Convert newlines → \r for PTY: terminals expect CR (Enter), not LF.
            // Also handle literal escape sequences (\\n, \\r\\n) since LLMs often
            // produce these as text instead of actual newline characters.
            // Order matters: literal sequences first, then real chars.
            let converted = data
                .replace("\\r\\n", "\r")
                .replace("\\n", "\r")
                .replace("\r\n", "\r")
                .replace('\n', "\r");
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

        McpRequest::SendKeys { terminal_id, keys } => {
            // Validate terminal exists
            if !app_state.terminals.read().contains_key(terminal_id) {
                return McpResponse::Error {
                    message: format!("Terminal {} not found", terminal_id),
                };
            }

            // Auto-focus so the user sees the keystrokes
            auto_focus_terminal(terminal_id, app_state, app_handle);

            // Convert each key name to bytes and concatenate
            let mut all_bytes = Vec::new();
            for key in keys {
                match godly_protocol::keys::key_to_bytes(key) {
                    Ok(bytes) => all_bytes.extend(bytes),
                    Err(e) => return McpResponse::Error { message: e },
                }
            }
            let request = godly_protocol::Request::Write {
                session_id: terminal_id.clone(),
                data: all_bytes,
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

        McpRequest::EraseContent { terminal_id, count } => {
            // Validate terminal exists
            if !app_state.terminals.read().contains_key(terminal_id) {
                return McpResponse::Error {
                    message: format!("Terminal {} not found", terminal_id),
                };
            }

            // Auto-focus so the user sees the erasure
            auto_focus_terminal(terminal_id, app_state, app_handle);

            let backspaces = vec![0x08u8; *count];
            let request = godly_protocol::Request::Write {
                session_id: terminal_id.clone(),
                data: backspaces,
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

        McpRequest::ExecuteCommand {
            terminal_id,
            command,
            idle_ms,
            timeout_ms,
        } => {
            // Validate terminal exists
            if !app_state.terminals.read().contains_key(terminal_id) {
                return McpResponse::Error {
                    message: format!("Terminal {} not found", terminal_id),
                };
            }

            // Auto-focus so the user sees the command executing
            auto_focus_terminal(terminal_id, app_state, app_handle);

            // 1. Snapshot buffer length before command
            let before_len = match daemon.send_request(&godly_protocol::Request::ReadBuffer {
                session_id: terminal_id.clone(),
            }) {
                Ok(godly_protocol::Response::Buffer { data, .. }) => data.len(),
                Ok(godly_protocol::Response::Error { message }) => {
                    return McpResponse::Error { message };
                }
                Err(e) => return McpResponse::Error { message: e },
                _ => 0,
            };

            // 2. Write command + Enter
            let write_req = godly_protocol::Request::Write {
                session_id: terminal_id.clone(),
                data: format!("{}\r", command).into_bytes(),
            };
            match daemon.send_request(&write_req) {
                Ok(godly_protocol::Response::Ok) => {}
                Ok(godly_protocol::Response::Error { message }) => {
                    return McpResponse::Error { message };
                }
                Err(e) => return McpResponse::Error { message: e },
                _ => {}
            }

            // 3. Wait for idle (reuses same pattern as WaitForIdle handler)
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_millis(*timeout_ms);
            let poll_ms = (*idle_ms / 4).min(500).max(50);
            let mut completed = false;
            let mut last_ago = 0u64;
            let mut running = true;
            let mut input_expected = None;

            loop {
                let req = godly_protocol::Request::GetLastOutputTime {
                    session_id: terminal_id.clone(),
                };
                match daemon.send_request(&req) {
                    Ok(godly_protocol::Response::LastOutputTime {
                        epoch_ms,
                        running: is_running,
                        input_expected: ie,
                        ..
                    }) => {
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
                    Ok(godly_protocol::Response::Error { message }) => {
                        return McpResponse::Error { message };
                    }
                    Err(e) => {
                        return McpResponse::Error { message: e };
                    }
                    _ => {
                        return McpResponse::Error {
                            message: "Unexpected daemon response".to_string(),
                        };
                    }
                }

                std::thread::sleep(std::time::Duration::from_millis(poll_ms));
            }

            // 4. Read new output (delta since before_len)
            let output = match daemon.send_request(&godly_protocol::Request::ReadBuffer {
                session_id: terminal_id.clone(),
            }) {
                Ok(godly_protocol::Response::Buffer { data, .. }) => {
                    let new_data = if data.len() > before_len {
                        &data[before_len..]
                    } else {
                        &data[..]
                    };
                    let text = String::from_utf8_lossy(new_data).into_owned();
                    let stripped = strip_ansi(&text);
                    strip_command_echo(&stripped, command)
                }
                Ok(godly_protocol::Response::Error { message }) => {
                    return McpResponse::Error { message };
                }
                Err(e) => return McpResponse::Error { message: e },
                _ => String::new(),
            };

            McpResponse::CommandOutput {
                output,
                completed,
                last_output_ago_ms: last_ago,
                running,
                input_expected,
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
                    Ok(godly_protocol::Response::LastOutputTime { epoch_ms, running, .. }) => {
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

        // === Split view control ===

        McpRequest::CreateSplit {
            workspace_id,
            left_terminal_id,
            right_terminal_id,
            direction,
            ratio,
        } => {
            // Validate both terminals exist
            {
                let terminals = app_state.terminals.read();
                if !terminals.contains_key(left_terminal_id) {
                    return McpResponse::Error {
                        message: format!("Terminal {} not found", left_terminal_id),
                    };
                }
                if !terminals.contains_key(right_terminal_id) {
                    return McpResponse::Error {
                        message: format!("Terminal {} not found", right_terminal_id),
                    };
                }
            }

            // Validate direction
            if direction != "horizontal" && direction != "vertical" {
                return McpResponse::Error {
                    message: format!("Invalid direction '{}', must be 'horizontal' or 'vertical'", direction),
                };
            }

            // Clamp ratio
            let ratio = ratio.clamp(0.15, 0.85);

            // Legacy split view
            app_state.set_split_view(
                workspace_id,
                crate::state::SplitView {
                    left_terminal_id: left_terminal_id.clone(),
                    right_terminal_id: right_terminal_id.clone(),
                    direction: direction.clone(),
                    ratio,
                },
            );

            // Also create a layout tree for backward compat
            let dir = if direction == "vertical" {
                godly_protocol::SplitDirection::Vertical
            } else {
                godly_protocol::SplitDirection::Horizontal
            };
            app_state.set_layout_tree_validated(
                workspace_id,
                godly_protocol::LayoutNode::Split {
                    direction: dir,
                    ratio,
                    first: Box::new(godly_protocol::LayoutNode::Leaf {
                        terminal_id: left_terminal_id.clone(),
                    }),
                    second: Box::new(godly_protocol::LayoutNode::Leaf {
                        terminal_id: right_terminal_id.clone(),
                    }),
                },
            );

            auto_save.mark_dirty();

            #[derive(serde::Serialize, Clone)]
            struct McpSplitPayload {
                workspace_id: String,
                left_terminal_id: String,
                right_terminal_id: String,
                direction: String,
                ratio: f64,
            }
            let _ = app_handle.emit(
                "mcp-set-split-view",
                McpSplitPayload {
                    workspace_id: workspace_id.clone(),
                    left_terminal_id: left_terminal_id.clone(),
                    right_terminal_id: right_terminal_id.clone(),
                    direction: direction.clone(),
                    ratio,
                },
            );

            McpResponse::Ok
        }

        McpRequest::ClearSplit { workspace_id } => {
            app_state.clear_split_view(workspace_id);
            app_state.clear_layout_tree(workspace_id);
            app_state.set_zoomed_pane(workspace_id, None);
            auto_save.mark_dirty();
            let _ = app_handle.emit("mcp-clear-split-view", workspace_id);
            McpResponse::Ok
        }

        McpRequest::GetSplitState { workspace_id } => {
            let views = app_state.get_all_split_views();
            match views.get(workspace_id) {
                Some(sv) => McpResponse::SplitState {
                    workspace_id: workspace_id.clone(),
                    left_terminal_id: sv.left_terminal_id.clone(),
                    right_terminal_id: sv.right_terminal_id.clone(),
                    direction: sv.direction.clone(),
                    ratio: sv.ratio,
                },
                None => McpResponse::NoSplit,
            }
        }

        // === Layout tree commands ===

        McpRequest::SplitTerminal {
            workspace_id,
            target_terminal_id,
            new_terminal_id,
            direction,
            ratio,
        } => {
            // Validate workspace exists
            if !app_state.workspaces.read().contains_key(workspace_id) {
                return McpResponse::Error {
                    message: format!("Workspace {} not found", workspace_id),
                };
            }

            // Validate both terminals exist
            {
                let terminals = app_state.terminals.read();
                if !terminals.contains_key(target_terminal_id) {
                    return McpResponse::Error {
                        message: format!("Terminal {} not found", target_terminal_id),
                    };
                }
                if !terminals.contains_key(new_terminal_id) {
                    return McpResponse::Error {
                        message: format!("Terminal {} not found", new_terminal_id),
                    };
                }
            }

            // Validate direction
            if direction != "horizontal" && direction != "vertical" {
                return McpResponse::Error {
                    message: format!("Invalid direction '{}', must be 'horizontal' or 'vertical'", direction),
                };
            }

            let dir = if direction == "vertical" {
                godly_protocol::SplitDirection::Vertical
            } else {
                godly_protocol::SplitDirection::Horizontal
            };

            if let Err(msg) = app_state.split_terminal_in_tree(
                workspace_id,
                target_terminal_id,
                new_terminal_id,
                dir,
                *ratio,
            ) {
                return McpResponse::Error { message: msg };
            }

            auto_save.mark_dirty();

            #[derive(serde::Serialize, Clone)]
            struct SplitTerminalPayload {
                workspace_id: String,
                target_terminal_id: String,
                new_terminal_id: String,
                direction: String,
                ratio: f64,
            }
            let _ = app_handle.emit(
                "mcp-split-terminal",
                SplitTerminalPayload {
                    workspace_id: workspace_id.clone(),
                    target_terminal_id: target_terminal_id.clone(),
                    new_terminal_id: new_terminal_id.clone(),
                    direction: direction.clone(),
                    ratio: ratio.clamp(0.15, 0.85),
                },
            );

            McpResponse::Ok
        }

        McpRequest::SelfSplit {
            session_id,
            direction,
            ratio,
            cwd,
            command,
        } => {
            use std::collections::HashMap;
            use uuid::Uuid;

            // 1. Look up the calling terminal by session_id
            let (workspace_id, calling_terminal_id) = {
                let terminals = app_state.terminals.read();
                match terminals.get(session_id) {
                    Some(t) => (t.workspace_id.clone(), t.id.clone()),
                    None => {
                        return McpResponse::Error {
                            message: format!(
                                "Session {} not found — is GODLY_SESSION_ID correct?",
                                session_id
                            ),
                        };
                    }
                }
            };

            // 2. Validate direction
            if direction != "horizontal" && direction != "vertical" {
                return McpResponse::Error {
                    message: format!(
                        "Invalid direction '{}', must be 'horizontal' or 'vertical'",
                        direction
                    ),
                };
            }

            // 3. Create new terminal in the SAME workspace as the caller
            let new_terminal_id = Uuid::new_v4().to_string();

            let shell = app_state
                .get_workspace(&workspace_id)
                .map(|ws| to_protocol_shell_type(&ws.shell_type))
                .unwrap_or(godly_protocol::ShellType::Windows);

            let working_dir = if let Some(dir) = cwd {
                Some(dir.clone())
            } else {
                app_state
                    .get_workspace(&workspace_id)
                    .map(|ws| ws.folder_path)
            };

            let process_name = shell.display_name();

            let mut env_vars = HashMap::new();
            env_vars.insert("GODLY_SESSION_ID".to_string(), new_terminal_id.clone());
            env_vars.insert("GODLY_WORKSPACE_ID".to_string(), workspace_id.clone());

            let create_req = godly_protocol::Request::CreateSession {
                id: new_terminal_id.clone(),
                shell_type: shell.clone(),
                cwd: working_dir.clone(),
                rows: 24,
                cols: 80,
                env: Some(env_vars),
            };

            match daemon.send_request(&create_req) {
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
                session_id: new_terminal_id.clone(),
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
                    session_id: new_terminal_id.clone(),
                    data: format!("{}\r", cmd).into_bytes(),
                };
                let _ = daemon.send_request(&write_req);
            }

            // Store metadata
            let app_shell = from_protocol_shell_type(&shell);
            app_state.add_session_metadata(
                new_terminal_id.clone(),
                crate::state::SessionMetadata {
                    shell_type: app_shell,
                    cwd: working_dir,
                    worktree_path: None,
                    worktree_branch: None,
                },
            );

            app_state.add_terminal(crate::state::Terminal {
                id: new_terminal_id.clone(),
                workspace_id: workspace_id.clone(),
                name: String::from("Terminal"),
                process_name,
            });

            // Notify frontend about the new terminal
            #[derive(serde::Serialize, Clone)]
            struct McpTerminalCreatedPayload {
                terminal_id: String,
                workspace_id: String,
            }
            let _ = app_handle.emit(
                "mcp-terminal-created",
                McpTerminalCreatedPayload {
                    terminal_id: new_terminal_id.clone(),
                    workspace_id: workspace_id.clone(),
                },
            );

            // 4. Split the calling terminal pane
            let clamped_ratio = ratio.clamp(0.15, 0.85);
            let dir = if direction == "vertical" {
                godly_protocol::SplitDirection::Vertical
            } else {
                godly_protocol::SplitDirection::Horizontal
            };

            if let Err(msg) = app_state.split_terminal_in_tree(
                &workspace_id,
                &calling_terminal_id,
                &new_terminal_id,
                dir,
                clamped_ratio,
            ) {
                return McpResponse::Error { message: msg };
            }

            auto_save.mark_dirty();

            #[derive(serde::Serialize, Clone)]
            struct SplitTerminalPayload {
                workspace_id: String,
                target_terminal_id: String,
                new_terminal_id: String,
                direction: String,
                ratio: f64,
            }
            let _ = app_handle.emit(
                "mcp-split-terminal",
                SplitTerminalPayload {
                    workspace_id: workspace_id.clone(),
                    target_terminal_id: calling_terminal_id.clone(),
                    new_terminal_id: new_terminal_id.clone(),
                    direction: direction.clone(),
                    ratio: clamped_ratio,
                },
            );

            McpResponse::SplitCreated {
                original_terminal_id: calling_terminal_id,
                new_terminal_id,
                workspace_id,
                direction: direction.clone(),
                ratio: clamped_ratio,
            }
        }

        McpRequest::UnsplitTerminal {
            workspace_id,
            terminal_id,
        } => {
            // Validate workspace exists
            if !app_state.workspaces.read().contains_key(workspace_id) {
                return McpResponse::Error {
                    message: format!("Workspace {} not found", workspace_id),
                };
            }

            if let Err(msg) = app_state.unsplit_terminal_in_tree(workspace_id, terminal_id) {
                return McpResponse::Error { message: msg };
            }

            // Clear zoom if the unsplit removed the zoomed pane
            if let Some(zoomed) = app_state.get_zoomed_pane(workspace_id) {
                if zoomed == *terminal_id {
                    app_state.set_zoomed_pane(workspace_id, None);
                }
            }

            auto_save.mark_dirty();

            #[derive(serde::Serialize, Clone)]
            struct UnsplitPayload {
                workspace_id: String,
                terminal_id: String,
            }
            let _ = app_handle.emit(
                "mcp-unsplit-terminal",
                UnsplitPayload {
                    workspace_id: workspace_id.clone(),
                    terminal_id: terminal_id.clone(),
                },
            );

            McpResponse::Ok
        }

        McpRequest::GetLayoutTree { workspace_id } => {
            McpResponse::LayoutTree(app_state.get_layout_tree(workspace_id))
        }

        McpRequest::SwapPanes {
            workspace_id,
            terminal_id_a,
            terminal_id_b,
        } => {
            // Validate workspace exists
            if !app_state.workspaces.read().contains_key(workspace_id) {
                return McpResponse::Error {
                    message: format!("Workspace {} not found", workspace_id),
                };
            }

            // Validate both terminals exist
            {
                let terminals = app_state.terminals.read();
                if !terminals.contains_key(terminal_id_a) {
                    return McpResponse::Error {
                        message: format!("Terminal {} not found", terminal_id_a),
                    };
                }
                if !terminals.contains_key(terminal_id_b) {
                    return McpResponse::Error {
                        message: format!("Terminal {} not found", terminal_id_b),
                    };
                }
            }

            if let Err(msg) = app_state.swap_panes_in_tree(workspace_id, terminal_id_a, terminal_id_b) {
                return McpResponse::Error { message: msg };
            }

            auto_save.mark_dirty();

            #[derive(serde::Serialize, Clone)]
            struct SwapPayload {
                workspace_id: String,
                terminal_id_a: String,
                terminal_id_b: String,
            }
            let _ = app_handle.emit(
                "mcp-swap-panes",
                SwapPayload {
                    workspace_id: workspace_id.clone(),
                    terminal_id_a: terminal_id_a.clone(),
                    terminal_id_b: terminal_id_b.clone(),
                },
            );

            McpResponse::Ok
        }

        McpRequest::ZoomPane {
            workspace_id,
            terminal_id,
        } => {
            // Validate workspace exists
            if !app_state.workspaces.read().contains_key(workspace_id) {
                return McpResponse::Error {
                    message: format!("Workspace {} not found", workspace_id),
                };
            }

            // If zooming, validate terminal exists
            if let Some(tid) = terminal_id {
                if !app_state.terminals.read().contains_key(tid) {
                    return McpResponse::Error {
                        message: format!("Terminal {} not found", tid),
                    };
                }
            }

            app_state.set_zoomed_pane(workspace_id, terminal_id.clone());

            #[derive(serde::Serialize, Clone)]
            struct ZoomPayload {
                workspace_id: String,
                terminal_id: Option<String>,
            }
            let _ = app_handle.emit(
                "mcp-zoom-pane",
                ZoomPayload {
                    workspace_id: workspace_id.clone(),
                    terminal_id: terminal_id.clone(),
                },
            );

            McpResponse::Ok
        }

        // === Shell settings ===

        McpRequest::ListAvailableShells => {
            McpResponse::AvailableShells {
                shells: vec![
                    "windows".to_string(),
                    "pwsh".to_string(),
                    "cmd".to_string(),
                    "wsl".to_string(),
                    "custom".to_string(),
                ],
            }
        }

        McpRequest::GetDefaultShell => {
            let js_req = McpRequest::ExecuteJs {
                script: r#"
                    const store = window.__TERMINAL_SETTINGS_STORE__;
                    if (!store) return JSON.stringify({ error: '__TERMINAL_SETTINGS_STORE__ not found' });
                    const shell = store.getDefaultShell();
                    return JSON.stringify(shell);
                "#.to_string(),
            };
            match handle_mcp_request(&js_req, app_state, daemon, auto_save, app_handle, llm_state) {
                McpResponse::JsResult { error: Some(e), .. } => McpResponse::Error { message: e },
                McpResponse::JsResult { result: Some(json_str), .. } => {
                    match serde_json::from_str::<serde_json::Value>(&json_str) {
                        Ok(val) => {
                            if let Some(err) = val.get("error").and_then(|e| e.as_str()) {
                                return McpResponse::Error { message: err.to_string() };
                            }
                            let shell_type = val.get("type")
                                .and_then(|t| t.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let wsl_distribution = val.get("distribution")
                                .and_then(|d| d.as_str())
                                .map(String::from);
                            let custom_program = val.get("program")
                                .and_then(|p| p.as_str())
                                .map(String::from);
                            let custom_args = val.get("args")
                                .and_then(|a| a.as_array())
                                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());
                            McpResponse::ShellInfo {
                                shell_type,
                                wsl_distribution,
                                custom_program,
                                custom_args,
                            }
                        }
                        Err(e) => McpResponse::Error {
                            message: format!("Failed to parse shell info: {}", e),
                        },
                    }
                }
                McpResponse::JsResult { result: None, .. } => McpResponse::Error {
                    message: "get_default_shell returned no result".to_string(),
                },
                other => other,
            }
        }

        McpRequest::SetDefaultShell {
            shell_type,
            wsl_distribution,
            custom_program,
            custom_args,
        } => {
            // Validate and build the JS shell object
            let shell_js = match shell_type.as_str() {
                "windows" | "pwsh" | "cmd" => {
                    format!("{{ type: '{}' }}", shell_type)
                }
                "wsl" => {
                    match &wsl_distribution {
                        Some(dist) => format!(
                            "{{ type: 'wsl', distribution: '{}' }}",
                            dist.replace('\'', "\\'")
                        ),
                        None => "{ type: 'wsl' }".to_string(),
                    }
                }
                "custom" => {
                    let program = match &custom_program {
                        Some(p) => p.replace('\'', "\\'"),
                        None => {
                            return McpResponse::Error {
                                message: "custom_program is required for shell_type='custom'".to_string(),
                            };
                        }
                    };
                    match &custom_args {
                        Some(args) => {
                            let args_js: Vec<String> = args.iter()
                                .map(|a| format!("'{}'", a.replace('\'', "\\'")))
                                .collect();
                            format!(
                                "{{ type: 'custom', program: '{}', args: [{}] }}",
                                program,
                                args_js.join(", ")
                            )
                        }
                        None => format!("{{ type: 'custom', program: '{}' }}", program),
                    }
                }
                other => {
                    return McpResponse::Error {
                        message: format!(
                            "Invalid shell_type: '{}'. Valid values: windows, pwsh, cmd, wsl, custom",
                            other
                        ),
                    };
                }
            };

            let js_req = McpRequest::ExecuteJs {
                script: format!(
                    r#"
                    const store = window.__TERMINAL_SETTINGS_STORE__;
                    if (!store) return JSON.stringify({{ error: '__TERMINAL_SETTINGS_STORE__ not found' }});
                    store.setDefaultShell({});
                    return JSON.stringify({{ success: true }});
                    "#,
                    shell_js,
                ),
            };
            match handle_mcp_request(&js_req, app_state, daemon, auto_save, app_handle, llm_state) {
                McpResponse::JsResult { error: Some(e), .. } => McpResponse::Error { message: e },
                McpResponse::JsResult { result: Some(json_str), .. } => {
                    match serde_json::from_str::<serde_json::Value>(&json_str) {
                        Ok(val) => {
                            if let Some(err) = val.get("error").and_then(|e| e.as_str()) {
                                return McpResponse::Error { message: err.to_string() };
                            }
                            McpResponse::Ok
                        }
                        Err(e) => McpResponse::Error {
                            message: format!("Failed to parse set_default_shell result: {}", e),
                        },
                    }
                }
                McpResponse::JsResult { result: None, .. } => McpResponse::Error {
                    message: "set_default_shell returned no result".to_string(),
                },
                other => other,
            }
        }

        // === JS bridge ===

        McpRequest::ExecuteJs { script } => {
            use tauri::Manager;

            let window = match app_handle.get_webview_window("main") {
                Some(w) => w,
                None => {
                    return McpResponse::Error {
                        message: "Main window not found".to_string(),
                    };
                }
            };

            // Generate unique request ID
            let request_id = uuid::Uuid::new_v4().to_string();
            let (tx, rx) = std::sync::mpsc::channel::<(Option<String>, Option<String>)>();

            // Store the sender in the shared state
            {
                let js_state: tauri::State<'_, crate::JsCallbackState> =
                    app_handle.state::<crate::JsCallbackState>();
                js_state.senders.lock().insert(request_id.clone(), tx);
            }

            // Wrap the user's script: execute it, then invoke the callback command
            let wrapped = format!(
                r#"(async () => {{
    try {{
        const __result = await (async () => {{ {script} }})();
        await window.__TAURI__.core.invoke('mcp_js_result', {{
            id: '{request_id}',
            result: JSON.stringify(__result) ?? 'undefined',
            error: null,
        }});
    }} catch (e) {{
        await window.__TAURI__.core.invoke('mcp_js_result', {{
            id: '{request_id}',
            result: null,
            error: e.message || String(e),
        }});
    }}
}})();"#,
            );

            if let Err(e) = window.eval(&wrapped) {
                // Clean up sender
                let js_state: tauri::State<'_, crate::JsCallbackState> =
                    app_handle.state::<crate::JsCallbackState>();
                js_state.senders.lock().remove(&request_id);
                return McpResponse::Error {
                    message: format!("Failed to eval JS: {}", e),
                };
            }

            // Wait for the callback with 10s timeout
            match rx.recv_timeout(std::time::Duration::from_secs(10)) {
                Ok((result, error)) => McpResponse::JsResult { result, error },
                Err(_) => {
                    let js_state: tauri::State<'_, crate::JsCallbackState> =
                        app_handle.state::<crate::JsCallbackState>();
                    js_state.senders.lock().remove(&request_id);
                    McpResponse::Error {
                        message: "JS execution timed out after 10s".to_string(),
                    }
                }
            }
        }

        // === Screenshot capture ===
        //
        // Strategy: Use execute_js to capture the canvas, then write the file
        // from Rust. This avoids passing large base64 strings through Tauri IPC
        // (which causes timeouts). Instead, JS returns just the base64 data via
        // the JsCallback channel, and Rust decodes + saves it.

        McpRequest::CaptureScreenshot { terminal_id } => {
            use tauri::Manager;

            let window = match app_handle.get_webview_window("main") {
                Some(w) => w,
                None => {
                    return McpResponse::Error {
                        message: "Main window not found".to_string(),
                    };
                }
            };

            // Build JS to capture the canvas as a data URL
            let selector = match terminal_id {
                Some(tid) => format!("[data-terminal-id=\"{}\"] canvas", tid),
                None => "canvas".to_string(),
            };

            // Use the JS callback mechanism (same as execute_js) to return the data
            let request_id = uuid::Uuid::new_v4().to_string();
            let (tx, rx) = std::sync::mpsc::channel::<(Option<String>, Option<String>)>();

            {
                let js_state: tauri::State<'_, crate::JsCallbackState> =
                    app_handle.state::<crate::JsCallbackState>();
                js_state.senders.lock().insert(request_id.clone(), tx);
            }

            // JS captures canvas and returns the base64 data (without prefix) to keep it smaller
            let wrapped = format!(
                r#"(async () => {{
    try {{
        const canvas = document.querySelector('{selector}');
        if (!canvas) {{
            await window.__TAURI__.core.invoke('mcp_js_result', {{
                id: '{request_id}',
                result: null,
                error: 'No canvas element found matching selector: {selector}',
            }});
            return;
        }}
        const dataUrl = canvas.toDataURL('image/png');
        // Strip the data:image/png;base64, prefix to reduce transfer size
        const base64 = dataUrl.replace(/^data:image\/png;base64,/, '');
        await window.__TAURI__.core.invoke('mcp_js_result', {{
            id: '{request_id}',
            result: base64,
            error: null,
        }});
    }} catch (e) {{
        await window.__TAURI__.core.invoke('mcp_js_result', {{
            id: '{request_id}',
            result: null,
            error: e.message || String(e),
        }});
    }}
}})();"#,
            );

            if let Err(e) = window.eval(&wrapped) {
                let js_state: tauri::State<'_, crate::JsCallbackState> =
                    app_handle.state::<crate::JsCallbackState>();
                js_state.senders.lock().remove(&request_id);
                return McpResponse::Error {
                    message: format!("Failed to eval screenshot JS: {}", e),
                };
            }

            // Wait for the base64 data
            match rx.recv_timeout(std::time::Duration::from_secs(15)) {
                Ok((Some(base64_data), None)) | Ok((Some(base64_data), Some(_))) if !base64_data.is_empty() => {
                    // Decode base64 and save to file
                    use base64::Engine;
                    match base64::engine::general_purpose::STANDARD.decode(&base64_data) {
                        Ok(bytes) => {
                            let temp_dir = std::env::temp_dir().join("godly-screenshots");
                            let _ = std::fs::create_dir_all(&temp_dir);
                            let path = temp_dir.join(format!("screenshot-{}.png", &request_id[..8]));
                            match std::fs::write(&path, &bytes) {
                                Ok(_) => McpResponse::Screenshot {
                                    path: path.to_string_lossy().to_string(),
                                },
                                Err(e) => McpResponse::Error {
                                    message: format!("Failed to write screenshot: {}", e),
                                },
                            }
                        }
                        Err(e) => McpResponse::Error {
                            message: format!("Failed to decode screenshot base64: {}", e),
                        },
                    }
                }
                Ok((_, Some(error))) => McpResponse::Error {
                    message: format!("Screenshot JS error: {}", error),
                },
                Ok(_) => McpResponse::Error {
                    message: "Screenshot capture returned empty data".to_string(),
                },
                Err(_) => {
                    let js_state: tauri::State<'_, crate::JsCallbackState> =
                        app_handle.state::<crate::JsCallbackState>();
                    js_state.senders.lock().remove(&request_id);
                    McpResponse::Error {
                        message: "Screenshot capture timed out after 15s".to_string(),
                    }
                }
            }
        }

        McpRequest::ExportTerminalInfo { terminal_id } => {
            // Resolve terminal: use provided ID or fall back to active terminal
            let tid = terminal_id
                .clone()
                .or_else(|| app_state.get_active_terminal_id());

            let tid = match tid {
                Some(id) => id,
                None => {
                    return McpResponse::Error {
                        message: "No terminal_id provided and no active terminal".to_string(),
                    };
                }
            };

            let terminals = app_state.terminals.read();
            let terminal = match terminals.get(&tid) {
                Some(t) => t,
                None => {
                    return McpResponse::Error {
                        message: format!("Terminal {} not found", tid),
                    };
                }
            };

            let workspace_id = terminal.workspace_id.clone();
            let terminal_name = terminal.name.clone();
            let terminal_id = terminal.id.clone();
            drop(terminals);

            // Look up workspace name and tab number
            let workspace_name = app_state
                .get_workspace(&workspace_id)
                .map(|w| w.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            let tab_number = app_state
                .get_workspace(&workspace_id)
                .and_then(|w| {
                    w.tab_order
                        .iter()
                        .position(|id| id == &terminal_id)
                        .map(|i| i + 1)
                })
                .unwrap_or(0);

            let tab_label = if tab_number > 0 {
                format!(" (#{tab_number})")
            } else {
                String::new()
            };

            let snippet = format!(
                "Terminal: {}{}\n\
                 Terminal ID: {}\n\
                 Workspace ID: {}\n\
                 Workspace: {}\n\
                 \n\
                 To read this terminal via MCP:\n  \
                 read_terminal(terminal_id=\"{}\")\n  \
                 read_grid(terminal_id=\"{}\")",
                terminal_name, tab_label,
                terminal_id,
                workspace_id,
                workspace_name,
                terminal_id,
                terminal_id,
            );

            McpResponse::TerminalOutput { content: snippet }
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

/// Strip the command echo from terminal output.
///
/// Terminals echo the command back before showing output. If the first line
/// of the output ends with the command text, remove that line.
fn strip_command_echo(text: &str, command: &str) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    let first = lines[0].trim_end();
    let cmd_trimmed = command.trim();
    if first.ends_with(cmd_trimmed) || first == cmd_trimmed {
        lines.remove(0);
    }

    while lines.last().map_or(false, |l| l.trim().is_empty()) {
        lines.pop();
    }

    lines.join("\n")
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
