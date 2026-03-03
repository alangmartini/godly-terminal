use iced::Color;

/// Parse a color string from RichGridData into an Iced Color.
///
/// Handles:
/// - `"#rrggbb"` hex colors
/// - `"default"` → returns the provided default color
/// - Empty string → returns default
pub fn parse_color(s: &str, default: Color) -> Color {
    if s.is_empty() || s == "default" {
        return default;
    }

    if s.starts_with('#') && s.len() == 7 {
        let r = u8::from_str_radix(&s[1..3], 16).unwrap_or(0);
        let g = u8::from_str_radix(&s[3..5], 16).unwrap_or(0);
        let b = u8::from_str_radix(&s[5..7], 16).unwrap_or(0);
        return Color::from_rgb8(r, g, b);
    }

    default
}

/// Apply dim attribute: reduce brightness by 50%.
pub fn dim_color(color: Color) -> Color {
    Color::from_rgba(color.r * 0.5, color.g * 0.5, color.b * 0.5, color.a)
}

/// Brighten a color by 20% (clamped to 1.0).
///
/// Used for bold text rendering — most terminal emulators display bold text
/// with slightly brighter foreground colors.
pub fn brighten_color(color: Color) -> Color {
    Color {
        r: (color.r * 1.2).min(1.0),
        g: (color.g * 1.2).min(1.0),
        b: (color.b * 1.2).min(1.0),
        a: color.a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_color() {
        let c = parse_color("#ff0000", Color::WHITE);
        assert!((c.r - 1.0).abs() < 0.01);
        assert!(c.g.abs() < 0.01);
        assert!(c.b.abs() < 0.01);
    }

    #[test]
    fn parse_hex_color_blue() {
        let c = parse_color("#0000ff", Color::WHITE);
        assert!(c.r.abs() < 0.01);
        assert!(c.g.abs() < 0.01);
        assert!((c.b - 1.0).abs() < 0.01);
    }

    #[test]
    fn parse_default_returns_default() {
        let c = parse_color("default", Color::from_rgb(0.1, 0.2, 0.3));
        assert!((c.r - 0.1).abs() < 0.01);
        assert!((c.g - 0.2).abs() < 0.01);
        assert!((c.b - 0.3).abs() < 0.01);
    }

    #[test]
    fn parse_empty_returns_default() {
        let c = parse_color("", Color::WHITE);
        assert!((c.r - 1.0).abs() < 0.01);
    }

    #[test]
    fn dim_halves_brightness() {
        let c = dim_color(Color::from_rgb(1.0, 0.8, 0.6));
        assert!((c.r - 0.5).abs() < 0.01);
        assert!((c.g - 0.4).abs() < 0.01);
        assert!((c.b - 0.3).abs() < 0.01);
    }

    #[test]
    fn brighten_increases_by_20_percent() {
        let c = brighten_color(Color::from_rgb(0.5, 0.4, 0.3));
        assert!((c.r - 0.6).abs() < 0.01);
        assert!((c.g - 0.48).abs() < 0.01);
        assert!((c.b - 0.36).abs() < 0.01);
    }

    #[test]
    fn brighten_clamps_to_1() {
        let c = brighten_color(Color::from_rgb(0.9, 1.0, 0.85));
        assert!((c.r - 1.0).abs() < 0.01); // 0.9 * 1.2 = 1.08 → clamped to 1.0
        assert!((c.g - 1.0).abs() < 0.01); // 1.0 * 1.2 = 1.2 → clamped to 1.0
        assert!((c.b - 1.0).abs() < 0.01); // 0.85 * 1.2 = 1.02 → clamped to 1.0
    }

    #[test]
    fn brighten_preserves_alpha() {
        let c = brighten_color(Color::from_rgba(0.5, 0.5, 0.5, 0.7));
        assert!((c.a - 0.7).abs() < 0.01);
    }
}
