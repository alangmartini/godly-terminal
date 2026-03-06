use iced::Color;

// Dusk palette ported from the legacy web theme (src/themes/builtin.ts).
pub const BG_PRIMARY: Color = Color::from_rgb(0.1137, 0.1216, 0.1294); // #1d1f21
pub const BG_SECONDARY: Color = Color::from_rgb(0.0941, 0.1020, 0.1059); // #181a1b
pub const BG_TERTIARY: Color = Color::from_rgb(0.1804, 0.1686, 0.1569); // #2e2b28
pub const BG_ACTIVE: Color = Color::from_rgb(0.2392, 0.2157, 0.2000); // #3d3733

pub const TEXT_PRIMARY: Color = Color::from_rgb(0.6902, 0.6706, 0.6235); // #b0ab9f
pub const TEXT_SECONDARY: Color = Color::from_rgb(0.4196, 0.4000, 0.3608); // #6b665c
pub const TEXT_ACTIVE: Color = Color::from_rgb(0.7725, 0.7529, 0.7137); // #c5c0b6

pub const ACCENT: Color = Color::from_rgb(0.8314, 0.6627, 0.4157); // #d4a96a
pub const ACCENT_HOVER: Color = Color::from_rgb(0.8784, 0.7451, 0.5333); // #e0be88
pub const BORDER: Color = Color::from_rgb(0.1804, 0.1686, 0.1569); // #2e2b28
pub const DANGER: Color = Color::from_rgb(0.7608, 0.4392, 0.4392); // #c27070

pub const PANE_BG: Color = Color::from_rgb(0.0745, 0.0784, 0.0863); // #131416
pub const PANE_BORDER: Color = Color::from_rgb(0.2471, 0.2235, 0.2039); // #3f3934
pub const PANE_FOCUSED_BORDER: Color = Color::from_rgb(0.8784, 0.7451, 0.5333); // #e0be88
pub const EMPTY_STATE_BG: Color = Color::from_rgb(0.1294, 0.1333, 0.1451); // #212225

pub const BACKDROP: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.58);

// --- Design system tokens (L27-L32) ---

// L27: Font families.
// Iced uses the system default sans-serif for UI chrome text.
// Terminal cells use the configured monospace font via the renderer.
// These constants document the intent for downstream usage.
pub const UI_FONT_SIZE_SM: f32 = 11.0;
pub const UI_FONT_SIZE: f32 = 13.0;
pub const UI_FONT_SIZE_LG: f32 = 15.0;

// L29: Spacing scale (4px increments).
pub const SPACE_XXS: f32 = 2.0;
pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 12.0;
pub const SPACE_LG: f32 = 16.0;
pub const SPACE_XL: f32 = 24.0;
pub const SPACE_XXL: f32 = 32.0;

// L30: Border radius scale.
pub const RADIUS_SM: f32 = 4.0;
pub const RADIUS_MD: f32 = 6.0;
pub const RADIUS_LG: f32 = 8.0;
pub const RADIUS_XL: f32 = 12.0;

// L31: Shadow/elevation for floating elements.
pub const SHADOW_COLOR: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.40);
pub const SHADOW_LIGHT: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.20);

// L32: Transition/animation timing (in milliseconds).
pub const TRANSITION_HOVER_MS: u64 = 150;
pub const TRANSITION_STATE_MS: u64 = 200;
