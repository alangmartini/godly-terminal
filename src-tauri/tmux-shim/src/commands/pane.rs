//! Pane management commands: split-window, select-pane, list-panes, kill-pane.
//!
//! These translate tmux pane commands into Godly Terminal MCP operations.

use crate::cli::TmuxArgs;
use crate::format;
use crate::mcp_client::McpPipeClient;
use crate::state;

/// Dispatch pane commands.
pub fn handle(command: &str, args: &[String]) -> i32 {
    match command {
        "split-window" => split_window(args),
        "select-pane" => select_pane(args),
        "list-panes" => list_panes(args),
        "kill-pane" => kill_pane(args),
        _ => {
            eprintln!("tmux: unknown pane command: {}", command);
            1
        }
    }
}

/// Map tmux split flags to Godly Terminal direction strings.
///
/// tmux `-h` = horizontal split (panes side-by-side) = Godly `"horizontal"`
/// tmux `-v` = vertical split (panes stacked)        = Godly `"vertical"`
/// Default (no flag) = vertical (stacked).
fn direction_from_flags(parsed: &TmuxArgs) -> &'static str {
    if parsed.has_flag('h') {
        "horizontal"
    } else {
        "vertical"
    }
}

/// `split-window [-h|-v] [-t <target>] [-P] [-F <format>] [-c <dir>]`
///
/// Creates a new terminal and splits the target pane.
fn split_window(args: &[String]) -> i32 {
    let parsed = TmuxArgs::parse(args);
    let target = parsed.get_option('t');
    let cwd = parsed.get_option('c').map(|s| s.to_string());
    let direction = direction_from_flags(&parsed);

    // Load state to resolve target
    let current_state = match state::load() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("tmux: failed to read state: {}", e);
            return 1;
        }
    };

    let target_terminal_id = match current_state.resolve_target(target) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("tmux: {}", e);
            return 1;
        }
    };
    let session_name = match current_state.resolve_session(target) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("tmux: {}", e);
            return 1;
        }
    };
    let workspace_id = match current_state.workspace_for_session(&session_name) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("tmux: {}", e);
            return 1;
        }
    };

    // MCP: create terminal and split
    let client = match McpPipeClient::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("tmux: {}", e);
            return 1;
        }
    };

    let new_terminal_id = match client.create_terminal(&workspace_id, cwd) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("tmux: failed to create terminal: {}", e);
            return 1;
        }
    };

    if let Err(e) = client.split_terminal(
        &workspace_id,
        &target_terminal_id,
        &new_terminal_id,
        direction,
        0.5,
    ) {
        eprintln!("tmux: failed to split: {}", e);
        return 1;
    }

    // Store the new pane in state
    let pane_id = match state::with_state(|st| {
        let id = st.allocate_pane_id(new_terminal_id.clone(), session_name.clone());
        Ok(id)
    }) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("tmux: {}", e);
            return 1;
        }
    };

    // Print info if -P was specified
    if parsed.has_flag('P') {
        if let Some(fmt) = parsed.get_option('F') {
            let vars = format::pane_format_vars(&pane_id, 0, 80, 24, false, &session_name);
            println!("{}", format::expand_format(fmt, &vars));
        } else {
            println!("{}:0.{}", session_name, pane_id);
        }
    }

    0
}

/// `select-pane -t <target>`
///
/// Focus the specified pane.
fn select_pane(args: &[String]) -> i32 {
    let parsed = TmuxArgs::parse(args);
    let target = parsed.get_option('t');

    let state = match state::load() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("tmux: failed to read state: {}", e);
            return 1;
        }
    };

    let terminal_id = match state.resolve_target(target) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("tmux: {}", e);
            return 1;
        }
    };

    let client = match McpPipeClient::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("tmux: {}", e);
            return 1;
        }
    };

    if let Err(e) = client.focus_terminal(&terminal_id) {
        eprintln!("tmux: {}", e);
        return 1;
    }

    0
}

/// `list-panes [-t <session>] [-F <format>]`
///
/// List panes, optionally filtered by session.
fn list_panes(args: &[String]) -> i32 {
    let parsed = TmuxArgs::parse(args);
    let session_filter = parsed.get_option('t');
    let format_str = parsed.get_option('F');

    let state = match state::load() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("tmux: failed to read state: {}", e);
            return 1;
        }
    };

    let mut panes: Vec<(String, &state::PaneMapping)> = state
        .panes
        .iter()
        .filter(|(_id, entry)| session_filter.map(|s| entry.session == s).unwrap_or(true))
        .map(|(id, entry)| (id.clone(), entry))
        .collect();

    panes.sort_by(|a, b| {
        let a_num: u32 = a.0.trim_start_matches('%').parse().unwrap_or(0);
        let b_num: u32 = b.0.trim_start_matches('%').parse().unwrap_or(0);
        a_num.cmp(&b_num)
    });

    if panes.is_empty() {
        return 0;
    }

    for (idx, (pane_id, entry)) in panes.iter().enumerate() {
        let (cols, rows): (u16, u16) = (80, 24);
        let active = idx == 0;

        if let Some(fmt) = format_str {
            let vars = format::pane_format_vars(pane_id, idx, cols, rows, active, &entry.session);
            println!("{}", format::expand_format(fmt, &vars));
        } else {
            println!(
                "{}",
                format::default_list_panes_line(idx, cols, rows, pane_id, active)
            );
        }
    }

    0
}

/// `kill-pane -t <target>`
///
/// Close the specified pane and remove it from state.
fn kill_pane(args: &[String]) -> i32 {
    let parsed = TmuxArgs::parse(args);
    let target = parsed.get_option('t');

    // Resolve pane -> terminal ID from current state
    let (terminal_id, pane_id_to_remove) = {
        let st = match state::load() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("tmux: failed to read state: {}", e);
                return 1;
            }
        };

        let tid = match st.resolve_target(target) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("tmux: {}", e);
                return 1;
            }
        };

        // Find the pane_id key for this terminal
        let pid = st
            .panes
            .iter()
            .find(|(_id, entry)| entry.terminal_id == tid)
            .map(|(id, _)| id.clone());

        (tid, pid)
    };

    // Close via MCP
    let client = match McpPipeClient::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("tmux: {}", e);
            return 1;
        }
    };

    if let Err(e) = client.close_terminal(&terminal_id) {
        eprintln!("tmux: {}", e);
        return 1;
    }

    // Remove from state
    if let Some(pid) = pane_id_to_remove {
        if let Err(e) = state::with_state(|st| {
            st.panes.remove(&pid);
            Ok(())
        }) {
            eprintln!("tmux: {}", e);
            return 1;
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn direction_horizontal_flag() {
        let parsed = TmuxArgs::parse(&args(&["-h"]));
        assert_eq!(direction_from_flags(&parsed), "horizontal");
    }

    #[test]
    fn direction_vertical_flag() {
        let parsed = TmuxArgs::parse(&args(&["-v"]));
        assert_eq!(direction_from_flags(&parsed), "vertical");
    }

    #[test]
    fn direction_default_is_vertical() {
        let parsed = TmuxArgs::parse(&args(&[]));
        assert_eq!(direction_from_flags(&parsed), "vertical");
    }

    #[test]
    fn direction_h_takes_precedence() {
        let parsed = TmuxArgs::parse(&args(&["-h", "-v"]));
        assert_eq!(direction_from_flags(&parsed), "horizontal");
    }

    #[test]
    fn handle_dispatches_unknown_command() {
        let code = handle("nonexistent", &[]);
        assert_eq!(code, 1);
    }
}
