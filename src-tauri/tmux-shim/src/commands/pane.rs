//! Pane management commands: split-window, select-pane, list-panes, kill-pane.
//!
//! These translate tmux pane commands into Godly Terminal MCP operations.

use crate::cli::TmuxArgs;
use crate::daemon_client::DaemonPipeClient;
use crate::format::{self, FormatVars};
use crate::mcp_client::McpPipeClient;
use crate::state;

/// `split-window [-h|-v] [-t <target>] [-P] [-F <format>] [-c <dir>]`
///
/// Creates a new terminal and splits the target pane.
/// `-h` = horizontal split (panes side-by-side), `-v` = vertical (stacked, default).
pub fn split_window(args: &TmuxArgs) -> Result<(), String> {
    let direction = if args.has_flag('h') {
        "horizontal"
    } else {
        "vertical"
    };

    let target = args.get_option('t');
    let cwd = args.get_option('c').map(|s| s.to_string());

    let (pane_id, session_name) = state::with_state(|state| {
        let target_terminal_id = state.resolve_pane(target)?;

        // Find the session for the target pane
        let pane_id_str = state.resolve_pane_id(target);
        let session_name = state
            .pane_session(&pane_id_str)
            .ok_or_else(|| format!("pane '{}' has no session", pane_id_str))?
            .to_string();
        let workspace_id = state.workspace_for_session(&session_name)?;

        // Create a new terminal via MCP
        let mcp = McpPipeClient::connect()?;
        let new_terminal_id = mcp.create_terminal(&workspace_id, cwd)?;

        // Split the target terminal
        mcp.split_terminal(
            &workspace_id,
            &target_terminal_id,
            &new_terminal_id,
            direction,
            0.5,
        )?;

        // Register the new pane in state
        let new_pane_id = state.allocate_pane_id(new_terminal_id, session_name.clone());

        Ok((new_pane_id, session_name))
    })?;

    // Print info if -P was specified
    if args.has_flag('P') {
        let format_str = args.get_option('F');
        if let Some(fmt) = format_str {
            let vars = FormatVars {
                pane_id: pane_id.clone(),
                session_name: session_name.clone(),
                pane_width: 80,
                pane_height: 24,
                pane_active: false,
                window_index: 0,
                ..Default::default()
            };
            println!("{}", format::expand(fmt, &vars));
        } else {
            // Default -P output: session:window.pane
            println!("{}:0.{}", session_name, pane_id);
        }
    }

    Ok(())
}

/// `select-pane -t <target>`
///
/// Focus the specified pane.
pub fn select_pane(args: &TmuxArgs) -> Result<(), String> {
    let target = args.get_option('t');
    let state = state::load().map_err(|e| format!("failed to load state: {}", e))?;
    let terminal_id = state.resolve_pane(target)?;

    let mcp = McpPipeClient::connect()?;
    mcp.focus_terminal(&terminal_id)
}

/// `list-panes [-t <session>] [-F <format>]`
///
/// List panes, optionally filtered by session.
pub fn list_panes(args: &TmuxArgs) -> Result<(), String> {
    let session_filter = args.get_option('t');
    let format_str = args.get_option('F');

    let state = state::load().map_err(|e| format!("failed to load state: {}", e))?;

    let mut panes: Vec<(String, &state::PaneMapping)> = state
        .panes
        .iter()
        .filter(|(_id, entry)| {
            session_filter
                .map(|s| entry.session == s)
                .unwrap_or(true)
        })
        .map(|(id, entry)| (id.clone(), entry))
        .collect();

    // Sort by pane ID numerically
    panes.sort_by(|a, b| {
        let a_num: u32 = a.0.trim_start_matches('%').parse().unwrap_or(0);
        let b_num: u32 = b.0.trim_start_matches('%').parse().unwrap_or(0);
        a_num.cmp(&b_num)
    });

    if panes.is_empty() {
        return Ok(());
    }

    // Try to connect to daemon for grid dimensions
    let daemon = DaemonPipeClient::connect().ok();

    for (idx, (pane_id, entry)) in panes.iter().enumerate() {
        let (cols, rows) = daemon
            .as_ref()
            .and_then(|d| d.read_grid_size(&entry.terminal_id).ok())
            .unwrap_or((80, 24));

        let active = idx == 0;

        let vars = FormatVars {
            pane_id: pane_id.clone(),
            session_name: entry.session.clone(),
            pane_width: cols as u32,
            pane_height: rows as u32,
            pane_index: idx as u32,
            pane_active: active,
            window_index: 0,
        };

        if let Some(fmt) = format_str {
            println!("{}", format::expand(fmt, &vars));
        } else {
            println!("{}", format::default_list_panes_line(&vars));
        }
    }

    Ok(())
}

/// `kill-pane -t <target>`
///
/// Close the specified pane and remove it from state.
pub fn kill_pane(args: &TmuxArgs) -> Result<(), String> {
    let target = args.get_option('t');

    state::with_state(|state| {
        let terminal_id = state.resolve_pane(target)?;

        // Close the terminal via MCP
        let mcp = McpPipeClient::connect()?;
        mcp.close_terminal(&terminal_id)?;

        // Find and remove the pane from state
        let pane_id = state
            .panes
            .iter()
            .find(|(_id, entry)| entry.terminal_id == terminal_id)
            .map(|(id, _)| id.clone());

        if let Some(id) = pane_id {
            state.panes.remove(&id);
        }

        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_horizontal_flag() {
        let args = TmuxArgs::parse(&["-h".to_string()]);
        assert!(args.has_flag('h'));
        assert!(!args.has_flag('v'));
    }

    #[test]
    fn direction_vertical_flag() {
        let args = TmuxArgs::parse(&["-v".to_string()]);
        assert!(args.has_flag('v'));
        assert!(!args.has_flag('h'));
    }

    #[test]
    fn direction_default_is_vertical() {
        let args = TmuxArgs::parse(&[]);
        assert!(!args.has_flag('h'));
    }
}
