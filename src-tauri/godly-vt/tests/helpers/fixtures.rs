use serde::de::Deserialize as _;
use std::io::Read as _;

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct FixtureCell {
    contents: String,
    #[serde(default, skip_serializing_if = "is_default")]
    is_wide: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    is_wide_continuation: bool,
    #[serde(
        default,
        deserialize_with = "deserialize_color",
        serialize_with = "serialize_color",
        skip_serializing_if = "is_default"
    )]
    fgcolor: godly_vt::Color,
    #[serde(
        default,
        deserialize_with = "deserialize_color",
        serialize_with = "serialize_color",
        skip_serializing_if = "is_default"
    )]
    bgcolor: godly_vt::Color,
    #[serde(default, skip_serializing_if = "is_default")]
    bold: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    dim: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    italic: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    underline: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    inverse: bool,
}

impl FixtureCell {
    #[allow(dead_code)]
    pub fn from_cell(cell: &godly_vt::Cell) -> Self {
        Self {
            contents: cell.contents().to_string(),
            is_wide: cell.is_wide(),
            is_wide_continuation: cell.is_wide_continuation(),
            fgcolor: cell.fgcolor(),
            bgcolor: cell.bgcolor(),
            bold: cell.bold(),
            dim: cell.dim(),
            italic: cell.italic(),
            underline: cell.underline(),
            inverse: cell.inverse(),
        }
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct FixtureScreen {
    contents: String,
    cells: std::collections::BTreeMap<String, FixtureCell>,
    cursor_position: (u16, u16),
    #[serde(default, skip_serializing_if = "is_default")]
    application_keypad: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    application_cursor: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    hide_cursor: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    bracketed_paste: bool,
    #[serde(
        default,
        deserialize_with = "deserialize_mouse_protocol_mode",
        serialize_with = "serialize_mouse_protocol_mode",
        skip_serializing_if = "is_default"
    )]
    mouse_protocol_mode: godly_vt::MouseProtocolMode,
    #[serde(
        default,
        deserialize_with = "deserialize_mouse_protocol_encoding",
        serialize_with = "serialize_mouse_protocol_encoding",
        skip_serializing_if = "is_default"
    )]
    mouse_protocol_encoding: godly_vt::MouseProtocolEncoding,
}

impl FixtureScreen {
    fn load<R: std::io::Read>(r: R) -> Self {
        serde_json::from_reader(r).unwrap()
    }

    #[allow(dead_code)]
    pub fn from_screen(screen: &godly_vt::Screen) -> Self {
        let empty_screen = godly_vt::Parser::default().screen().clone();
        let empty_cell = empty_screen.cell(0, 0).unwrap();
        let mut cells = std::collections::BTreeMap::new();
        let (rows, cols) = screen.size();
        for row in 0..rows {
            for col in 0..cols {
                let cell = screen.cell(row, col).unwrap();
                if cell != empty_cell {
                    cells.insert(
                        format!("{row},{col}"),
                        FixtureCell::from_cell(cell),
                    );
                }
            }
        }
        Self {
            contents: screen.contents(),
            cells,
            cursor_position: screen.cursor_position(),
            application_keypad: screen.application_keypad(),
            application_cursor: screen.application_cursor(),
            hide_cursor: screen.hide_cursor(),
            bracketed_paste: screen.bracketed_paste(),
            mouse_protocol_mode: screen.mouse_protocol_mode(),
            mouse_protocol_encoding: screen.mouse_protocol_encoding(),
        }
    }
}

fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

fn deserialize_color<'a, D>(
    deserializer: D,
) -> std::result::Result<godly_vt::Color, D::Error>
where
    D: serde::de::Deserializer<'a>,
{
    let val = <Option<String>>::deserialize(deserializer)?;
    match val {
        None => Ok(godly_vt::Color::Default),
        Some(x) if x.starts_with('#') => {
            let x = x.as_bytes();
            if x.len() != 7 {
                return Err(serde::de::Error::custom("invalid rgb color"));
            }
            let r =
                super::hex(x[1], x[2]).map_err(serde::de::Error::custom)?;
            let g =
                super::hex(x[3], x[4]).map_err(serde::de::Error::custom)?;
            let b =
                super::hex(x[5], x[6]).map_err(serde::de::Error::custom)?;
            Ok(godly_vt::Color::Rgb(r, g, b))
        }
        Some(x) => Ok(godly_vt::Color::Idx(
            x.parse().map_err(serde::de::Error::custom)?,
        )),
    }
}

fn serialize_color<S>(
    color: &godly_vt::Color,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let s = match color {
        godly_vt::Color::Default => unreachable!(),
        godly_vt::Color::Idx(n) => format!("{n}"),
        godly_vt::Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
    };
    serializer.serialize_str(&s)
}

fn deserialize_mouse_protocol_mode<'a, D>(
    deserializer: D,
) -> std::result::Result<godly_vt::MouseProtocolMode, D::Error>
where
    D: serde::de::Deserializer<'a>,
{
    let name = <String>::deserialize(deserializer)?;
    match name.as_ref() {
        "none" => Ok(godly_vt::MouseProtocolMode::None),
        "press" => Ok(godly_vt::MouseProtocolMode::Press),
        "press_release" => Ok(godly_vt::MouseProtocolMode::PressRelease),
        "button_motion" => Ok(godly_vt::MouseProtocolMode::ButtonMotion),
        "any_motion" => Ok(godly_vt::MouseProtocolMode::AnyMotion),
        _ => unimplemented!(),
    }
}

fn serialize_mouse_protocol_mode<S>(
    mode: &godly_vt::MouseProtocolMode,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let s = match mode {
        godly_vt::MouseProtocolMode::None => "none",
        godly_vt::MouseProtocolMode::Press => "press",
        godly_vt::MouseProtocolMode::PressRelease => "press_release",
        godly_vt::MouseProtocolMode::ButtonMotion => "button_motion",
        godly_vt::MouseProtocolMode::AnyMotion => "any_motion",
    };
    serializer.serialize_str(s)
}

fn deserialize_mouse_protocol_encoding<'a, D>(
    deserializer: D,
) -> std::result::Result<godly_vt::MouseProtocolEncoding, D::Error>
where
    D: serde::de::Deserializer<'a>,
{
    let name = <String>::deserialize(deserializer)?;
    match name.as_ref() {
        "default" => Ok(godly_vt::MouseProtocolEncoding::Default),
        "utf8" => Ok(godly_vt::MouseProtocolEncoding::Utf8),
        "sgr" => Ok(godly_vt::MouseProtocolEncoding::Sgr),
        _ => unimplemented!(),
    }
}

fn serialize_mouse_protocol_encoding<S>(
    encoding: &godly_vt::MouseProtocolEncoding,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let s = match encoding {
        godly_vt::MouseProtocolEncoding::Default => "default",
        godly_vt::MouseProtocolEncoding::Utf8 => "utf8",
        godly_vt::MouseProtocolEncoding::Sgr => "sgr",
    };
    serializer.serialize_str(s)
}

fn load_input(name: &str, i: usize) -> Option<Vec<u8>> {
    let mut file = std::fs::File::open(format!(
        "tests/data/fixtures/{name}/{i}.typescript"
    ))
    .ok()?;
    let mut input = vec![];
    file.read_to_end(&mut input).unwrap();
    Some(input)
}

fn load_screen(name: &str, i: usize) -> Option<FixtureScreen> {
    let mut file =
        std::fs::File::open(format!("tests/data/fixtures/{name}/{i}.json"))
            .ok()?;
    Some(FixtureScreen::load(&mut file))
}

fn assert_produces(input: &[u8], expected: &FixtureScreen) {
    let mut parser = godly_vt::Parser::default();
    parser.process(input);

    assert_eq!(parser.screen().contents(), expected.contents);
    assert_eq!(parser.screen().cursor_position(), expected.cursor_position);
    assert_eq!(
        parser.screen().application_keypad(),
        expected.application_keypad
    );
    assert_eq!(
        parser.screen().application_cursor(),
        expected.application_cursor
    );
    assert_eq!(parser.screen().hide_cursor(), expected.hide_cursor);
    assert_eq!(parser.screen().bracketed_paste(), expected.bracketed_paste);
    assert_eq!(
        parser.screen().mouse_protocol_mode(),
        expected.mouse_protocol_mode
    );
    assert_eq!(
        parser.screen().mouse_protocol_encoding(),
        expected.mouse_protocol_encoding
    );

    let (rows, cols) = parser.screen().size();
    for row in 0..rows {
        for col in 0..cols {
            let expected_cell = expected
                .cells
                .get(&format!("{row},{col}"))
                .cloned()
                .unwrap_or_default();
            let got_cell = parser.screen().cell(row, col).unwrap();
            assert_eq!(got_cell.contents(), expected_cell.contents);
            assert_eq!(got_cell.is_wide(), expected_cell.is_wide);
            assert_eq!(
                got_cell.is_wide_continuation(),
                expected_cell.is_wide_continuation
            );
            assert_eq!(got_cell.fgcolor(), expected_cell.fgcolor);
            assert_eq!(got_cell.bgcolor(), expected_cell.bgcolor);
            assert_eq!(got_cell.bold(), expected_cell.bold);
            assert_eq!(got_cell.dim(), expected_cell.dim);
            assert_eq!(got_cell.italic(), expected_cell.italic);
            assert_eq!(got_cell.underline(), expected_cell.underline);
            assert_eq!(got_cell.inverse(), expected_cell.inverse);
        }
    }
}

#[allow(dead_code)]
pub fn fixture(name: &str) {
    let mut i = 1;
    let mut prev_input = vec![];
    while let Some(input) = load_input(name, i) {
        super::assert_reproduces_state_from(&input, &prev_input);
        prev_input.extend(input);

        let expected = load_screen(name, i).unwrap();
        assert_produces(&prev_input, &expected);

        i += 1;
    }
    assert!(i > 1, "couldn't find fixtures to test");
}

/// Regenerate fixture JSON files from current godly-vt behavior.
/// Call with a fixture name (e.g. "ri", "decstbm") to overwrite the
/// JSON files with the actual parser output.
#[allow(dead_code)]
pub fn regenerate_fixture(name: &str) {
    let mut i = 1;
    let mut prev_input = vec![];
    while let Some(input) = load_input(name, i) {
        prev_input.extend(input);

        let mut parser = godly_vt::Parser::default();
        parser.process(&prev_input);
        let screen = FixtureScreen::from_screen(parser.screen());

        let json = serde_json::to_string_pretty(&screen).unwrap();
        let path = format!("tests/data/fixtures/{name}/{i}.json");
        std::fs::write(&path, json).unwrap();
        eprintln!("  wrote {path}");

        i += 1;
    }
    assert!(i > 1, "couldn't find fixtures for {name}");
}
