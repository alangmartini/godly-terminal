// iTerm2 inline image protocol handler for godly-vt
//
// Clean-room implementation from the public specification:
// https://iterm2.com/documentation-images.html
//
// This code was written WITHOUT referencing iTerm2's GPLv2 source code.
//
// Protocol: OSC 1337 ; File=[params] : <base64-data> BEL
// Parameters are semicolon-separated key=value pairs.

use super::{DecodedImage, ImageStore};

/// Parsed parameters from an iTerm2 File= OSC sequence.
#[derive(Debug, Default, Clone)]
pub struct Iterm2ImageParams {
    /// Whether to display inline (must be "1" for inline display).
    pub inline: bool,
    /// File name (optional, informational).
    pub name: Option<String>,
    /// File size in bytes (optional, for progress indication).
    pub size: Option<usize>,
    /// Desired width (e.g., "auto", "10", "50%", "100px").
    pub width: Option<String>,
    /// Desired height (e.g., "auto", "10", "50%", "100px").
    pub height: Option<String>,
    /// Whether to preserve aspect ratio (default true).
    pub preserve_aspect_ratio: bool,
    /// The base64-encoded image data.
    pub data: Vec<u8>,
}

/// Parse an iTerm2 File= OSC sequence.
///
/// The OSC 1337 params come as multiple OSC params. The first is "1337",
/// the second starts with "File=". We parse the key=value pairs and extract
/// the base64 payload after the colon separator.
pub fn parse_iterm2_params(osc_params: &[&[u8]]) -> Option<Iterm2ImageParams> {
    // OSC 1337 ; File=<params>:<base64-data>
    // osc_params[0] = b"1337"
    // osc_params[1] = b"File=inline=1;width=auto:base64data..."
    if osc_params.len() < 2 {
        return None;
    }

    if osc_params[0] != b"1337" {
        return None;
    }

    let file_param = osc_params[1];
    if !file_param.starts_with(b"File=") {
        return None;
    }

    // Split on ':' to separate params from data
    let after_file = &file_param[5..]; // Skip "File="
    let colon_pos = after_file.iter().position(|&b| b == b':')?;
    let params_bytes = &after_file[..colon_pos];
    let data_bytes = &after_file[colon_pos + 1..];

    let mut result = Iterm2ImageParams {
        preserve_aspect_ratio: true, // default
        data: data_bytes.to_vec(),
        ..Default::default()
    };

    // Parse semicolon-separated key=value pairs within the File= section
    let params_str = std::str::from_utf8(params_bytes).ok()?;
    for pair in params_str.split(';') {
        if let Some((key, value)) = pair.split_once('=') {
            match key {
                "inline" => result.inline = value == "1",
                "name" => {
                    // Name is base64-encoded
                    #[cfg(feature = "images")]
                    {
                        use base64::Engine;
                        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(value) {
                            result.name = String::from_utf8(decoded).ok();
                        }
                    }
                    #[cfg(not(feature = "images"))]
                    {
                        result.name = Some(value.to_string());
                    }
                }
                "size" => result.size = value.parse().ok(),
                "width" => result.width = Some(value.to_string()),
                "height" => result.height = Some(value.to_string()),
                "preserveAspectRatio" => {
                    result.preserve_aspect_ratio = value != "0";
                }
                _ => {} // Unknown params ignored
            }
        }
    }

    Some(result)
}

/// Decode an iTerm2 inline image to RGBA pixel data.
///
/// The image data is base64-encoded and can be in any format supported
/// by the `image` crate (PNG, JPEG, GIF, etc.).
#[cfg(feature = "images")]
pub fn decode_iterm2_image(params: &Iterm2ImageParams) -> Result<DecodedImage, String> {
    use base64::Engine;
    use image::ImageReader;
    use std::io::Cursor;

    if !params.inline {
        return Err("not an inline image".to_string());
    }

    if params.data.is_empty() {
        return Err("empty image data".to_string());
    }

    // Base64 decode
    let raw = base64::engine::general_purpose::STANDARD
        .decode(&params.data)
        .map_err(|e| format!("base64 decode error: {e}"))?;

    // Decode image format
    let reader = ImageReader::new(Cursor::new(&raw))
        .with_guessed_format()
        .map_err(|e| format!("image format error: {e}"))?;

    let img = reader
        .decode()
        .map_err(|e| format!("image decode error: {e}"))?;

    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();

    if !ImageStore::validate_dimensions(width, height) {
        return Err("image dimensions exceed limits".to_string());
    }

    let pixels = rgba.into_raw();
    let content_hash = ImageStore::content_hash(&pixels);

    Ok(DecodedImage {
        pixels,
        width,
        height,
        content_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_iterm2_basic() {
        let params: Vec<&[u8]> = vec![
            b"1337",
            b"File=inline=1:AQAAAA==",
        ];
        let result = parse_iterm2_params(&params).unwrap();
        assert!(result.inline);
        assert_eq!(result.data, b"AQAAAA==");
    }

    #[test]
    fn test_parse_iterm2_with_size() {
        let params: Vec<&[u8]> = vec![
            b"1337",
            b"File=inline=1;size=1234;width=auto;height=auto:AQAAAA==",
        ];
        let result = parse_iterm2_params(&params).unwrap();
        assert!(result.inline);
        assert_eq!(result.size, Some(1234));
        assert_eq!(result.width.as_deref(), Some("auto"));
        assert_eq!(result.height.as_deref(), Some("auto"));
    }

    #[test]
    fn test_parse_iterm2_not_inline() {
        let params: Vec<&[u8]> = vec![
            b"1337",
            b"File=inline=0:AQAAAA==",
        ];
        let result = parse_iterm2_params(&params).unwrap();
        assert!(!result.inline);
    }

    #[test]
    fn test_parse_iterm2_wrong_osc() {
        let params: Vec<&[u8]> = vec![b"999", b"File=inline=1:data"];
        assert!(parse_iterm2_params(&params).is_none());
    }

    #[test]
    fn test_parse_iterm2_no_file_prefix() {
        let params: Vec<&[u8]> = vec![b"1337", b"NotFile=data"];
        assert!(parse_iterm2_params(&params).is_none());
    }

    #[test]
    fn test_parse_iterm2_preserve_aspect_ratio() {
        let params: Vec<&[u8]> = vec![
            b"1337",
            b"File=inline=1;preserveAspectRatio=0:data",
        ];
        let result = parse_iterm2_params(&params).unwrap();
        assert!(!result.preserve_aspect_ratio);
    }

    #[test]
    fn test_parse_iterm2_empty_params() {
        let params: Vec<&[u8]> = vec![];
        assert!(parse_iterm2_params(&params).is_none());
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_decode_iterm2_requires_inline() {
        let params = Iterm2ImageParams {
            inline: false,
            data: b"data".to_vec(),
            ..Default::default()
        };
        assert!(decode_iterm2_image(&params).is_err());
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_decode_iterm2_empty_data() {
        let params = Iterm2ImageParams {
            inline: true,
            data: vec![],
            ..Default::default()
        };
        assert!(decode_iterm2_image(&params).is_err());
    }
}
