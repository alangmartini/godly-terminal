use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, State};
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
        ShellType::Pwsh => godly_protocol::ShellType::Pwsh,
        ShellType::Cmd => godly_protocol::ShellType::Cmd,
        ShellType::Wsl { distribution } => godly_protocol::ShellType::Wsl {
            distribution: distribution.clone(),
        },
        ShellType::Custom { program, args } => godly_protocol::ShellType::Custom {
            program: program.clone(),
            args: args.clone(),
        },
    }
}

/// Convert protocol ShellType to app ShellType
fn from_protocol_shell_type(st: &godly_protocol::ShellType) -> ShellType {
    match st {
        godly_protocol::ShellType::Windows => ShellType::Windows,
        godly_protocol::ShellType::Pwsh => ShellType::Pwsh,
        godly_protocol::ShellType::Cmd => ShellType::Cmd,
        godly_protocol::ShellType::Wsl { distribution } => ShellType::Wsl {
            distribution: distribution.clone(),
        },
        godly_protocol::ShellType::Custom { program, args } => ShellType::Custom {
            program: program.clone(),
            args: args.clone(),
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
        if ws.worktree_mode {
            // Auto-detect WSL from the workspace folder path
            let wsl = crate::worktree::WslConfig::from_path(&ws.folder_path);
            // get_repo_root fails if not a git repo, so this combines the
            // is_git_repo + get_repo_root checks into a single subprocess call.
            match crate::worktree::get_repo_root(&ws.folder_path, wsl.as_ref()) {
                Ok(repo_root) => {
                    let custom_name = worktree_name.as_deref();
                    match crate::worktree::create_worktree(&repo_root, &terminal_id, custom_name, wsl.as_ref()) {
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
                Err(_) => {
                    // Not a git repo — fall back to workspace directory
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
    let process_name = shell_type.display_name();

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

    // Track this session so it gets re-attached after daemon reconnection
    daemon.track_attach(terminal_id.clone());

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

    // Stop tracking this session for reconnection
    daemon.track_detach(&terminal_id);

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
    // Fire-and-forget: don't block the Tauri thread pool waiting for the
    // daemon's Ok response. Blocking here caused ~2s input lag under rapid
    // keystrokes (e.g. arrow-up) because threads saturated waiting on IPC.
    daemon.send_fire_and_forget(&request)
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

    // Track this session so it gets re-attached after daemon reconnection
    daemon.track_attach(session_id.clone());

    // Get session info to populate metadata
    let sessions_response = daemon.send_request(&Request::ListSessions)?;
    if let Response::SessionList { sessions } = sessions_response {
        if let Some(info) = sessions.iter().find(|s| s.id == session_id) {
            let shell_type = from_protocol_shell_type(&info.shell_type);
            let process_name = shell_type.display_name();

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

/// Sync the active terminal ID from frontend to backend state (for MCP get_active_terminal)
#[tauri::command]
pub fn sync_active_terminal(
    terminal_id: Option<String>,
    state: State<Arc<AppState>>,
) -> Result<(), String> {
    state.set_active_terminal_id(terminal_id);
    Ok(())
}

// ── Quick Claude ─────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
pub struct QuickClaudeResult {
    pub terminal_id: String,
    pub worktree_branch: Option<String>,
}

/// Create a terminal with a worktree, start Claude Code, and write a prompt — all
/// in the background. Returns immediately with the terminal ID so the caller can
/// fire multiple in rapid succession without waiting.
#[tauri::command]
pub fn quick_claude(
    workspace_id: String,
    prompt: String,
    branch_name: Option<String>,
    skip_fetch: Option<bool>,
    state: State<Arc<AppState>>,
    daemon: State<Arc<DaemonClient>>,
    auto_save: State<Arc<AutoSaveManager>>,
    app_handle: tauri::AppHandle,
) -> Result<QuickClaudeResult, String> {
    let terminal_id = Uuid::new_v4().to_string();

    let workspace = state
        .get_workspace(&workspace_id)
        .ok_or("Workspace not found")?;

    // Determine working directory (worktree or fallback to workspace folder)
    let mut worktree_path_result: Option<String> = None;
    let mut worktree_branch: Option<String> = None;
    let should_skip_fetch = skip_fetch.unwrap_or(true);

    let working_dir = {
        let wsl = crate::worktree::WslConfig::from_path(&workspace.folder_path);
        match crate::worktree::get_repo_root(&workspace.folder_path, wsl.as_ref()) {
            Ok(repo_root) => {
                let custom_name = branch_name.as_deref();
                match crate::worktree::create_worktree_with_options(
                    &repo_root,
                    &terminal_id,
                    custom_name,
                    wsl.as_ref(),
                    should_skip_fetch,
                ) {
                    Ok(wt_result) => {
                        eprintln!(
                            "[quick_claude] Created worktree at: {} (branch: {})",
                            wt_result.path, wt_result.branch
                        );
                        worktree_path_result = Some(wt_result.path.clone());
                        worktree_branch = Some(wt_result.branch);
                        Some(wt_result.path)
                    }
                    Err(e) => {
                        return Err(format!("Worktree creation failed: {}", e));
                    }
                }
            }
            Err(_) => {
                // Not a git repo — fall back to workspace directory
                Some(workspace.folder_path.clone())
            }
        }
    };

    // Shell type from workspace
    let shell_type = workspace.shell_type.clone();
    let process_name = shell_type.display_name();

    // Build env vars
    let mut env_vars = HashMap::new();
    env_vars.insert("GODLY_SESSION_ID".to_string(), terminal_id.clone());
    env_vars.insert("GODLY_WORKSPACE_ID".to_string(), workspace_id.clone());
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let mcp_name = if cfg!(windows) { "godly-mcp.exe" } else { "godly-mcp" };
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

    // Attach
    let attach_request = Request::Attach {
        session_id: terminal_id.clone(),
    };
    let attach_response = daemon.send_request(&attach_request)?;
    match attach_response {
        Response::Ok | Response::Buffer { .. } => {}
        Response::Error { message } => return Err(format!("Failed to attach: {}", message)),
        other => return Err(format!("Unexpected attach response: {:?}", other)),
    }

    daemon.track_attach(terminal_id.clone());

    // Store metadata
    state.add_session_metadata(
        terminal_id.clone(),
        SessionMetadata {
            shell_type,
            cwd: working_dir,
            worktree_path: worktree_path_result,
            worktree_branch: worktree_branch.clone(),
        },
    );

    let terminal_name = worktree_branch
        .clone()
        .unwrap_or_else(|| "Quick Claude".to_string());

    state.add_terminal(Terminal {
        id: terminal_id.clone(),
        workspace_id,
        name: terminal_name.clone(),
        process_name,
    });

    auto_save.mark_dirty();

    // Spawn background thread: start dclaude → wait for ready → write prompt
    let daemon_bg = daemon.inner().clone();
    let app_handle_bg = app_handle.clone();
    let terminal_id_bg = terminal_id.clone();
    std::thread::spawn(move || {
        quick_claude_background(daemon_bg, app_handle_bg, terminal_id_bg, prompt, terminal_name);
    });

    Ok(QuickClaudeResult {
        terminal_id,
        worktree_branch,
    })
}

/// Background task: waits for shell, starts Claude Code, waits for ready, writes prompt.
/// Called from both the Tauri command and MCP handler.
pub(crate) fn quick_claude_background(
    daemon: Arc<DaemonClient>,
    app_handle: tauri::AppHandle,
    terminal_id: String,
    prompt: String,
    display_name: String,
) {
    // Step 1: Wait for shell to be ready (idle for 500ms, timeout 5s)
    let shell_ready = poll_idle(&daemon, &terminal_id, 500, 5_000);
    if !shell_ready {
        eprintln!("[quick_claude] Shell did not become idle in time, writing claude command anyway");
    }

    // Step 2: Write claude command
    let claude_cmd = Request::Write {
        session_id: terminal_id.clone(),
        data: "claude -dangerously-skip-permissions\r".as_bytes().to_vec(),
    };
    if let Err(e) = daemon.send_request(&claude_cmd) {
        eprintln!("[quick_claude] Failed to write claude command: {}", e);
        return;
    }

    // Step 3: Wait for Claude to be ready (idle for 2000ms after startup burst, timeout 60s)
    let claude_ready = poll_idle(&daemon, &terminal_id, 2_000, 60_000);
    if !claude_ready {
        eprintln!("[quick_claude] Claude did not become idle within timeout, writing prompt anyway");
    }

    // Step 4: Small delay to ensure Claude is fully accepting input
    std::thread::sleep(std::time::Duration::from_millis(300));

    // Step 5: Write the prompt (convert \n → \r for PTY)
    let prompt_data = format!("{}\r", prompt.replace("\r\n", "\r").replace('\n', "\r"));
    let prompt_req = Request::Write {
        session_id: terminal_id.clone(),
        data: prompt_data.into_bytes(),
    };
    if let Err(e) = daemon.send_request(&prompt_req) {
        eprintln!("[quick_claude] Failed to write prompt: {}", e);
        return;
    }

    eprintln!("[quick_claude] Prompt delivered to {}", display_name);

    // Step 6: Emit toast notification
    #[derive(serde::Serialize, Clone)]
    struct QuickClaudeReadyPayload {
        terminal_id: String,
        display_name: String,
    }
    let _ = app_handle.emit(
        "quick-claude-ready",
        QuickClaudeReadyPayload {
            terminal_id,
            display_name,
        },
    );
}

/// Poll until the terminal has been idle for `idle_ms` milliseconds, or timeout.
fn poll_idle(daemon: &DaemonClient, session_id: &str, idle_ms: u64, timeout_ms: u64) -> bool {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    let poll_interval = (idle_ms / 4).min(500).max(50);

    loop {
        let req = Request::GetLastOutputTime {
            session_id: session_id.to_string(),
        };
        match daemon.send_request(&req) {
            Ok(Response::LastOutputTime { epoch_ms, running }) => {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let ago = now_ms.saturating_sub(epoch_ms);

                if ago >= idle_ms {
                    return true;
                }
                if !running {
                    return true;
                }
            }
            Ok(_) | Err(_) => {
                // Can't reach daemon or unexpected response — bail out
                return false;
            }
        }

        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(poll_interval));
    }
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
        daemon.track_detach(terminal_id);
    }
    Ok(())
}
