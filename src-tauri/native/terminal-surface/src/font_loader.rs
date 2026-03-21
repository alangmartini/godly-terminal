use fontdb::{Database, Query, Style, Weight};

/// System font loader backed by `fontdb`.
///
/// Queries the operating system's font database to locate and load font files
/// by family name. Used to initialize the `SwashRasterizer` with the user's
/// chosen monospace font or a fallback.
pub struct FontLoader {
    db: Database,
}

impl FontLoader {
    /// Create a new font loader with system fonts pre-loaded.
    pub fn new() -> Self {
        let mut db = Database::new();
        db.load_system_fonts();
        Self { db }
    }

    /// Load font bytes for the given family name and style.
    ///
    /// Returns `None` if no matching font is found.
    pub fn load_font(&self, family: &str, bold: bool, italic: bool) -> Option<Vec<u8>> {
        let weight = if bold { Weight::BOLD } else { Weight::NORMAL };
        let style = if italic { Style::Italic } else { Style::Normal };

        let query = Query {
            families: &[fontdb::Family::Name(family)],
            weight,
            style,
            stretch: fontdb::Stretch::Normal,
        };

        let face_id = self.db.query(&query)?;
        let mut result = None;
        self.db
            .with_face_data(face_id, |data, _index| {
                result = Some(data.to_vec());
            })?;
        result
    }
}
