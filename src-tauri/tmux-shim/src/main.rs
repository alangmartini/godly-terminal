//! `godly-tmux-shim` — a tmux-compatible CLI that translates tmux commands
//! into Godly Terminal operations via MCP and daemon pipes.
//!
//! This binary is named `tmux.exe` so that tools expecting tmux (e.g. Claude
//! Code's Agent Teams) can use it as a drop-in replacement.

#[macro_use]
mod cli;
mod commands;
#[allow(dead_code)]
mod daemon_client;
mod format;
mod mcp_client;
mod state;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();

    // Strip `-L <socket>` — the shim is a single-instance emulator, not a real
    // tmux server, so all socket names map to the same state. Claude Code uses
    // `-L claude-swarm-<pid>` for agent team sessions.
    if args.len() > 2 && args[1] == "-L" {
        args.drain(1..3);
    }

    // Seed tmux-state.json on first use. The daemon sets TMUX/TMUX_PANE env
    // vars but doesn't create the state file, so the shim initializes it here
    // by querying the active workspace via MCP. This is a no-op once state exists.
    if let Err(e) = state::ensure_initialized(|| {
        let client = mcp_client::McpPipeClient::connect()?;
        let ws = client.get_active_workspace()?;
        Ok(ws.id)
    }) {
        eprintln!("tmux: warning: failed to initialize state: {}", e);
        // Continue — some commands (like -V) don't need state
    }

    let exit_code = if args.len() <= 1 {
        // Bare `tmux` = `new-session`
        commands::session::handle("new-session", &args[1..])
    } else {
        match args[1].as_str() {
            "-V" => {
                println!("tmux 3.4");
                0
            }

            // Session lifecycle
            "new-session" | "new" => commands::session::handle("new-session", &args[2..]),
            "has-session" | "has" => commands::session::handle("has-session", &args[2..]),
            "kill-session" => commands::session::handle("kill-session", &args[2..]),
            "list-sessions" | "ls" => commands::session::handle("list-sessions", &args[2..]),

            // Pane management
            "split-window" | "splitw" => commands::pane::handle("split-window", &args[2..]),
            "select-pane" | "selectp" => commands::pane::handle("select-pane", &args[2..]),
            "list-panes" | "lsp" => commands::pane::handle("list-panes", &args[2..]),
            "kill-pane" | "killp" => commands::pane::handle("kill-pane", &args[2..]),

            // I/O commands
            "send-keys" | "send" => commands::io::handle("send-keys", &args[2..]),
            "display-message" | "display" => commands::io::handle("display-message", &args[2..]),
            "capture-pane" | "capturep" => commands::io::handle("capture-pane", &args[2..]),

            // No-op commands: these are used by Claude Code's TmuxBackend for
            // cosmetic/layout purposes. We accept them silently to avoid error
            // exit codes that could abort the agent team workflow.
            "set-option" | "set" => 0,
            "select-layout" | "selectl" => 0,
            "resize-pane" | "resizep" => 0,
            "new-window" | "neww" => 0,
            "break-pane" | "breakp" => 0,
            "join-pane" | "joinp" => 0,
            "switch-client" | "switchc" => 0,
            "show-options" | "show" => 0,

            unknown => {
                eprintln!("tmux: unknown command: {}", unknown);
                1
            }
        }
    };

    std::process::exit(exit_code);
}
