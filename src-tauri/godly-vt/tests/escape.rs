mod helpers;

#[test]
fn deckpam() {
    helpers::fixture("deckpam");
}

#[test]
fn ri() {
    helpers::fixture("ri");
}

#[test]
fn ris() {
    helpers::fixture("ris");
}

#[test]
fn vb() {
    struct State {
        vb: usize,
    }

    impl godly_vt::Callbacks for State {
        fn visual_bell(&mut self, _: &mut godly_vt::Screen) {
            self.vb += 1;
        }
    }

    let mut parser =
        godly_vt::Parser::new_with_callbacks(24, 80, 0, State { vb: 0 });
    assert_eq!(parser.callbacks().vb, 0);

    let screen = parser.screen().clone();
    parser.process(b"\x1bg");
    assert_eq!(parser.callbacks().vb, 1);
    assert_eq!(parser.screen().contents_diff(&screen), b"");

    let screen = parser.screen().clone();
    parser.process(b"\x1bg");
    assert_eq!(parser.callbacks().vb, 2);
    assert_eq!(parser.screen().contents_diff(&screen), b"");

    let screen = parser.screen().clone();
    parser.process(b"\x1bg\x1bg\x1bg");
    assert_eq!(parser.callbacks().vb, 5);
    assert_eq!(parser.screen().contents_diff(&screen), b"");

    let screen = parser.screen().clone();
    parser.process(b"foo");
    assert_eq!(parser.callbacks().vb, 5);
    assert_eq!(parser.screen().contents_diff(&screen), b"foo");

    let screen = parser.screen().clone();
    parser.process(b"ba\x1bgr");
    assert_eq!(parser.callbacks().vb, 6);
    assert_eq!(parser.screen().contents_diff(&screen), b"bar");
}

#[test]
fn bell_pending_set_on_bel_char() {
    // BEL (0x07) should set the bell_pending flag
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // Initially no bell pending
    assert!(!parser.take_bell_pending());

    // Process BEL character
    parser.process(b"\x07");
    assert!(parser.take_bell_pending());

    // take_bell_pending clears the flag
    assert!(!parser.take_bell_pending());
}

#[test]
fn bell_pending_cleared_after_take() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    // Multiple BELs in one process call — flag should still be set once
    parser.process(b"\x07\x07\x07");
    assert!(parser.take_bell_pending());
    // After taking, should be cleared
    assert!(!parser.take_bell_pending());
}

#[test]
fn bell_pending_survives_mixed_output() {
    // BEL mixed with normal text output
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    parser.process(b"hello\x07world");
    assert!(parser.take_bell_pending());
    assert_eq!(parser.screen().contents(), "helloworld");
}

#[test]
fn bell_pending_not_set_by_normal_text() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    parser.process(b"just normal text");
    assert!(!parser.take_bell_pending());
}

#[test]
fn bell_pending_not_set_by_visual_bell() {
    // Visual bell (ESC g) should NOT set bell_pending — only audible BEL (0x07) does
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    parser.process(b"\x1bg");
    assert!(!parser.take_bell_pending());
}

#[test]
fn decsc() {
    helpers::fixture("decsc");
}

#[test]
fn decsc_resize() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);
    parser.process(b"foo\x1b[20;70Hbar\x1b7");
    assert_eq!(parser.screen().contents(), "foo\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n                                                                     bar");
    assert_eq!(parser.screen().cursor_position(), (19, 72));
    parser.process(b"\x1b[H");
    assert_eq!(parser.screen().cursor_position(), (0, 0));
    parser.screen_mut().set_size(15, 60);
    assert_eq!(parser.screen().contents(), "foo");
    assert_eq!(parser.screen().cursor_position(), (0, 0));
    parser.process(b"y\x1b8z");
    assert_eq!(parser.screen().contents(), "yoo\n\n\n\n\n\n\n\n\n\n\n\n\n\n                                                           z");
    assert_eq!(parser.screen().cursor_position(), (14, 60));
}
