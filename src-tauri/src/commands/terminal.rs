use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

use crate::daemon_client::DaemonClient;
use crate::persistence::AutoSaveManager;
use crate::state::{AppState, SessionMetadata, ShellType, Terminal};

use godly_protocol::{Request, Response, SessionInfo};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CreateTerminalResult {
    pub id: String,
    pub worktree_branch: Option<String>,
}

/// Convert app ShellType to protocol ShellType
fn to_protocol_shell_type(st: &ShellType) -> godly_protocol::ShellType {
    match st {
        ShellType::Windows => godly_protocol::ShellType::Windows,
        ShellType::Wsl { distribution } => godly_protocol::ShellType::Wsl {
            distribution: distribution.clone(),
        },
    }
}

/// Convert protocol ShellType to app ShellType
fn from_protocol_shell_type(st: &godly_protocol::ShellType) -> ShellType {
    match st {
        godly_protocol::ShellType::Windows => ShellType::Windows,
        godly_protocol::ShellType::Wsl { distribution } => ShellType::Wsl {
            distribution: distribution.clone(),
        },
    }
}

#[tauri::command]
pub fn create_terminal(
    workspace_id: String,
    cwd_override: Option<String>,
    shell_type_override: Option<ShellType>,
    id_override: Option<String>,
    worktree_name: Option<String>,
    name_override: Option<String>,
    state: State<Arc<AppState>>,
    daemon: State<Arc<DaemonClient>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<CreateTerminalResult, String> {
    let terminal_id = id_override.unwrap_or_else(|| Uuid::new_v4().to_string());

    // Get workspace info
    let workspace = state.get_workspace(&workspace_id);

    // Determine working directory: cwd_override > worktree (if enabled) > workspace folder
    let mut worktree_path_result: Option<String> = None;
    let mut worktree_branch: Option<String> = None;
    let working_dir = if let Some(cwd) = cwd_override {
        // Explicit CWD override (e.g., restore) - skip worktree creation
        Some(cwd)
    } else if let Some(ref ws) = workspace {
        if ws.worktree_mode && crate::worktree::is_git_repo(&ws.folder_path) {
            // Worktree mode enabled and it's a git repo - create a worktree
            match crate::worktree::get_repo_root(&ws.folder_path) {
                Ok(repo_root) => {
                    let custom_name = worktree_name.as_deref();
                    match crate::worktree::create_worktree(&repo_root, &terminal_id, custom_name) {
                        Ok(wt_result) => {
                            eprintln!("[terminal] Created worktree at: {} (branch: {})", wt_result.path, wt_result.branch);
                            worktree_path_result = Some(wt_result.path.clone());
                            worktree_branch = Some(wt_result.branch);
                            Some(wt_result.path)
                        }
                        Err(e) => {
                            eprintln!("[terminal] Warning: worktree creation failed, falling back to workspace dir: {}", e);
                            Some(ws.folder_path.clone())
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[terminal] Warning: could not get repo root, falling back to workspace dir: {}", e);
                    Some(ws.folder_path.clone())
                }
            }
        } else {
            Some(ws.folder_path.clone())
        }
    } else {
        None
    };

    // Determine shell type: shell_type_override > workspace shell type > default
    let shell_type = shell_type_override
        .or_else(|| workspace.as_ref().map(|w| w.shell_type.clone()))
        .unwrap_or_default();

    // Determine initial process name based on shell type
    let process_name = match &shell_type {
        ShellType::Windows => String::from("powershell"),
        ShellType::Wsl { distribution } => {
            distribution.clone().unwrap_or_else(|| String::from("wsl"))
        }
    };

    // Build environment variables for the PTY session
    let mut env_vars = HashMap::new();
    env_vars.insert("GODLY_SESSION_ID".to_string(), terminal_id.clone());
    env_vars.insert("GODLY_WORKSPACE_ID".to_string(), workspace_id.clone());

    // Provide path to godly-mcp binary so tools can discover it
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let mcp_name = if cfg!(windows) {
                "godly-mcp.exe"
            } else {
                "godly-mcp"
            };
            let mcp_path = exe_dir.join(mcp_name);
            if mcp_path.exists() {
                env_vars.insert(
                    "GODLY_MCP_BINARY".to_string(),
                    mcp_path.to_string_lossy().to_string(),
                );
            }
        }
    }

    // Create session via daemon
    let request = Request::CreateSession {
        id: terminal_id.clone(),
        shell_type: to_protocol_shell_type(&shell_type),
        cwd: working_dir.clone(),
        rows: 24,
        cols: 80,
        env: Some(env_vars),
    };

    let response = daemon.send_request(&request)?;
    match response {
        Response::SessionCreated { .. } => {}
        Response::Error { message } => return Err(message),
        other => return Err(format!("Unexpected response: {:?}", other)),
    }

    // Attach to the session to start receiving output
    let attach_request = Request::Attach {
        session_id: terminal_id.clone(),
    };
    let attach_response = daemon.send_request(&attach_request)?;
    match attach_response {
        Response::Ok | Response::Buffer { .. } => {}
        Response::Error { message } => return Err(format!("Failed to attach: {}", message)),
        other => return Err(format!("Unexpected attach response: {:?}", other)),
    }

    // Store session metadata for persistence
    state.add_session_metadata(
        terminal_id.clone(),
        SessionMetadata {
            shell_type: shell_type.clone(),
            cwd: working_dir,
            worktree_path: worktree_path_result,
            worktree_branch: worktree_branch.clone(),
        },
    );

    // Create terminal record
    let terminal = Terminal {
        id: terminal_id.clone(),
        workspace_id,
        name: name_override.unwrap_or_else(|| String::from("Terminal")),
        process_name,
    };
    state.add_terminal(terminal);

    auto_save.mark_dirty();

    Ok(CreateTerminalResult { id: terminal_id, worktree_branch })
}

#[tauri::command]
pub fn close_terminal(
    terminal_id: String,
    state: State<Arc<AppState>>,
    daemon: State<Arc<DaemonClient>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    // Close session via daemon
    let request = Request::CloseSession {
        session_id: terminal_id.clone(),
    };
    let response = daemon.send_request(&request)?;
    match response {
        Response::Ok => {}
        Response::Error { message } => {
            eprintln!("[terminal] Warning: close session error: {}", message);
        }
        _ => {}
    }

    // Remove metadata and terminal record
    state.remove_session_metadata(&terminal_id);
    state.remove_terminal(&terminal_id);

    auto_save.mark_dirty();

    Ok(())
}

#[tauri::command]
pub fn write_to_terminal(
    terminal_id: String,
    data: String,
    daemon: State<Arc<DaemonClient>>,
) -> Result<(), String> {
    let request = Request::Write {
        session_id: terminal_id,
        data: data.into_bytes(),
    };
    let response = daemon.send_request(&request)?;
    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub fn resize_terminal(
    terminal_id: String,
    rows: u16,
    cols: u16,
    daemon: State<Arc<DaemonClient>>,
) -> Result<(), String> {
    let request = Request::Resize {
        session_id: terminal_id,
        rows,
        cols,
    };
    let response = daemon.send_request(&request)?;
    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub fn rename_terminal(
    terminal_id: String,
    name: String,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    state.update_terminal_name(&terminal_id, name);
    auto_save.mark_dirty();
    Ok(())
}

/// List live sessions from the daemon (for reconnection on app restart)
#[tauri::command]
pub fn reconnect_sessions(
    daemon: State<Arc<DaemonClient>>,
) -> Result<Vec<SessionInfo>, String> {
    let request = Request::ListSessions;
    let response = daemon.send_request(&request)?;
    match response {
        Response::SessionList { sessions } => Ok(sessions),
        Response::Error { message } => Err(message),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Attach to an existing daemon session (for reconnection)
#[tauri::command]
pub fn attach_session(
    session_id: String,
    workspace_id: String,
    name: String,
    state: State<Arc<AppState>>,
    daemon: State<Arc<DaemonClient>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    let request = Request::Attach {
        session_id: session_id.clone(),
    };
    let response = daemon.send_request(&request)?;

    match response {
        Response::Ok | Response::Buffer { .. } => {}
        Response::Error { message } => return Err(message),
        other => return Err(format!("Unexpected response: {:?}", other)),
    }

    // Get session info to populate metadata
    let sessions_response = daemon.send_request(&Request::ListSessions)?;
    if let Response::SessionList { sessions } = sessions_response {
        if let Some(info) = sessions.iter().find(|s| s.id == session_id) {
            let shell_type = from_protocol_shell_type(&info.shell_type);
            let process_name = match &shell_type {
                ShellType::Windows => String::from("powershell"),
                ShellType::Wsl { distribution } => {
                    distribution.clone().unwrap_or_else(|| String::from("wsl"))
                }
            };

            state.add_session_metadata(
                session_id.clone(),
                SessionMetadata {
                    shell_type,
                    cwd: info.cwd.clone(),
                    worktree_path: None,
                    worktree_branch: None,
                },
            );

            state.add_terminal(Terminal {
                id: session_id,
                workspace_id,
                name,
                process_name,
            });

            auto_save.mark_dirty();
        }
    }

    Ok(())
}

/// Detach all sessions (called on window close instead of killing them)
#[tauri::command]
pub fn detach_all_sessions(
    daemon: State<Arc<DaemonClient>>,
    state: State<Arc<AppState>>,
) -> Result<(), String> {
    let terminals = state.terminals.read();
    for terminal_id in terminals.keys() {
        let request = Request::Detach {
            session_id: terminal_id.clone(),
        };
        let _ = daemon.send_request(&request);
    }
    Ok(())
}
