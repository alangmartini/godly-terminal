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

    // Try text first.
    match clipboard.get_text() {
        Ok(text) if !text.is_empty() => return Ok(text),
        _ => {}
    }

    // No text — check for an image on the clipboard.
    match clipboard.get_image() {
        Ok(img) => {
            let path = save_clipboard_image_as_png(img.width, img.height, &img.bytes)?;
            Ok(path)
        }
        Err(_) => {
            // Neither text nor image — return empty.
            Ok(String::new())
        }
    }
}

/// Encode raw RGBA pixels as PNG and write to a temp file.
/// Returns the absolute path to the saved file.
fn save_clipboard_image_as_png(
    width: usize,
    height: usize,
    rgba: &[u8],
) -> Result<String, String> {
    let dir = std::env::temp_dir().join("godly-clipboard");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create temp dir: {}", e))?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let filename = format!("clipboard-{}.png", stamp);
    let path = dir.join(&filename);

    let file = std::fs::File::create(&path)
        .map_err(|e| format!("Failed to create temp PNG: {}", e))?;
    let writer = std::io::BufWriter::new(file);

    let mut encoder = png::Encoder::new(writer, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut png_writer = encoder
        .write_header()
        .map_err(|e| format!("PNG header write failed: {}", e))?;
    png_writer
        .write_image_data(rgba)
        .map_err(|e| format!("PNG data write failed: {}", e))?;

    let abs = path
        .canonicalize()
        .unwrap_or(path.clone())
        .to_string_lossy()
        .to_string();

    // Strip Windows UNC prefix (\\?\) so the path is clean for terminal paste.
    let clean = abs.strip_prefix(r"\\?\").unwrap_or(&abs).to_string();

    log::info!("Clipboard image saved: {}", clean);
    Ok(clean)
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
        // With the image fallback, empty text may still be empty if no image is present.
        assert!(pasted.is_empty() || pasted.ends_with(".png"));
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

    #[test]
    fn save_clipboard_image_creates_png() {
        // 2x2 red RGBA image
        let rgba: Vec<u8> = vec![
            255, 0, 0, 255, 255, 0, 0, 255,
            255, 0, 0, 255, 255, 0, 0, 255,
        ];
        let path = save_clipboard_image_as_png(2, 2, &rgba).expect("should save PNG");
        assert!(path.ends_with(".png"));
        assert!(std::path::Path::new(&path).exists());
        // Verify it's a valid PNG (starts with PNG signature).
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
        // Clean up.
        let _ = std::fs::remove_file(&path);
    }
}
