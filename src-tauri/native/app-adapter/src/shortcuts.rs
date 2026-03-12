use std::collections::{HashMap, HashSet};

use iced::keyboard::{key::Named, Key, Modifiers};

/// App-level actions triggered by keyboard shortcuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    TogglePerfOverlay,
    Find,
    WhisperToggle,
}

/// Flat index order matching the categories in `shortcuts_tab.rs`.
///
/// Tabs:       0=NewTab, 1=CloseTab, 2=NextTab, 3=PreviousTab, 4=RenameTab
/// Split:      5=SplitRight, 6=SplitDown, 7=Unsplit, 8=FocusNextPane
/// Clipboard:  9=Copy, 10=Paste, 11=SelectAll
/// Scrollback: 12=ScrollPageUp, 13=ScrollPageDown, 14=ScrollToTop, 15=ScrollToBottom
/// Zoom:       16=ZoomIn, 17=ZoomOut, 18=ZoomReset
/// Workspaces: 19=NextWorkspace, 20=PrevWorkspace, 21=ToggleSidebar, 22=OpenSettings
const FLAT_ACTION_ORDER: &[AppAction] = &[
    // Tabs
    AppAction::NewTab,
    AppAction::CloseTab,
    AppAction::NextTab,
    AppAction::PreviousTab,
    AppAction::RenameTab,
    // Split Panes
    AppAction::SplitRight,
    AppAction::SplitDown,
    AppAction::Unsplit,
    AppAction::FocusNextPane,
    // Clipboard
    AppAction::Copy,
    AppAction::Paste,
    AppAction::SelectAll,
    // Scrollback
    AppAction::ScrollPageUp,
    AppAction::ScrollPageDown,
    AppAction::ScrollToTop,
    AppAction::ScrollToBottom,
    // Zoom
    AppAction::ZoomIn,
    AppAction::ZoomOut,
    AppAction::ZoomReset,
    // Workspaces
    AppAction::NextWorkspace,
    AppAction::PrevWorkspace,
    AppAction::ToggleSidebar,
    AppAction::OpenSettings,
];

/// Maps a flat category index to the corresponding `AppAction`.
pub fn flat_index_to_action(index: usize) -> Option<AppAction> {
    FLAT_ACTION_ORDER.get(index).copied()
}

// ---------------------------------------------------------------------------
// Chord normalisation
// ---------------------------------------------------------------------------

/// Normalises a key+modifiers into a canonical display string like `"Ctrl+Shift+/"`.
///
/// This MUST produce the same output as `format_key_chord` in `app.rs` so that
/// the chord captured during rebinding matches the chord produced at resolution
/// time.
pub fn normalize_chord(key: &Key, modifiers: Modifiers) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if modifiers.control() {
        parts.push("Ctrl");
    }
    if modifiers.shift() {
        parts.push("Shift");
    }
    if modifiers.alt() {
        parts.push("Alt");
    }
    match key {
        Key::Character(ch) => {
            let upper = ch.as_str().to_uppercase();
            let prefix = parts.join("+");
            if prefix.is_empty() {
                upper
            } else {
                format!("{prefix}+{upper}")
            }
        }
        Key::Named(named) => {
            let name = named_key_label(named);
            parts.push(name);
            parts.join("+")
        }
        Key::Unidentified => {
            parts.push("?");
            parts.join("+")
        }
    }
}

fn named_key_label(named: &Named) -> &'static str {
    match named {
        Named::Tab => "Tab",
        Named::Enter => "Enter",
        Named::Space => "Space",
        Named::Backspace => "Backspace",
        Named::Delete => "Delete",
        Named::Home => "Home",
        Named::End => "End",
        Named::PageUp => "PageUp",
        Named::PageDown => "PageDown",
        Named::ArrowUp => "Up",
        Named::ArrowDown => "Down",
        Named::ArrowLeft => "Left",
        Named::ArrowRight => "Right",
        Named::F1 => "F1",
        Named::F2 => "F2",
        Named::F3 => "F3",
        Named::F4 => "F4",
        Named::F5 => "F5",
        Named::F6 => "F6",
        Named::F7 => "F7",
        Named::F8 => "F8",
        Named::F9 => "F9",
        Named::F10 => "F10",
        Named::F11 => "F11",
        Named::F12 => "F12",
        _ => "?",
    }
}

// ---------------------------------------------------------------------------
// Shortcut resolver (override-aware)
// ---------------------------------------------------------------------------

/// Resolves keyboard shortcuts with support for custom overrides.
///
/// Custom bindings take priority over defaults.  When an action is rebound,
/// its original default binding no longer triggers that action.
#[derive(Debug, Clone)]
pub struct ShortcutResolver {
    /// Custom chord string → action.
    custom: HashMap<String, AppAction>,
    /// Actions whose default binding has been overridden.
    rebound: HashSet<AppAction>,
}

impl ShortcutResolver {
    /// An empty resolver with no custom bindings (all defaults active).
    pub fn empty() -> Self {
        Self {
            custom: HashMap::new(),
            rebound: HashSet::new(),
        }
    }

    /// Builds a resolver from the flat-index → display-chord override map.
    pub fn from_overrides(overrides: &HashMap<usize, String>) -> Self {
        let mut custom = HashMap::new();
        let mut rebound = HashSet::new();
        for (&index, chord) in overrides {
            if let Some(action) = flat_index_to_action(index) {
                custom.insert(chord.clone(), action);
                rebound.insert(action);
            }
        }
        Self { custom, rebound }
    }

    /// Resolves a key event to an `AppAction`, checking custom bindings first.
    pub fn resolve(&self, key: &Key, modifiers: Modifiers) -> Option<AppAction> {
        let chord = normalize_chord(key, modifiers);

        // Custom bindings take priority.
        if let Some(&action) = self.custom.get(&chord) {
            return Some(action);
        }

        // Fall back to defaults, but not for actions that have been rebound.
        let default = check_app_shortcut(key, modifiers);
        match default {
            Some(action) if self.rebound.contains(&action) => None,
            other => other,
        }
    }
}

// ---------------------------------------------------------------------------
// Default shortcut table (unchanged)
// ---------------------------------------------------------------------------

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
        "o" if shift => Some(AppAction::TogglePerfOverlay),
        "f" if !shift => Some(AppAction::Find),
        "m" if shift => Some(AppAction::WhisperToggle),
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

    // -----------------------------------------------------------------------
    // Default shortcut tests (unchanged)
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Flat index mapping tests
    // -----------------------------------------------------------------------

    #[test]
    fn flat_index_0_is_new_tab() {
        assert_eq!(flat_index_to_action(0), Some(AppAction::NewTab));
    }

    #[test]
    fn flat_index_5_is_split_right() {
        assert_eq!(flat_index_to_action(5), Some(AppAction::SplitRight));
    }

    #[test]
    fn flat_index_7_is_unsplit() {
        assert_eq!(flat_index_to_action(7), Some(AppAction::Unsplit));
    }

    #[test]
    fn flat_index_out_of_range_is_none() {
        assert_eq!(flat_index_to_action(999), None);
    }

    // -----------------------------------------------------------------------
    // normalize_chord tests
    // -----------------------------------------------------------------------

    #[test]
    fn normalize_ctrl_backslash() {
        assert_eq!(normalize_chord(&char_key("\\"), CTRL), "Ctrl+\\");
    }

    #[test]
    fn normalize_ctrl_shift_slash() {
        assert_eq!(normalize_chord(&char_key("/"), ctrl_shift()), "Ctrl+Shift+/");
    }

    #[test]
    fn normalize_ctrl_t() {
        assert_eq!(normalize_chord(&char_key("t"), CTRL), "Ctrl+T");
    }

    #[test]
    fn normalize_f2_no_modifiers() {
        assert_eq!(normalize_chord(&named_key(Named::F2), NONE), "F2");
    }

    #[test]
    fn normalize_ctrl_alt_left() {
        assert_eq!(
            normalize_chord(&named_key(Named::ArrowLeft), ctrl_alt()),
            "Ctrl+Alt+Left"
        );
    }

    // -----------------------------------------------------------------------
    // ShortcutResolver tests — the core regression tests for this bug
    // -----------------------------------------------------------------------

    #[test]
    fn resolver_empty_uses_defaults() {
        let r = ShortcutResolver::empty();
        assert_eq!(r.resolve(&char_key("t"), CTRL), Some(AppAction::NewTab));
        assert_eq!(
            r.resolve(&char_key("\\"), CTRL),
            Some(AppAction::SplitRight)
        );
    }

    #[test]
    fn resolver_custom_binding_triggers_action() {
        // Rebind SplitRight (index 5) from Ctrl+\ to Ctrl+Shift+/
        let mut overrides = HashMap::new();
        overrides.insert(5, "Ctrl+Shift+/".to_string());
        let r = ShortcutResolver::from_overrides(&overrides);

        // Custom chord triggers SplitRight
        assert_eq!(
            r.resolve(&char_key("/"), ctrl_shift()),
            Some(AppAction::SplitRight)
        );
    }

    #[test]
    fn resolver_default_disabled_after_rebind() {
        // Rebind SplitRight (index 5) to Ctrl+Shift+/
        let mut overrides = HashMap::new();
        overrides.insert(5, "Ctrl+Shift+/".to_string());
        let r = ShortcutResolver::from_overrides(&overrides);

        // Default Ctrl+\ no longer triggers SplitRight
        assert_eq!(r.resolve(&char_key("\\"), CTRL), None);
    }

    #[test]
    fn resolver_non_rebound_defaults_still_work() {
        let mut overrides = HashMap::new();
        overrides.insert(5, "Ctrl+Shift+/".to_string());
        let r = ShortcutResolver::from_overrides(&overrides);

        // Other defaults unaffected
        assert_eq!(r.resolve(&char_key("t"), CTRL), Some(AppAction::NewTab));
        assert_eq!(r.resolve(&char_key("w"), CTRL), Some(AppAction::CloseTab));
    }

    #[test]
    fn resolver_custom_overrides_conflicting_default() {
        // Rebind SplitRight to Ctrl+T (conflicts with NewTab default)
        let mut overrides = HashMap::new();
        overrides.insert(5, "Ctrl+T".to_string());
        let r = ShortcutResolver::from_overrides(&overrides);

        // Custom binding wins over default
        assert_eq!(
            r.resolve(&char_key("T"), CTRL),
            Some(AppAction::SplitRight)
        );
    }

    #[test]
    fn resolver_multiple_overrides() {
        let mut overrides = HashMap::new();
        overrides.insert(5, "Ctrl+Shift+/".to_string());  // SplitRight
        overrides.insert(6, "Ctrl+Shift+.".to_string());  // SplitDown
        let r = ShortcutResolver::from_overrides(&overrides);

        assert_eq!(
            r.resolve(&char_key("/"), ctrl_shift()),
            Some(AppAction::SplitRight)
        );
        assert_eq!(
            r.resolve(&char_key("."), ctrl_shift()),
            Some(AppAction::SplitDown)
        );
        // Both defaults disabled
        assert_eq!(r.resolve(&char_key("\\"), CTRL), None);
        assert_eq!(r.resolve(&char_key("\\"), ctrl_alt()), None);
    }

    #[test]
    fn resolver_invalid_flat_index_ignored() {
        let mut overrides = HashMap::new();
        overrides.insert(999, "Ctrl+X".to_string());
        let r = ShortcutResolver::from_overrides(&overrides);

        // Ctrl+X should not trigger anything (999 is out of range)
        assert_eq!(r.resolve(&char_key("X"), CTRL), None);
    }
}
