use iced::keyboard::{key::Named, Key, Modifiers};

/// App-level actions triggered by keyboard shortcuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    NewTab,
    CloseTab,
    NextTab,
    PreviousTab,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Copy,
    Paste,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,
}

/// Check if a key event matches an app-level shortcut.
///
/// Returns `Some(action)` if the key should be intercepted by the app,
/// or `None` if it should be forwarded to the PTY.
pub fn check_app_shortcut(key: &Key, modifiers: Modifiers) -> Option<AppAction> {
    let ctrl = modifiers.control();
    let shift = modifiers.shift();
    let alt = modifiers.alt();

    match key {
        Key::Character(ch) => {
            let s = ch.as_str();
            check_character_shortcut(s, ctrl, shift, alt)
        }
        Key::Named(named) => check_named_shortcut(named, ctrl, shift, alt),
        Key::Unidentified => None,
    }
}

/// Match character-based shortcuts (Ctrl+T, Ctrl+W, etc.).
fn check_character_shortcut(s: &str, ctrl: bool, shift: bool, alt: bool) -> Option<AppAction> {
    // All character shortcuts require Ctrl, no Alt
    if !ctrl || alt {
        return None;
    }

    match s.to_ascii_lowercase().as_str() {
        // Ctrl+T (no shift) → NewTab
        "t" if !shift => Some(AppAction::NewTab),
        // Ctrl+W (no shift) → CloseTab
        "w" if !shift => Some(AppAction::CloseTab),
        // Ctrl+= or Ctrl++ → ZoomIn
        // On US keyboard, '+' is Shift+'=', so both '=' and '+' should trigger ZoomIn.
        // Ctrl+= (no shift) or Ctrl+Shift+= (which produces '+') both work.
        "=" | "+" => Some(AppAction::ZoomIn),
        // Ctrl+- (no shift) → ZoomOut
        "-" if !shift => Some(AppAction::ZoomOut),
        // Ctrl+0 (no shift) → ZoomReset
        "0" if !shift => Some(AppAction::ZoomReset),
        // Ctrl+Shift+C → Copy
        "c" if shift => Some(AppAction::Copy),
        // Ctrl+Shift+V → Paste
        "v" if shift => Some(AppAction::Paste),
        _ => None,
    }
}

/// Match named-key shortcuts (Tab, PageUp, Home, etc.).
fn check_named_shortcut(named: &Named, ctrl: bool, shift: bool, alt: bool) -> Option<AppAction> {
    // No Alt for any of these shortcuts
    if alt {
        return None;
    }

    match named {
        // Ctrl+Tab (no shift) → NextTab
        Named::Tab if ctrl && !shift => Some(AppAction::NextTab),
        // Ctrl+Shift+Tab → PreviousTab
        Named::Tab if ctrl && shift => Some(AppAction::PreviousTab),
        // Shift+PageUp (no ctrl) → ScrollPageUp
        Named::PageUp if shift && !ctrl => Some(AppAction::ScrollPageUp),
        // Shift+PageDown (no ctrl) → ScrollPageDown
        Named::PageDown if shift && !ctrl => Some(AppAction::ScrollPageDown),
        // Ctrl+Home (no shift) → ScrollToTop
        Named::Home if ctrl && !shift => Some(AppAction::ScrollToTop),
        // Ctrl+End (no shift) → ScrollToBottom
        Named::End if ctrl && !shift => Some(AppAction::ScrollToBottom),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper constructors ──────────────────────────────────────────

    fn char_key(s: &str) -> Key {
        Key::Character(s.into())
    }

    fn named_key(n: Named) -> Key {
        Key::Named(n)
    }

    const NONE: Modifiers = Modifiers::empty();
    const CTRL: Modifiers = Modifiers::CTRL;

    fn ctrl_shift() -> Modifiers {
        Modifiers::CTRL.union(Modifiers::SHIFT)
    }

    fn shift() -> Modifiers {
        Modifiers::SHIFT
    }

    fn alt() -> Modifiers {
        Modifiers::ALT
    }

    fn ctrl_alt() -> Modifiers {
        Modifiers::CTRL.union(Modifiers::ALT)
    }

    // ── NewTab: Ctrl+T ───────────────────────────────────────────────

    #[test]
    fn ctrl_t_is_new_tab() {
        assert_eq!(
            check_app_shortcut(&char_key("t"), CTRL),
            Some(AppAction::NewTab)
        );
    }

    #[test]
    fn ctrl_uppercase_t_is_new_tab() {
        // Iced may send uppercase "T" — we match case-insensitively
        assert_eq!(
            check_app_shortcut(&char_key("T"), CTRL),
            Some(AppAction::NewTab)
        );
    }

    #[test]
    fn ctrl_shift_t_is_not_shortcut() {
        // Ctrl+Shift+T should pass through to PTY (not intercepted)
        assert_eq!(check_app_shortcut(&char_key("t"), ctrl_shift()), None);
    }

    #[test]
    fn t_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("t"), NONE), None);
    }

    #[test]
    fn shift_t_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("t"), shift()), None);
    }

    #[test]
    fn ctrl_alt_t_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("t"), ctrl_alt()), None);
    }

    // ── CloseTab: Ctrl+W ─────────────────────────────────────────────

    #[test]
    fn ctrl_w_is_close_tab() {
        assert_eq!(
            check_app_shortcut(&char_key("w"), CTRL),
            Some(AppAction::CloseTab)
        );
    }

    #[test]
    fn ctrl_uppercase_w_is_close_tab() {
        assert_eq!(
            check_app_shortcut(&char_key("W"), CTRL),
            Some(AppAction::CloseTab)
        );
    }

    #[test]
    fn ctrl_shift_w_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("w"), ctrl_shift()), None);
    }

    #[test]
    fn w_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("w"), NONE), None);
    }

    // ── NextTab: Ctrl+Tab ────────────────────────────────────────────

    #[test]
    fn ctrl_tab_is_next_tab() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::Tab), CTRL),
            Some(AppAction::NextTab)
        );
    }

    #[test]
    fn tab_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&named_key(Named::Tab), NONE), None);
    }

    #[test]
    fn shift_tab_is_not_shortcut() {
        // Shift+Tab is reverse-tab in terminal, not an app shortcut
        assert_eq!(check_app_shortcut(&named_key(Named::Tab), shift()), None);
    }

    // ── PreviousTab: Ctrl+Shift+Tab ──────────────────────────────────

    #[test]
    fn ctrl_shift_tab_is_previous_tab() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::Tab), ctrl_shift()),
            Some(AppAction::PreviousTab)
        );
    }

    // ── ZoomIn: Ctrl+= or Ctrl++ ────────────────────────────────────

    #[test]
    fn ctrl_equals_is_zoom_in() {
        assert_eq!(
            check_app_shortcut(&char_key("="), CTRL),
            Some(AppAction::ZoomIn)
        );
    }

    #[test]
    fn ctrl_plus_is_zoom_in() {
        assert_eq!(
            check_app_shortcut(&char_key("+"), CTRL),
            Some(AppAction::ZoomIn)
        );
    }

    #[test]
    fn ctrl_shift_equals_is_zoom_in() {
        // On US keyboard Ctrl+Shift+= produces "+" — but even if Iced reports
        // "=" with shift, we still accept it as ZoomIn.
        assert_eq!(
            check_app_shortcut(&char_key("="), ctrl_shift()),
            Some(AppAction::ZoomIn)
        );
    }

    #[test]
    fn equals_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("="), NONE), None);
    }

    // ── ZoomOut: Ctrl+- ──────────────────────────────────────────────

    #[test]
    fn ctrl_minus_is_zoom_out() {
        assert_eq!(
            check_app_shortcut(&char_key("-"), CTRL),
            Some(AppAction::ZoomOut)
        );
    }

    #[test]
    fn minus_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("-"), NONE), None);
    }

    #[test]
    fn ctrl_shift_minus_is_not_shortcut() {
        // Ctrl+Shift+- (underscore) should not trigger ZoomOut
        assert_eq!(check_app_shortcut(&char_key("-"), ctrl_shift()), None);
    }

    // ── ZoomReset: Ctrl+0 ────────────────────────────────────────────

    #[test]
    fn ctrl_0_is_zoom_reset() {
        assert_eq!(
            check_app_shortcut(&char_key("0"), CTRL),
            Some(AppAction::ZoomReset)
        );
    }

    #[test]
    fn zero_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("0"), NONE), None);
    }

    #[test]
    fn ctrl_shift_0_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("0"), ctrl_shift()), None);
    }

    // ── Copy: Ctrl+Shift+C ──────────────────────────────────────────

    #[test]
    fn ctrl_shift_c_is_copy() {
        assert_eq!(
            check_app_shortcut(&char_key("c"), ctrl_shift()),
            Some(AppAction::Copy)
        );
    }

    #[test]
    fn ctrl_shift_uppercase_c_is_copy() {
        assert_eq!(
            check_app_shortcut(&char_key("C"), ctrl_shift()),
            Some(AppAction::Copy)
        );
    }

    #[test]
    fn ctrl_c_alone_is_not_copy() {
        // Ctrl+C is SIGINT — must pass through to PTY, not intercept as Copy
        assert_eq!(check_app_shortcut(&char_key("c"), CTRL), None);
    }

    #[test]
    fn c_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("c"), NONE), None);
    }

    // ── Paste: Ctrl+Shift+V ─────────────────────────────────────────

    #[test]
    fn ctrl_shift_v_is_paste() {
        assert_eq!(
            check_app_shortcut(&char_key("v"), ctrl_shift()),
            Some(AppAction::Paste)
        );
    }

    #[test]
    fn ctrl_shift_uppercase_v_is_paste() {
        assert_eq!(
            check_app_shortcut(&char_key("V"), ctrl_shift()),
            Some(AppAction::Paste)
        );
    }

    #[test]
    fn ctrl_v_alone_is_not_paste() {
        // Ctrl+V should pass through to PTY (some shells use it for literal input)
        assert_eq!(check_app_shortcut(&char_key("v"), CTRL), None);
    }

    // ── ScrollPageUp: Shift+PageUp ───────────────────────────────────

    #[test]
    fn shift_pageup_is_scroll_page_up() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::PageUp), shift()),
            Some(AppAction::ScrollPageUp)
        );
    }

    #[test]
    fn pageup_alone_is_not_shortcut() {
        // PageUp without Shift goes to PTY
        assert_eq!(
            check_app_shortcut(&named_key(Named::PageUp), NONE),
            None
        );
    }

    #[test]
    fn ctrl_pageup_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::PageUp), CTRL),
            None
        );
    }

    #[test]
    fn ctrl_shift_pageup_is_not_shortcut() {
        // Only Shift+PageUp is a shortcut, not Ctrl+Shift+PageUp
        assert_eq!(
            check_app_shortcut(&named_key(Named::PageUp), ctrl_shift()),
            None
        );
    }

    // ── ScrollPageDown: Shift+PageDown ───────────────────────────────

    #[test]
    fn shift_pagedown_is_scroll_page_down() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::PageDown), shift()),
            Some(AppAction::ScrollPageDown)
        );
    }

    #[test]
    fn pagedown_alone_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::PageDown), NONE),
            None
        );
    }

    #[test]
    fn ctrl_pagedown_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::PageDown), CTRL),
            None
        );
    }

    // ── ScrollToTop: Ctrl+Home ───────────────────────────────────────

    #[test]
    fn ctrl_home_is_scroll_to_top() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::Home), CTRL),
            Some(AppAction::ScrollToTop)
        );
    }

    #[test]
    fn home_alone_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::Home), NONE),
            None
        );
    }

    #[test]
    fn shift_home_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::Home), shift()),
            None
        );
    }

    #[test]
    fn ctrl_shift_home_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::Home), ctrl_shift()),
            None
        );
    }

    // ── ScrollToBottom: Ctrl+End ─────────────────────────────────────

    #[test]
    fn ctrl_end_is_scroll_to_bottom() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::End), CTRL),
            Some(AppAction::ScrollToBottom)
        );
    }

    #[test]
    fn end_alone_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::End), NONE),
            None
        );
    }

    #[test]
    fn shift_end_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::End), shift()),
            None
        );
    }

    #[test]
    fn ctrl_shift_end_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::End), ctrl_shift()),
            None
        );
    }

    // ── Cross-cutting: unrelated keys produce None ───────────────────

    #[test]
    fn unidentified_key_is_none() {
        assert_eq!(check_app_shortcut(&Key::Unidentified, CTRL), None);
    }

    #[test]
    fn random_char_with_ctrl_is_none() {
        assert_eq!(check_app_shortcut(&char_key("x"), CTRL), None);
    }

    #[test]
    fn enter_with_ctrl_is_none() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::Enter), CTRL),
            None
        );
    }

    #[test]
    fn f1_with_ctrl_is_none() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::F1), CTRL),
            None
        );
    }

    #[test]
    fn alt_t_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("t"), alt()), None);
    }

    #[test]
    fn alt_tab_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&named_key(Named::Tab), alt()), None);
    }
}
