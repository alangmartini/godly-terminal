//! I/O commands: `send-keys` and `display-message`.
//!
//! These are the primary commands Claude Code uses to interact with terminal
//! panes — sending keystrokes/text via the daemon pipe and querying pane info.

use crate::cli::parse_command_args;
use crate::daemon_client::DaemonClient;
use crate::format::{expand, FormatContext};
use crate::state::TmuxState;
use godly_protocol::{Request, Response};

// ── send-keys ──────────────────────────────────────────────────────────────

/// Execute `tmux send-keys [-t target] [-l] <keys...>`.
///
/// Each positional argument is a separate "key" in tmux semantics:
/// - In normal mode, named keys (Enter, Escape, C-c, etc.) are translated
///   to their escape sequences; everything else passes through as literal text.
/// - With `-l`, all arguments are sent as literal text (no key name lookup).
pub fn send_keys(args: &[String]) -> Result<(), String> {
    let (opts, positional) = parse_command_args(args, &["-l"], &["-t"]);

    let literal_mode = opts.contains_key("-l");
    let target = opts.get("-t").map(|s| s.as_str());

    if positional.is_empty() {
        return Ok(()); // tmux silently succeeds with no keys
    }

    let state = TmuxState::load()?;
    let terminal_id = state.resolve_pane(target)?;

    let mut data = Vec::new();
    for key_arg in &positional {
        if literal_mode {
            data.extend_from_slice(key_arg.as_bytes());
        } else {
            data.extend_from_slice(translate_key(key_arg).as_bytes());
        }
    }

    let mut client = DaemonClient::connect()?;
    let resp = client.request(&Request::Write {
        session_id: terminal_id,
        data,
    })?;

    match resp {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(format!("daemon error: {}", message)),
        other => Err(format!("unexpected daemon response: {:?}", other)),
    }
}

/// Translate a tmux key name to its byte sequence.
///
/// In tmux, each positional argument to `send-keys` is either:
/// - A named key (Enter, C-c, Up, F1, etc.) → translated to escape sequence
/// - Literal text → passed through unchanged
pub fn translate_key(key: &str) -> String {
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
                key.to_string() // pass through unknown C- combos
            }
        }

        _ => key.to_string(),
    }
}

// ── display-message ────────────────────────────────────────────────────────

/// Execute `tmux display-message [-p] [-t target] [-F format] [message]`.
///
/// With `-p`, prints the expanded message to stdout (Claude Code reads this).
/// Without `-p`, the message would go to the tmux status bar — we just print
/// it since we don't have a status bar.
pub fn display_message(args: &[String]) -> Result<(), String> {
    let (opts, positional) = parse_command_args(args, &["-p"], &["-t", "-F"]);

    let target = opts.get("-t").map(|s| s.as_str());

    let message = if let Some(fmt) = opts.get("-F") {
        fmt.clone()
    } else if !positional.is_empty() {
        positional.join(" ")
    } else {
        "#{session_name}".to_string()
    };

    let state = TmuxState::load()?;
    let pane_id = state.resolve_pane_id(target);

    let ctx = FormatContext {
        state: &state,
        pane_id: &pane_id,
    };
    let expanded = expand(&message, &ctx);

    println!("{}", expanded);

    Ok(())
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Key translation tests ──────────────────────────────────────────

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
        assert_eq!(translate_key("F6"), "\x1b[17~");
        assert_eq!(translate_key("F7"), "\x1b[18~");
        assert_eq!(translate_key("F8"), "\x1b[19~");
        assert_eq!(translate_key("F9"), "\x1b[20~");
        assert_eq!(translate_key("F10"), "\x1b[21~");
        assert_eq!(translate_key("F11"), "\x1b[23~");
        assert_eq!(translate_key("F12"), "\x1b[24~");
    }

    #[test]
    fn translate_named_ctrl_sequences() {
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
        // C-e = \x05 (e=101, 101-97+1=5)
        assert_eq!(translate_key("C-e"), "\x05");
        // C-f = \x06
        assert_eq!(translate_key("C-f"), "\x06");
        // C-g = \x07
        assert_eq!(translate_key("C-g"), "\x07");
        // C-h = \x08
        assert_eq!(translate_key("C-h"), "\x08");
        // C-j = \x0a
        assert_eq!(translate_key("C-j"), "\x0a");
        // C-k = \x0b
        assert_eq!(translate_key("C-k"), "\x0b");
        // C-n = \x0e
        assert_eq!(translate_key("C-n"), "\x0e");
        // C-o = \x0f
        assert_eq!(translate_key("C-o"), "\x0f");
        // C-p = \x10
        assert_eq!(translate_key("C-p"), "\x10");
        // C-r = \x12
        assert_eq!(translate_key("C-r"), "\x12");
        // C-s = \x13
        assert_eq!(translate_key("C-s"), "\x13");
        // C-t = \x14
        assert_eq!(translate_key("C-t"), "\x14");
        // C-v = \x16
        assert_eq!(translate_key("C-v"), "\x16");
        // C-x = \x18
        assert_eq!(translate_key("C-x"), "\x18");
        // C-y = \x19
        assert_eq!(translate_key("C-y"), "\x19");
    }

    #[test]
    fn translate_ctrl_uppercase() {
        // Uppercase should produce the same ctrl code
        assert_eq!(translate_key("C-A"), "\x01");
        assert_eq!(translate_key("C-Z"), "\x1a");
    }

    #[test]
    fn translate_literal_passthrough() {
        assert_eq!(translate_key("echo hello"), "echo hello");
        assert_eq!(translate_key("ls -la"), "ls -la");
        assert_eq!(translate_key("a"), "a");
        assert_eq!(translate_key("Hello World"), "Hello World");
    }

    #[test]
    fn translate_empty_string() {
        assert_eq!(translate_key(""), "");
    }

    // ── Argument assembly tests ────────────────────────────────────────

    #[test]
    fn send_keys_arg_assembly_normal_mode() {
        // Simulates: tmux send-keys "echo hello" Enter
        let keys = vec!["echo hello".to_string(), "Enter".to_string()];
        let mut data = Vec::new();
        for key in &keys {
            data.extend_from_slice(translate_key(key).as_bytes());
        }
        assert_eq!(String::from_utf8(data).unwrap(), "echo hello\r");
    }

    #[test]
    fn send_keys_arg_assembly_literal_mode() {
        // In literal mode, "Enter" is NOT translated
        let keys = vec!["echo hello".to_string(), "Enter".to_string()];
        let mut data = Vec::new();
        for key in &keys {
            data.extend_from_slice(key.as_bytes()); // literal mode: no translation
        }
        assert_eq!(String::from_utf8(data).unwrap(), "echo helloEnter");
    }

    #[test]
    fn send_keys_arg_assembly_multiple_special_keys() {
        // Simulates: tmux send-keys C-c "cd /tmp" Enter
        let keys = vec![
            "C-c".to_string(),
            "cd /tmp".to_string(),
            "Enter".to_string(),
        ];
        let mut data = Vec::new();
        for key in &keys {
            data.extend_from_slice(translate_key(key).as_bytes());
        }
        assert_eq!(data, b"\x03cd /tmp\r");
    }

    #[test]
    fn send_keys_arg_assembly_arrow_sequence() {
        // Simulates: tmux send-keys Up Up Enter
        let keys = vec![
            "Up".to_string(),
            "Up".to_string(),
            "Enter".to_string(),
        ];
        let mut data = Vec::new();
        for key in &keys {
            data.extend_from_slice(translate_key(key).as_bytes());
        }
        assert_eq!(
            String::from_utf8(data).unwrap(),
            "\x1b[A\x1b[A\r"
        );
    }

    #[test]
    fn send_keys_arg_assembly_mixed_text_and_keys() {
        // Simulates: tmux send-keys "git " "commit" Space "-m" Space "\"fix\"" Enter
        let keys = vec![
            "git ".to_string(),
            "commit".to_string(),
            "Space".to_string(),
            "-m".to_string(),
            "Space".to_string(),
            "\"fix\"".to_string(),
            "Enter".to_string(),
        ];
        let mut data = Vec::new();
        for key in &keys {
            data.extend_from_slice(translate_key(key).as_bytes());
        }
        assert_eq!(
            String::from_utf8(data).unwrap(),
            "git commit -m \"fix\"\r"
        );
    }

    // ── CLI parsing integration ────────────────────────────────────────

    #[test]
    fn parse_send_keys_with_target() {
        let args = vec![
            "-t".to_string(),
            "%1".to_string(),
            "echo".to_string(),
            "Enter".to_string(),
        ];
        let (opts, positional) = parse_command_args(&args, &["-l"], &["-t"]);
        assert_eq!(opts.get("-t").unwrap(), "%1");
        assert!(!opts.contains_key("-l"));
        assert_eq!(positional, vec!["echo", "Enter"]);
    }

    #[test]
    fn parse_send_keys_literal_mode() {
        let args = vec![
            "-l".to_string(),
            "-t".to_string(),
            "%0".to_string(),
            "Enter".to_string(),
        ];
        let (opts, positional) = parse_command_args(&args, &["-l"], &["-t"]);
        assert!(opts.contains_key("-l"));
        assert_eq!(opts.get("-t").unwrap(), "%0");
        assert_eq!(positional, vec!["Enter"]);
    }

    #[test]
    fn parse_display_message_with_format() {
        let args = vec![
            "-p".to_string(),
            "-F".to_string(),
            "#{pane_id}".to_string(),
        ];
        let (opts, positional) = parse_command_args(&args, &["-p"], &["-t", "-F"]);
        assert!(opts.contains_key("-p"));
        assert_eq!(opts.get("-F").unwrap(), "#{pane_id}");
        assert!(positional.is_empty());
    }

    #[test]
    fn parse_display_message_with_positional() {
        let args = vec!["-p".to_string(), "hello world".to_string()];
        let (opts, positional) = parse_command_args(&args, &["-p"], &["-t", "-F"]);
        assert!(opts.contains_key("-p"));
        assert_eq!(positional, vec!["hello world"]);
    }
}
