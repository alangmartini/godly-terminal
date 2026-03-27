//! Pane management commands: split-window, select-pane, list-panes, kill-pane.
//!
//! These translate tmux pane commands into Godly Terminal MCP operations.

use crate::cli::TmuxArgs;
use crate::format::{self, DEFAULT_PANE_COLS, DEFAULT_PANE_ROWS};
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

    trace!(
        "split-window: target={:?} direction={} cwd={:?}",
        target,
        direction,
        cwd
    );

    let current_state = tmux_try!(state::load(), "failed to read state");
    let target_terminal_id = tmux_try!(current_state.resolve_target(target));
    let session_name = tmux_try!(current_state.resolve_session(target));
    let workspace_id = tmux_try!(current_state.workspace_for_session(&session_name));

    let client = tmux_try!(McpPipeClient::connect());
    let new_terminal_id = tmux_try!(
        client.create_terminal(&workspace_id, cwd),
        "failed to create terminal"
    );

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

    let pane_id = tmux_try!(state::with_state(|st| {
        let id = st.allocate_pane_id(new_terminal_id.clone(), session_name.clone());
        Ok(id)
    }));

    if parsed.has_flag('P') {
        if let Some(fmt) = parsed.get_option('F') {
            let vars = format::pane_format_vars(
                &pane_id,
                0,
                DEFAULT_PANE_COLS,
                DEFAULT_PANE_ROWS,
                false,
                &session_name,
            );
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

    let state = tmux_try!(state::load(), "failed to read state");
    let terminal_id = tmux_try!(state.resolve_target(target));
    let client = tmux_try!(McpPipeClient::connect());
    tmux_try!(client.focus_terminal(&terminal_id));

    0
}

/// `list-panes [-t <session>] [-F <format>]`
///
/// List panes, optionally filtered by session.
/// Handles tmux target formats: `session`, `session:window`.
fn list_panes(args: &[String]) -> i32 {
    let parsed = TmuxArgs::parse(args);
    let raw_filter = parsed.get_option('t');
    let session_filter = raw_filter.map(state::strip_target_suffix);
    let format_str = parsed.get_option('F');

    trace!(
        "list-panes: raw_target={:?} session_filter={:?}",
        raw_filter,
        session_filter
    );

    let state = tmux_try!(state::load(), "failed to read state");

    let mut panes: Vec<(String, &state::PaneMapping)> = state
        .panes
        .iter()
        .filter(|(_id, entry)| session_filter.map(|s| entry.session == s).unwrap_or(true))
        .map(|(id, entry)| (id.clone(), entry))
        .collect();

    trace!(
        "list-panes: matched {} panes (state has {} total)",
        panes.len(),
        state.panes.len()
    );

    panes.sort_by(|a, b| {
        let a_num: u32 = a.0.trim_start_matches('%').parse().unwrap_or(0);
        let b_num: u32 = b.0.trim_start_matches('%').parse().unwrap_or(0);
        a_num.cmp(&b_num)
    });

    if panes.is_empty() {
        return 0;
    }

    for (idx, (pane_id, entry)) in panes.iter().enumerate() {
        let active = idx == 0;

        if let Some(fmt) = format_str {
            let vars = format::pane_format_vars(
                pane_id,
                idx,
                DEFAULT_PANE_COLS,
                DEFAULT_PANE_ROWS,
                active,
                &entry.session,
            );
            println!("{}", format::expand_format(fmt, &vars));
        } else {
            println!(
                "{}",
                format::default_list_panes_line(
                    idx,
                    DEFAULT_PANE_COLS,
                    DEFAULT_PANE_ROWS,
                    pane_id,
                    active,
                )
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

    let (terminal_id, pane_id_to_remove) = {
        let st = tmux_try!(state::load(), "failed to read state");
        let tid = tmux_try!(st.resolve_target(target));

        let pid = st
            .panes
            .iter()
            .find(|(_id, entry)| entry.terminal_id == tid)
            .map(|(id, _)| id.clone());

        (tid, pid)
    };

    let client = tmux_try!(McpPipeClient::connect());
    tmux_try!(client.close_terminal(&terminal_id));

    if let Some(pid) = pane_id_to_remove {
        tmux_try!(state::with_state(|st| {
            st.panes.remove(&pid);
            Ok(())
        }));
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
