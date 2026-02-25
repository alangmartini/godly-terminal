/// Parse a hex color string like "#cd3131" into normalized [f32; 4] RGBA.
/// Returns None if the string is not a valid 6-digit hex color with '#' prefix.
pub fn parse_hex_color(hex: &str) -> Option<[f32; 4]> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some([
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        1.0,
    ])
}

/// Resolve foreground and background colors for a cell, applying defaults and inverse.
///
/// Colors from RichGridData are either hex strings like "#cd3131" or "default".
/// When a color is "default", the theme default is used.
/// When `inverse` is true, foreground and background are swapped after resolution.
pub fn resolve_cell_colors(
    fg: &str,
    bg: &str,
    inverse: bool,
    default_fg: [f32; 4],
    default_bg: [f32; 4],
) -> ([f32; 4], [f32; 4]) {
    let fg_color = if fg == "default" {
        default_fg
    } else {
        parse_hex_color(fg).unwrap_or(default_fg)
    };

    let bg_color = if bg == "default" {
        default_bg
    } else {
        parse_hex_color(bg).unwrap_or(default_bg)
    };

    if inverse {
        (bg_color, fg_color)
    } else {
        (fg_color, bg_color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_red() {
        let c = parse_hex_color("#ff0000").unwrap();
        assert!((c[0] - 1.0).abs() < 0.01);
        assert!(c[1].abs() < 0.01);
        assert!(c[2].abs() < 0.01);
        assert!((c[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn parse_green() {
        let c = parse_hex_color("#00ff00").unwrap();
        assert!(c[0].abs() < 0.01);
        assert!((c[1] - 1.0).abs() < 0.01);
        assert!(c[2].abs() < 0.01);
    }

    #[test]
    fn parse_blue() {
        let c = parse_hex_color("#0000ff").unwrap();
        assert!(c[0].abs() < 0.01);
        assert!(c[1].abs() < 0.01);
        assert!((c[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn parse_mixed_color() {
        let c = parse_hex_color("#cd3131").unwrap();
        assert!((c[0] - 0xcd as f32 / 255.0).abs() < 0.01);
        assert!((c[1] - 0x31 as f32 / 255.0).abs() < 0.01);
        assert!((c[2] - 0x31 as f32 / 255.0).abs() < 0.01);
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert!(parse_hex_color("default").is_none());
        assert!(parse_hex_color("").is_none());
        assert!(parse_hex_color("#fff").is_none());
        assert!(parse_hex_color("#zzzzzz").is_none());
        assert!(parse_hex_color("ff0000").is_none());
    }

    #[test]
    fn resolve_defaults() {
        let dfg = [1.0, 1.0, 1.0, 1.0];
        let dbg = [0.0, 0.0, 0.0, 1.0];
        let (fg, bg) = resolve_cell_colors("default", "default", false, dfg, dbg);
        assert_eq!(fg, dfg);
        assert_eq!(bg, dbg);
    }

    #[test]
    fn resolve_explicit_colors() {
        let dfg = [1.0, 1.0, 1.0, 1.0];
        let dbg = [0.0, 0.0, 0.0, 1.0];
        let (fg, bg) = resolve_cell_colors("#ff0000", "#00ff00", false, dfg, dbg);
        assert!((fg[0] - 1.0).abs() < 0.01);
        assert!((bg[1] - 1.0).abs() < 0.01);
    }

    #[test]
    fn resolve_inverse_swaps() {
        let dfg = [1.0, 1.0, 1.0, 1.0];
        let dbg = [0.0, 0.0, 0.0, 1.0];
        let (fg, bg) = resolve_cell_colors("default", "default", true, dfg, dbg);
        // Inverse: fg gets default_bg, bg gets default_fg
        assert_eq!(fg, dbg);
        assert_eq!(bg, dfg);
    }

    #[test]
    fn resolve_invalid_hex_uses_default() {
        let dfg = [0.8, 0.8, 0.8, 1.0];
        let dbg = [0.1, 0.1, 0.1, 1.0];
        let (fg, bg) = resolve_cell_colors("not-a-color", "also-bad", false, dfg, dbg);
        assert_eq!(fg, dfg);
        assert_eq!(bg, dbg);
    }
}
