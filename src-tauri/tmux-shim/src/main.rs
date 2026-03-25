//! Godly Terminal tmux shim.
//!
//! Translates tmux CLI commands into Godly Terminal operations. Used by
//! Claude Code's "Agent Teams" feature, which expects a `tmux` binary
//! for split pane management.
//!
//! Dual-backend:
//! - MCP pipe: layout operations (split, resize, etc.)
//! - Daemon pipe: I/O operations (write, read grid, etc.)

mod cli;
mod commands;
mod daemon_client;
mod format;
mod mcp_client;
mod state;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    let parsed = match cli::parse(args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("tmux: {}", e);
            return ExitCode::from(1);
        }
    };

    let result = dispatch(&parsed.command, &parsed.args);

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tmux: {}", e);
            ExitCode::from(1)
        }
    }
}

fn dispatch(command: &str, args: &[String]) -> Result<(), String> {
    match command {
        // I/O commands (fully implemented)
        "send-keys" | "send" => commands::io::send_keys(args),
        "display-message" | "display" => commands::io::display_message(args),

        // Session commands (stubs)
        "new-session" | "new" => commands::session::new_session(args),
        "has-session" | "has" => commands::session::has_session(args),
        "kill-session" => commands::session::kill_session(args),
        "list-sessions" | "ls" => commands::session::list_sessions(args),

        // Pane commands (stubs)
        "split-window" | "splitw" => commands::pane::split_window(args),
        "select-pane" | "selectp" => commands::pane::select_pane(args),
        "resize-pane" | "resizep" => commands::pane::resize_pane(args),
        "list-panes" | "lsp" => commands::pane::list_panes(args),

        _ => Err(format!("unknown command: {}", command)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_unknown_command_returns_error() {
        let result = dispatch("nonexistent", &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown command"));
    }

    #[test]
    fn dispatch_stub_commands_return_not_implemented() {
        for cmd in &[
            "new-session",
            "has-session",
            "kill-session",
            "list-sessions",
            "split-window",
            "select-pane",
            "resize-pane",
            "list-panes",
        ] {
            let result = dispatch(cmd, &[]);
            assert!(result.is_err());
            assert!(
                result.unwrap_err().contains("not yet implemented"),
                "Expected 'not yet implemented' for {}",
                cmd
            );
        }
    }

    #[test]
    fn dispatch_aliases_work() {
        assert!(dispatch("new", &[]).is_err());
        assert!(dispatch("has", &[]).is_err());
        assert!(dispatch("ls", &[]).is_err());
        assert!(dispatch("splitw", &[]).is_err());
        assert!(dispatch("selectp", &[]).is_err());
        assert!(dispatch("resizep", &[]).is_err());
        assert!(dispatch("lsp", &[]).is_err());
    }
}
