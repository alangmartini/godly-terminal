mod cli;
mod commands;
#[allow(dead_code)]
mod daemon_client;
mod format;
mod mcp_client;
mod state;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let exit_code = if args.len() <= 1 {
        // Bare `tmux` = `new-session`
        commands::session::handle("new-session", &args[1..])
    } else {
        match args[1].as_str() {
            "-V" => {
                println!("tmux 3.4");
                0
            }
            "new-session" | "new" => commands::session::handle("new-session", &args[2..]),
            "has-session" | "has" => commands::session::handle("has-session", &args[2..]),
            "kill-session" => commands::session::handle("kill-session", &args[2..]),
            "list-sessions" | "ls" => commands::session::handle("list-sessions", &args[2..]),
            "split-window" | "splitw" => commands::pane::handle_pane_command(&args[1..]),
            "select-pane" | "selectp" => commands::pane::handle_pane_command(&args[1..]),
            "list-panes" | "lsp" => commands::pane::handle_pane_command(&args[1..]),
            "kill-pane" | "killp" => commands::pane::handle_pane_command(&args[1..]),
            "send-keys" | "send" => commands::io::handle_io_command(&args[1..]),
            "display-message" | "display" => commands::io::handle_io_command(&args[1..]),
            "capture-pane" | "capturep" => commands::io::handle_io_command(&args[1..]),
            unknown => {
                eprintln!("tmux: unknown command: {}", unknown);
                1
            }
        }
    };

    std::process::exit(exit_code);
}
