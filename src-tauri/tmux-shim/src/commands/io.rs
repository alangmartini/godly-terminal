//! I/O commands: `send-keys`, `display-message`, `capture-pane`.
//!
//! These are the primary commands Claude Code uses to interact with terminal
//! panes — sending keystrokes/text, querying pane info, and reading output.

use crate::cli::TmuxArgs;
use crate::format;
use crate::mcp_client::McpPipeClient;
use crate::state;

/// Dispatch I/O commands.
pub fn handle(command: &str, args: &[String]) -> i32 {
    match command {
        "send-keys" => send_keys(args),
        "display-message" => display_message(args),
        "capture-pane" => capture_pane(args),
        _ => {
            eprintln!("tmux: unknown io command: {}", command);
            1
        }
    }
}

/// `send-keys [-t target] [-l] <keys...>`
///
/// Each positional argument is a tmux "key":
/// - In normal mode, named keys (Enter, Escape, C-c, etc.) are translated
///   to escape sequences; everything else passes through as literal text.
/// - With `-l`, all arguments are sent as literal text (no key name lookup).
fn send_keys(args: &[String]) -> i32 {
    let parsed = TmuxArgs::parse(args);
    let literal_mode = parsed.has_flag('l');
    let target = parsed.get_option('t');

    if parsed.positional.is_empty() {
        return 0; // tmux silently succeeds with no keys
    }

    let st = tmux_try!(state::load(), "failed to read state");
    let terminal_id = tmux_try!(st.resolve_target(target));

    let mut data = String::new();
    for key_arg in &parsed.positional {
        if literal_mode {
            data.push_str(key_arg);
        } else {
            data.push_str(&translate_key(key_arg));
        }
    }

    let client = tmux_try!(McpPipeClient::connect());
    tmux_try!(client.write_to_terminal(&terminal_id, &data));

    0
}

/// Translate a tmux key name to its byte sequence.
///
/// Named keys (Enter, Escape, Up, etc.) are translated to escape sequences.
/// The generic `C-<letter>` handler covers all ctrl sequences.
/// Anything else passes through unchanged.
pub fn translate_key(key: &str) -> String {
    match key {
        // Named keys with special byte sequences
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

        // Generic C-<letter> pattern (covers C-a through C-z)
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

/// `display-message [-p] [-t target] [-F format] [message]`
///
/// With `-p`, prints the expanded message to stdout (Claude Code reads this).
/// Without `-p`, we just print it since we don't have a tmux status bar.
fn display_message(args: &[String]) -> i32 {
    let parsed = TmuxArgs::parse(args);
    let target = parsed.get_option('t');

    let message = if let Some(fmt) = parsed.get_option('F') {
        fmt.to_string()
    } else if !parsed.positional.is_empty() {
        parsed.positional.join(" ")
    } else {
        "#{session_name}".to_string()
    };

    let st = tmux_try!(state::load(), "failed to read state");
    let pane_id = st.resolve_pane_id(target);
    let session_name = st.pane_session(&pane_id).unwrap_or("default").to_string();

    let vars = format::session_format_vars(&pane_id, &session_name);
    println!("{}", format::expand_format(&message, &vars));

    0
}

/// `capture-pane [-t target] [-p]`
///
/// Read the terminal content and print to stdout.
fn capture_pane(args: &[String]) -> i32 {
    let parsed = TmuxArgs::parse(args);
    let target = parsed.get_option('t');

    let st = tmux_try!(state::load(), "failed to read state");
    let terminal_id = tmux_try!(st.resolve_target(target));
    let client = tmux_try!(McpPipeClient::connect());

    match client.read_terminal(&terminal_id) {
        Ok(content) => {
            print!("{}", content);
            0
        }
        Err(e) => {
            eprintln!("tmux: {}", e);
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Key translation tests ──

    #[test]
    fn translate_enter() {
        assert_eq!(translate_key("Enter"), "\r");
        assert_eq!(translate_key("C-m"), "\r");
    }

    #[test]
    fn translate_escape() {
        assert_eq!(translate_key("Escape"), "\x1b");
        assert_eq!(translate_key("C-["), "\x1b");
    }

    #[test]
    fn translate_tab() {
        assert_eq!(translate_key("Tab"), "\t");
        assert_eq!(translate_key("C-i"), "\t");
    }

    #[test]
    fn translate_space() {
        assert_eq!(translate_key("Space"), " ");
    }

    #[test]
    fn translate_backspace() {
        assert_eq!(translate_key("BSpace"), "\x7f");
    }

    #[test]
    fn translate_backtab() {
        assert_eq!(translate_key("BTab"), "\x1b[Z");
    }

    #[test]
    fn translate_delete() {
        assert_eq!(translate_key("DC"), "\x1b[3~");
    }

    #[test]
    fn translate_navigation_keys() {
        assert_eq!(translate_key("Home"), "\x1b[H");
        assert_eq!(translate_key("End"), "\x1b[F");
        assert_eq!(translate_key("NPage"), "\x1b[6~");
        assert_eq!(translate_key("PPage"), "\x1b[5~");
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
        assert_eq!(translate_key("F2"), "\x1bOQ");
        assert_eq!(translate_key("F3"), "\x1bOR");
        assert_eq!(translate_key("F4"), "\x1bOS");
        assert_eq!(translate_key("F5"), "\x1b[15~");
        assert_eq!(translate_key("F12"), "\x1b[24~");
    }

    #[test]
    fn translate_ctrl_sequences_via_generic_handler() {
        assert_eq!(translate_key("C-c"), "\x03");
        assert_eq!(translate_key("C-d"), "\x04");
        assert_eq!(translate_key("C-z"), "\x1a");
        assert_eq!(translate_key("C-l"), "\x0c");
        assert_eq!(translate_key("C-a"), "\x01");
        assert_eq!(translate_key("C-b"), "\x02");
        assert_eq!(translate_key("C-u"), "\x15");
        assert_eq!(translate_key("C-w"), "\x17");
    }

    #[test]
    fn translate_generic_ctrl_letter() {
        assert_eq!(translate_key("C-e"), "\x05");
        assert_eq!(translate_key("C-f"), "\x06");
        assert_eq!(translate_key("C-g"), "\x07");
        assert_eq!(translate_key("C-h"), "\x08");
        assert_eq!(translate_key("C-n"), "\x0e");
        assert_eq!(translate_key("C-r"), "\x12");
    }

    #[test]
    fn translate_ctrl_uppercase() {
        assert_eq!(translate_key("C-A"), "\x01");
        assert_eq!(translate_key("C-Z"), "\x1a");
    }

    #[test]
    fn translate_literal_passthrough() {
        assert_eq!(translate_key("echo hello"), "echo hello");
        assert_eq!(translate_key("ls -la"), "ls -la");
        assert_eq!(translate_key("a"), "a");
    }

    #[test]
    fn translate_empty_string() {
        assert_eq!(translate_key(""), "");
    }

    // ── Key assembly tests ──

    #[test]
    fn send_keys_arg_assembly_normal_mode() {
        let keys = vec!["echo hello".to_string(), "Enter".to_string()];
        let mut data = String::new();
        for key in &keys {
            data.push_str(&translate_key(key));
        }
        assert_eq!(data, "echo hello\r");
    }

    #[test]
    fn send_keys_arg_assembly_literal_mode() {
        let keys = vec!["echo hello".to_string(), "Enter".to_string()];
        let mut data = String::new();
        for key in &keys {
            data.push_str(key); // literal: no translation
        }
        assert_eq!(data, "echo helloEnter");
    }

    #[test]
    fn send_keys_arg_assembly_ctrl_c_then_command() {
        let keys = vec![
            "C-c".to_string(),
            "cd /tmp".to_string(),
            "Enter".to_string(),
        ];
        let mut data = String::new();
        for key in &keys {
            data.push_str(&translate_key(key));
        }
        assert_eq!(data, "\x03cd /tmp\r");
    }

    #[test]
    fn send_keys_arg_assembly_arrow_sequence() {
        let keys = vec!["Up".to_string(), "Up".to_string(), "Enter".to_string()];
        let mut data = String::new();
        for key in &keys {
            data.push_str(&translate_key(key));
        }
        assert_eq!(data, "\x1b[A\x1b[A\r");
    }

    #[test]
    fn send_keys_arg_assembly_mixed_text_and_keys() {
        let keys = vec![
            "git ".to_string(),
            "commit".to_string(),
            "Space".to_string(),
            "-m".to_string(),
            "Space".to_string(),
            "\"fix\"".to_string(),
            "Enter".to_string(),
        ];
        let mut data = String::new();
        for key in &keys {
            data.push_str(&translate_key(key));
        }
        assert_eq!(data, "git commit -m \"fix\"\r");
    }

    #[test]
    fn handle_dispatches_unknown_command() {
        let code = handle("nonexistent", &[]);
        assert_eq!(code, 1);
    }
}
