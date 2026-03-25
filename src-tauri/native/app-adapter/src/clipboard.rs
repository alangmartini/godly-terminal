use arboard::Clipboard;

#[cfg(target_os = "windows")]
use {
    winapi::um::winuser::{OpenClipboard, GetClipboardData},
    winapi::um::shellapi::DragQueryFileW,
    winapi::um::winuser::CF_HDROP,
};

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

/// Check clipboard for an image. Returns the saved PNG path if found.
/// Does not consume text clipboard content — only looks for image data.
pub fn check_clipboard_image() -> Result<Option<String>, String> {
    let mut clipboard =
        Clipboard::new().map_err(|e| format!("Failed to access clipboard: {}", e))?;
    match clipboard.get_image() {
        Ok(img) => {
            let path = save_clipboard_image_as_png(img.width, img.height, &img.bytes)?;
            Ok(Some(path))
        }
        Err(e) => {
            log::debug!("No bitmap image found on clipboard: {}", e);
            Ok(None)
        }
    }
}

/// Check clipboard for image files (CF_HDROP format).
/// Windows File Explorer puts file paths on the clipboard, not bitmap data.
/// Returns a list of image file paths found.
#[cfg(target_os = "windows")]
pub fn check_clipboard_image_files() -> Result<Vec<String>, String> {
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err("Failed to open clipboard".to_string());
        }

        let handle = GetClipboardData(CF_HDROP as u32);
        let _guard = ClipboardGuard;

        if handle.is_null() {
            log::debug!("No CF_HDROP data on clipboard");
            return Ok(Vec::new());
        }

        let hdrop = handle as winapi::um::shellapi::HDROP;
        let count = DragQueryFileW(hdrop, 0xFFFFFFFF, std::ptr::null_mut(), 0);

        let mut paths = Vec::new();
        for i in 0..count {
            let len = DragQueryFileW(hdrop, i, std::ptr::null_mut(), 0) + 1;
            let mut buf = vec![0u16; len as usize];
            DragQueryFileW(hdrop, i, buf.as_mut_ptr(), len);

            // Remove null terminator for conversion.
            buf.pop();
            let path = String::from_utf16(&buf)
                .unwrap_or_else(|_| String::new());

            // Check if it's an image file by extension.
            if is_image_file(&path) {
                paths.push(path);
            }
        }

        if !paths.is_empty() {
            log::info!("Found {} image file(s) on clipboard", paths.len());
        } else {
            log::debug!("No image files found in CF_HDROP clipboard data");
        }

        Ok(paths)
    }
}

#[cfg(target_os = "windows")]
struct ClipboardGuard;

#[cfg(target_os = "windows")]
impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = winapi::um::winuser::CloseClipboard();
        }
    }
}

/// Check if a file path is an image file based on extension.
fn is_image_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    matches!(
        lower.split('.').last().unwrap_or(""),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "tiff" | "ico"
    )
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

    // ── Bug #732 reproduction ──────────────────────────────────────────
    //
    // Quick Claude image paste fails when clipboard has ONLY image data
    // (no text). The app.rs paste flow has two detection paths:
    //
    //   Path A — keyboard::listen() Ctrl+V handler (app.rs:1904-1908):
    //     Dead when text_editor has focus because Iced marks the event
    //     as Status::Captured; keyboard::listen() only sees Ignored.
    //
    //   Path B — QuickClaudeDialogPromptAction handler (app.rs:2622-2666):
    //     Checks `is_paste` which is true only when text_editor emits
    //     Edit::Paste. Iced's text_editor only emits Edit::Paste when
    //     clipboard.read(Kind::Standard) returns Some(text).
    //
    // Result: image-only clipboard → no text → no Edit::Paste → no
    // image detection. Both paths are dead for the most common
    // screenshot paste scenario.

    /// Bug #732: Image-only clipboard paste is detectable by our
    /// clipboard module, but the text-based paste trigger that app.rs
    /// relies on does not fire — proving the detection gap.
    ///
    /// This test puts image-only data on the clipboard and asserts the
    /// contract that SHOULD hold: a paste operation should trigger image
    /// detection regardless of whether clipboard also has text.
    /// It FAILS on current code because the paste trigger requires text.
    #[test]
    #[ignore]
    fn image_only_clipboard_paste_detection_gap() {
        // 1. Put image-only data on the clipboard (simulate a screenshot).
        let mut clipboard = Clipboard::new().expect("clipboard access");
        clipboard.clear().expect("clear clipboard");

        let img = arboard::ImageData {
            width: 2,
            height: 2,
            bytes: std::borrow::Cow::Owned(vec![
                255, 0, 0, 255, 0, 255, 0, 255,
                0, 0, 255, 255, 255, 255, 0, 255,
            ]),
        };
        clipboard.set_image(img).expect("set image on clipboard");

        // 2. Confirm check_clipboard_image() CAN find the image.
        //    The detection function itself works fine.
        let detected = check_clipboard_image().expect("detection should not error");
        assert!(
            detected.is_some(),
            "check_clipboard_image() should find the image — detection works"
        );
        // Clean up temp file.
        if let Some(ref path) = detected {
            let _ = std::fs::remove_file(path);
        }

        // 3. Simulate what Iced's text_editor does on Ctrl+V:
        //    It calls clipboard.read(Kind::Standard) → arboard get_text().
        //    If text is present → Edit::Paste(text) is emitted.
        //    If text is absent  → NO action is emitted (event still Captured).
        let text_result = clipboard.get_text();
        let iced_would_emit_paste = text_result
            .as_ref()
            .map(|t| !t.is_empty())
            .unwrap_or(false);

        // 4. In app.rs, image detection only runs when is_paste is true,
        //    which requires Edit::Paste to have been emitted.
        //    Bug #732: is_paste is the ONLY trigger for image detection
        //    when text_editor has focus. No paste action = no image check.
        let image_check_would_run = iced_would_emit_paste;

        // EXPECTED: image detection triggers on paste regardless of text.
        // ACTUAL (buggy): image detection requires text to be present.
        assert!(
            image_check_would_run,
            "Bug #732: Image-only clipboard (no text) does not trigger image \
             detection in Quick Claude. Iced text_editor requires text for \
             Edit::Paste, but clipboard has only image data. Neither the \
             keyboard listener (captured by widget) nor the prompt action \
             handler (no Paste action) reaches check_clipboard_for_quick_claude_image()."
        );
    }

    /// Companion to image_only_clipboard_paste_detection_gap: confirms
    /// that text-based clipboard paste DOES trigger the image detection
    /// path (the working case from PR #729).
    #[test]
    #[ignore]
    fn text_plus_image_clipboard_paste_detection_works() {
        let mut clipboard = Clipboard::new().expect("clipboard access");
        clipboard.clear().expect("clear clipboard");

        // Set text on clipboard (simulates copy from a web page with
        // both text and image content).
        clipboard
            .set_text("hello world")
            .expect("set text on clipboard");

        // Iced text_editor will call clipboard.read() → finds text →
        // emits Edit::Paste("hello world") → app.rs detects is_paste →
        // calls check_clipboard_for_quick_claude_image().
        let text_result = clipboard.get_text();
        let iced_would_emit_paste = text_result
            .as_ref()
            .map(|t| !t.is_empty())
            .unwrap_or(false);

        assert!(
            iced_would_emit_paste,
            "Text clipboard should trigger Iced text_editor Edit::Paste — \
             this is the working path that PR #729 fixed"
        );
    }
}
