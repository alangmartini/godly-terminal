/// Terminal color theme for GPU rendering.
///
/// All colors are normalized RGBA in the range [0.0, 1.0].
#[derive(Debug, Clone)]
pub struct TerminalTheme {
    /// Default text foreground color.
    pub foreground: [f32; 4],
    /// Default background color.
    pub background: [f32; 4],
    /// Cursor block color.
    pub cursor_color: [f32; 4],
    /// Selection highlight background color.
    pub selection_bg: [f32; 4],
}

impl Default for TerminalTheme {
    fn default() -> Self {
        // Dark theme: light gray text on dark background
        Self {
            foreground: [0.8, 0.8, 0.8, 1.0],
            background: [0.12, 0.12, 0.12, 1.0],
            cursor_color: [0.8, 0.8, 0.8, 0.8],
            selection_bg: [0.3, 0.4, 0.6, 0.5],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_has_valid_colors() {
        let theme = TerminalTheme::default();
        for color in &[theme.foreground, theme.background, theme.cursor_color, theme.selection_bg] {
            for component in color {
                assert!(*component >= 0.0 && *component <= 1.0);
            }
        }
    }

    #[test]
    fn default_theme_foreground_is_light() {
        let theme = TerminalTheme::default();
        // Foreground should be lighter than background (dark theme)
        assert!(theme.foreground[0] > theme.background[0]);
    }
}
