// Kitty graphics protocol handler for godly-vt
//
// Clean-room implementation from the public specification:
// https://sw.kovidgoyal.net/kitty/graphics-protocol/
//
// This code was written WITHOUT referencing Kitty's GPLv3 source code.

use super::{DecodedImage, ImageStore, ImageUpload};

/// Parsed key-value control data from a Kitty graphics APC payload.
#[derive(Debug, Default, Clone)]
pub struct KittyCommand {
    /// Action: 't' (transmit), 'T' (transmit+display), 'p' (place),
    /// 'd' (delete), 'q' (query), 'f' (frame), 'a' (animation).
    pub action: char,
    /// Pixel format: 24 (RGB), 32 (RGBA), 100 (PNG).
    pub format: u32,
    /// Image ID for referencing uploads.
    pub image_id: u32,
    /// Image number (alternative to image_id).
    pub image_number: u32,
    /// Placement ID for display operations.
    pub placement_id: u32,
    /// Width in pixels (for raw data).
    pub width: u32,
    /// Height in pixels (for raw data).
    pub height: u32,
    /// Whether more data follows (chunked transfer): 0 = final, 1 = more.
    pub more_data: u32,
    /// Compression: 'z' for zlib.
    pub compression: char,
    /// Delete specifier for 'a=d'.
    pub delete_specifier: char,
    /// Columns to display (placement).
    pub columns: u32,
    /// Rows to display (placement).
    pub rows: u32,
    /// X offset in pixels within the cell.
    pub x_offset: u32,
    /// Y offset in pixels within the cell.
    pub y_offset: u32,
    /// Z-index for layering.
    pub z_index: i32,
    /// Whether the cursor should move after placement.
    pub cursor_movement: u32,
    /// Quiet mode: suppress responses.
    pub quiet: u32,
    /// The base64-encoded payload data (after the semicolon).
    pub payload: Vec<u8>,
}

/// Parse the APC payload for a Kitty graphics command.
///
/// The format is: `G<key>=<value>,<key>=<value>,...;<base64-payload>`
/// The leading 'G' is expected to already be consumed by the caller.
pub fn parse_kitty_command(data: &[u8]) -> Option<KittyCommand> {
    if data.is_empty() {
        return None;
    }

    // Split on first ';' to separate control data from payload
    let (control, payload) = match data.iter().position(|&b| b == b';') {
        Some(pos) => (&data[..pos], &data[pos + 1..]),
        None => (data, &[][..]),
    };

    let mut cmd = KittyCommand {
        action: 't', // default action is transmit
        format: 32,  // default format is RGBA
        payload: payload.to_vec(),
        ..Default::default()
    };

    // Parse key=value pairs separated by commas
    for pair in control.split(|&b| b == b',') {
        if pair.len() < 2 || pair[1] != b'=' {
            continue;
        }
        let key = pair[0];
        let value = &pair[2..];
        let value_str = std::str::from_utf8(value).unwrap_or("");

        match key {
            b'a' => {
                if let Some(c) = value.first() {
                    cmd.action = *c as char;
                }
            }
            b'f' => cmd.format = value_str.parse().unwrap_or(32),
            b'i' => cmd.image_id = value_str.parse().unwrap_or(0),
            b'I' => cmd.image_number = value_str.parse().unwrap_or(0),
            b'p' => cmd.placement_id = value_str.parse().unwrap_or(0),
            b's' => cmd.width = value_str.parse().unwrap_or(0),
            b'v' => cmd.height = value_str.parse().unwrap_or(0),
            b'm' => cmd.more_data = value_str.parse().unwrap_or(0),
            b'o' => {
                if let Some(c) = value.first() {
                    cmd.compression = *c as char;
                }
            }
            b'd' => {
                if let Some(c) = value.first() {
                    cmd.delete_specifier = *c as char;
                }
            }
            b'c' => cmd.columns = value_str.parse().unwrap_or(0),
            b'r' => cmd.rows = value_str.parse().unwrap_or(0),
            b'x' => cmd.x_offset = value_str.parse().unwrap_or(0),
            b'y' => cmd.y_offset = value_str.parse().unwrap_or(0),
            b'z' => cmd.z_index = value_str.parse().unwrap_or(0),
            b'C' => cmd.cursor_movement = value_str.parse().unwrap_or(0),
            b'q' => cmd.quiet = value_str.parse().unwrap_or(0),
            _ => {} // Unknown keys are silently ignored per spec
        }
    }

    Some(cmd)
}

/// Decode the payload of a Kitty graphics command.
///
/// Handles base64 decoding and optional zlib decompression.
#[cfg(feature = "images")]
pub fn decode_payload(cmd: &KittyCommand) -> Result<Vec<u8>, String> {
    use base64::Engine;

    if cmd.payload.is_empty() {
        return Ok(vec![]);
    }

    // Base64 decode
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&cmd.payload)
        .map_err(|e| format!("base64 decode error: {e}"))?;

    // Zlib decompress if requested
    if cmd.compression == 'z' {
        use flate2::read::ZlibDecoder;
        use std::io::Read;
        let mut decoder = ZlibDecoder::new(&decoded[..]);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| format!("zlib decompress error: {e}"))?;
        Ok(decompressed)
    } else {
        Ok(decoded)
    }
}

/// Convert raw pixel data to a DecodedImage.
///
/// Handles format conversion: f=32 (RGBA), f=24 (RGB), f=100 (PNG).
#[cfg(feature = "images")]
pub fn decode_image_data(
    data: &[u8],
    format: u32,
    width: u32,
    height: u32,
) -> Result<DecodedImage, String> {
    match format {
        32 => {
            // Raw RGBA
            let expected = (width as usize) * (height as usize) * 4;
            if data.len() != expected {
                return Err(format!(
                    "RGBA data size mismatch: expected {expected}, got {}",
                    data.len()
                ));
            }
            let content_hash = ImageStore::content_hash(data);
            Ok(DecodedImage {
                pixels: data.to_vec(),
                width,
                height,
                content_hash,
            })
        }
        24 => {
            // Raw RGB — convert to RGBA
            let expected = (width as usize) * (height as usize) * 3;
            if data.len() != expected {
                return Err(format!(
                    "RGB data size mismatch: expected {expected}, got {}",
                    data.len()
                ));
            }
            let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
            for chunk in data.chunks_exact(3) {
                rgba.extend_from_slice(chunk);
                rgba.push(255); // Full alpha
            }
            let content_hash = ImageStore::content_hash(&rgba);
            Ok(DecodedImage {
                pixels: rgba,
                width,
                height,
                content_hash,
            })
        }
        100 => {
            // PNG — decode using image crate
            decode_png(data)
        }
        _ => Err(format!("unsupported format: {format}")),
    }
}

/// Decode a PNG image to RGBA pixel data.
#[cfg(feature = "images")]
fn decode_png(data: &[u8]) -> Result<DecodedImage, String> {
    use image::ImageReader;
    use std::io::Cursor;

    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| format!("image format error: {e}"))?;

    let img = reader.decode().map_err(|e| format!("image decode error: {e}"))?;
    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let pixels = rgba.into_raw();
    let content_hash = ImageStore::content_hash(&pixels);

    Ok(DecodedImage {
        pixels,
        width,
        height,
        content_hash,
    })
}

/// Process a Kitty graphics command against the image store.
///
/// Returns placement information if the command results in image display.
#[cfg(feature = "images")]
pub fn process_kitty_command(
    store: &mut ImageStore,
    cmd: &KittyCommand,
) -> Result<Option<KittyPlacement>, String> {
    match cmd.action {
        't' => {
            // Transmit only (upload without display)
            handle_transmit(store, cmd, false)
        }
        'T' => {
            // Transmit and display
            handle_transmit(store, cmd, true)
        }
        'p' => {
            // Place (display existing upload)
            handle_place(store, cmd)
        }
        'd' => {
            // Delete
            handle_delete(store, cmd);
            Ok(None)
        }
        _ => Ok(None),
    }
}

/// Result of a Kitty placement operation.
#[cfg(feature = "images")]
#[derive(Debug)]
pub struct KittyPlacement {
    /// Content hash of the placed image.
    pub image_hash: u64,
    /// Width of the image in pixels.
    pub width: u32,
    /// Height of the image in pixels.
    pub height: u32,
    /// Placement ID.
    pub placement_id: u32,
    /// Z-index.
    pub z_index: i32,
    /// Number of columns to display.
    pub columns: u32,
    /// Number of rows to display.
    pub rows: u32,
}

#[cfg(feature = "images")]
fn handle_transmit(
    store: &mut ImageStore,
    cmd: &KittyCommand,
    display: bool,
) -> Result<Option<KittyPlacement>, String> {
    let image_id = if cmd.image_id != 0 {
        cmd.image_id
    } else {
        store.next_image_id()
    };

    if cmd.more_data == 1 {
        // Chunked transfer — start or continue upload
        let decoded = decode_payload(cmd)?;

        if store.finish_upload(image_id).is_none()
            && store
                .uploads
                .get(&image_id)
                .is_none()
        {
            // New upload
            let upload = ImageUpload {
                image_id,
                image_number: cmd.image_number,
                data: decoded,
                format: cmd.format,
                width: cmd.width,
                height: cmd.height,
                compressed: false, // Already decompressed
            };
            store.begin_upload(upload);
        } else {
            store.append_upload_data(image_id, &decoded);
        }
        return Ok(None);
    }

    // Final or non-chunked transfer
    let decoded = decode_payload(cmd)?;

    // Check if this is the final chunk of a multi-part upload
    let full_data = if let Some(mut upload) = store.finish_upload(image_id) {
        upload.data.extend_from_slice(&decoded);
        upload.data
    } else {
        decoded
    };

    if full_data.is_empty() {
        return Ok(None);
    }

    let image = decode_image_data(&full_data, cmd.format, cmd.width, cmd.height)?;

    if !ImageStore::validate_dimensions(image.width, image.height) {
        return Err("image dimensions exceed limits".to_string());
    }

    let hash = store.store_with_id(image_id, image.clone());

    if display {
        Ok(Some(KittyPlacement {
            image_hash: hash,
            width: image.width,
            height: image.height,
            placement_id: cmd.placement_id,
            z_index: cmd.z_index,
            columns: cmd.columns,
            rows: cmd.rows,
        }))
    } else {
        Ok(None)
    }
}

#[cfg(feature = "images")]
fn handle_place(
    store: &mut ImageStore,
    cmd: &KittyCommand,
) -> Result<Option<KittyPlacement>, String> {
    let hash = store
        .hash_for_id(cmd.image_id)
        .ok_or_else(|| format!("image id {} not found", cmd.image_id))?;
    let image = store
        .get(hash)
        .ok_or_else(|| format!("image hash {hash} not found"))?;

    Ok(Some(KittyPlacement {
        image_hash: hash,
        width: image.width,
        height: image.height,
        placement_id: cmd.placement_id,
        z_index: cmd.z_index,
        columns: cmd.columns,
        rows: cmd.rows,
    }))
}

#[cfg(feature = "images")]
fn handle_delete(store: &mut ImageStore, cmd: &KittyCommand) {
    match cmd.delete_specifier {
        'a' | 'A' => {
            // Delete all images
            let hashes: Vec<u64> = store.images.keys().copied().collect();
            for hash in hashes {
                store.remove(hash);
            }
        }
        'i' | 'I' => {
            // Delete by image ID
            store.remove_by_id(cmd.image_id);
        }
        _ => {
            // Default: delete by image ID
            if cmd.image_id != 0 {
                store.remove_by_id(cmd.image_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_kitty_command_basic() {
        let data = b"a=T,f=100,s=10,v=20";
        let cmd = parse_kitty_command(data).unwrap();
        assert_eq!(cmd.action, 'T');
        assert_eq!(cmd.format, 100);
        assert_eq!(cmd.width, 10);
        assert_eq!(cmd.height, 20);
    }

    #[test]
    fn test_parse_kitty_command_with_payload() {
        let data = b"a=t,f=32,s=2,v=2;AQAAAA==";
        let cmd = parse_kitty_command(data).unwrap();
        assert_eq!(cmd.action, 't');
        assert_eq!(cmd.format, 32);
        assert_eq!(cmd.payload, b"AQAAAA==");
    }

    #[test]
    fn test_parse_kitty_command_defaults() {
        let data = b"i=42";
        let cmd = parse_kitty_command(data).unwrap();
        assert_eq!(cmd.action, 't');
        assert_eq!(cmd.format, 32);
        assert_eq!(cmd.image_id, 42);
    }

    #[test]
    fn test_parse_kitty_command_delete() {
        let data = b"a=d,d=a";
        let cmd = parse_kitty_command(data).unwrap();
        assert_eq!(cmd.action, 'd');
        assert_eq!(cmd.delete_specifier, 'a');
    }

    #[test]
    fn test_parse_kitty_command_chunked() {
        let data = b"a=T,m=1;AQAA";
        let cmd = parse_kitty_command(data).unwrap();
        assert_eq!(cmd.more_data, 1);
        assert_eq!(cmd.payload, b"AQAA");
    }

    #[test]
    fn test_parse_kitty_command_z_index() {
        let data = b"z=-1";
        let cmd = parse_kitty_command(data).unwrap();
        assert_eq!(cmd.z_index, -1);
    }

    #[test]
    fn test_parse_kitty_command_empty() {
        assert!(parse_kitty_command(b"").is_none());
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_decode_payload_base64() {
        let cmd = KittyCommand {
            payload: b"AQIDBA==".to_vec(),
            ..Default::default()
        };
        let decoded = decode_payload(&cmd).unwrap();
        assert_eq!(decoded, vec![1, 2, 3, 4]);
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_decode_image_rgba() {
        // 2x2 RGBA image = 16 bytes
        let data = vec![255u8; 16];
        let img = decode_image_data(&data, 32, 2, 2).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.pixels.len(), 16);
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_decode_image_rgb_to_rgba() {
        // 2x2 RGB image = 12 bytes, should be converted to 16 bytes RGBA
        let data = vec![255u8; 12];
        let img = decode_image_data(&data, 24, 2, 2).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.pixels.len(), 16);
        // Check alpha was added
        assert_eq!(img.pixels[3], 255);
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_kitty_transmit_and_display() {
        use base64::Engine;

        let mut store = ImageStore::new(1024 * 1024);

        // Create a minimal 1x1 RGBA image
        let pixel_data = vec![255u8, 0, 0, 255]; // Red pixel
        let b64 = base64::engine::general_purpose::STANDARD.encode(&pixel_data);

        let cmd = KittyCommand {
            action: 'T',
            format: 32,
            width: 1,
            height: 1,
            image_id: 1,
            payload: b64.into_bytes(),
            ..Default::default()
        };

        let result = process_kitty_command(&mut store, &cmd).unwrap();
        assert!(result.is_some());
        let placement = result.unwrap();
        assert_eq!(placement.width, 1);
        assert_eq!(placement.height, 1);
        assert_eq!(store.image_count(), 1);
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_kitty_upload_then_place() {
        use base64::Engine;

        let mut store = ImageStore::new(1024 * 1024);

        // Upload without display
        let pixel_data = vec![0u8, 255, 0, 255]; // Green pixel
        let b64 = base64::engine::general_purpose::STANDARD.encode(&pixel_data);

        let upload_cmd = KittyCommand {
            action: 't',
            format: 32,
            width: 1,
            height: 1,
            image_id: 42,
            payload: b64.into_bytes(),
            ..Default::default()
        };

        let result = process_kitty_command(&mut store, &upload_cmd).unwrap();
        assert!(result.is_none()); // No display for 't'

        // Now place it
        let place_cmd = KittyCommand {
            action: 'p',
            image_id: 42,
            placement_id: 1,
            ..Default::default()
        };

        let result = process_kitty_command(&mut store, &place_cmd).unwrap();
        assert!(result.is_some());
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_kitty_delete() {
        use base64::Engine;

        let mut store = ImageStore::new(1024 * 1024);

        // Upload an image
        let pixel_data = vec![0u8; 4];
        let b64 = base64::engine::general_purpose::STANDARD.encode(&pixel_data);

        let cmd = KittyCommand {
            action: 't',
            format: 32,
            width: 1,
            height: 1,
            image_id: 1,
            payload: b64.into_bytes(),
            ..Default::default()
        };
        process_kitty_command(&mut store, &cmd).unwrap();
        assert_eq!(store.image_count(), 1);

        // Delete it
        let delete_cmd = KittyCommand {
            action: 'd',
            image_id: 1,
            delete_specifier: 'i',
            ..Default::default()
        };
        process_kitty_command(&mut store, &delete_cmd).unwrap();
        assert_eq!(store.image_count(), 0);
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_kitty_chunked_transfer() {
        use base64::Engine;

        let mut store = ImageStore::new(1024 * 1024);

        // 1x1 RGBA pixel split into two chunks
        let pixel_data = vec![255u8, 128, 64, 255];
        let b64 = base64::engine::general_purpose::STANDARD.encode(&pixel_data);
        let b64_bytes = b64.as_bytes();
        let mid = b64_bytes.len() / 2;

        // First chunk
        let cmd1 = KittyCommand {
            action: 'T',
            format: 32,
            width: 1,
            height: 1,
            image_id: 5,
            more_data: 1,
            payload: b64_bytes[..mid].to_vec(),
            ..Default::default()
        };
        let result = process_kitty_command(&mut store, &cmd1).unwrap();
        assert!(result.is_none()); // Not done yet

        // Final chunk
        let cmd2 = KittyCommand {
            action: 'T',
            format: 32,
            width: 1,
            height: 1,
            image_id: 5,
            more_data: 0,
            payload: b64_bytes[mid..].to_vec(),
            ..Default::default()
        };
        let result = process_kitty_command(&mut store, &cmd2).unwrap();
        assert!(result.is_some());
        assert_eq!(store.image_count(), 1);
    }
}
