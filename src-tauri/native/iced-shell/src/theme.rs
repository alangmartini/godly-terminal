use iced::Color;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

// ---------------------------------------------------------------------------
// Serde helper for iced::Color (serializes as [r, g, b, a] array).
// ---------------------------------------------------------------------------

mod color_serde {
    use iced::Color;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(c: &Color, s: S) -> Result<S::Ok, S::Error> {
        [c.r, c.g, c.b, c.a].serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
        let [r, g, b, a] = <[f32; 4]>::deserialize(d)?;
        Ok(Color::from_rgba(r, g, b, a))
    }
}

// ---------------------------------------------------------------------------
// Terminal-specific color palette (16 ANSI colors + extras).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TerminalPalette {
    #[serde(with = "color_serde")]
    pub black: Color,
    #[serde(with = "color_serde")]
    pub red: Color,
    #[serde(with = "color_serde")]
    pub green: Color,
    #[serde(with = "color_serde")]
    pub yellow: Color,
    #[serde(with = "color_serde")]
    pub blue: Color,
    #[serde(with = "color_serde")]
    pub magenta: Color,
    #[serde(with = "color_serde")]
    pub cyan: Color,
    #[serde(with = "color_serde")]
    pub white: Color,
    #[serde(with = "color_serde")]
    pub bright_black: Color,
    #[serde(with = "color_serde")]
    pub bright_red: Color,
    #[serde(with = "color_serde")]
    pub bright_green: Color,
    #[serde(with = "color_serde")]
    pub bright_yellow: Color,
    #[serde(with = "color_serde")]
    pub bright_blue: Color,
    #[serde(with = "color_serde")]
    pub bright_magenta: Color,
    #[serde(with = "color_serde")]
    pub bright_cyan: Color,
    #[serde(with = "color_serde")]
    pub bright_white: Color,
    #[serde(with = "color_serde")]
    pub foreground: Color,
    #[serde(with = "color_serde")]
    pub background: Color,
    #[serde(with = "color_serde")]
    pub cursor: Color,
    #[serde(with = "color_serde")]
    pub selection: Color,
}

// ---------------------------------------------------------------------------
// Full UI + terminal theme palette.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThemePalette {
    #[serde(with = "color_serde")]
    pub bg_primary: Color,
    #[serde(with = "color_serde")]
    pub bg_secondary: Color,
    #[serde(with = "color_serde")]
    pub bg_tertiary: Color,
    #[serde(with = "color_serde")]
    pub bg_active: Color,
    #[serde(with = "color_serde")]
    pub text_primary: Color,
    #[serde(with = "color_serde")]
    pub text_secondary: Color,
    #[serde(with = "color_serde")]
    pub text_active: Color,
    #[serde(with = "color_serde")]
    pub accent: Color,
    #[serde(with = "color_serde")]
    pub accent_hover: Color,
    #[serde(with = "color_serde")]
    pub border: Color,
    #[serde(with = "color_serde")]
    pub danger: Color,
    #[serde(with = "color_serde")]
    pub pane_bg: Color,
    #[serde(with = "color_serde")]
    pub pane_border: Color,
    #[serde(with = "color_serde")]
    pub pane_focused_border: Color,
    #[serde(with = "color_serde")]
    pub empty_state_bg: Color,
    #[serde(with = "color_serde")]
    pub backdrop: Color,
    pub terminal: TerminalPalette,
}

// ---------------------------------------------------------------------------
// Theme identifiers.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeId {
    Dusk,
    TokyoNight,
    Dracula,
    Nord,
    GruvboxDark,
    OneDark,
    Catppuccin,
    SolarizedDark,
    Monokai,
    AyuDark,
    RosePine,
}

impl ThemeId {
    pub fn all() -> &'static [ThemeId] {
        &[
            ThemeId::Dusk,
            ThemeId::TokyoNight,
            ThemeId::Dracula,
            ThemeId::Nord,
            ThemeId::GruvboxDark,
            ThemeId::OneDark,
            ThemeId::Catppuccin,
            ThemeId::SolarizedDark,
            ThemeId::Monokai,
            ThemeId::AyuDark,
            ThemeId::RosePine,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            ThemeId::Dusk => "Dusk",
            ThemeId::TokyoNight => "Tokyo Night",
            ThemeId::Dracula => "Dracula",
            ThemeId::Nord => "Nord",
            ThemeId::GruvboxDark => "Gruvbox Dark",
            ThemeId::OneDark => "One Dark",
            ThemeId::Catppuccin => "Catppuccin",
            ThemeId::SolarizedDark => "Solarized Dark",
            ThemeId::Monokai => "Monokai",
            ThemeId::AyuDark => "Ayu Dark",
            ThemeId::RosePine => "Rosé Pine",
        }
    }

    /// Preview swatch colors for theme picker UI: [bg, accent, fg, border, terminal_bg].
    pub fn preview_colors(self) -> [Color; 5] {
        let p = palette(self);
        [
            p.bg_primary,
            p.accent,
            p.text_primary,
            p.border,
            p.terminal.background,
        ]
    }
}

// ---------------------------------------------------------------------------
// Global active palette (RwLock).
// ---------------------------------------------------------------------------

static ACTIVE_PALETTE: RwLock<Option<ThemePalette>> = RwLock::new(None);

pub fn set_active_theme(id: ThemeId) {
    let p = palette(id);
    *ACTIVE_PALETTE.write().unwrap() = Some(p);
}

fn active() -> ThemePalette {
    ACTIVE_PALETTE
        .read()
        .unwrap()
        .clone()
        .unwrap_or_else(|| palette(ThemeId::Dusk))
}

/// Returns the active theme's terminal palette.
pub fn active_terminal_palette() -> TerminalPalette {
    active().terminal
}

// ---------------------------------------------------------------------------
// Custom themes (F5: JSON import/export + persistence).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CustomTheme {
    pub id: String,
    pub name: String,
    pub ui: ThemePalette,
    pub terminal: TerminalPalette,
}

impl CustomTheme {
    /// Preview swatch colors matching `ThemeId::preview_colors()`.
    pub fn preview_colors(&self) -> [Color; 5] {
        [
            self.ui.bg_primary,
            self.ui.accent,
            self.ui.text_primary,
            self.ui.border,
            self.terminal.background,
        ]
    }
}

/// Set the active palette from a custom theme.
pub fn set_active_custom_theme(theme: &CustomTheme) {
    let mut p = theme.ui.clone();
    p.terminal = theme.terminal.clone();
    *ACTIVE_PALETTE.write().unwrap() = Some(p);
}

/// Parse and validate a JSON string as a `CustomTheme`.
pub fn validate_custom_theme(json: &str) -> Result<CustomTheme, String> {
    let theme: CustomTheme =
        serde_json::from_str(json).map_err(|e| format!("Invalid theme JSON: {e}"))?;
    if theme.id.is_empty() {
        return Err("Theme 'id' must not be empty".into());
    }
    if theme.name.is_empty() {
        return Err("Theme 'name' must not be empty".into());
    }
    Ok(theme)
}

const CUSTOM_THEMES_FILE: &str = "custom-themes.json";

/// Default directory for custom theme persistence.
pub fn custom_themes_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .ok()
        .or_else(|| std::env::var("HOME").ok())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let directory_name = format!("com.godly.terminal{}", godly_protocol::instance_suffix());
    base.join(directory_name).join("native")
}

/// Load custom themes from `custom-themes.json` in `dir`.
pub fn load_custom_themes(dir: &Path) -> Vec<CustomTheme> {
    let path = dir.join(CUSTOM_THEMES_FILE);
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|e| {
            log::warn!("Failed to parse {}: {}", path.display(), e);
            Vec::new()
        }),
        Err(_) => Vec::new(),
    }
}

/// Save custom themes to `custom-themes.json` in `dir`.
pub fn save_custom_themes(dir: &Path, themes: &[CustomTheme]) -> Result<(), String> {
    let path = dir.join(CUSTOM_THEMES_FILE);
    std::fs::create_dir_all(dir).map_err(|e| format!("Failed to create dir: {e}"))?;
    let json =
        serde_json::to_string_pretty(themes).map_err(|e| format!("Serialization failed: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

/// Export a single custom theme to a file in `dir`, named `{name}.json`.
pub fn export_theme_to_file(theme: &CustomTheme, dir: &Path) -> Result<PathBuf, String> {
    let safe_name: String = theme
        .name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let filename = format!("{}.json", safe_name.trim());
    let path = dir.join(filename);
    let json =
        serde_json::to_string_pretty(theme).map_err(|e| format!("Serialization failed: {e}"))?;
    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
    Ok(path)
}

// ---------------------------------------------------------------------------
// Backward-compatible accessors (previously pub const, now pub fn).
// Import sites stay identical: `use crate::theme::{BG_PRIMARY, ACCENT, ...};`
// Usage sites must append `()`: `BG_PRIMARY` → `BG_PRIMARY()`.
// ---------------------------------------------------------------------------

#[allow(non_snake_case)]
pub fn BG_PRIMARY() -> Color {
    active().bg_primary
}
#[allow(non_snake_case)]
pub fn BG_SECONDARY() -> Color {
    active().bg_secondary
}
#[allow(non_snake_case)]
pub fn BG_TERTIARY() -> Color {
    active().bg_tertiary
}
#[allow(non_snake_case)]
pub fn BG_ACTIVE() -> Color {
    active().bg_active
}
#[allow(non_snake_case)]
pub fn TEXT_PRIMARY() -> Color {
    active().text_primary
}
#[allow(non_snake_case)]
pub fn TEXT_SECONDARY() -> Color {
    active().text_secondary
}
#[allow(non_snake_case)]
pub fn TEXT_ACTIVE() -> Color {
    active().text_active
}
#[allow(non_snake_case)]
pub fn ACCENT() -> Color {
    active().accent
}
#[allow(non_snake_case)]
pub fn ACCENT_HOVER() -> Color {
    active().accent_hover
}
#[allow(non_snake_case)]
pub fn BORDER() -> Color {
    active().border
}
#[allow(non_snake_case)]
pub fn DANGER() -> Color {
    active().danger
}
#[allow(non_snake_case)]
pub fn PANE_BG() -> Color {
    active().pane_bg
}
#[allow(non_snake_case)]
pub fn PANE_BORDER() -> Color {
    active().pane_border
}
#[allow(non_snake_case)]
pub fn PANE_FOCUSED_BORDER() -> Color {
    active().pane_focused_border
}
#[allow(non_snake_case)]
pub fn EMPTY_STATE_BG() -> Color {
    active().empty_state_bg
}
#[allow(non_snake_case)]
pub fn BACKDROP() -> Color {
    active().backdrop
}

// ---------------------------------------------------------------------------
// Design system tokens (theme-independent, stay as pub const).
// ---------------------------------------------------------------------------

pub const UI_FONT_SIZE_SM: f32 = 11.0;
pub const UI_FONT_SIZE: f32 = 13.0;
pub const UI_FONT_SIZE_LG: f32 = 15.0;

pub const SPACE_XXS: f32 = 2.0;
pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 12.0;
pub const SPACE_LG: f32 = 16.0;
pub const SPACE_XL: f32 = 24.0;
pub const SPACE_XXL: f32 = 32.0;

pub const RADIUS_SM: f32 = 4.0;
pub const RADIUS_MD: f32 = 6.0;
pub const RADIUS_LG: f32 = 8.0;
pub const RADIUS_XL: f32 = 12.0;

pub const SHADOW_COLOR: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.40);
pub const SHADOW_LIGHT: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.20);

pub const TRANSITION_HOVER_MS: u64 = 150;
pub const TRANSITION_STATE_MS: u64 = 200;

// ---------------------------------------------------------------------------
// Palette definitions.
// ---------------------------------------------------------------------------

pub fn palette(id: ThemeId) -> ThemePalette {
    match id {
        ThemeId::Dusk => dusk(),
        ThemeId::TokyoNight => tokyo_night(),
        ThemeId::Dracula => dracula(),
        ThemeId::Nord => nord(),
        ThemeId::GruvboxDark => gruvbox_dark(),
        ThemeId::OneDark => one_dark(),
        ThemeId::Catppuccin => catppuccin(),
        ThemeId::SolarizedDark => solarized_dark(),
        ThemeId::Monokai => monokai(),
        ThemeId::AyuDark => ayu_dark(),
        ThemeId::RosePine => rose_pine(),
    }
}

fn dusk() -> ThemePalette {
    ThemePalette {
        bg_primary: Color::from_rgb(0.1137, 0.1216, 0.1294),
        bg_secondary: Color::from_rgb(0.0941, 0.1020, 0.1059),
        bg_tertiary: Color::from_rgb(0.1804, 0.1686, 0.1569),
        bg_active: Color::from_rgb(0.2392, 0.2157, 0.2000),
        text_primary: Color::from_rgb(0.6902, 0.6706, 0.6235),
        text_secondary: Color::from_rgb(0.4196, 0.4000, 0.3608),
        text_active: Color::from_rgb(0.7725, 0.7529, 0.7137),
        accent: Color::from_rgb(0.8314, 0.6627, 0.4157),
        accent_hover: Color::from_rgb(0.8784, 0.7451, 0.5333),
        border: Color::from_rgb(0.1804, 0.1686, 0.1569),
        danger: Color::from_rgb(0.7608, 0.4392, 0.4392),
        pane_bg: Color::from_rgb(0.0745, 0.0784, 0.0863),
        pane_border: Color::from_rgb(0.2471, 0.2235, 0.2039),
        pane_focused_border: Color::from_rgb(0.8784, 0.7451, 0.5333),
        empty_state_bg: Color::from_rgb(0.1294, 0.1333, 0.1451),
        backdrop: Color::from_rgba(0.0, 0.0, 0.0, 0.58),
        terminal: TerminalPalette {
            black: Color::from_rgb8(0x1d, 0x1f, 0x21),
            red: Color::from_rgb8(0xcc, 0x66, 0x66),
            green: Color::from_rgb8(0xb5, 0xbd, 0x68),
            yellow: Color::from_rgb8(0xf0, 0xc6, 0x74),
            blue: Color::from_rgb8(0x81, 0xa2, 0xbe),
            magenta: Color::from_rgb8(0xb2, 0x94, 0xbb),
            cyan: Color::from_rgb8(0x8a, 0xbe, 0xb7),
            white: Color::from_rgb8(0xc5, 0xc8, 0xc6),
            bright_black: Color::from_rgb8(0x96, 0x98, 0x96),
            bright_red: Color::from_rgb8(0xde, 0x93, 0x5f),
            bright_green: Color::from_rgb8(0xb5, 0xbd, 0x68),
            bright_yellow: Color::from_rgb8(0xf0, 0xc6, 0x74),
            bright_blue: Color::from_rgb8(0x81, 0xa2, 0xbe),
            bright_magenta: Color::from_rgb8(0xb2, 0x94, 0xbb),
            bright_cyan: Color::from_rgb8(0x8a, 0xbe, 0xb7),
            bright_white: Color::from_rgb8(0xff, 0xff, 0xff),
            foreground: Color::from_rgb8(0xc5, 0xc8, 0xc6),
            background: Color::from_rgb8(0x13, 0x14, 0x16),
            cursor: Color::from_rgb8(0xd4, 0xa9, 0x6a),
            selection: Color::from_rgba(0.83, 0.66, 0.42, 0.30),
        },
    }
}

fn tokyo_night() -> ThemePalette {
    ThemePalette {
        bg_primary: Color::from_rgb8(0x1a, 0x1b, 0x26),
        bg_secondary: Color::from_rgb8(0x16, 0x16, 0x1e),
        bg_tertiary: Color::from_rgb8(0x24, 0x28, 0x3b),
        bg_active: Color::from_rgb8(0x33, 0x46, 0x7c),
        text_primary: Color::from_rgb8(0xa9, 0xb1, 0xd6),
        text_secondary: Color::from_rgb8(0x56, 0x5f, 0x89),
        text_active: Color::from_rgb8(0xc0, 0xca, 0xf5),
        accent: Color::from_rgb8(0x7a, 0xa2, 0xf7),
        accent_hover: Color::from_rgb8(0x89, 0xdd, 0xff),
        border: Color::from_rgb8(0x29, 0x2e, 0x42),
        danger: Color::from_rgb8(0xf7, 0x76, 0x8e),
        pane_bg: Color::from_rgb8(0x16, 0x16, 0x1e),
        pane_border: Color::from_rgb8(0x29, 0x2e, 0x42),
        pane_focused_border: Color::from_rgb8(0x7a, 0xa2, 0xf7),
        empty_state_bg: Color::from_rgb8(0x1e, 0x20, 0x30),
        backdrop: Color::from_rgba(0.0, 0.0, 0.0, 0.58),
        terminal: TerminalPalette {
            black: Color::from_rgb8(0x15, 0x16, 0x1e),
            red: Color::from_rgb8(0xf7, 0x76, 0x8e),
            green: Color::from_rgb8(0x9e, 0xce, 0x6a),
            yellow: Color::from_rgb8(0xe0, 0xaf, 0x68),
            blue: Color::from_rgb8(0x7a, 0xa2, 0xf7),
            magenta: Color::from_rgb8(0xbb, 0x9a, 0xf7),
            cyan: Color::from_rgb8(0x7d, 0xcf, 0xff),
            white: Color::from_rgb8(0xa9, 0xb1, 0xd6),
            bright_black: Color::from_rgb8(0x41, 0x48, 0x68),
            bright_red: Color::from_rgb8(0xf7, 0x76, 0x8e),
            bright_green: Color::from_rgb8(0x9e, 0xce, 0x6a),
            bright_yellow: Color::from_rgb8(0xe0, 0xaf, 0x68),
            bright_blue: Color::from_rgb8(0x7a, 0xa2, 0xf7),
            bright_magenta: Color::from_rgb8(0xbb, 0x9a, 0xf7),
            bright_cyan: Color::from_rgb8(0x7d, 0xcf, 0xff),
            bright_white: Color::from_rgb8(0xc0, 0xca, 0xf5),
            foreground: Color::from_rgb8(0xa9, 0xb1, 0xd6),
            background: Color::from_rgb8(0x1a, 0x1b, 0x26),
            cursor: Color::from_rgb8(0xc0, 0xca, 0xf5),
            selection: Color::from_rgba8(0x33, 0x46, 0x7c, 0.4),
        },
    }
}

fn dracula() -> ThemePalette {
    ThemePalette {
        bg_primary: Color::from_rgb8(0x28, 0x2a, 0x36),
        bg_secondary: Color::from_rgb8(0x21, 0x22, 0x2c),
        bg_tertiary: Color::from_rgb8(0x34, 0x35, 0x46),
        bg_active: Color::from_rgb8(0x44, 0x47, 0x5a),
        text_primary: Color::from_rgb8(0xf8, 0xf8, 0xf2),
        text_secondary: Color::from_rgb8(0x62, 0x72, 0xa4),
        text_active: Color::from_rgb8(0xf8, 0xf8, 0xf2),
        accent: Color::from_rgb8(0xbd, 0x93, 0xf9),
        accent_hover: Color::from_rgb8(0xcf, 0xa9, 0xff),
        border: Color::from_rgb8(0x34, 0x35, 0x46),
        danger: Color::from_rgb8(0xff, 0x55, 0x55),
        pane_bg: Color::from_rgb8(0x21, 0x22, 0x2c),
        pane_border: Color::from_rgb8(0x44, 0x47, 0x5a),
        pane_focused_border: Color::from_rgb8(0xbd, 0x93, 0xf9),
        empty_state_bg: Color::from_rgb8(0x28, 0x2a, 0x36),
        backdrop: Color::from_rgba(0.0, 0.0, 0.0, 0.58),
        terminal: TerminalPalette {
            black: Color::from_rgb8(0x21, 0x22, 0x2c),
            red: Color::from_rgb8(0xff, 0x55, 0x55),
            green: Color::from_rgb8(0x50, 0xfa, 0x7b),
            yellow: Color::from_rgb8(0xf1, 0xfa, 0x8c),
            blue: Color::from_rgb8(0xbd, 0x93, 0xf9),
            magenta: Color::from_rgb8(0xff, 0x79, 0xc6),
            cyan: Color::from_rgb8(0x8b, 0xe9, 0xfd),
            white: Color::from_rgb8(0xf8, 0xf8, 0xf2),
            bright_black: Color::from_rgb8(0x62, 0x72, 0xa4),
            bright_red: Color::from_rgb8(0xff, 0x6e, 0x6e),
            bright_green: Color::from_rgb8(0x69, 0xff, 0x94),
            bright_yellow: Color::from_rgb8(0xff, 0xff, 0xa5),
            bright_blue: Color::from_rgb8(0xd6, 0xac, 0xff),
            bright_magenta: Color::from_rgb8(0xff, 0x92, 0xdf),
            bright_cyan: Color::from_rgb8(0xa4, 0xff, 0xff),
            bright_white: Color::from_rgb8(0xff, 0xff, 0xff),
            foreground: Color::from_rgb8(0xf8, 0xf8, 0xf2),
            background: Color::from_rgb8(0x28, 0x2a, 0x36),
            cursor: Color::from_rgb8(0xf8, 0xf8, 0xf2),
            selection: Color::from_rgba8(0x44, 0x47, 0x5a, 0.5),
        },
    }
}

fn nord() -> ThemePalette {
    ThemePalette {
        bg_primary: Color::from_rgb8(0x2e, 0x34, 0x40),
        bg_secondary: Color::from_rgb8(0x29, 0x2e, 0x39),
        bg_tertiary: Color::from_rgb8(0x3b, 0x42, 0x52),
        bg_active: Color::from_rgb8(0x43, 0x4c, 0x5e),
        text_primary: Color::from_rgb8(0xd8, 0xde, 0xe9),
        text_secondary: Color::from_rgb8(0x7b, 0x88, 0xa1),
        text_active: Color::from_rgb8(0xec, 0xef, 0xf4),
        accent: Color::from_rgb8(0x88, 0xc0, 0xd0),
        accent_hover: Color::from_rgb8(0x8f, 0xbc, 0xbb),
        border: Color::from_rgb8(0x3b, 0x42, 0x52),
        danger: Color::from_rgb8(0xbf, 0x61, 0x6a),
        pane_bg: Color::from_rgb8(0x29, 0x2e, 0x39),
        pane_border: Color::from_rgb8(0x3b, 0x42, 0x52),
        pane_focused_border: Color::from_rgb8(0x88, 0xc0, 0xd0),
        empty_state_bg: Color::from_rgb8(0x2e, 0x34, 0x40),
        backdrop: Color::from_rgba(0.0, 0.0, 0.0, 0.58),
        terminal: TerminalPalette {
            black: Color::from_rgb8(0x3b, 0x42, 0x52),
            red: Color::from_rgb8(0xbf, 0x61, 0x6a),
            green: Color::from_rgb8(0xa3, 0xbe, 0x8c),
            yellow: Color::from_rgb8(0xeb, 0xcb, 0x8b),
            blue: Color::from_rgb8(0x81, 0xa1, 0xc1),
            magenta: Color::from_rgb8(0xb4, 0x8e, 0xad),
            cyan: Color::from_rgb8(0x88, 0xc0, 0xd0),
            white: Color::from_rgb8(0xe5, 0xe9, 0xf0),
            bright_black: Color::from_rgb8(0x4c, 0x56, 0x6a),
            bright_red: Color::from_rgb8(0xbf, 0x61, 0x6a),
            bright_green: Color::from_rgb8(0xa3, 0xbe, 0x8c),
            bright_yellow: Color::from_rgb8(0xeb, 0xcb, 0x8b),
            bright_blue: Color::from_rgb8(0x81, 0xa1, 0xc1),
            bright_magenta: Color::from_rgb8(0xb4, 0x8e, 0xad),
            bright_cyan: Color::from_rgb8(0x8f, 0xbc, 0xbb),
            bright_white: Color::from_rgb8(0xec, 0xef, 0xf4),
            foreground: Color::from_rgb8(0xd8, 0xde, 0xe9),
            background: Color::from_rgb8(0x2e, 0x34, 0x40),
            cursor: Color::from_rgb8(0xd8, 0xde, 0xe9),
            selection: Color::from_rgba8(0x43, 0x4c, 0x5e, 0.5),
        },
    }
}

fn gruvbox_dark() -> ThemePalette {
    ThemePalette {
        bg_primary: Color::from_rgb8(0x28, 0x28, 0x28),
        bg_secondary: Color::from_rgb8(0x1d, 0x20, 0x21),
        bg_tertiary: Color::from_rgb8(0x3c, 0x38, 0x36),
        bg_active: Color::from_rgb8(0x50, 0x49, 0x45),
        text_primary: Color::from_rgb8(0xeb, 0xdb, 0xb2),
        text_secondary: Color::from_rgb8(0x92, 0x83, 0x74),
        text_active: Color::from_rgb8(0xfb, 0xf1, 0xc7),
        accent: Color::from_rgb8(0xfe, 0x80, 0x19),
        accent_hover: Color::from_rgb8(0xfa, 0xbd, 0x2f),
        border: Color::from_rgb8(0x3c, 0x38, 0x36),
        danger: Color::from_rgb8(0xfb, 0x49, 0x34),
        pane_bg: Color::from_rgb8(0x1d, 0x20, 0x21),
        pane_border: Color::from_rgb8(0x50, 0x49, 0x45),
        pane_focused_border: Color::from_rgb8(0xfe, 0x80, 0x19),
        empty_state_bg: Color::from_rgb8(0x28, 0x28, 0x28),
        backdrop: Color::from_rgba(0.0, 0.0, 0.0, 0.58),
        terminal: TerminalPalette {
            black: Color::from_rgb8(0x28, 0x28, 0x28),
            red: Color::from_rgb8(0xcc, 0x24, 0x1d),
            green: Color::from_rgb8(0x98, 0x97, 0x1a),
            yellow: Color::from_rgb8(0xd7, 0x99, 0x21),
            blue: Color::from_rgb8(0x45, 0x85, 0x88),
            magenta: Color::from_rgb8(0xb1, 0x62, 0x86),
            cyan: Color::from_rgb8(0x68, 0x9d, 0x6a),
            white: Color::from_rgb8(0xa8, 0x99, 0x84),
            bright_black: Color::from_rgb8(0x92, 0x83, 0x74),
            bright_red: Color::from_rgb8(0xfb, 0x49, 0x34),
            bright_green: Color::from_rgb8(0xb8, 0xbb, 0x26),
            bright_yellow: Color::from_rgb8(0xfa, 0xbd, 0x2f),
            bright_blue: Color::from_rgb8(0x83, 0xa5, 0x98),
            bright_magenta: Color::from_rgb8(0xd3, 0x86, 0x9b),
            bright_cyan: Color::from_rgb8(0x8e, 0xc0, 0x7c),
            bright_white: Color::from_rgb8(0xeb, 0xdb, 0xb2),
            foreground: Color::from_rgb8(0xeb, 0xdb, 0xb2),
            background: Color::from_rgb8(0x28, 0x28, 0x28),
            cursor: Color::from_rgb8(0xeb, 0xdb, 0xb2),
            selection: Color::from_rgba8(0x50, 0x49, 0x45, 0.5),
        },
    }
}

fn one_dark() -> ThemePalette {
    ThemePalette {
        bg_primary: Color::from_rgb8(0x28, 0x2c, 0x34),
        bg_secondary: Color::from_rgb8(0x21, 0x25, 0x2b),
        bg_tertiary: Color::from_rgb8(0x2c, 0x31, 0x3c),
        bg_active: Color::from_rgb8(0x3e, 0x44, 0x51),
        text_primary: Color::from_rgb8(0xab, 0xb2, 0xbf),
        text_secondary: Color::from_rgb8(0x5c, 0x63, 0x70),
        text_active: Color::from_rgb8(0xd7, 0xda, 0xe0),
        accent: Color::from_rgb8(0x61, 0xaf, 0xef),
        accent_hover: Color::from_rgb8(0x74, 0xbf, 0xf5),
        border: Color::from_rgb8(0x3e, 0x44, 0x51),
        danger: Color::from_rgb8(0xe0, 0x6c, 0x75),
        pane_bg: Color::from_rgb8(0x21, 0x25, 0x2b),
        pane_border: Color::from_rgb8(0x3e, 0x44, 0x51),
        pane_focused_border: Color::from_rgb8(0x61, 0xaf, 0xef),
        empty_state_bg: Color::from_rgb8(0x28, 0x2c, 0x34),
        backdrop: Color::from_rgba(0.0, 0.0, 0.0, 0.58),
        terminal: TerminalPalette {
            black: Color::from_rgb8(0x28, 0x2c, 0x34),
            red: Color::from_rgb8(0xe0, 0x6c, 0x75),
            green: Color::from_rgb8(0x98, 0xc3, 0x79),
            yellow: Color::from_rgb8(0xe5, 0xc0, 0x7b),
            blue: Color::from_rgb8(0x61, 0xaf, 0xef),
            magenta: Color::from_rgb8(0xc6, 0x78, 0xdd),
            cyan: Color::from_rgb8(0x56, 0xb6, 0xc2),
            white: Color::from_rgb8(0xab, 0xb2, 0xbf),
            bright_black: Color::from_rgb8(0x5c, 0x63, 0x70),
            bright_red: Color::from_rgb8(0xe0, 0x6c, 0x75),
            bright_green: Color::from_rgb8(0x98, 0xc3, 0x79),
            bright_yellow: Color::from_rgb8(0xe5, 0xc0, 0x7b),
            bright_blue: Color::from_rgb8(0x61, 0xaf, 0xef),
            bright_magenta: Color::from_rgb8(0xc6, 0x78, 0xdd),
            bright_cyan: Color::from_rgb8(0x56, 0xb6, 0xc2),
            bright_white: Color::from_rgb8(0xd7, 0xda, 0xe0),
            foreground: Color::from_rgb8(0xab, 0xb2, 0xbf),
            background: Color::from_rgb8(0x28, 0x2c, 0x34),
            cursor: Color::from_rgb8(0x52, 0x8b, 0xff),
            selection: Color::from_rgba8(0x3e, 0x44, 0x51, 0.5),
        },
    }
}

fn catppuccin() -> ThemePalette {
    ThemePalette {
        bg_primary: Color::from_rgb8(0x1e, 0x1e, 0x2e),
        bg_secondary: Color::from_rgb8(0x18, 0x18, 0x25),
        bg_tertiary: Color::from_rgb8(0x31, 0x32, 0x44),
        bg_active: Color::from_rgb8(0x45, 0x47, 0x5a),
        text_primary: Color::from_rgb8(0xcd, 0xd6, 0xf4),
        text_secondary: Color::from_rgb8(0x6c, 0x70, 0x86),
        text_active: Color::from_rgb8(0xcd, 0xd6, 0xf4),
        accent: Color::from_rgb8(0xcb, 0xa6, 0xf7),
        accent_hover: Color::from_rgb8(0xd4, 0xb7, 0xf9),
        border: Color::from_rgb8(0x31, 0x32, 0x44),
        danger: Color::from_rgb8(0xf3, 0x8b, 0xa8),
        pane_bg: Color::from_rgb8(0x18, 0x18, 0x25),
        pane_border: Color::from_rgb8(0x45, 0x47, 0x5a),
        pane_focused_border: Color::from_rgb8(0xcb, 0xa6, 0xf7),
        empty_state_bg: Color::from_rgb8(0x1e, 0x1e, 0x2e),
        backdrop: Color::from_rgba(0.0, 0.0, 0.0, 0.58),
        terminal: TerminalPalette {
            black: Color::from_rgb8(0x45, 0x47, 0x5a),
            red: Color::from_rgb8(0xf3, 0x8b, 0xa8),
            green: Color::from_rgb8(0xa6, 0xe3, 0xa1),
            yellow: Color::from_rgb8(0xf9, 0xe2, 0xaf),
            blue: Color::from_rgb8(0x89, 0xb4, 0xfa),
            magenta: Color::from_rgb8(0xcb, 0xa6, 0xf7),
            cyan: Color::from_rgb8(0x94, 0xe2, 0xd5),
            white: Color::from_rgb8(0xba, 0xc2, 0xde),
            bright_black: Color::from_rgb8(0x58, 0x5b, 0x70),
            bright_red: Color::from_rgb8(0xf3, 0x8b, 0xa8),
            bright_green: Color::from_rgb8(0xa6, 0xe3, 0xa1),
            bright_yellow: Color::from_rgb8(0xf9, 0xe2, 0xaf),
            bright_blue: Color::from_rgb8(0x89, 0xb4, 0xfa),
            bright_magenta: Color::from_rgb8(0xcb, 0xa6, 0xf7),
            bright_cyan: Color::from_rgb8(0x94, 0xe2, 0xd5),
            bright_white: Color::from_rgb8(0xa6, 0xad, 0xc8),
            foreground: Color::from_rgb8(0xcd, 0xd6, 0xf4),
            background: Color::from_rgb8(0x1e, 0x1e, 0x2e),
            cursor: Color::from_rgb8(0xf5, 0xe0, 0xdc),
            selection: Color::from_rgba8(0x45, 0x47, 0x5a, 0.5),
        },
    }
}

fn solarized_dark() -> ThemePalette {
    ThemePalette {
        bg_primary: Color::from_rgb8(0x00, 0x2b, 0x36),
        bg_secondary: Color::from_rgb8(0x00, 0x21, 0x2b),
        bg_tertiary: Color::from_rgb8(0x07, 0x36, 0x42),
        bg_active: Color::from_rgb8(0x09, 0x40, 0x4f),
        text_primary: Color::from_rgb8(0x83, 0x94, 0x96),
        text_secondary: Color::from_rgb8(0x58, 0x6e, 0x75),
        text_active: Color::from_rgb8(0x93, 0xa1, 0xa1),
        accent: Color::from_rgb8(0xb5, 0x89, 0x00),
        accent_hover: Color::from_rgb8(0xcb, 0x4b, 0x16),
        border: Color::from_rgb8(0x07, 0x36, 0x42),
        danger: Color::from_rgb8(0xdc, 0x32, 0x2f),
        pane_bg: Color::from_rgb8(0x00, 0x21, 0x2b),
        pane_border: Color::from_rgb8(0x07, 0x36, 0x42),
        pane_focused_border: Color::from_rgb8(0xb5, 0x89, 0x00),
        empty_state_bg: Color::from_rgb8(0x00, 0x2b, 0x36),
        backdrop: Color::from_rgba(0.0, 0.0, 0.0, 0.58),
        terminal: TerminalPalette {
            black: Color::from_rgb8(0x07, 0x36, 0x42),
            red: Color::from_rgb8(0xdc, 0x32, 0x2f),
            green: Color::from_rgb8(0x85, 0x99, 0x00),
            yellow: Color::from_rgb8(0xb5, 0x89, 0x00),
            blue: Color::from_rgb8(0x26, 0x8b, 0xd2),
            magenta: Color::from_rgb8(0xd3, 0x36, 0x82),
            cyan: Color::from_rgb8(0x2a, 0xa1, 0x98),
            white: Color::from_rgb8(0xee, 0xe8, 0xd5),
            bright_black: Color::from_rgb8(0x00, 0x2b, 0x36),
            bright_red: Color::from_rgb8(0xcb, 0x4b, 0x16),
            bright_green: Color::from_rgb8(0x58, 0x6e, 0x75),
            bright_yellow: Color::from_rgb8(0x65, 0x7b, 0x83),
            bright_blue: Color::from_rgb8(0x83, 0x94, 0x96),
            bright_magenta: Color::from_rgb8(0x6c, 0x71, 0xc4),
            bright_cyan: Color::from_rgb8(0x93, 0xa1, 0xa1),
            bright_white: Color::from_rgb8(0xfd, 0xf6, 0xe3),
            foreground: Color::from_rgb8(0x83, 0x94, 0x96),
            background: Color::from_rgb8(0x00, 0x2b, 0x36),
            cursor: Color::from_rgb8(0x83, 0x94, 0x96),
            selection: Color::from_rgba8(0x07, 0x36, 0x42, 0.5),
        },
    }
}

fn monokai() -> ThemePalette {
    ThemePalette {
        bg_primary: Color::from_rgb8(0x27, 0x28, 0x22),
        bg_secondary: Color::from_rgb8(0x1e, 0x1f, 0x1a),
        bg_tertiary: Color::from_rgb8(0x3e, 0x3d, 0x32),
        bg_active: Color::from_rgb8(0x49, 0x48, 0x3e),
        text_primary: Color::from_rgb8(0xf8, 0xf8, 0xf2),
        text_secondary: Color::from_rgb8(0x75, 0x71, 0x5e),
        text_active: Color::from_rgb8(0xf8, 0xf8, 0xf2),
        accent: Color::from_rgb8(0xf4, 0xbf, 0x75),
        accent_hover: Color::from_rgb8(0xe6, 0xdb, 0x74),
        border: Color::from_rgb8(0x3e, 0x3d, 0x32),
        danger: Color::from_rgb8(0xf9, 0x26, 0x72),
        pane_bg: Color::from_rgb8(0x1e, 0x1f, 0x1a),
        pane_border: Color::from_rgb8(0x49, 0x48, 0x3e),
        pane_focused_border: Color::from_rgb8(0xf4, 0xbf, 0x75),
        empty_state_bg: Color::from_rgb8(0x27, 0x28, 0x22),
        backdrop: Color::from_rgba(0.0, 0.0, 0.0, 0.58),
        terminal: TerminalPalette {
            black: Color::from_rgb8(0x27, 0x28, 0x22),
            red: Color::from_rgb8(0xf9, 0x26, 0x72),
            green: Color::from_rgb8(0xa6, 0xe2, 0x2e),
            yellow: Color::from_rgb8(0xf4, 0xbf, 0x75),
            blue: Color::from_rgb8(0x66, 0xd9, 0xef),
            magenta: Color::from_rgb8(0xae, 0x81, 0xff),
            cyan: Color::from_rgb8(0xa1, 0xef, 0xe4),
            white: Color::from_rgb8(0xf8, 0xf8, 0xf2),
            bright_black: Color::from_rgb8(0x75, 0x71, 0x5e),
            bright_red: Color::from_rgb8(0xf9, 0x26, 0x72),
            bright_green: Color::from_rgb8(0xa6, 0xe2, 0x2e),
            bright_yellow: Color::from_rgb8(0xf4, 0xbf, 0x75),
            bright_blue: Color::from_rgb8(0x66, 0xd9, 0xef),
            bright_magenta: Color::from_rgb8(0xae, 0x81, 0xff),
            bright_cyan: Color::from_rgb8(0xa1, 0xef, 0xe4),
            bright_white: Color::from_rgb8(0xf9, 0xf8, 0xf5),
            foreground: Color::from_rgb8(0xf8, 0xf8, 0xf2),
            background: Color::from_rgb8(0x27, 0x28, 0x22),
            cursor: Color::from_rgb8(0xf8, 0xf8, 0xf0),
            selection: Color::from_rgba8(0x49, 0x48, 0x3e, 0.5),
        },
    }
}

fn ayu_dark() -> ThemePalette {
    ThemePalette {
        bg_primary: Color::from_rgb8(0x0a, 0x0e, 0x14),
        bg_secondary: Color::from_rgb8(0x07, 0x0a, 0x0f),
        bg_tertiary: Color::from_rgb8(0x1a, 0x1f, 0x29),
        bg_active: Color::from_rgb8(0x27, 0x2d, 0x38),
        text_primary: Color::from_rgb8(0xb3, 0xb1, 0xad),
        text_secondary: Color::from_rgb8(0x5c, 0x67, 0x73),
        text_active: Color::from_rgb8(0xe6, 0xe1, 0xcf),
        accent: Color::from_rgb8(0xff, 0xb4, 0x54),
        accent_hover: Color::from_rgb8(0xf2, 0x9e, 0x74),
        border: Color::from_rgb8(0x1a, 0x1f, 0x29),
        danger: Color::from_rgb8(0xff, 0x33, 0x33),
        pane_bg: Color::from_rgb8(0x07, 0x0a, 0x0f),
        pane_border: Color::from_rgb8(0x27, 0x2d, 0x38),
        pane_focused_border: Color::from_rgb8(0xff, 0xb4, 0x54),
        empty_state_bg: Color::from_rgb8(0x0a, 0x0e, 0x14),
        backdrop: Color::from_rgba(0.0, 0.0, 0.0, 0.58),
        terminal: TerminalPalette {
            black: Color::from_rgb8(0x01, 0x06, 0x0e),
            red: Color::from_rgb8(0xea, 0x6c, 0x73),
            green: Color::from_rgb8(0x91, 0xb3, 0x62),
            yellow: Color::from_rgb8(0xf9, 0xaf, 0x4f),
            blue: Color::from_rgb8(0x53, 0xbd, 0xfa),
            magenta: Color::from_rgb8(0xfa, 0xe9, 0x94),
            cyan: Color::from_rgb8(0x90, 0xe1, 0xc6),
            white: Color::from_rgb8(0xc7, 0xc7, 0xc7),
            bright_black: Color::from_rgb8(0x68, 0x68, 0x68),
            bright_red: Color::from_rgb8(0xf0, 0x71, 0x78),
            bright_green: Color::from_rgb8(0xc2, 0xd9, 0x4c),
            bright_yellow: Color::from_rgb8(0xff, 0xb4, 0x54),
            bright_blue: Color::from_rgb8(0x59, 0xc2, 0xff),
            bright_magenta: Color::from_rgb8(0xff, 0xee, 0x99),
            bright_cyan: Color::from_rgb8(0x95, 0xe6, 0xcb),
            bright_white: Color::from_rgb8(0xff, 0xff, 0xff),
            foreground: Color::from_rgb8(0xb3, 0xb1, 0xad),
            background: Color::from_rgb8(0x0a, 0x0e, 0x14),
            cursor: Color::from_rgb8(0xe6, 0xb4, 0x50),
            selection: Color::from_rgba8(0x27, 0x2d, 0x38, 0.5),
        },
    }
}

fn rose_pine() -> ThemePalette {
    ThemePalette {
        bg_primary: Color::from_rgb8(0x19, 0x17, 0x24),
        bg_secondary: Color::from_rgb8(0x13, 0x11, 0x1b),
        bg_tertiary: Color::from_rgb8(0x26, 0x23, 0x3a),
        bg_active: Color::from_rgb8(0x39, 0x35, 0x52),
        text_primary: Color::from_rgb8(0xe0, 0xde, 0xf4),
        text_secondary: Color::from_rgb8(0x6e, 0x6a, 0x86),
        text_active: Color::from_rgb8(0xe0, 0xde, 0xf4),
        accent: Color::from_rgb8(0xeb, 0xbc, 0xba),
        accent_hover: Color::from_rgb8(0xf2, 0xce, 0xcd),
        border: Color::from_rgb8(0x26, 0x23, 0x3a),
        danger: Color::from_rgb8(0xeb, 0x6f, 0x92),
        pane_bg: Color::from_rgb8(0x13, 0x11, 0x1b),
        pane_border: Color::from_rgb8(0x39, 0x35, 0x52),
        pane_focused_border: Color::from_rgb8(0xeb, 0xbc, 0xba),
        empty_state_bg: Color::from_rgb8(0x19, 0x17, 0x24),
        backdrop: Color::from_rgba(0.0, 0.0, 0.0, 0.58),
        terminal: TerminalPalette {
            black: Color::from_rgb8(0x26, 0x23, 0x3a),
            red: Color::from_rgb8(0xeb, 0x6f, 0x92),
            green: Color::from_rgb8(0x31, 0x74, 0x8f),
            yellow: Color::from_rgb8(0xf6, 0xc1, 0x77),
            blue: Color::from_rgb8(0x9c, 0xcf, 0xd8),
            magenta: Color::from_rgb8(0xc4, 0xa7, 0xe7),
            cyan: Color::from_rgb8(0xea, 0x9a, 0x97),
            white: Color::from_rgb8(0xe0, 0xde, 0xf4),
            bright_black: Color::from_rgb8(0x6e, 0x6a, 0x86),
            bright_red: Color::from_rgb8(0xeb, 0x6f, 0x92),
            bright_green: Color::from_rgb8(0x31, 0x74, 0x8f),
            bright_yellow: Color::from_rgb8(0xf6, 0xc1, 0x77),
            bright_blue: Color::from_rgb8(0x9c, 0xcf, 0xd8),
            bright_magenta: Color::from_rgb8(0xc4, 0xa7, 0xe7),
            bright_cyan: Color::from_rgb8(0xea, 0x9a, 0x97),
            bright_white: Color::from_rgb8(0xe0, 0xde, 0xf4),
            foreground: Color::from_rgb8(0xe0, 0xde, 0xf4),
            background: Color::from_rgb8(0x19, 0x17, 0x24),
            cursor: Color::from_rgb8(0x52, 0x4f, 0x67),
            selection: Color::from_rgba8(0x39, 0x35, 0x52, 0.5),
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_themes_returns_11() {
        assert_eq!(ThemeId::all().len(), 11);
    }

    #[test]
    fn dusk_matches_legacy_constants() {
        let p = palette(ThemeId::Dusk);
        assert_eq!(
            format!("{:?}", p.bg_primary),
            format!("{:?}", Color::from_rgb(0.1137, 0.1216, 0.1294))
        );
        assert_eq!(
            format!("{:?}", p.accent),
            format!("{:?}", Color::from_rgb(0.8314, 0.6627, 0.4157))
        );
    }

    #[test]
    fn set_active_theme_updates_accessors() {
        set_active_theme(ThemeId::Dusk);
        let dusk_accent = ACCENT();
        set_active_theme(ThemeId::Dracula);
        let dracula_accent = ACCENT();
        assert_ne!(
            format!("{:?}", dusk_accent),
            format!("{:?}", dracula_accent)
        );
        // Restore
        set_active_theme(ThemeId::Dusk);
    }

    #[test]
    fn each_theme_has_unique_label() {
        let labels: Vec<&str> = ThemeId::all().iter().map(|t| t.label()).collect();
        let unique: std::collections::HashSet<&str> = labels.iter().copied().collect();
        assert_eq!(labels.len(), unique.len());
    }

    #[test]
    fn palette_returns_valid_colors_for_all_themes() {
        for &id in ThemeId::all() {
            let p = palette(id);
            assert!(p.bg_primary.a > 0.0);
            assert!(p.accent.a > 0.0);
            assert!(p.text_primary.a > 0.0);
        }
    }

    #[test]
    fn preview_colors_returns_five_for_all_themes() {
        for &id in ThemeId::all() {
            let colors = id.preview_colors();
            assert_eq!(colors.len(), 5);
        }
    }
}
