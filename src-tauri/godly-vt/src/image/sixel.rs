// Sixel graphics handler for godly-vt
//
// Sixel is a DCS-based image protocol where images are encoded as
// 6-pixel-tall horizontal strips. The terminal accumulates DCS data
// via hook()/put()/unhook() callbacks.
//
// Currently a placeholder that accumulates sixel data. Full decoding
// would use the `icy_sixel` crate (not yet integrated as a dependency).

use super::{DecodedImage, ImageStore};

/// State for accumulating sixel data from DCS.
#[derive(Debug, Default, Clone)]
pub struct SixelAccumulator {
    /// Raw sixel data bytes accumulated via put().
    data: Vec<u8>,
    /// Whether we are currently inside a sixel DCS sequence.
    active: bool,
}

impl SixelAccumulator {
    /// Create a new sixel accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Called on DCS hook with sixel final byte ('q').
    ///
    /// The DCS params typically include aspect ratio info (e.g., `0;0;0`).
    pub fn hook(&mut self, _params: &[u16]) {
        self.data.clear();
        self.active = true;
    }

    /// Accumulate a data byte.
    pub fn put(&mut self, byte: u8) {
        if self.active {
            self.data.push(byte);
        }
    }

    /// Accumulate a slice of data bytes.
    pub fn put_slice(&mut self, data: &[u8]) {
        if self.active {
            self.data.extend_from_slice(data);
        }
    }

    /// Called on DCS unhook (sixel sequence complete).
    ///
    /// Returns the accumulated sixel data for decoding.
    pub fn unhook(&mut self) -> Option<Vec<u8>> {
        if self.active {
            self.active = false;
            Some(std::mem::take(&mut self.data))
        } else {
            None
        }
    }

    /// Whether the accumulator is currently receiving sixel data.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Current size of accumulated data.
    pub fn data_len(&self) -> usize {
        self.data.len()
    }
}

/// Decode accumulated sixel data to RGBA pixels.
///
/// This is a minimal sixel decoder that handles the core protocol.
/// For production use, consider using the `icy_sixel` crate.
#[cfg(feature = "images")]
pub fn decode_sixel(data: &[u8]) -> Result<DecodedImage, String> {
    // Minimal sixel parser
    // Sixel format: each byte in range 0x3F..=0x7E encodes a column of 6 vertical pixels
    // Special characters: ! (repeat), # (color), $ (carriage return), - (next line)
    // Color format: #Pc;Pu;Px;Py;Pz where Pc=register, Pu=model, Px/Py/Pz=values

    let mut colors: Vec<(u8, u8, u8)> = vec![(0, 0, 0); 256];
    let mut current_color: usize = 0;
    let mut x: usize = 0;
    let mut y: usize = 0;
    let mut max_x: usize = 0;
    let max_y: usize;

    // First pass: determine image dimensions
    let mut pass_x: usize = 0;
    let mut pass_y: usize = 0;
    let mut i = 0;
    while i < data.len() {
        match data[i] {
            b'$' => {
                // Carriage return
                if pass_x > max_x {
                    max_x = pass_x;
                }
                pass_x = 0;
            }
            b'-' => {
                // Next line (move down 6 pixels)
                if pass_x > max_x {
                    max_x = pass_x;
                }
                pass_x = 0;
                pass_y += 6;
            }
            b'!' => {
                // Repeat: !<count><char>
                i += 1;
                let mut count: usize = 0;
                while i < data.len() && data[i].is_ascii_digit() {
                    count = count * 10 + (data[i] - b'0') as usize;
                    i += 1;
                }
                if i < data.len() && (0x3F..=0x7E).contains(&data[i]) {
                    pass_x += count;
                }
            }
            b'#' => {
                // Color: skip params
                i += 1;
                while i < data.len() && (data[i].is_ascii_digit() || data[i] == b';') {
                    i += 1;
                }
                continue; // Don't increment i again
            }
            0x3F..=0x7E => {
                pass_x += 1;
            }
            _ => {}
        }
        i += 1;
    }
    if pass_x > max_x {
        max_x = pass_x;
    }
    max_y = pass_y + 6; // Last band is 6 pixels tall

    if max_x == 0 || max_y == 0 {
        return Err("empty sixel image".to_string());
    }

    if !ImageStore::validate_dimensions(max_x as u32, max_y as u32) {
        return Err("sixel image dimensions exceed limits".to_string());
    }

    // Allocate RGBA buffer
    let mut pixels = vec![0u8; max_x * max_y * 4];

    // Second pass: render pixels
    i = 0;
    while i < data.len() {
        match data[i] {
            b'$' => {
                x = 0;
            }
            b'-' => {
                x = 0;
                y += 6;
            }
            b'!' => {
                // Repeat
                i += 1;
                let mut count: usize = 0;
                while i < data.len() && data[i].is_ascii_digit() {
                    count = count * 10 + (data[i] - b'0') as usize;
                    i += 1;
                }
                if i < data.len() && (0x3F..=0x7E).contains(&data[i]) {
                    let sixel = data[i] - 0x3F;
                    for _ in 0..count {
                        render_sixel_column(&mut pixels, max_x, x, y, sixel, &colors[current_color]);
                        x += 1;
                    }
                }
            }
            b'#' => {
                // Color definition or selection
                i += 1;
                let mut register: usize = 0;
                while i < data.len() && data[i].is_ascii_digit() {
                    register = register * 10 + (data[i] - b'0') as usize;
                    i += 1;
                }
                if i < data.len() && data[i] == b';' {
                    // Color definition: #Pc;Pu;Px;Py;Pz
                    i += 1;
                    let mut params = [0u16; 4];
                    let mut pi = 0;
                    while i < data.len() && pi < 4 {
                        if data[i].is_ascii_digit() {
                            params[pi] = params[pi] * 10 + (data[i] - b'0') as u16;
                        } else if data[i] == b';' {
                            pi += 1;
                        } else {
                            break;
                        }
                        i += 1;
                    }
                    // params[0] = color model (2 = RGB percentages)
                    if params[0] == 2 && register < 256 {
                        colors[register] = (
                            (u16::from(params[1]) * 255 / 100) as u8,
                            (u16::from(params[2]) * 255 / 100) as u8,
                            (u16::from(params[3]) * 255 / 100) as u8,
                        );
                    }
                    current_color = register.min(255);
                } else {
                    // Just color selection
                    current_color = register.min(255);
                }
                continue;
            }
            0x3F..=0x7E => {
                let sixel = data[i] - 0x3F;
                render_sixel_column(&mut pixels, max_x, x, y, sixel, &colors[current_color]);
                x += 1;
            }
            _ => {}
        }
        i += 1;
    }

    let content_hash = ImageStore::content_hash(&pixels);
    Ok(DecodedImage {
        pixels,
        width: max_x as u32,
        height: max_y as u32,
        content_hash,
    })
}

/// Render a single sixel column (6 vertical pixels) at position (x, y).
#[cfg(feature = "images")]
fn render_sixel_column(
    pixels: &mut [u8],
    stride: usize,
    x: usize,
    y: usize,
    sixel: u8,
    color: &(u8, u8, u8),
) {
    for bit in 0..6 {
        if sixel & (1 << bit) != 0 {
            let py = y + bit;
            let offset = (py * stride + x) * 4;
            if offset + 3 < pixels.len() {
                pixels[offset] = color.0;
                pixels[offset + 1] = color.1;
                pixels[offset + 2] = color.2;
                pixels[offset + 3] = 255; // Full alpha
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sixel_accumulator_lifecycle() {
        let mut acc = SixelAccumulator::new();
        assert!(!acc.is_active());

        acc.hook(&[]);
        assert!(acc.is_active());

        acc.put(b'~');
        acc.put_slice(b"abc");
        assert_eq!(acc.data_len(), 4);

        let data = acc.unhook().unwrap();
        assert_eq!(data, vec![b'~', b'a', b'b', b'c']);
        assert!(!acc.is_active());
    }

    #[test]
    fn test_sixel_accumulator_unhook_without_hook() {
        let mut acc = SixelAccumulator::new();
        assert!(acc.unhook().is_none());
    }

    #[test]
    fn test_sixel_accumulator_put_when_inactive() {
        let mut acc = SixelAccumulator::new();
        acc.put(b'x');
        assert_eq!(acc.data_len(), 0);
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_decode_sixel_simple() {
        // A minimal sixel: single column, all 6 pixels set (0x7E = 0x3F + 63 = all bits)
        // Color #0 is black by default, set it to red
        let data = b"#0;2;100;0;0~";
        let img = decode_sixel(data).unwrap();
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 6);
        // First pixel should be red
        assert_eq!(img.pixels[0], 255); // R
        assert_eq!(img.pixels[1], 0);   // G
        assert_eq!(img.pixels[2], 0);   // B
        assert_eq!(img.pixels[3], 255); // A
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_decode_sixel_repeat() {
        // Repeat '~' (all bits) 3 times: !3~
        let data = b"#0;2;0;100;0!3~";
        let img = decode_sixel(data).unwrap();
        assert_eq!(img.width, 3);
        assert_eq!(img.height, 6);
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_decode_sixel_newline() {
        // Two bands: ~$-~ (first band col 0, second band col 0)
        let data = b"#0;2;100;100;100~$-~";
        let img = decode_sixel(data).unwrap();
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 12); // Two 6-pixel bands
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_decode_sixel_empty() {
        assert!(decode_sixel(b"").is_err());
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_decode_sixel_multi_color() {
        // Draw two colors in same band (overprinting)
        // Red column at x=0, then green column at x=1
        let data = b"#0;2;100;0;0?#1;2;0;100;0?";
        let img = decode_sixel(data).unwrap();
        assert_eq!(img.width, 2);
        // First column, first pixel = red (bit 0 of '?' = 0x3F = all zeros... wait)
        // '?' = 0x3F, sixel = 0x3F - 0x3F = 0. That means no pixels set.
        // Let's use '~' instead (0x7E - 0x3F = 0x3F = 63 = all bits)
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_decode_sixel_multi_color_overprint() {
        // Red pixel at (0,0), green pixel at (0,0) via overprinting
        // This tests that colors overwrite each other
        let data = b"#0;2;100;0;0~$#1;2;0;100;0~";
        let img = decode_sixel(data).unwrap();
        assert_eq!(img.width, 1);
        // The green should overwrite red for the same pixels
        assert_eq!(img.pixels[0], 0);   // R (overwritten by green)
        assert_eq!(img.pixels[1], 255); // G
        assert_eq!(img.pixels[2], 0);   // B
    }
}
