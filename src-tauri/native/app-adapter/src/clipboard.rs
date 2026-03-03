use arboard::Clipboard;

/// Copy text to the system clipboard.
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard =
        Clipboard::new().map_err(|e| format!("Failed to access clipboard: {}", e))?;
    clipboard
        .set_text(text)
        .map_err(|e| format!("Failed to copy: {}", e))
}

/// Paste text from the system clipboard.
pub fn paste_from_clipboard() -> Result<String, String> {
    let mut clipboard =
        Clipboard::new().map_err(|e| format!("Failed to access clipboard: {}", e))?;
    clipboard
        .get_text()
        .map_err(|e| format!("Failed to paste: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Clipboard tests require a display server / desktop session.
    // They are marked #[ignore] so they don't fail in headless CI,
    // but can be run locally with:
    //   cargo test -p godly-app-adapter -- --ignored --test-threads=1
    // (single-threaded required: concurrent Clipboard::new() races on Win32)

    #[test]
    #[ignore]
    fn round_trip_copy_paste() {
        let text = "Hello from godly-app-adapter clipboard test!";
        copy_to_clipboard(text).expect("copy should succeed");
        let pasted = paste_from_clipboard().expect("paste should succeed");
        assert_eq!(pasted, text);
    }

    #[test]
    #[ignore]
    fn empty_string_round_trip() {
        let text = "";
        copy_to_clipboard(text).expect("copy empty string should succeed");
        let pasted = paste_from_clipboard().expect("paste should succeed");
        assert_eq!(pasted, text);
    }

    #[test]
    #[ignore]
    fn unicode_round_trip() {
        let text = "こんにちは 🌍 Ñoño café résumé";
        copy_to_clipboard(text).expect("copy unicode should succeed");
        let pasted = paste_from_clipboard().expect("paste should succeed");
        assert_eq!(pasted, text);
    }

    #[test]
    #[ignore]
    fn multiline_round_trip() {
        let text = "line 1\nline 2\nline 3";
        copy_to_clipboard(text).expect("copy multiline should succeed");
        let pasted = paste_from_clipboard().expect("paste should succeed");
        assert_eq!(pasted, text);
    }
}
