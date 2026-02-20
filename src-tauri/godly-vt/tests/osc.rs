mod helpers;

#[test]
fn title_icon_name() {
    #[derive(Default)]
    struct Window {
        title: String,
        icon_name: String,
    }
    impl godly_vt::Callbacks for Window {
        fn set_window_icon_name(
            &mut self,
            _: &mut godly_vt::Screen,
            icon_name: &[u8],
        ) {
            self.icon_name =
                std::str::from_utf8(icon_name).unwrap().to_string();
        }
        fn set_window_title(&mut self, _: &mut godly_vt::Screen, title: &[u8]) {
            self.title = std::str::from_utf8(title).unwrap().to_string();
        }
    }

    let mut parser =
        godly_vt::Parser::new_with_callbacks(24, 80, 0, Window::default());
    assert_eq!(parser.callbacks().icon_name, "");
    assert_eq!(parser.callbacks().title, "");
    parser.process(b"\x1b]1;icon_name\x07");
    assert_eq!(parser.callbacks().icon_name, "icon_name");
    assert_eq!(parser.callbacks().title, "");
    parser.process(b"\x1b]2;title\x07");
    assert_eq!(parser.callbacks().icon_name, "icon_name");
    assert_eq!(parser.callbacks().title, "title");
    parser.process(b"\x1b]0;both\x07");
    assert_eq!(parser.callbacks().icon_name, "both");
    assert_eq!(parser.callbacks().title, "both");
}

#[test]
fn clipboard() {
    #[derive(Default)]
    struct Clipboard {
        clipboard: std::collections::HashMap<Vec<u8>, Vec<u8>>,
        pasted: Vec<Vec<u8>>,
    }
    impl godly_vt::Callbacks for Clipboard {
        fn copy_to_clipboard(
            &mut self,
            _: &mut godly_vt::Screen,
            ty: &[u8],
            data: &[u8],
        ) {
            self.clipboard.insert(ty.to_vec(), data.to_vec());
        }

        fn paste_from_clipboard(&mut self, _: &mut godly_vt::Screen, ty: &[u8]) {
            self.pasted.push(ty.to_vec());
        }

        fn unhandled_osc(&mut self, _: &mut godly_vt::Screen, params: &[&[u8]]) {
            panic!("unhandled osc: {params:?}");
        }
    }

    let mut parser =
        godly_vt::Parser::new_with_callbacks(24, 80, 0, Clipboard::default());
    assert!(parser.callbacks().clipboard.is_empty());
    assert!(parser.callbacks().pasted.is_empty());
    parser.process(b"\x1b]52;c;?\x07");
    assert!(parser.callbacks().clipboard.is_empty());
    assert_eq!(&parser.callbacks().pasted, &[b"c"]);
    parser.process(b"\x1b]52;c;abcdef==\x07");
    assert_eq!(parser.callbacks().clipboard.len(), 1);
    assert_eq!(
        parser.callbacks().clipboard.get(&b"c"[..]),
        Some(&b"abcdef==".to_vec())
    );
    assert_eq!(&parser.callbacks().pasted, &[b"c"]);
}

/// Verify that OSC title/icon_name are stored on Screen (not just callbacks).
/// This is the fix for Bug #182: the daemon uses Parser::new() (no callbacks),
/// so titles must be accessible via screen.window_title().
#[test]
fn title_stored_on_screen() {
    let mut parser = godly_vt::Parser::new(24, 80, 0);

    assert_eq!(parser.screen().window_title(), "");
    assert_eq!(parser.screen().window_icon_name(), "");

    // OSC 2 sets window title
    parser.process(b"\x1b]2;my title\x07");
    assert_eq!(parser.screen().window_title(), "my title");
    assert_eq!(parser.screen().window_icon_name(), "");

    // OSC 1 sets icon name
    parser.process(b"\x1b]1;my icon\x07");
    assert_eq!(parser.screen().window_title(), "my title");
    assert_eq!(parser.screen().window_icon_name(), "my icon");

    // OSC 0 sets both
    parser.process(b"\x1b]0;both values\x07");
    assert_eq!(parser.screen().window_title(), "both values");
    assert_eq!(parser.screen().window_icon_name(), "both values");

    // Title with ST terminator (ESC \)
    parser.process(b"\x1b]2;st title\x1b\\");
    assert_eq!(parser.screen().window_title(), "st title");

    // Empty title clears
    parser.process(b"\x1b]2;\x07");
    assert_eq!(parser.screen().window_title(), "");
}

#[test]
fn unknown_osc() {
    helpers::fixture("unknown_osc");
}
