use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiFontKind {
    Sans,
    Serif,
    Mono,
}

/// CSS-style font weight matching the web reference values.
/// Maps directly to DirectWrite `DWRITE_FONT_WEIGHT_*` constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum FontWeight {
    Regular = 400,
    Medium = 500,
    SemiBold = 600,
    Bold = 700,
}

impl FontWeight {
    pub fn from_bold(bold: bool) -> Self {
        if bold { FontWeight::Bold } else { FontWeight::Regular }
    }

    /// Returns `true` for weights that should select the bold font face
    /// in rasterizers that only distinguish regular vs bold (e.g. GlyphKey).
    pub fn is_bold(self) -> bool {
        (self as u16) >= 600
    }
}

#[derive(Debug, Clone)]
pub struct UiTextLayout {
    pub width: f32,
    pub line_height: f32,
    pub glyph_offsets: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct UiFontFamilies {
    pub sans: String,
    pub serif: String,
    pub mono: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct LayoutKey {
    font: UiFontKind,
    text: String,
    size_q4: u16,
    weight: FontWeight,
    italic: bool,
}

#[cfg(windows)]
struct LayoutCache {
    factory: windows::Win32::Graphics::DirectWrite::IDWriteFactory,
    families: UiFontFamilies,
    cache: HashMap<LayoutKey, UiTextLayout>,
}

pub struct UiTextLayoutEngine {
    #[cfg(windows)]
    inner: RefCell<LayoutCache>,
}

impl UiTextLayoutEngine {
    #[cfg(windows)]
    pub fn new(families: UiFontFamilies) -> windows::core::Result<Self> {
        use windows::Win32::Graphics::DirectWrite::{
            DWriteCreateFactory, IDWriteFactory, DWRITE_FACTORY_TYPE_SHARED,
        };

        let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        Ok(Self {
            inner: RefCell::new(LayoutCache {
                factory,
                families,
                cache: HashMap::new(),
            }),
        })
    }

    #[cfg(not(windows))]
    pub fn new(_families: UiFontFamilies) -> Result<Self, ()> {
        Ok(Self {})
    }

    /// Layout text with a boolean bold flag (backward compat).
    pub fn layout(
        &self,
        font: UiFontKind,
        text: &str,
        font_size_px: f32,
        bold: bool,
        italic: bool,
    ) -> Option<UiTextLayout> {
        self.layout_weighted(font, text, font_size_px, FontWeight::from_bold(bold), italic)
    }

    /// Layout text with a specific CSS-style font weight.
    pub fn layout_weighted(
        &self,
        font: UiFontKind,
        text: &str,
        font_size_px: f32,
        weight: FontWeight,
        italic: bool,
    ) -> Option<UiTextLayout> {
        #[cfg(windows)]
        {
            let key = LayoutKey {
                font,
                text: text.to_string(),
                size_q4: (font_size_px.max(0.0) * 4.0).round() as u16,
                weight,
                italic,
            };
            let mut inner = self.inner.borrow_mut();
            if let Some(layout) = inner.cache.get(&key) {
                return Some(layout.clone());
            }
            let layout = compute_layout(&inner.factory, &inner.families, &key)?;
            inner.cache.insert(key, layout.clone());
            Some(layout)
        }
        #[cfg(not(windows))]
        {
            let _ = (font, text, font_size_px, weight, italic);
            None
        }
    }
}

#[cfg(windows)]
fn compute_layout(
    factory: &windows::Win32::Graphics::DirectWrite::IDWriteFactory,
    families: &UiFontFamilies,
    key: &LayoutKey,
) -> Option<UiTextLayout> {
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::DirectWrite::{
        DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_WEIGHT, DWRITE_HIT_TEST_METRICS,
        DWRITE_TEXT_METRICS, DWRITE_WORD_WRAPPING_NO_WRAP,
    };

    let family = match key.font {
        UiFontKind::Sans => &families.sans,
        UiFontKind::Serif => &families.serif,
        UiFontKind::Mono => &families.mono,
    };
    let family_wide = wide(family);
    let locale = wide("en-US");
    let utf16: Vec<u16> = key.text.encode_utf16().collect();

    let weight = DWRITE_FONT_WEIGHT(key.weight as i32);
    let style = if key.italic {
        DWRITE_FONT_STYLE_ITALIC
    } else {
        DWRITE_FONT_STYLE_NORMAL
    };
    let font_size_px = key.size_q4 as f32 / 4.0;

    let format = unsafe {
        factory
            .CreateTextFormat(
                PCWSTR(family_wide.as_ptr()),
                None,
                weight,
                style,
                DWRITE_FONT_STRETCH_NORMAL,
                font_size_px,
                PCWSTR(locale.as_ptr()),
            )
            .ok()?
    };
    let _ = unsafe { format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP) };

    let layout = unsafe {
        factory
            .CreateTextLayout(&utf16, &format, 4096.0, 4096.0)
            .ok()?
    };

    let mut metrics = DWRITE_TEXT_METRICS::default();
    unsafe { layout.GetMetrics(&mut metrics).ok()? };

    let mut glyph_offsets = Vec::with_capacity(key.text.chars().count());
    let mut utf16_index = 0u32;
    for ch in key.text.chars() {
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut hit = DWRITE_HIT_TEST_METRICS::default();
        unsafe {
            layout
                .HitTestTextPosition(utf16_index, false, &mut x, &mut y, &mut hit)
                .ok()?;
        }
        glyph_offsets.push(x);
        utf16_index += ch.len_utf16() as u32;
    }

    Some(UiTextLayout {
        width: metrics.widthIncludingTrailingWhitespace.max(metrics.width),
        line_height: metrics.height.max(font_size_px),
        glyph_offsets,
    })
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
