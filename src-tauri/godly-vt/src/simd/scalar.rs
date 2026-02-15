/// Scalar fallback for control character scanning.
///
/// Finds the first byte that is a control character (< 0x20) or DEL (0x7F).
/// Returns `None` if no control character is found.
#[inline]
pub fn scan_for_control(data: &[u8]) -> Option<usize> {
    for (i, &b) in data.iter().enumerate() {
        if b < 0x20 || b == 0x7F {
            return Some(i);
        }
    }
    None
}

/// Scalar fallback for ASCII detection.
///
/// Returns `true` if every byte in `data` has its high bit clear (< 0x80).
#[inline]
pub fn is_all_ascii(data: &[u8]) -> bool {
    for &b in data {
        if b >= 0x80 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_empty() {
        assert_eq!(scan_for_control(&[]), None);
    }

    #[test]
    fn scan_all_printable() {
        assert_eq!(scan_for_control(b"Hello, World!"), None);
    }

    #[test]
    fn scan_finds_esc() {
        assert_eq!(scan_for_control(b"abc\x1bdef"), Some(3));
    }

    #[test]
    fn scan_finds_null() {
        assert_eq!(scan_for_control(b"\x00abc"), Some(0));
    }

    #[test]
    fn scan_finds_del() {
        assert_eq!(scan_for_control(b"abc\x7f"), Some(3));
    }

    #[test]
    fn scan_finds_newline() {
        assert_eq!(scan_for_control(b"abc\ndef"), Some(3));
    }

    #[test]
    fn scan_high_bytes_are_not_control() {
        // Bytes 0x80-0xFF are NOT control characters for our purposes
        let data: Vec<u8> = (0x20..=0xFF).filter(|&b| b != 0x7F).collect();
        assert_eq!(scan_for_control(&data), None);
    }

    #[test]
    fn ascii_empty() {
        assert!(is_all_ascii(&[]));
    }

    #[test]
    fn ascii_pure() {
        assert!(is_all_ascii(b"Hello, World! 123"));
    }

    #[test]
    fn ascii_with_high_bit() {
        assert!(!is_all_ascii(&[0x80]));
        assert!(!is_all_ascii(b"abc\xc0"));
        assert!(!is_all_ascii(b"\xff"));
    }

    #[test]
    fn ascii_includes_control() {
        // Control chars (< 0x80) are still ASCII
        assert!(is_all_ascii(&[0x00, 0x1F, 0x7F]));
    }
}
