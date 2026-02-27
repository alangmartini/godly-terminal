use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, State};
use uuid::Uuid;

use crate::daemon_client::DaemonClient;
use crate::llm_state::LlmState;
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
    // Convert \n → \r for PTY: terminals expect CR (Enter), not LF.
    // Frontend already sends \r for Enter, so this is a no-op for keyboard input
    // but fixes programmatic writes (MCP, paste) that use \n.
    let converted = data
        .replace("\r\n", "\r")
        .replace('\n', "\r");
    let request = Request::Write {
        session_id: terminal_id,
        data: converted.into_bytes(),
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
    // Fire-and-forget: don't block the Tauri thread pool waiting for the
    // daemon's Ok response. Blocking here caused the terminal to freeze when
    // maximizing with an active TUI (e.g. Claude Code) — each synchronous
    // resize took 1-4s during heavy output because the client pipe was flooded
    // with output events that had to be drained before the response arrived.
    // With 10+ rapid resize events from the maximize animation, the thread pool
    // was blocked for 10-30s. See #244.
    daemon.send_fire_and_forget(&request)
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
pub async fn quick_claude(
    workspace_id: String,
    prompt: String,
    branch_name: Option<String>,
    skip_fetch: Option<bool>,
    no_worktree: Option<bool>,
    state: State<'_, Arc<AppState>>,
    daemon: State<'_, Arc<DaemonClient>>,
    auto_save: State<'_, Arc<AutoSaveManager>>,
    llm: State<'_, Arc<LlmState>>,
    app_handle: tauri::AppHandle,
) -> Result<QuickClaudeResult, String> {
    let terminal_id = Uuid::new_v4().to_string();

    let workspace = state
        .get_workspace(&workspace_id)
        .ok_or("Workspace not found")?;

    let use_worktree = !no_worktree.unwrap_or(false);

    // Auto-generate branch name from prompt if not provided
    let branch_name = if use_worktree && branch_name.is_none() {
        if let Some(api_key) = llm.get_api_key() {
            let model = llm.get_model();
            match godly_llm::generate_branch_name_gemini(&api_key, &prompt, &model).await {
                Ok(name) if godly_llm::is_quality_branch_name(&name) => Some(name),
                _ => None,
            }
        } else {
            None
        }
    } else {
        branch_name
    };

    // Determine working directory (worktree or fallback to workspace folder)
    let mut worktree_path_result: Option<String> = None;
    let mut worktree_branch: Option<String> = None;

    let working_dir = if use_worktree {
        let should_skip_fetch = skip_fetch.unwrap_or(true);
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
    } else {
        // No worktree — open in workspace directory (main branch)
        Some(workspace.folder_path.clone())
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
        data: "claude --dangerously-skip-permissions\r".as_bytes().to_vec(),
    };
    if let Err(e) = daemon.send_request(&claude_cmd) {
        eprintln!("[quick_claude] Failed to write claude command: {}", e);
        return;
    }

    // Step 3: Wait for Claude to be ready.
    // First, give Claude Code time to start and produce its initial output (logo,
    // version, config loading). Without this, poll_idle would see stale output
    // timestamps from the shell echo and return immediately.
    std::thread::sleep(std::time::Duration::from_millis(5_000));

    // Step 3b: Poll for idle, checking for trust prompt on each iteration.
    // Claude Code's ink TUI blinks the cursor every ~500ms, producing output
    // that resets the idle timer. Use a 400ms threshold to detect the gap
    // between blinks. Timeout after 25s (30s total with the 5s above).
    // If a trust prompt appears at any point, auto-accept it and keep polling.
    let claude_ready = poll_idle_or_trust(&daemon, &terminal_id, 400, 25_000);
    if !claude_ready {
        eprintln!("[quick_claude] Claude did not become idle within timeout, writing prompt anyway");
    }

    // Step 4: Small delay to ensure Claude is fully accepting input
    std::thread::sleep(std::time::Duration::from_millis(300));

    // Step 5: Write the prompt text (convert \n → \r for PTY line breaks).
    // IMPORTANT: Do NOT send \r (Enter) with the text. Enter must arrive as a
    // SEPARATE stdin read so ink interprets it as a keypress, not a paste.
    let prompt_text = prompt.replace("\r\n", "\r").replace('\n', "\r");
    let text_req = Request::Write {
        session_id: terminal_id.clone(),
        data: prompt_text.into_bytes(),
    };
    if let Err(e) = daemon.send_request(&text_req) {
        eprintln!("[quick_claude] Failed to write prompt text: {}", e);
        return;
    }

    // Step 5b: Wait for the TUI to echo the prompt text before sending Enter.
    //
    // Bug #393: A fixed 100ms delay between text and Enter is insufficient.
    // If the TUI hasn't started reading stdin yet (still initializing), both
    // writes accumulate in the PTY buffer and arrive as one chunk — ink treats
    // the merged \r as a literal newline (paste), not a submit keypress.
    //
    // Fix: Poll SearchBuffer until the prompt text appears in the terminal
    // output. Ink echoes typed text to the input area, so the text showing up
    // in output means the TUI has consumed it from stdin and is actively
    // reading. Sending \r AFTER this guarantees a separate stdin read.
    let search_prefix: String = prompt.chars().take(40).collect();
    let echo_detected = poll_text_in_output(&daemon, &terminal_id, &search_prefix, 30_000);
    if !echo_detected {
        eprintln!("[quick_claude] Prompt text not echoed within 30s, sending Enter anyway");
    }

    // Step 5c: Small buffer after echo detection, then send Enter.
    std::thread::sleep(std::time::Duration::from_millis(200));

    let submit_req = Request::Write {
        session_id: terminal_id.clone(),
        data: b"\r".to_vec(),
    };
    if let Err(e) = daemon.send_request(&submit_req) {
        eprintln!("[quick_claude] Failed to send submit key: {}", e);
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
            Ok(Response::LastOutputTime { epoch_ms, running, .. }) => {
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

/// Combined poll loop that checks for both idle state AND trust prompt on each
/// iteration. If the trust prompt is detected, auto-accepts it (sends \r),
/// waits for Claude to process, then continues polling for idle.
/// Returns true if idle was detected before timeout, false otherwise.
fn poll_idle_or_trust(
    daemon: &DaemonClient,
    session_id: &str,
    idle_ms: u64,
    timeout_ms: u64,
) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    let poll_interval = (idle_ms / 4).min(500).max(50);
    // Check trust prompt every ~2s to avoid spamming SearchBuffer
    let trust_check_interval = 2_000u64 / poll_interval;
    let mut iteration = 0u64;

    loop {
        // Check idle state
        let req = Request::GetLastOutputTime {
            session_id: session_id.to_string(),
        };
        let is_idle = match daemon.send_request(&req) {
            Ok(Response::LastOutputTime { epoch_ms, running, .. }) => {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let ago = now_ms.saturating_sub(epoch_ms);

                ago >= idle_ms || !running
            }
            Ok(_) | Err(_) => {
                return false;
            }
        };

        // Check trust prompt every ~2s, AND always when idle is detected.
        // Bug #411: The trust prompt screen is idle (waiting for Enter), so
        // the idle check fires immediately. We must check for the trust prompt
        // before returning, otherwise it's never auto-accepted.
        if is_idle || iteration % trust_check_interval == 0 {
            if has_trust_prompt(daemon, session_id) {
                eprintln!("[quick_claude] Detected trust prompt, auto-accepting");
                let accept_req = Request::Write {
                    session_id: session_id.to_string(),
                    data: b"\r".to_vec(),
                };
                let _ = daemon.send_request(&accept_req);
                // Give Claude time to process the acceptance and continue startup
                std::thread::sleep(std::time::Duration::from_millis(3_000));
                // Continue polling — don't return, Claude needs time to finish startup
            } else if is_idle {
                return true;
            }
        } else if is_idle {
            return true;
        }

        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(poll_interval));
        iteration += 1;
    }
}

/// Check if Claude Code is showing a workspace trust prompt.
///
/// Two search strategies are used:
/// 1. **Raw output history** (`SearchBuffer`) — fast search through the PTY
///    byte ring buffer with ANSI stripping.
/// 2. **Rendered grid** (`ReadGrid`) — searches the visible screen text.
///    This is the fallback for TUI apps (ink, @clack/prompts) that render
///    via cursor-positioning sequences. After stripping ANSI from raw bytes,
///    the text fragments may not be contiguous, but the grid always contains
///    the final rendered text as visible on screen.
fn has_trust_prompt(daemon: &DaemonClient, session_id: &str) -> bool {
    const NEEDLES: &[&str] = &[
        "Do you trust the files",
        "I trust this folder",
        "Quick safety check",
    ];

    // Strategy 1: Search raw output history (fast path)
    for needle in NEEDLES {
        let req = Request::SearchBuffer {
            session_id: session_id.to_string(),
            text: needle.to_string(),
            strip_ansi: true,
        };
        if matches!(
            daemon.send_request(&req),
            Ok(Response::SearchResult { found: true, .. })
        ) {
            return true;
        }
    }

    // Strategy 2: Search the rendered grid (fallback for TUI renderers).
    // The grid contains exactly what the user sees on screen — no ANSI
    // ambiguity, no cursor-positioning fragmentation.
    let grid_req = Request::ReadGrid {
        session_id: session_id.to_string(),
    };
    if let Ok(Response::Grid { grid }) = daemon.send_request(&grid_req) {
        let screen_text = grid.rows.join(" ");
        for needle in NEEDLES {
            if screen_text.contains(needle) {
                eprintln!("[quick_claude] Trust prompt found via grid search: {:?}", needle);
                return true;
            }
        }
    }

    false
}

/// Poll until the specified text appears in the session's output buffer.
/// Used to confirm that ink's TUI has consumed text from stdin (it echoes
/// typed characters to the input area, which shows up in terminal output).
/// Returns true if found before timeout, false otherwise.
fn poll_text_in_output(
    daemon: &DaemonClient,
    session_id: &str,
    text: &str,
    timeout_ms: u64,
) -> bool {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);

    loop {
        let req = Request::SearchBuffer {
            session_id: session_id.to_string(),
            text: text.to_string(),
            strip_ansi: true,
        };
        if matches!(
            daemon.send_request(&req),
            Ok(Response::SearchResult { found: true, .. })
        ) {
            return true;
        }

        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

/// Pause output streaming for a session (session stays alive, VT parser
/// keeps running, but no Output/GridDiff events are sent to the client).
#[tauri::command]
pub fn pause_session(
    session_id: String,
    daemon: State<Arc<DaemonClient>>,
) -> Result<(), String> {
    let request = Request::PauseSession {
        session_id,
    };
    daemon.send_fire_and_forget(&request)
}

/// Resume output streaming for a previously paused session.
#[tauri::command]
pub fn resume_session(
    session_id: String,
    daemon: State<Arc<DaemonClient>>,
) -> Result<(), String> {
    let request = Request::ResumeSession {
        session_id,
    };
    daemon.send_fire_and_forget(&request)
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
