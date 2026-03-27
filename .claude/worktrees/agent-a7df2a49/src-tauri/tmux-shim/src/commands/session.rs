use crate::cli::TmuxArgs;
use crate::format;
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

    let client = tmux_try!(McpPipeClient::connect());
    let workspace = tmux_try!(client.get_active_workspace());

    // Look up GODLY_SESSION_ID for the initial pane mapping
    let godly_session_id = std::env::var("GODLY_SESSION_ID").ok();

    let terminal_id = if let Some(session_id) = godly_session_id {
        session_id
    } else if cwd.is_some() {
        tmux_try!(
            client.create_terminal(&workspace.id, cwd.clone()),
            "failed to create terminal"
        )
    } else {
        // No session ID and no cwd — use a placeholder; the caller will create panes
        String::new()
    };

    // Store the session mapping in state
    let pane_id = tmux_try!(state::with_state(|state| {
        if state.sessions.contains_key(&session_name) {
            return Err(format!("duplicate session: {}", session_name));
        }

        state.sessions.insert(
            session_name.clone(),
            state::SessionMapping {
                workspace_id: workspace.id.clone(),
            },
        );

        let pane_id = if !terminal_id.is_empty() {
            state.allocate_pane_id(terminal_id.clone(), session_name.clone())
        } else {
            String::new()
        };

        Ok(pane_id)
    }));

    if print_info && !pane_id.is_empty() {
        let vars = format::session_format_vars(&pane_id, &session_name);
        println!("{}", format::expand_format(format_str, &vars));
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

    if !pane_terminal_ids.is_empty() {
        let client = tmux_try!(McpPipeClient::connect());

        for terminal_id in &pane_terminal_ids {
            if terminal_id.is_empty() {
                continue;
            }
            if let Err(e) = client.close_terminal(terminal_id) {
                eprintln!(
                    "tmux: warning: failed to close terminal {}: {}",
                    terminal_id, e
                );
            }
        }
    }

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
    let state = tmux_try!(state::load(), "failed to read state");

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
