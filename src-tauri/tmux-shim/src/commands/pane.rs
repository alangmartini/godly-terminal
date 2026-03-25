//! Pane management commands: split-window, select-pane, list-panes, kill-pane.
//!
//! These translate tmux pane commands into Godly Terminal MCP operations.

use crate::cli::TmuxArgs;
use crate::daemon_client::DaemonClient;
use crate::format;
use crate::mcp_client::McpClient;
use crate::state::{PaneEntry, TmuxState};

/// Map tmux split flags to Godly Terminal direction strings.
///
/// tmux `-h` = horizontal split (panes side-by-side) = Godly `"horizontal"`
/// tmux `-v` = vertical split (panes stacked)        = Godly `"vertical"`
/// Default (no flag) = vertical (stacked).
pub fn direction_from_flags(args: &TmuxArgs) -> &'static str {
    if args.has_flag("-h") {
        "horizontal"
    } else {
        "vertical"
    }
}

/// `split-window [-h|-v] [-t <target>] [-P] [-F <format>] [-c <dir>]`
///
/// Creates a new terminal and splits the target pane.
pub fn split_window(args: &TmuxArgs) -> Result<(), String> {
    let mut state = TmuxState::load()?;
    let mut mcp = McpClient::connect()?;

    let target = args.flag("-t");
    let target_terminal_id = state.resolve_target(target)?;
    let session_name = state.resolve_session(target)?;
    let workspace_id = state.workspace_for_session(&session_name)?;
    let direction = direction_from_flags(args);
    let cwd = args.flag("-c").map(|s| s.to_string());

    let new_terminal_id = mcp.create_terminal(&workspace_id, cwd)?;
    mcp.split_terminal(
        &workspace_id,
        &target_terminal_id,
        &new_terminal_id,
        direction,
        0.5,
    )?;

    let pane_id = state.alloc_pane_id();
    state.panes.insert(
        pane_id.clone(),
        PaneEntry {
            terminal_id: new_terminal_id,
            session: session_name.clone(),
        },
    );
    state.save()?;

    if args.has_flag("-P") {
        if let Some(fmt) = args.flag("-F") {
            let vars = format::pane_format_vars(&pane_id, 0, 80, 24, false, &session_name);
            println!("{}", format::expand_format(fmt, &vars));
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
    let state = TmuxState::load()?;
    let mut mcp = McpClient::connect()?;

    let target = args.flag("-t");
    let terminal_id = state.resolve_target(target)?;
    mcp.focus_terminal(&terminal_id)?;

    Ok(())
}

/// `list-panes [-t <session>] [-F <format>]`
///
/// List panes, optionally filtered by session. Queries the daemon for
/// grid dimensions of each pane.
pub fn list_panes(args: &TmuxArgs) -> Result<(), String> {
    let state = TmuxState::load()?;
    let session_filter = args.flag("-t");
    let format_str = args.flag("-F");

    let mut panes: Vec<(String, &PaneEntry)> = state
        .panes
        .iter()
        .filter(|(_id, entry)| {
            session_filter
                .map(|s| entry.session == s)
                .unwrap_or(true)
        })
        .map(|(id, entry)| (id.clone(), entry))
        .collect();

    panes.sort_by(|a, b| {
        let a_num: u32 = a.0.trim_start_matches('%').parse().unwrap_or(0);
        let b_num: u32 = b.0.trim_start_matches('%').parse().unwrap_or(0);
        a_num.cmp(&b_num)
    });

    if panes.is_empty() {
        return Ok(());
    }

    let mut daemon = DaemonClient::connect().ok();

    for (idx, (pane_id, entry)) in panes.iter().enumerate() {
        let (cols, rows) = daemon
            .as_mut()
            .and_then(|d| d.read_grid_size(&entry.terminal_id).ok())
            .unwrap_or((80, 24));

        let active = idx == 0;

        if let Some(fmt) = format_str {
            let vars =
                format::pane_format_vars(pane_id, idx, cols, rows, active, &entry.session);
            println!("{}", format::expand_format(fmt, &vars));
        } else {
            println!(
                "{}",
                format::default_list_panes_line(idx, cols, rows, pane_id, active)
            );
        }
    }

    Ok(())
}

/// `kill-pane -t <target>`
///
/// Close the specified pane and remove it from state.
pub fn kill_pane(args: &TmuxArgs) -> Result<(), String> {
    let mut state = TmuxState::load()?;
    let mut mcp = McpClient::connect()?;

    let target = args.flag("-t");
    let terminal_id = state.resolve_target(target)?;

    mcp.close_terminal(&terminal_id)?;

    let pane_id = state
        .panes
        .iter()
        .find(|(_id, entry)| entry.terminal_id == terminal_id)
        .map(|(id, _)| id.clone());

    if let Some(id) = pane_id {
        state.panes.remove(&id);
    }

    state.save()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::TmuxArgs;

    fn parse_args(args: &[&str]) -> TmuxArgs {
        TmuxArgs::parse(args.iter().map(|s| s.to_string())).unwrap()
    }

    #[test]
    fn direction_horizontal_flag() {
        let args = parse_args(&["split-window", "-h"]);
        assert_eq!(direction_from_flags(&args), "horizontal");
    }

    #[test]
    fn direction_vertical_flag() {
        let args = parse_args(&["split-window", "-v"]);
        assert_eq!(direction_from_flags(&args), "vertical");
    }

    #[test]
    fn direction_default_is_vertical() {
        let args = parse_args(&["split-window"]);
        assert_eq!(direction_from_flags(&args), "vertical");
    }

    #[test]
    fn direction_h_takes_precedence() {
        // If both -h and -v are given, -h wins (because we check -h first)
        let args = parse_args(&["split-window", "-h", "-v"]);
        assert_eq!(direction_from_flags(&args), "horizontal");
    }
}
