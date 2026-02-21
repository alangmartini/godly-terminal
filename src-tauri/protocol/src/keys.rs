/// Map a friendly key name to its terminal byte sequence.
///
/// Supports:
/// - Control keys: `ctrl+a` through `ctrl+z`, `ctrl+[`, `ctrl+]`, `ctrl+\\`, `ctrl+^`, `ctrl+_`
/// - Navigation: `up`, `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`
/// - Editing: `enter`, `tab`, `escape`, `backspace`, `delete`, `insert`
/// - Function keys: `f1` through `f12`
/// - Signals: `ctrl+c` (SIGINT), `ctrl+d` (EOF), `ctrl+z` (SIGTSTP), `ctrl+\\` (SIGQUIT)
/// - Space: `space`
pub fn key_to_bytes(key: &str) -> Result<Vec<u8>, String> {
    let lower = key.trim().to_lowercase();

    // ctrl+<key> combinations
    if let Some(suffix) = lower.strip_prefix("ctrl+") {
        return ctrl_key(suffix);
    }

    match lower.as_str() {
        // Whitespace / editing
        "enter" | "return" | "cr" => Ok(vec![0x0D]),
        "tab" => Ok(vec![0x09]),
        "escape" | "esc" => Ok(vec![0x1B]),
        "backspace" | "bs" => Ok(vec![0x08]),
        "delete" | "del" => Ok(b"\x1b[3~".to_vec()),
        "insert" | "ins" => Ok(b"\x1b[2~".to_vec()),
        "space" => Ok(vec![0x20]),

        // Arrow keys
        "up" => Ok(b"\x1b[A".to_vec()),
        "down" => Ok(b"\x1b[B".to_vec()),
        "right" => Ok(b"\x1b[C".to_vec()),
        "left" => Ok(b"\x1b[D".to_vec()),

        // Navigation
        "home" => Ok(b"\x1b[H".to_vec()),
        "end" => Ok(b"\x1b[F".to_vec()),
        "pageup" | "pgup" => Ok(b"\x1b[5~".to_vec()),
        "pagedown" | "pgdn" => Ok(b"\x1b[6~".to_vec()),

        // Function keys (VT sequences)
        "f1" => Ok(b"\x1bOP".to_vec()),
        "f2" => Ok(b"\x1bOQ".to_vec()),
        "f3" => Ok(b"\x1bOR".to_vec()),
        "f4" => Ok(b"\x1bOS".to_vec()),
        "f5" => Ok(b"\x1b[15~".to_vec()),
        "f6" => Ok(b"\x1b[17~".to_vec()),
        "f7" => Ok(b"\x1b[18~".to_vec()),
        "f8" => Ok(b"\x1b[19~".to_vec()),
        "f9" => Ok(b"\x1b[20~".to_vec()),
        "f10" => Ok(b"\x1b[21~".to_vec()),
        "f11" => Ok(b"\x1b[23~".to_vec()),
        "f12" => Ok(b"\x1b[24~".to_vec()),

        _ => Err(format!(
            "Unknown key: '{}'. Supported: ctrl+a..z, enter, tab, escape, backspace, delete, \
             insert, space, up, down, left, right, home, end, pageup, pagedown, f1..f12",
            key
        )),
    }
}

/// Convert ctrl+<suffix> to the appropriate control byte.
fn ctrl_key(suffix: &str) -> Result<Vec<u8>, String> {
    match suffix {
        // ctrl+a (0x01) through ctrl+z (0x1A)
        "a" => Ok(vec![0x01]),
        "b" => Ok(vec![0x02]),
        "c" => Ok(vec![0x03]), // SIGINT
        "d" => Ok(vec![0x04]), // EOF
        "e" => Ok(vec![0x05]),
        "f" => Ok(vec![0x06]),
        "g" => Ok(vec![0x07]),
        "h" => Ok(vec![0x08]), // same as backspace
        "i" => Ok(vec![0x09]), // same as tab
        "j" => Ok(vec![0x0A]),
        "k" => Ok(vec![0x0B]),
        "l" => Ok(vec![0x0C]), // clear screen
        "m" => Ok(vec![0x0D]), // same as enter
        "n" => Ok(vec![0x0E]),
        "o" => Ok(vec![0x0F]),
        "p" => Ok(vec![0x10]),
        "q" => Ok(vec![0x11]),
        "r" => Ok(vec![0x12]),
        "s" => Ok(vec![0x13]),
        "t" => Ok(vec![0x14]),
        "u" => Ok(vec![0x15]),
        "v" => Ok(vec![0x16]),
        "w" => Ok(vec![0x17]),
        "x" => Ok(vec![0x18]),
        "y" => Ok(vec![0x19]),
        "z" => Ok(vec![0x1A]), // SIGTSTP

        // Other ctrl combos
        "[" => Ok(vec![0x1B]),  // same as escape
        "\\" => Ok(vec![0x1C]), // SIGQUIT
        "]" => Ok(vec![0x1D]),
        "^" => Ok(vec![0x1E]),
        "_" => Ok(vec![0x1F]),

        _ => Err(format!(
            "Unknown ctrl combination: 'ctrl+{}'. Supported: ctrl+a..z, ctrl+[, ctrl+], ctrl+\\, ctrl+^, ctrl+_",
            suffix
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_c_is_0x03() {
        assert_eq!(key_to_bytes("ctrl+c").unwrap(), vec![0x03]);
    }

    #[test]
    fn ctrl_d_is_0x04() {
        assert_eq!(key_to_bytes("ctrl+d").unwrap(), vec![0x04]);
    }

    #[test]
    fn ctrl_z_is_0x1a() {
        assert_eq!(key_to_bytes("ctrl+z").unwrap(), vec![0x1A]);
    }

    #[test]
    fn enter_is_cr() {
        assert_eq!(key_to_bytes("enter").unwrap(), vec![0x0D]);
        assert_eq!(key_to_bytes("return").unwrap(), vec![0x0D]);
        assert_eq!(key_to_bytes("cr").unwrap(), vec![0x0D]);
    }

    #[test]
    fn escape_is_0x1b() {
        assert_eq!(key_to_bytes("escape").unwrap(), vec![0x1B]);
        assert_eq!(key_to_bytes("esc").unwrap(), vec![0x1B]);
    }

    #[test]
    fn arrow_keys() {
        assert_eq!(key_to_bytes("up").unwrap(), b"\x1b[A".to_vec());
        assert_eq!(key_to_bytes("down").unwrap(), b"\x1b[B".to_vec());
        assert_eq!(key_to_bytes("right").unwrap(), b"\x1b[C".to_vec());
        assert_eq!(key_to_bytes("left").unwrap(), b"\x1b[D".to_vec());
    }

    #[test]
    fn function_keys() {
        assert_eq!(key_to_bytes("f1").unwrap(), b"\x1bOP".to_vec());
        assert_eq!(key_to_bytes("f5").unwrap(), b"\x1b[15~".to_vec());
        assert_eq!(key_to_bytes("f12").unwrap(), b"\x1b[24~".to_vec());
    }

    #[test]
    fn navigation_keys() {
        assert_eq!(key_to_bytes("home").unwrap(), b"\x1b[H".to_vec());
        assert_eq!(key_to_bytes("end").unwrap(), b"\x1b[F".to_vec());
        assert_eq!(key_to_bytes("pageup").unwrap(), b"\x1b[5~".to_vec());
        assert_eq!(key_to_bytes("pagedown").unwrap(), b"\x1b[6~".to_vec());
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(key_to_bytes("Ctrl+C").unwrap(), vec![0x03]);
        assert_eq!(key_to_bytes("ENTER").unwrap(), vec![0x0D]);
        assert_eq!(key_to_bytes("Up").unwrap(), b"\x1b[A".to_vec());
    }

    #[test]
    fn whitespace_trimmed() {
        assert_eq!(key_to_bytes("  ctrl+c  ").unwrap(), vec![0x03]);
    }

    #[test]
    fn backspace() {
        assert_eq!(key_to_bytes("backspace").unwrap(), vec![0x08]);
        assert_eq!(key_to_bytes("bs").unwrap(), vec![0x08]);
    }

    #[test]
    fn delete() {
        assert_eq!(key_to_bytes("delete").unwrap(), b"\x1b[3~".to_vec());
        assert_eq!(key_to_bytes("del").unwrap(), b"\x1b[3~".to_vec());
    }

    #[test]
    fn space() {
        assert_eq!(key_to_bytes("space").unwrap(), vec![0x20]);
    }

    #[test]
    fn unknown_key_returns_error() {
        assert!(key_to_bytes("nonexistent").is_err());
        assert!(key_to_bytes("ctrl+1").is_err());
    }

    #[test]
    fn tab() {
        assert_eq!(key_to_bytes("tab").unwrap(), vec![0x09]);
    }

    #[test]
    fn insert() {
        assert_eq!(key_to_bytes("insert").unwrap(), b"\x1b[2~".to_vec());
        assert_eq!(key_to_bytes("ins").unwrap(), b"\x1b[2~".to_vec());
    }

    #[test]
    fn ctrl_backslash_is_sigquit() {
        assert_eq!(key_to_bytes("ctrl+\\").unwrap(), vec![0x1C]);
    }
}
