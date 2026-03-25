use godly_protocol::{McpRequest, McpResponse};

use crate::cli::TmuxArgs;
use crate::format::{self, FormatVars};
use crate::mcp_client::McpPipeClient;
use crate::state;

/// Handle session lifecycle commands: new-session, has-session, kill-session, list-sessions.
pub fn handle(command: &str, args: &[String]) -> i32 {
    match command {
        "new-session" => new_session(args),
        "has-session" => has_session(args),
        "kill-session" => kill_session(args),
        "list-sessions" => list_sessions(args),
        _ => {
            eprintln!("tmux: unknown session command: {}", command);
            1
        }
    }
}

/// `new-session -d -s <name> [-c <dir>] [-P] [-F <format>]`
///
/// Gets the active workspace, stores session->workspace mapping, and maps
/// `$GODLY_SESSION_ID` as the initial pane (%0).
fn new_session(args: &[String]) -> i32 {
    let parsed = TmuxArgs::parse(args);

    let session_name = match parsed.get_option('s') {
        Some(name) => name.to_string(),
        None => {
            // tmux defaults to "0", "1", etc. if no name given
            "0".to_string()
        }
    };

    let cwd = parsed.get_option('c').map(|s| s.to_string());
    let print_info = parsed.has_flag('P');
    let format_str = parsed.get_option('F').unwrap_or("#{session_name}:");

    // Connect to Godly Terminal MCP pipe
    let client = match McpPipeClient::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("tmux: {}", e);
            return 1;
        }
    };

    // Get the active workspace
    let workspace = match client.send_request(&McpRequest::GetActiveWorkspace) {
        Ok(McpResponse::ActiveWorkspace {
            workspace: Some(ws),
        }) => ws,
        Ok(McpResponse::ActiveWorkspace { workspace: None }) => {
            eprintln!("tmux: no active workspace");
            return 1;
        }
        Ok(McpResponse::Error { message }) => {
            eprintln!("tmux: {}", message);
            return 1;
        }
        Ok(other) => {
            eprintln!("tmux: unexpected response: {:?}", other);
            return 1;
        }
        Err(e) => {
            eprintln!("tmux: MCP error: {}", e);
            return 1;
        }
    };

    // Look up GODLY_SESSION_ID for the initial pane mapping
    let godly_session_id = std::env::var("GODLY_SESSION_ID").ok();

    // If a cwd was specified but we don't have a session ID, create a new terminal
    let terminal_id = if let Some(session_id) = godly_session_id {
        // Use the existing terminal session that spawned us
        session_id
    } else if cwd.is_some() {
        // Create a new terminal in the workspace
        match client.send_request(&McpRequest::CreateTerminal {
            workspace_id: workspace.id.clone(),
            shell_type: None,
            cwd: cwd.clone(),
            worktree_name: None,
            worktree: None,
            command: None,
            focus: Some(false),
        }) {
            Ok(McpResponse::Created { id, .. }) => id,
            Ok(McpResponse::Error { message }) => {
                eprintln!("tmux: failed to create terminal: {}", message);
                return 1;
            }
            Ok(other) => {
                eprintln!("tmux: unexpected response creating terminal: {:?}", other);
                return 1;
            }
            Err(e) => {
                eprintln!("tmux: MCP error creating terminal: {}", e);
                return 1;
            }
        }
    } else {
        // No session ID and no cwd — use a placeholder; the caller will create panes
        String::new()
    };

    // Store the session mapping in state
    let pane_id = match state::with_state(|state| {
        // Check if session already exists
        if state.sessions.contains_key(&session_name) {
            return Err(format!("duplicate session: {}", session_name));
        }

        state.sessions.insert(
            session_name.clone(),
            state::SessionMapping {
                workspace_id: workspace.id.clone(),
            },
        );

        // Map the initial terminal as %N
        let pane_id = if !terminal_id.is_empty() {
            state.allocate_pane_id(terminal_id.clone(), session_name.clone())
        } else {
            String::new()
        };

        Ok(pane_id)
    }) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("tmux: {}", e);
            return 1;
        }
    };

    // Print info if -P was specified
    if print_info && !pane_id.is_empty() {
        let vars = FormatVars {
            pane_id,
            session_name: session_name.clone(),
            pane_width: 80,
            pane_height: 24,
            window_index: 0,
        };
        println!("{}", format::expand(format_str, &vars));
    }

    0
}

/// `has-session -t <name>`
///
/// Check if a session exists. Exit 0 if yes, 1 if no.
fn has_session(args: &[String]) -> i32 {
    let parsed = TmuxArgs::parse(args);

    let target = match parsed.get_option('t') {
        Some(name) => name.to_string(),
        None => {
            eprintln!("tmux: has-session requires -t <session-name>");
            return 1;
        }
    };

    match state::load() {
        Ok(state) => {
            if state.sessions.contains_key(&target) {
                0
            } else {
                1
            }
        }
        Err(e) => {
            eprintln!("tmux: failed to read state: {}", e);
            1
        }
    }
}

/// `kill-session -t <name>`
///
/// Close all panes in the session via MCP, then remove from state.
fn kill_session(args: &[String]) -> i32 {
    let parsed = TmuxArgs::parse(args);

    let target = match parsed.get_option('t') {
        Some(name) => name.to_string(),
        None => {
            eprintln!("tmux: kill-session requires -t <session-name>");
            return 1;
        }
    };

    // Load state to get all terminal IDs for this session
    let pane_terminal_ids: Vec<String> = match state::load() {
        Ok(state) => {
            if !state.sessions.contains_key(&target) {
                eprintln!("tmux: session not found: {}", target);
                return 1;
            }
            state
                .session_panes(&target)
                .iter()
                .filter_map(|pane_id| state.panes.get(pane_id).map(|p| p.terminal_id.clone()))
                .collect()
        }
        Err(e) => {
            eprintln!("tmux: failed to read state: {}", e);
            return 1;
        }
    };

    // Close each terminal via MCP
    if !pane_terminal_ids.is_empty() {
        let client = match McpPipeClient::connect() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("tmux: {}", e);
                return 1;
            }
        };

        for terminal_id in &pane_terminal_ids {
            if terminal_id.is_empty() {
                continue;
            }
            match client.send_request(&McpRequest::CloseTerminal {
                terminal_id: terminal_id.clone(),
            }) {
                Ok(McpResponse::Ok) => {}
                Ok(McpResponse::Error { message }) => {
                    eprintln!(
                        "tmux: warning: failed to close terminal {}: {}",
                        terminal_id, message
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!(
                        "tmux: warning: MCP error closing terminal {}: {}",
                        terminal_id, e
                    );
                }
            }
        }
    }

    // Remove session from state
    match state::with_state(|state| {
        state.remove_session(&target);
        Ok(())
    }) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("tmux: {}", e);
            1
        }
    }
}

/// `list-sessions` / `ls`
///
/// Print sessions in tmux format: `session_name: N windows (created ...)`
fn list_sessions(_args: &[String]) -> i32 {
    let state = match state::load() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("tmux: failed to read state: {}", e);
            return 1;
        }
    };

    if state.sessions.is_empty() {
        // tmux exits 1 with "no server running" when there are no sessions
        eprintln!("no server running on this host");
        return 1;
    }

    for (name, _mapping) in &state.sessions {
        let pane_count = state.session_panes(name).len();
        // tmux counts "windows", but we map 1 session = 1 window with N panes
        println!("{}: 1 windows (created -) [{} panes]", name, pane_count);
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_dispatches_unknown_command() {
        let code = handle("nonexistent", &[]);
        assert_eq!(code, 1);
    }
}
