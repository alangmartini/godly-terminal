use iced::keyboard::{key::Named, Key, Modifiers};

/// Convert an Iced keyboard event into PTY input bytes.
///
/// Returns `None` if the key shouldn't produce terminal input
/// (e.g., standalone modifier keys, unrecognized combos).
pub fn key_to_pty_bytes(key: &Key, modifiers: Modifiers) -> Option<Vec<u8>> {
    match key {
        // Printable characters
        Key::Character(ch) => {
            let s = ch.as_str();
            if modifiers.control() && s.len() == 1 {
                // Ctrl+key: produce control character (0x01..0x1A)
                let c = s.as_bytes()[0];
                let ctrl_char = match c.to_ascii_lowercase() {
                    b'a'..=b'z' => c.to_ascii_lowercase() - b'a' + 1,
                    b'[' => 0x1B, // ESC
                    b'\\' => 0x1C,
                    b']' => 0x1D,
                    b'^' => 0x1E,
                    b'_' => 0x1F,
                    _ => return None,
                };
                Some(vec![ctrl_char])
            } else {
                // Normal character — send as UTF-8
                Some(s.as_bytes().to_vec())
            }
        }

        // Named keys
        Key::Named(named) => named_key_to_bytes(named, modifiers),

        Key::Unidentified => None,
    }
}

/// Convert a named key to PTY bytes.
fn named_key_to_bytes(key: &Named, modifiers: Modifiers) -> Option<Vec<u8>> {
    match key {
        Named::Enter => Some(b"\r".to_vec()),
        Named::Backspace => Some(vec![0x7F]), // DEL
        Named::Tab => {
            if modifiers.shift() {
                Some(b"\x1b[Z".to_vec()) // Shift+Tab → reverse tab
            } else {
                Some(b"\t".to_vec())
            }
        }
        Named::Escape => Some(vec![0x1B]),
        Named::Space => Some(b" ".to_vec()),
        Named::Delete => Some(b"\x1b[3~".to_vec()),
        Named::Insert => Some(b"\x1b[2~".to_vec()),
        Named::Home => Some(b"\x1b[H".to_vec()),
        Named::End => Some(b"\x1b[F".to_vec()),
        Named::PageUp => Some(b"\x1b[5~".to_vec()),
        Named::PageDown => Some(b"\x1b[6~".to_vec()),

        // Arrow keys
        Named::ArrowUp => Some(b"\x1b[A".to_vec()),
        Named::ArrowDown => Some(b"\x1b[B".to_vec()),
        Named::ArrowRight => Some(b"\x1b[C".to_vec()),
        Named::ArrowLeft => Some(b"\x1b[D".to_vec()),

        // Function keys
        Named::F1 => Some(b"\x1bOP".to_vec()),
        Named::F2 => Some(b"\x1bOQ".to_vec()),
        Named::F3 => Some(b"\x1bOR".to_vec()),
        Named::F4 => Some(b"\x1bOS".to_vec()),
        Named::F5 => Some(b"\x1b[15~".to_vec()),
        Named::F6 => Some(b"\x1b[17~".to_vec()),
        Named::F7 => Some(b"\x1b[18~".to_vec()),
        Named::F8 => Some(b"\x1b[19~".to_vec()),
        Named::F9 => Some(b"\x1b[20~".to_vec()),
        Named::F10 => Some(b"\x1b[21~".to_vec()),
        Named::F11 => Some(b"\x1b[23~".to_vec()),
        Named::F12 => Some(b"\x1b[24~".to_vec()),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_char() {
        let bytes = key_to_pty_bytes(&Key::Character("a".into()), Modifiers::empty());
        assert_eq!(bytes, Some(b"a".to_vec()));
    }

    #[test]
    fn ctrl_c() {
        let bytes = key_to_pty_bytes(&Key::Character("c".into()), Modifiers::CTRL);
        assert_eq!(bytes, Some(vec![0x03]));
    }

    #[test]
    fn enter_key() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::Enter), Modifiers::empty());
        assert_eq!(bytes, Some(b"\r".to_vec()));
    }

    #[test]
    fn arrow_up() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::ArrowUp), Modifiers::empty());
        assert_eq!(bytes, Some(b"\x1b[A".to_vec()));
    }

    #[test]
    fn backspace() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::Backspace), Modifiers::empty());
        assert_eq!(bytes, Some(vec![0x7F]));
    }

    #[test]
    fn f1_key() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::F1), Modifiers::empty());
        assert_eq!(bytes, Some(b"\x1bOP".to_vec()));
    }

    #[test]
    fn shift_tab() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::Tab), Modifiers::SHIFT);
        assert_eq!(bytes, Some(b"\x1b[Z".to_vec()));
    }

    #[test]
    fn ctrl_bracket() {
        let bytes = key_to_pty_bytes(&Key::Character("[".into()), Modifiers::CTRL);
        assert_eq!(bytes, Some(vec![0x1B])); // ESC
    }

    #[test]
    fn utf8_char() {
        let bytes = key_to_pty_bytes(&Key::Character("é".into()), Modifiers::empty());
        assert_eq!(bytes, Some("é".as_bytes().to_vec()));
    }
}
