// Originally from vt100-rust by Jesse Luehrs (https://github.com/doy/vt100-rust)
// Copyright (c) Jesse Luehrs
// Licensed under MIT — see LICENSE-vt100 in this crate
// Modified for godly-vt

use unicode_width::UnicodeWidthChar as _;

// chosen to make the size of the cell struct 32 bytes (text-only path)
const CONTENT_BYTES: usize = 22;

const IS_WIDE: u8 = 0b1000_0000;
const IS_WIDE_CONTINUATION: u8 = 0b0100_0000;
const LEN_BITS: u8 = 0b0001_1111;

/// What a cell contains.
///
/// Most cells are text characters. When the `images` feature is enabled,
/// cells can also hold image fragments from inline image protocols
/// (Kitty, iTerm2, Sixel).
#[derive(Clone, Debug)]
pub enum CellContent {
    /// Normal text character(s), stored inline as UTF-8 bytes.
    /// This is the default and most common variant.
    Text,
    /// Part of an inline image. The cell displays a region of a larger image.
    #[cfg(feature = "images")]
    ImageFragment(crate::image::ImageCellRef),
}

impl PartialEq for CellContent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (CellContent::Text, CellContent::Text) => true,
            #[cfg(feature = "images")]
            (CellContent::ImageFragment(a), CellContent::ImageFragment(b)) => a == b,
            #[cfg(feature = "images")]
            _ => false,
        }
    }
}

impl Eq for CellContent {}

impl Default for CellContent {
    fn default() -> Self {
        CellContent::Text
    }
}

/// Represents a single terminal cell.
///
/// Each cell holds either text content (the common case) or an image
/// fragment reference. Text content is stored inline as UTF-8 bytes
/// with no heap allocation.
#[derive(Clone, Debug)]
pub struct Cell {
    /// Inline UTF-8 text storage. Only used when content is Text.
    contents: [u8; CONTENT_BYTES],
    /// Packed length + wide flags. Only used when content is Text.
    len: u8,
    /// Cell attributes (colors, bold, etc.). Used for all content types.
    attrs: crate::attrs::Attrs,
    /// What type of content this cell holds.
    content_type: CellContent,
}

impl Eq for Cell {}

impl PartialEq<Self> for Cell {
    fn eq(&self, other: &Self) -> bool {
        if self.content_type != other.content_type {
            return false;
        }
        if self.attrs != other.attrs {
            return false;
        }
        match &self.content_type {
            CellContent::Text => {
                if self.len != other.len {
                    return false;
                }
                let len = self.len();
                self.contents[..len] == other.contents[..len]
            },
            #[cfg(feature = "images")]
            CellContent::ImageFragment(_) => {
                // ImageFragment equality already checked via content_type
                true
            },
        }
    }
}

impl Cell {
    pub(crate) fn new() -> Self {
        Self {
            contents: Default::default(),
            len: 0,
            attrs: crate::attrs::Attrs::default(),
            content_type: CellContent::Text,
        }
    }

    fn len(&self) -> usize {
        usize::from(self.len & LEN_BITS)
    }

    pub(crate) fn set(&mut self, c: char, a: crate::attrs::Attrs) {
        self.content_type = CellContent::Text;
        self.len = 0;
        self.append_char(0, c);
        // strings in this context should always be an arbitrary character
        // followed by zero or more zero-width characters, so we should only
        // have to look at the first character
        self.set_wide(c.width().unwrap_or(1) > 1);
        self.attrs = a;
    }

    pub(crate) fn append(&mut self, c: char) {
        // Only append to text cells
        if !matches!(self.content_type, CellContent::Text) {
            return;
        }
        let len = self.len();
        if len >= CONTENT_BYTES - 4 {
            return;
        }
        if len == 0 {
            self.contents[0] = b' ';
            self.len += 1;
        }

        // we already checked that we have space for another codepoint
        self.append_char(self.len(), c);
    }

    // Writes bytes representing c at start
    // Requires caller to verify start <= CODEPOINTS_IN_CELL * 4
    fn append_char(&mut self, start: usize, c: char) {
        c.encode_utf8(&mut self.contents[start..]);
        self.len += u8::try_from(c.len_utf8()).unwrap();
    }

    pub(crate) fn clear(&mut self, attrs: crate::attrs::Attrs) {
        self.content_type = CellContent::Text;
        self.len = 0;
        self.attrs = attrs;
    }

    /// Returns the text contents of the cell.
    ///
    /// Can include multiple unicode characters if combining characters are
    /// used, but will contain at most one character with a non-zero character
    /// width.
    ///
    /// Returns an empty string for image fragment cells.
    // Since contents has been constructed by appending chars encoded as UTF-8 it will be valid UTF-8
    #[allow(clippy::missing_panics_doc)]
    #[must_use]
    pub fn contents(&self) -> &str {
        match &self.content_type {
            CellContent::Text => {
                std::str::from_utf8(&self.contents[..self.len()]).unwrap()
            },
            #[cfg(feature = "images")]
            CellContent::ImageFragment(_) => "",
        }
    }

    /// Returns whether the cell contains any text data.
    #[must_use]
    pub fn has_contents(&self) -> bool {
        match &self.content_type {
            CellContent::Text => self.len() > 0,
            #[cfg(feature = "images")]
            CellContent::ImageFragment(_) => true,
        }
    }

    /// Returns what type of content this cell holds.
    #[must_use]
    pub fn content_type(&self) -> &CellContent {
        &self.content_type
    }

    /// Returns whether this cell contains an image fragment.
    #[must_use]
    pub fn is_image(&self) -> bool {
        #[cfg(feature = "images")]
        {
            matches!(self.content_type, CellContent::ImageFragment(_))
        }
        #[cfg(not(feature = "images"))]
        {
            false
        }
    }

    /// Returns the image cell reference, if this cell is an image fragment.
    #[cfg(feature = "images")]
    #[must_use]
    pub fn image_ref(&self) -> Option<&crate::image::ImageCellRef> {
        match &self.content_type {
            CellContent::ImageFragment(r) => Some(r),
            _ => None,
        }
    }

    /// Set this cell to display an image fragment.
    #[cfg(feature = "images")]
    #[allow(dead_code)]
    pub(crate) fn set_image(
        &mut self,
        image_ref: crate::image::ImageCellRef,
        attrs: crate::attrs::Attrs,
    ) {
        self.content_type = CellContent::ImageFragment(image_ref);
        self.len = 0;
        self.contents = Default::default();
        self.attrs = attrs;
    }

    /// Returns whether the text data in the cell represents a wide character.
    #[must_use]
    pub fn is_wide(&self) -> bool {
        self.len & IS_WIDE != 0
    }

    /// Returns whether the cell contains the second half of a wide character
    /// (in other words, whether the previous cell in the row contains a wide
    /// character)
    #[must_use]
    pub fn is_wide_continuation(&self) -> bool {
        self.len & IS_WIDE_CONTINUATION != 0
    }

    fn set_wide(&mut self, wide: bool) {
        if wide {
            self.len |= IS_WIDE;
        } else {
            self.len &= !IS_WIDE;
        }
    }

    pub(crate) fn set_wide_continuation(&mut self, wide: bool) {
        if wide {
            self.len |= IS_WIDE_CONTINUATION;
        } else {
            self.len &= !IS_WIDE_CONTINUATION;
        }
    }

    pub(crate) fn attrs(&self) -> &crate::attrs::Attrs {
        &self.attrs
    }

    /// Returns the foreground color of the cell.
    #[must_use]
    pub fn fgcolor(&self) -> crate::Color {
        self.attrs.fgcolor
    }

    /// Returns the background color of the cell.
    #[must_use]
    pub fn bgcolor(&self) -> crate::Color {
        self.attrs.bgcolor
    }

    /// Returns whether the cell should be rendered with the bold text
    /// attribute.
    #[must_use]
    pub fn bold(&self) -> bool {
        self.attrs.bold()
    }

    /// Returns whether the cell should be rendered with the dim text
    /// attribute.
    #[must_use]
    pub fn dim(&self) -> bool {
        self.attrs.dim()
    }

    /// Returns whether the cell should be rendered with the italic text
    /// attribute.
    #[must_use]
    pub fn italic(&self) -> bool {
        self.attrs.italic()
    }

    /// Returns whether the cell should be rendered with the underlined text
    /// attribute.
    #[must_use]
    pub fn underline(&self) -> bool {
        self.attrs.underline()
    }

    /// Returns whether the cell should be rendered with the inverse text
    /// attribute.
    #[must_use]
    pub fn inverse(&self) -> bool {
        self.attrs.inverse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_default_is_text() {
        let cell = Cell::new();
        assert!(matches!(cell.content_type(), CellContent::Text));
        assert!(!cell.has_contents());
        assert!(!cell.is_image());
        assert_eq!(cell.contents(), "");
    }

    #[test]
    fn test_cell_set_text() {
        let mut cell = Cell::new();
        cell.set('A', crate::attrs::Attrs::default());
        assert!(matches!(cell.content_type(), CellContent::Text));
        assert!(cell.has_contents());
        assert_eq!(cell.contents(), "A");
    }

    #[test]
    fn test_cell_clear_resets_to_text() {
        let mut cell = Cell::new();
        cell.set('X', crate::attrs::Attrs::default());
        cell.clear(crate::attrs::Attrs::default());
        assert!(matches!(cell.content_type(), CellContent::Text));
        assert!(!cell.has_contents());
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_cell_set_image() {
        let mut cell = Cell::new();
        let img_ref = crate::image::ImageCellRef {
            image_hash: 42,
            placement_id: 1,
            tex_x: 0.0,
            tex_y: 0.0,
            tex_w: 0.5,
            tex_h: 0.5,
            z_index: 0,
        };
        cell.set_image(img_ref.clone(), crate::attrs::Attrs::default());

        assert!(cell.is_image());
        assert!(cell.has_contents());
        assert_eq!(cell.contents(), "");
        assert_eq!(cell.image_ref(), Some(&img_ref));
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_cell_clear_image_to_text() {
        let mut cell = Cell::new();
        let img_ref = crate::image::ImageCellRef {
            image_hash: 42,
            placement_id: 1,
            tex_x: 0.0,
            tex_y: 0.0,
            tex_w: 1.0,
            tex_h: 1.0,
            z_index: 0,
        };
        cell.set_image(img_ref, crate::attrs::Attrs::default());
        assert!(cell.is_image());

        cell.clear(crate::attrs::Attrs::default());
        assert!(!cell.is_image());
        assert!(matches!(cell.content_type(), CellContent::Text));
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_cell_set_text_over_image() {
        let mut cell = Cell::new();
        let img_ref = crate::image::ImageCellRef {
            image_hash: 42,
            placement_id: 1,
            tex_x: 0.0,
            tex_y: 0.0,
            tex_w: 1.0,
            tex_h: 1.0,
            z_index: 0,
        };
        cell.set_image(img_ref, crate::attrs::Attrs::default());
        assert!(cell.is_image());

        // Writing text over an image cell should replace it
        cell.set('B', crate::attrs::Attrs::default());
        assert!(!cell.is_image());
        assert_eq!(cell.contents(), "B");
    }

    #[cfg(feature = "images")]
    #[test]
    fn test_cell_append_ignored_for_image() {
        let mut cell = Cell::new();
        let img_ref = crate::image::ImageCellRef {
            image_hash: 42,
            placement_id: 1,
            tex_x: 0.0,
            tex_y: 0.0,
            tex_w: 1.0,
            tex_h: 1.0,
            z_index: 0,
        };
        cell.set_image(img_ref, crate::attrs::Attrs::default());

        // Appending combining characters to an image cell should be a no-op
        cell.append('\u{0301}');
        assert!(cell.is_image());
    }
}
