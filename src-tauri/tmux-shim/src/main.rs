//! `godly-tmux-shim` — a tmux-compatible CLI that translates tmux commands
//! into Godly Terminal operations via MCP and daemon pipes.
//!
//! This binary is named `tmux.exe` so that tools expecting tmux (e.g. Claude
//! Code's Agent Teams) can use it as a drop-in replacement.

mod cli;
mod commands;
mod daemon_client;
mod format;
mod mcp_client;
mod state;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args = match cli::TmuxArgs::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("tmux: {}", e);
            return ExitCode::from(1);
        }
    };

    let result = match args.subcommand.as_str() {
        // Pane commands (fully implemented)
        "split-window" => commands::pane::split_window(&args),
        "select-pane" | "selectp" => commands::pane::select_pane(&args),
        "list-panes" | "lsp" => commands::pane::list_panes(&args),
        "kill-pane" | "killp" => commands::pane::kill_pane(&args),

        // Session commands (stubs)
        "new-session" | "new" => commands::session::new_session(&args),
        "has-session" | "has" => commands::session::has_session(&args),
        "kill-session" => commands::session::kill_session(&args),
        "list-sessions" | "ls" => commands::session::list_sessions(&args),

        // I/O commands (stubs)
        "send-keys" | "send" => commands::io::send_keys(&args),
        "capture-pane" | "capturep" => commands::io::capture_pane(&args),
        "wait-for" | "wait" => commands::io::wait_for(&args),

        // Version — tools often check `tmux -V`
        "-V" => {
            println!("tmux 3.4 (godly-shim 0.1.0)");
            Ok(())
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
