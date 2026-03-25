/// Bug #678: Shift+Home / Shift+End should trigger scroll-to-top / scroll-to-bottom
/// but currently return None (only Ctrl+Home / Ctrl+End are bound).
///
/// Users expect Shift+Home/End to navigate viewport like standard terminal emulators.
/// After #181 unbound plain Home/End from scroll, no Shift-based alternative was added.
use godly_app_adapter::shortcuts::{check_app_shortcut, AppAction};
use iced::keyboard::{key::Named, Key, Modifiers};

fn named_key(n: Named) -> Key {
    Key::Named(n)
}

fn shift() -> Modifiers {
    Modifiers::SHIFT
}

// Bug #678: Shift+Home should scroll viewport to top of scrollback.
// Currently returns None — the keybinding is missing.
#[test]
fn shift_home_should_scroll_to_top() {
    assert_eq!(
        check_app_shortcut(&named_key(Named::Home), shift()),
        Some(AppAction::ScrollToTop),
        "Shift+Home must trigger ScrollToTop for viewport navigation"
    );
}

// Bug #678: Shift+End should scroll viewport to bottom of scrollback.
// Currently returns None — the keybinding is missing.
#[test]
fn shift_end_should_scroll_to_bottom() {
    assert_eq!(
        check_app_shortcut(&named_key(Named::End), shift()),
        Some(AppAction::ScrollToBottom),
        "Shift+End must trigger ScrollToBottom for viewport navigation"
    );
}
