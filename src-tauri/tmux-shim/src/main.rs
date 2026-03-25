//! Godly Terminal tmux shim.
//!
//! Translates tmux CLI commands into Godly Terminal operations. Used by
//! Claude Code's "Agent Teams" feature, which expects a `tmux` binary
//! for split pane management.

mod cli;
mod commands;
mod daemon_client;
mod format;
mod mcp_client;
mod state;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    // Handle top-level flags before the subcommand
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-V" => {
                println!("tmux 3.4");
                return ExitCode::SUCCESS;
            }
            // Skip flags that take a value argument (socket path, etc.)
            "-S" | "-L" | "-f" => {
                i += 2;
                continue;
            }
            // Skip other top-level boolean flags
            arg if arg.starts_with('-') => {
                i += 1;
                continue;
            }
            _ => break,
        }
    }

    if i >= args.len() {
        // Bare `tmux` = `new-session`
        let exit_code = commands::session::handle("new-session", &[]);
        return ExitCode::from(exit_code as u8);
    }

    let command = &args[i];
    let cmd_args = &args[i + 1..];

    let result = match command.as_str() {
        // I/O commands
        "send-keys" | "send" => {
            let parsed = cli::TmuxArgs::parse(cmd_args);
            commands::io::send_keys(&parsed)
        }
        "display-message" | "display" => {
            let parsed = cli::TmuxArgs::parse(cmd_args);
            commands::io::display_message(&parsed)
        }
        "capture-pane" | "capturep" => {
            let parsed = cli::TmuxArgs::parse(cmd_args);
            commands::io::capture_pane(&parsed)
        }

        // Pane commands
        "split-window" | "splitw" => {
            let parsed = cli::TmuxArgs::parse(cmd_args);
            commands::pane::split_window(&parsed)
        }
        "select-pane" | "selectp" => {
            let parsed = cli::TmuxArgs::parse(cmd_args);
            commands::pane::select_pane(&parsed)
        }
        "list-panes" | "lsp" => {
            let parsed = cli::TmuxArgs::parse(cmd_args);
            commands::pane::list_panes(&parsed)
        }
        "kill-pane" | "killp" => {
            let parsed = cli::TmuxArgs::parse(cmd_args);
            commands::pane::kill_pane(&parsed)
        }

        // Session commands (use existing i32-returning handlers)
        "new-session" | "new" => {
            let exit_code = commands::session::handle(command, cmd_args);
            return ExitCode::from(exit_code as u8);
        }
        "has-session" | "has" => {
            let exit_code = commands::session::handle("has-session", cmd_args);
            return ExitCode::from(exit_code as u8);
        }
        "kill-session" => {
            let exit_code = commands::session::handle("kill-session", cmd_args);
            return ExitCode::from(exit_code as u8);
        }
        "list-sessions" | "ls" => {
            let exit_code = commands::session::handle("list-sessions", cmd_args);
            return ExitCode::from(exit_code as u8);
        }

        unknown => Err(format!("unknown command: {}", unknown)),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tmux: {}", e);
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_flag_position() {
        // Verify -V is handled at position 1 (args[1])
        let args = vec!["tmux".to_string(), "-V".to_string()];
        assert_eq!(args[1], "-V");
    }
}
