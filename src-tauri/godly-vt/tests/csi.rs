mod helpers;

#[test]
fn absolute_movement() {
    helpers::fixture("absolute_movement");
}

#[test]
fn row_clamp() {
    let mut vt = godly_vt::Parser::default();
    assert_eq!(vt.screen().cursor_position(), (0, 0));
    vt.process(b"\x1b[15d");
    assert_eq!(vt.screen().cursor_position(), (14, 0));
    vt.process(b"\x1b[150d");
    assert_eq!(vt.screen().cursor_position(), (23, 0));
}

#[test]
fn relative_movement() {
    helpers::fixture("relative_movement");
}

#[test]
fn ed() {
    helpers::fixture("ed");
}

#[test]
fn el() {
    helpers::fixture("el");
}

#[test]
fn ich_dch_ech() {
    helpers::fixture("ich_dch_ech");
}

#[test]
fn il_dl() {
    helpers::fixture("il_dl");
}

#[test]
fn scroll() {
    helpers::fixture("scroll");
}

#[test]
fn xtwinops() {
    struct Callbacks;
    impl godly_vt::Callbacks for Callbacks {
        fn resize(
            &mut self,
            screen: &mut godly_vt::Screen,
            (rows, cols): (u16, u16),
        ) {
            screen.set_size(rows, cols);
        }
    }

    let mut vt = godly_vt::Parser::new_with_callbacks(24, 80, 0, Callbacks);
    assert_eq!(vt.screen().size(), (24, 80));
    vt.process(b"\x1b[8;24;80t");
    assert_eq!(vt.screen().size(), (24, 80));
    vt.process(b"\x1b[8t");
    assert_eq!(vt.screen().size(), (24, 80));
    vt.process(b"\x1b[8;80;24t");
    assert_eq!(vt.screen().size(), (80, 24));
    vt.process(b"\x1b[8;24t");
    assert_eq!(vt.screen().size(), (24, 24));

    let mut vt = godly_vt::Parser::new_with_callbacks(24, 80, 0, Callbacks);
    assert_eq!(vt.screen().size(), (24, 80));
    vt.process(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    assert_eq!(
        vt.screen().rows(0, 80).next().unwrap(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(vt.screen().rows(0, 80).nth(1).unwrap(), "aaaaaaaaaa");
    vt.process(
        b"\x1b[H\x1b[8;24;15tbbbbbbbbbbbbbbbbbbbb\x1b[8;24;80tcccccccccccccccccccc",
    );
    assert_eq!(vt.screen().rows(0, 80).next().unwrap(), "bbbbbbbbbbbbbbb");
    assert_eq!(
        vt.screen().rows(0, 80).nth(1).unwrap(),
        "bbbbbcccccccccccccccccccc"
    );
}

#[test]
fn decscusr_cursor_style() {
    use godly_vt::CursorStyle;

    let mut parser = godly_vt::Parser::new(24, 80, 0);
    assert_eq!(parser.screen().cursor_style(), CursorStyle::BlinkBlock);

    // CSI 2 SP q → steady block
    parser.process(b"\x1b[2 q");
    assert_eq!(parser.screen().cursor_style(), CursorStyle::SteadyBlock);

    // CSI 4 SP q → steady underline
    parser.process(b"\x1b[4 q");
    assert_eq!(parser.screen().cursor_style(), CursorStyle::SteadyUnderline);

    // CSI 5 SP q → blink bar
    parser.process(b"\x1b[5 q");
    assert_eq!(parser.screen().cursor_style(), CursorStyle::BlinkBar);

    // CSI 6 SP q → steady bar
    parser.process(b"\x1b[6 q");
    assert_eq!(parser.screen().cursor_style(), CursorStyle::SteadyBar);

    // CSI 0 SP q → reset to blink block (default)
    parser.process(b"\x1b[0 q");
    assert_eq!(parser.screen().cursor_style(), CursorStyle::BlinkBlock);

    // CSI 1 SP q → blink block
    parser.process(b"\x1b[1 q");
    assert_eq!(parser.screen().cursor_style(), CursorStyle::BlinkBlock);

    // CSI 3 SP q → blink underline
    parser.process(b"\x1b[3 q");
    assert_eq!(parser.screen().cursor_style(), CursorStyle::BlinkUnderline);
}
