//! I/O commands: `send-keys` and `display-message`.
//!
//! These are the primary commands Claude Code uses to interact with terminal
//! panes — sending keystrokes/text via the daemon pipe and querying pane info.

use crate::cli::TmuxArgs;
use crate::daemon_client::DaemonPipeClient;
use crate::format::{self, FormatVars};
use crate::state;

/// Execute `tmux send-keys [-t target] [-l] <keys...>`.
///
/// Each positional argument is a separate "key" in tmux semantics:
/// - In normal mode, named keys (Enter, Escape, C-c, etc.) are translated
///   to their escape sequences; everything else passes through as literal text.
/// - With `-l`, all arguments are sent as literal text (no key name lookup).
pub fn send_keys(args: &TmuxArgs) -> Result<(), String> {
    let literal_mode = args.has_flag('l');
    let target = args.get_option('t');

    if args.positional.is_empty() {
        return Ok(()); // tmux silently succeeds with no keys
    }

    let state = state::load().map_err(|e| format!("failed to load state: {}", e))?;
    let terminal_id = state.resolve_pane(target)?;

    let mut data = Vec::new();
    for key_arg in &args.positional {
        if literal_mode {
            data.extend_from_slice(key_arg.as_bytes());
        } else {
            data.extend_from_slice(translate_key(key_arg).as_bytes());
        }
    }

    let client = DaemonPipeClient::connect()?;
    client.write_to_terminal(&terminal_id, data)
}

/// Translate a tmux key name to its byte sequence.
fn translate_key(key: &str) -> String {
    match key {
        // Basic control keys
        "Enter" | "C-m" => "\r".to_string(),
        "Escape" | "C-[" => "\x1b".to_string(),
        "Tab" | "C-i" => "\t".to_string(),
        "Space" => " ".to_string(),
        "BSpace" => "\x7f".to_string(),
        "BTab" => "\x1b[Z".to_string(),
        "DC" => "\x1b[3~".to_string(),

        // Navigation
        "Home" => "\x1b[H".to_string(),
        "End" => "\x1b[F".to_string(),
        "NPage" => "\x1b[6~".to_string(),
        "PPage" => "\x1b[5~".to_string(),
        "Up" => "\x1b[A".to_string(),
        "Down" => "\x1b[B".to_string(),
        "Right" => "\x1b[C".to_string(),
        "Left" => "\x1b[D".to_string(),

        // Function keys
        "F1" => "\x1bOP".to_string(),
        "F2" => "\x1bOQ".to_string(),
        "F3" => "\x1bOR".to_string(),
        "F4" => "\x1bOS".to_string(),
        "F5" => "\x1b[15~".to_string(),
        "F6" => "\x1b[17~".to_string(),
        "F7" => "\x1b[18~".to_string(),
        "F8" => "\x1b[19~".to_string(),
        "F9" => "\x1b[20~".to_string(),
        "F10" => "\x1b[21~".to_string(),
        "F11" => "\x1b[23~".to_string(),
        "F12" => "\x1b[24~".to_string(),

        // Named control sequences
        "C-c" => "\x03".to_string(),
        "C-d" => "\x04".to_string(),
        "C-z" => "\x1a".to_string(),
        "C-l" => "\x0c".to_string(),
        "C-a" => "\x01".to_string(),
        "C-b" => "\x02".to_string(),
        "C-u" => "\x15".to_string(),
        "C-w" => "\x17".to_string(),

        // Generic C-<letter> pattern
        _ if key.starts_with("C-") && key.len() == 3 => {
            let letter = key.as_bytes()[2];
            if letter.is_ascii_lowercase() {
                let ctrl_code = letter - b'a' + 1;
                String::from(ctrl_code as char)
            } else if letter.is_ascii_uppercase() {
                let ctrl_code = letter - b'A' + 1;
                String::from(ctrl_code as char)
            } else {
                key.to_string()
            }
        }

        _ => key.to_string(),
    }
}

/// Execute `tmux display-message [-p] [-t target] [-F format] [message]`.
///
/// With `-p`, prints the expanded message to stdout (Claude Code reads this).
/// Without `-p`, the message would go to the tmux status bar — we just print
/// it since we don't have a status bar.
pub fn display_message(args: &TmuxArgs) -> Result<(), String> {
    let target = args.get_option('t');

    let message = if let Some(fmt) = args.get_option('F') {
        fmt.to_string()
    } else if !args.positional.is_empty() {
        args.positional.join(" ")
    } else {
        "#{session_name}".to_string()
    };

    let state = state::load().map_err(|e| format!("failed to load state: {}", e))?;
    let pane_id = state.resolve_pane_id(target);

    let session_name = state
        .pane_session(&pane_id)
        .unwrap_or("default")
        .to_string();
    let pane_index = state.pane_index(&pane_id) as u32;

    let vars = FormatVars {
        pane_id,
        session_name,
        pane_width: 80,
        pane_height: 24,
        pane_index,
        pane_active: true,
        window_index: 0,
    };
    let expanded = format::expand(&message, &vars);
    println!("{}", expanded);

    Ok(())
}

/// Execute `tmux capture-pane [-t target] [-p]`.
///
/// Captures the pane content by reading the grid from the daemon.
pub fn capture_pane(args: &TmuxArgs) -> Result<(), String> {
    let target = args.get_option('t');
    let state = state::load().map_err(|e| format!("failed to load state: {}", e))?;
    let terminal_id = state.resolve_pane(target)?;

    let client = DaemonPipeClient::connect()?;
    let resp = client
        .send_request(&godly_protocol::Request::ReadGrid {
            session_id: terminal_id,
        })
        .map_err(|e| format!("daemon error: {}", e))?;

    match resp {
        godly_protocol::Response::Grid { grid } => {
            for row in &grid.rows {
                println!("{}", row);
            }
            Ok(())
        }
        godly_protocol::Response::Error { message } => Err(message),
        other => Err(format!("unexpected response: {:?}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_enter() {
        assert_eq!(translate_key("Enter"), "\r");
        assert_eq!(translate_key("C-m"), "\r");
    }

    #[test]
    fn translate_escape() {
        assert_eq!(translate_key("Escape"), "\x1b");
    }

    #[test]
    fn translate_arrow_keys() {
        assert_eq!(translate_key("Up"), "\x1b[A");
        assert_eq!(translate_key("Down"), "\x1b[B");
        assert_eq!(translate_key("Right"), "\x1b[C");
        assert_eq!(translate_key("Left"), "\x1b[D");
    }

    #[test]
    fn translate_function_keys() {
        assert_eq!(translate_key("F1"), "\x1bOP");
        assert_eq!(translate_key("F12"), "\x1b[24~");
    }

    #[test]
    fn translate_ctrl_sequences() {
        assert_eq!(translate_key("C-c"), "\x03");
        assert_eq!(translate_key("C-d"), "\x04");
        assert_eq!(translate_key("C-z"), "\x1a");
    }

    #[test]
    fn translate_generic_ctrl_letter() {
        assert_eq!(translate_key("C-e"), "\x05");
        assert_eq!(translate_key("C-A"), "\x01");
    }

    #[test]
    fn translate_literal_passthrough() {
        assert_eq!(translate_key("echo hello"), "echo hello");
        assert_eq!(translate_key("a"), "a");
    }

    #[test]
    fn key_assembly_normal_mode() {
        let keys = vec!["echo hello", "Enter"];
        let mut data = Vec::new();
        for key in &keys {
            data.extend_from_slice(translate_key(key).as_bytes());
        }
        assert_eq!(String::from_utf8(data).unwrap(), "echo hello\r");
    }

    #[test]
    fn key_assembly_literal_mode() {
        let keys = vec!["echo hello", "Enter"];
        let mut data = Vec::new();
        for key in &keys {
            data.extend_from_slice(key.as_bytes());
        }
        assert_eq!(String::from_utf8(data).unwrap(), "echo helloEnter");
    }
}
