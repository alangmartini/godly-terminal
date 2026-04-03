use crate::keyboard::{Key, Modifiers, Named};

/// Convert an Iced keyboard event into PTY input bytes.
///
/// Returns `None` if the key shouldn't produce terminal input
/// (e.g., standalone modifier keys, unrecognized combos).
pub fn key_to_pty_bytes(key: &Key, modifiers: Modifiers) -> Option<Vec<u8>> {
    match key {
        // Printable characters
        Key::Character(ch) => {
            let s: &str = ch;
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
            } else if modifiers.shift() && s.len() == 1 && s.as_bytes()[0].is_ascii_lowercase() {
                // Shift+letter: produce uppercase ASCII
                Some(vec![s.as_bytes()[0].to_ascii_uppercase()])
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

/// Compute the xterm CSI modifier parameter: 1 + shift*1 + alt*2 + ctrl*4.
/// Returns 0 when no modifiers are held (caller uses this to decide format).
fn csi_modifier(modifiers: Modifiers) -> u8 {
    let mut m: u8 = 0;
    if modifiers.shift() {
        m += 1;
    }
    if modifiers.alt() {
        m += 2;
    }
    if modifiers.control() {
        m += 4;
    }
    if m == 0 {
        0
    } else {
        1 + m
    }
}

/// CSI letter sequence: `\x1b[A` without modifiers, `\x1b[1;{mod}A` with.
fn csi_letter(letter: u8, modifiers: Modifiers) -> Vec<u8> {
    let m = csi_modifier(modifiers);
    if m == 0 {
        vec![0x1B, b'[', letter]
    } else {
        format!("\x1b[1;{m}{}", letter as char).into_bytes()
    }
}

/// CSI tilde sequence: `\x1b[{n}~` without modifiers, `\x1b[{n};{mod}~` with.
fn csi_tilde(n: u8, modifiers: Modifiers) -> Vec<u8> {
    let m = csi_modifier(modifiers);
    if m == 0 {
        format!("\x1b[{n}~").into_bytes()
    } else {
        format!("\x1b[{n};{m}~").into_bytes()
    }
}

/// SS3 function key: `\x1bOP` without modifiers, `\x1b[1;{mod}P` with.
fn ss3_or_csi(letter: u8, modifiers: Modifiers) -> Vec<u8> {
    let m = csi_modifier(modifiers);
    if m == 0 {
        vec![0x1B, b'O', letter]
    } else {
        format!("\x1b[1;{m}{}", letter as char).into_bytes()
    }
}

/// Convert a named key to PTY bytes.
fn named_key_to_bytes(key: &Named, modifiers: Modifiers) -> Option<Vec<u8>> {
    match key {
        Named::Enter => {
            let m = csi_modifier(modifiers);
            if m == 0 {
                Some(b"\r".to_vec())
            } else {
                // CSI u format: \x1b[13;{mod}u  (keycode 13 = CR/Enter)
                Some(format!("\x1b[13;{m}u").into_bytes())
            }
        }
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

        Named::Delete => Some(csi_tilde(3, modifiers)),
        Named::Insert => Some(csi_tilde(2, modifiers)),
        Named::Home => Some(csi_letter(b'H', modifiers)),
        Named::End => Some(csi_letter(b'F', modifiers)),
        Named::PageUp => Some(csi_tilde(5, modifiers)),
        Named::PageDown => Some(csi_tilde(6, modifiers)),

        // Arrow keys
        Named::ArrowUp => Some(csi_letter(b'A', modifiers)),
        Named::ArrowDown => Some(csi_letter(b'B', modifiers)),
        Named::ArrowRight => Some(csi_letter(b'C', modifiers)),
        Named::ArrowLeft => Some(csi_letter(b'D', modifiers)),

        // Function keys (F1-F4 use SS3 format, F5+ use tilde)
        Named::F1 => Some(ss3_or_csi(b'P', modifiers)),
        Named::F2 => Some(ss3_or_csi(b'Q', modifiers)),
        Named::F3 => Some(ss3_or_csi(b'R', modifiers)),
        Named::F4 => Some(ss3_or_csi(b'S', modifiers)),
        Named::F5 => Some(csi_tilde(15, modifiers)),
        Named::F6 => Some(csi_tilde(17, modifiers)),
        Named::F7 => Some(csi_tilde(18, modifiers)),
        Named::F8 => Some(csi_tilde(19, modifiers)),
        Named::F9 => Some(csi_tilde(20, modifiers)),
        Named::F10 => Some(csi_tilde(21, modifiers)),
        Named::F11 => Some(csi_tilde(23, modifiers)),
        Named::F12 => Some(csi_tilde(24, modifiers)),

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

    // --- Modifier sequence tests ---

    #[test]
    fn shift_arrow_left() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::ArrowLeft), Modifiers::SHIFT);
        assert_eq!(bytes, Some(b"\x1b[1;2D".to_vec()));
    }

    #[test]
    fn ctrl_arrow_right() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::ArrowRight), Modifiers::CTRL);
        assert_eq!(bytes, Some(b"\x1b[1;5C".to_vec()));
    }

    #[test]
    fn shift_ctrl_arrow_up() {
        let mods = Modifiers::SHIFT | Modifiers::CTRL;
        let bytes = key_to_pty_bytes(&Key::Named(Named::ArrowUp), mods);
        assert_eq!(bytes, Some(b"\x1b[1;6A".to_vec()));
    }

    #[test]
    fn alt_arrow_down() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::ArrowDown), Modifiers::ALT);
        assert_eq!(bytes, Some(b"\x1b[1;3B".to_vec()));
    }

    #[test]
    fn shift_home() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::Home), Modifiers::SHIFT);
        assert_eq!(bytes, Some(b"\x1b[1;2H".to_vec()));
    }

    #[test]
    fn shift_end() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::End), Modifiers::SHIFT);
        assert_eq!(bytes, Some(b"\x1b[1;2F".to_vec()));
    }

    #[test]
    fn shift_delete() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::Delete), Modifiers::SHIFT);
        assert_eq!(bytes, Some(b"\x1b[3;2~".to_vec()));
    }

    #[test]
    fn ctrl_delete() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::Delete), Modifiers::CTRL);
        assert_eq!(bytes, Some(b"\x1b[3;5~".to_vec()));
    }

    #[test]
    fn shift_page_up() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::PageUp), Modifiers::SHIFT);
        assert_eq!(bytes, Some(b"\x1b[5;2~".to_vec()));
    }

    #[test]
    fn shift_f1() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::F1), Modifiers::SHIFT);
        assert_eq!(bytes, Some(b"\x1b[1;2P".to_vec()));
    }

    #[test]
    fn ctrl_f5() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::F5), Modifiers::CTRL);
        assert_eq!(bytes, Some(b"\x1b[15;5~".to_vec()));
    }

    #[test]
    fn shift_enter() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::Enter), Modifiers::SHIFT);
        assert_eq!(bytes, Some(b"\x1b[13;2u".to_vec()));
    }

    #[test]
    fn ctrl_enter() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::Enter), Modifiers::CTRL);
        assert_eq!(bytes, Some(b"\x1b[13;5u".to_vec()));
    }

    #[test]
    fn alt_enter() {
        let bytes = key_to_pty_bytes(&Key::Named(Named::Enter), Modifiers::ALT);
        assert_eq!(bytes, Some(b"\x1b[13;3u".to_vec()));
    }

    // --- Bug #668: Shift+letter should produce uppercase ---
    // key_to_pty_bytes receives the base key from Iced; the caller (app.rs)
    // now uses the `text` field instead for printable characters, but these
    // tests document the expected contract if the function is called directly.

    #[test]
    fn shift_letter_a_produces_uppercase() {
        // Bug #668: Shift+A must produce 'A' (0x41), not 'a' (0x61)
        let bytes = key_to_pty_bytes(&Key::Character("a".into()), Modifiers::SHIFT);
        assert_eq!(bytes, Some(b"A".to_vec()));
    }

    #[test]
    fn shift_letter_z_produces_uppercase() {
        let bytes = key_to_pty_bytes(&Key::Character("z".into()), Modifiers::SHIFT);
        assert_eq!(bytes, Some(b"Z".to_vec()));
    }

    #[test]
    fn shift_letter_m_produces_uppercase() {
        let bytes = key_to_pty_bytes(&Key::Character("m".into()), Modifiers::SHIFT);
        assert_eq!(bytes, Some(b"M".to_vec()));
    }
}
