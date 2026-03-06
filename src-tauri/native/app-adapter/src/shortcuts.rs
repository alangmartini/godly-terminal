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
    SplitRight,
    SplitDown,
    Unsplit,
    FocusNextPane,
    SelectAll,
    NextWorkspace,
    PrevWorkspace,
    ToggleSidebar,
    OpenSettings,
    RenameTab,
}

pub fn check_app_shortcut(key: &Key, modifiers: Modifiers) -> Option<AppAction> {
    let ctrl = modifiers.control();
    let shift = modifiers.shift();
    let alt = modifiers.alt();
    match key {
        Key::Character(ch) => check_character_shortcut(ch.as_str(), ctrl, shift, alt),
        Key::Named(named) => check_named_shortcut(named, ctrl, shift, alt),
        Key::Unidentified => None,
    }
}

fn check_character_shortcut(s: &str, ctrl: bool, shift: bool, alt: bool) -> Option<AppAction> {
    if alt && !ctrl && !shift {
        return match s {
            "\\" => Some(AppAction::FocusNextPane),
            _ => None,
        };
    }
    if ctrl && alt && !shift {
        return match s {
            "\\" => Some(AppAction::SplitDown),
            _ => None,
        };
    }
    if !ctrl || alt {
        return None;
    }
    if s == "\\" {
        return if !shift {
            Some(AppAction::SplitRight)
        } else {
            Some(AppAction::Unsplit)
        };
    }
    match s.to_ascii_lowercase().as_str() {
        "t" if !shift => Some(AppAction::NewTab),
        "w" if !shift => Some(AppAction::CloseTab),
        "b" if !shift => Some(AppAction::ToggleSidebar),
        "," if !shift => Some(AppAction::OpenSettings),
        "=" | "+" => Some(AppAction::ZoomIn),
        "-" if !shift => Some(AppAction::ZoomOut),
        "0" if !shift => Some(AppAction::ZoomReset),
        "c" if shift => Some(AppAction::Copy),
        "v" if shift => Some(AppAction::Paste),
        "a" if shift => Some(AppAction::SelectAll),
        _ => None,
    }
}

fn check_named_shortcut(named: &Named, ctrl: bool, shift: bool, alt: bool) -> Option<AppAction> {
    // Ctrl+Alt (no shift) — workspace navigation.
    if ctrl && alt && !shift {
        return match named {
            Named::ArrowRight => Some(AppAction::NextWorkspace),
            Named::ArrowLeft => Some(AppAction::PrevWorkspace),
            _ => None,
        };
    }
    // No modifiers — F2 rename.
    if !ctrl && !shift && !alt {
        return match named {
            Named::F2 => Some(AppAction::RenameTab),
            _ => None,
        };
    }
    if alt {
        return None;
    }
    match named {
        Named::Tab if ctrl && !shift => Some(AppAction::NextTab),
        Named::Tab if ctrl && shift => Some(AppAction::PreviousTab),
        Named::PageUp if shift && !ctrl => Some(AppAction::ScrollPageUp),
        Named::PageDown if shift && !ctrl => Some(AppAction::ScrollPageDown),
        Named::Home if ctrl && !shift => Some(AppAction::ScrollToTop),
        Named::End if ctrl && !shift => Some(AppAction::ScrollToBottom),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn alt_shift() -> Modifiers {
        Modifiers::ALT.union(Modifiers::SHIFT)
    }
    fn ctrl_alt_shift() -> Modifiers {
        Modifiers::CTRL
            .union(Modifiers::ALT)
            .union(Modifiers::SHIFT)
    }

    #[test]
    fn ctrl_t_is_new_tab() {
        assert_eq!(
            check_app_shortcut(&char_key("t"), CTRL),
            Some(AppAction::NewTab)
        );
    }
    #[test]
    fn ctrl_uppercase_t_is_new_tab() {
        assert_eq!(
            check_app_shortcut(&char_key("T"), CTRL),
            Some(AppAction::NewTab)
        );
    }
    #[test]
    fn ctrl_shift_t_is_not_shortcut() {
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
        assert_eq!(check_app_shortcut(&named_key(Named::Tab), shift()), None);
    }
    #[test]
    fn ctrl_shift_tab_is_previous_tab() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::Tab), ctrl_shift()),
            Some(AppAction::PreviousTab)
        );
    }
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
        assert_eq!(
            check_app_shortcut(&char_key("="), ctrl_shift()),
            Some(AppAction::ZoomIn)
        );
    }
    #[test]
    fn equals_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("="), NONE), None);
    }
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
        assert_eq!(check_app_shortcut(&char_key("-"), ctrl_shift()), None);
    }
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
        assert_eq!(check_app_shortcut(&char_key("c"), CTRL), None);
    }
    #[test]
    fn c_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("c"), NONE), None);
    }
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
        assert_eq!(check_app_shortcut(&char_key("v"), CTRL), None);
    }
    #[test]
    fn shift_pageup_is_scroll_page_up() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::PageUp), shift()),
            Some(AppAction::ScrollPageUp)
        );
    }
    #[test]
    fn pageup_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&named_key(Named::PageUp), NONE), None);
    }
    #[test]
    fn ctrl_pageup_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&named_key(Named::PageUp), CTRL), None);
    }
    #[test]
    fn ctrl_shift_pageup_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::PageUp), ctrl_shift()),
            None
        );
    }
    #[test]
    fn shift_pagedown_is_scroll_page_down() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::PageDown), shift()),
            Some(AppAction::ScrollPageDown)
        );
    }
    #[test]
    fn pagedown_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&named_key(Named::PageDown), NONE), None);
    }
    #[test]
    fn ctrl_pagedown_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&named_key(Named::PageDown), CTRL), None);
    }
    #[test]
    fn ctrl_home_is_scroll_to_top() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::Home), CTRL),
            Some(AppAction::ScrollToTop)
        );
    }
    #[test]
    fn home_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&named_key(Named::Home), NONE), None);
    }
    #[test]
    fn shift_home_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&named_key(Named::Home), shift()), None);
    }
    #[test]
    fn ctrl_shift_home_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::Home), ctrl_shift()),
            None
        );
    }
    #[test]
    fn ctrl_end_is_scroll_to_bottom() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::End), CTRL),
            Some(AppAction::ScrollToBottom)
        );
    }
    #[test]
    fn end_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&named_key(Named::End), NONE), None);
    }
    #[test]
    fn shift_end_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&named_key(Named::End), shift()), None);
    }
    #[test]
    fn ctrl_shift_end_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::End), ctrl_shift()),
            None
        );
    }
    #[test]
    fn ctrl_backslash_is_split_right() {
        assert_eq!(
            check_app_shortcut(&char_key("\\"), CTRL),
            Some(AppAction::SplitRight)
        );
    }
    #[test]
    fn backslash_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("\\"), NONE), None);
    }
    #[test]
    fn shift_backslash_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("\\"), shift()), None);
    }
    #[test]
    fn ctrl_alt_backslash_is_split_down() {
        assert_eq!(
            check_app_shortcut(&char_key("\\"), ctrl_alt()),
            Some(AppAction::SplitDown)
        );
    }
    #[test]
    fn ctrl_alt_shift_backslash_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("\\"), ctrl_alt_shift()), None);
    }
    #[test]
    fn ctrl_shift_backslash_is_unsplit() {
        assert_eq!(
            check_app_shortcut(&char_key("\\"), ctrl_shift()),
            Some(AppAction::Unsplit)
        );
    }
    #[test]
    fn alt_backslash_is_focus_next_pane() {
        assert_eq!(
            check_app_shortcut(&char_key("\\"), alt()),
            Some(AppAction::FocusNextPane)
        );
    }
    #[test]
    fn alt_shift_backslash_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("\\"), alt_shift()), None);
    }
    #[test]
    fn ctrl_shift_a_is_select_all() {
        assert_eq!(
            check_app_shortcut(&char_key("a"), ctrl_shift()),
            Some(AppAction::SelectAll)
        );
    }
    #[test]
    fn ctrl_shift_uppercase_a_is_select_all() {
        assert_eq!(
            check_app_shortcut(&char_key("A"), ctrl_shift()),
            Some(AppAction::SelectAll)
        );
    }
    #[test]
    fn ctrl_a_alone_is_not_select_all() {
        assert_eq!(check_app_shortcut(&char_key("a"), CTRL), None);
    }
    #[test]
    fn a_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("a"), NONE), None);
    }
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
        assert_eq!(check_app_shortcut(&named_key(Named::Enter), CTRL), None);
    }
    #[test]
    fn f1_with_ctrl_is_none() {
        assert_eq!(check_app_shortcut(&named_key(Named::F1), CTRL), None);
    }
    #[test]
    fn alt_t_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("t"), alt()), None);
    }
    #[test]
    fn alt_tab_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&named_key(Named::Tab), alt()), None);
    }
    // --- Workspace shortcuts ---
    #[test]
    fn ctrl_alt_right_is_next_workspace() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::ArrowRight), ctrl_alt()),
            Some(AppAction::NextWorkspace)
        );
    }
    #[test]
    fn ctrl_alt_left_is_prev_workspace() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::ArrowLeft), ctrl_alt()),
            Some(AppAction::PrevWorkspace)
        );
    }
    #[test]
    fn ctrl_alt_shift_right_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::ArrowRight), ctrl_alt_shift()),
            None
        );
    }
    #[test]
    fn ctrl_alt_shift_left_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::ArrowLeft), ctrl_alt_shift()),
            None
        );
    }
    #[test]
    fn ctrl_right_alone_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::ArrowRight), CTRL),
            None
        );
    }
    #[test]
    fn alt_right_alone_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::ArrowRight), alt()),
            None
        );
    }
    #[test]
    fn right_alone_is_not_shortcut() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::ArrowRight), NONE),
            None
        );
    }
    // --- Sidebar toggle ---
    #[test]
    fn ctrl_b_is_toggle_sidebar() {
        assert_eq!(
            check_app_shortcut(&char_key("b"), CTRL),
            Some(AppAction::ToggleSidebar)
        );
    }
    #[test]
    fn ctrl_uppercase_b_is_toggle_sidebar() {
        assert_eq!(
            check_app_shortcut(&char_key("B"), CTRL),
            Some(AppAction::ToggleSidebar)
        );
    }
    #[test]
    fn ctrl_shift_b_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("b"), ctrl_shift()), None);
    }
    #[test]
    fn b_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key("b"), NONE), None);
    }
    // --- Settings ---
    #[test]
    fn ctrl_comma_is_open_settings() {
        assert_eq!(
            check_app_shortcut(&char_key(","), CTRL),
            Some(AppAction::OpenSettings)
        );
    }
    #[test]
    fn ctrl_shift_comma_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key(","), ctrl_shift()), None);
    }
    #[test]
    fn comma_alone_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&char_key(","), NONE), None);
    }
    // --- Rename tab ---
    #[test]
    fn f2_is_rename_tab() {
        assert_eq!(
            check_app_shortcut(&named_key(Named::F2), NONE),
            Some(AppAction::RenameTab)
        );
    }
    #[test]
    fn ctrl_f2_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&named_key(Named::F2), CTRL), None);
    }
    #[test]
    fn shift_f2_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&named_key(Named::F2), shift()), None);
    }
    #[test]
    fn alt_f2_is_not_shortcut() {
        assert_eq!(check_app_shortcut(&named_key(Named::F2), alt()), None);
    }
}
