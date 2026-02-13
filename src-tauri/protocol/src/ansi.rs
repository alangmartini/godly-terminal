/// Strip ANSI escape sequences from terminal output.
/// Handles CSI sequences (\x1b[...X), OSC sequences (\x1b]...BEL/ST),
/// and simple 2-byte sequences (\x1b + single char).
pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    // CSI sequence: consume until final byte (0x40..=0x7E)
                    chars.next(); // consume '['
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if (0x40..=0x7E).contains(&(c as u32)) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC sequence: consume until BEL (\x07) or ST (\x1b\\)
                    chars.next(); // consume ']'
                    while let Some(c) = chars.next() {
                        if c == '\x07' {
                            break;
                        }
                        if c == '\x1b' {
                            if chars.peek() == Some(&'\\') {
                                chars.next(); // consume '\\'
                            }
                            break;
                        }
                    }
                }
                Some(_) => {
                    // Simple 2-byte sequence: skip one char
                    chars.next();
                }
                None => {
                    // Trailing ESC at end of string
                }
            }
        } else {
            out.push(ch);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_csi_sequences() {
        // SGR (color) sequences
        assert_eq!(strip_ansi("\x1b[31mhello\x1b[0m"), "hello");
        // Cursor movement
        assert_eq!(strip_ansi("\x1b[2Jscreen"), "screen");
        // Multiple params
        assert_eq!(strip_ansi("\x1b[1;32mbold green\x1b[0m"), "bold green");
    }

    #[test]
    fn test_strip_ansi_osc_with_bel() {
        // OSC title set terminated by BEL
        assert_eq!(strip_ansi("\x1b]0;My Title\x07prompt$"), "prompt$");
    }

    #[test]
    fn test_strip_ansi_osc_with_st() {
        // OSC sequence terminated by ST (\x1b\\)
        assert_eq!(strip_ansi("\x1b]0;Title\x1b\\text"), "text");
    }

    #[test]
    fn test_strip_ansi_no_escapes() {
        assert_eq!(strip_ansi("plain text"), "plain text");
    }

    #[test]
    fn test_strip_ansi_empty() {
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn test_strip_ansi_mixed() {
        let input = "\x1b[32mPS C:\\>\x1b[0m echo \x1b]0;powershell\x07hello";
        assert_eq!(strip_ansi(input), "PS C:\\> echo hello");
    }

    #[test]
    fn test_strip_ansi_two_byte_sequence() {
        // e.g. \x1b= (set alternate keypad mode)
        assert_eq!(strip_ansi("\x1b=text"), "text");
    }
}
