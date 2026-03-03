use iced::widget::{column, container, row, rule, scrollable, text, Space};
use iced::{Color, Element, Length, Padding};

/// A single shortcut entry.
pub struct ShortcutEntry {
    pub action: &'static str,
    pub keys: &'static str,
}

/// A category of shortcuts.
pub struct ShortcutCategory {
    pub name: &'static str,
    pub entries: &'static [ShortcutEntry],
}

// ---------------------------------------------------------------------------
// Shortcut data (hardcoded constants)
// ---------------------------------------------------------------------------

const TABS: &[ShortcutEntry] = &[
    ShortcutEntry {
        action: "New Tab",
        keys: "Ctrl+T",
    },
    ShortcutEntry {
        action: "Close Tab",
        keys: "Ctrl+W",
    },
    ShortcutEntry {
        action: "Next Tab",
        keys: "Ctrl+Tab",
    },
    ShortcutEntry {
        action: "Previous Tab",
        keys: "Ctrl+Shift+Tab",
    },
    ShortcutEntry {
        action: "Rename Tab",
        keys: "F2",
    },
];

const SPLIT_PANES: &[ShortcutEntry] = &[
    ShortcutEntry {
        action: "Split Right",
        keys: "Ctrl+\\",
    },
    ShortcutEntry {
        action: "Split Down",
        keys: "Ctrl+Alt+\\",
    },
    ShortcutEntry {
        action: "Unsplit",
        keys: "Ctrl+Shift+\\",
    },
    ShortcutEntry {
        action: "Focus Next Pane",
        keys: "Alt+\\",
    },
];

const CLIPBOARD: &[ShortcutEntry] = &[
    ShortcutEntry {
        action: "Copy",
        keys: "Ctrl+Shift+C",
    },
    ShortcutEntry {
        action: "Paste",
        keys: "Ctrl+Shift+V",
    },
    ShortcutEntry {
        action: "Select All",
        keys: "Ctrl+Shift+A",
    },
];

const SCROLLBACK: &[ShortcutEntry] = &[
    ShortcutEntry {
        action: "Page Up",
        keys: "Shift+PageUp",
    },
    ShortcutEntry {
        action: "Page Down",
        keys: "Shift+PageDown",
    },
    ShortcutEntry {
        action: "Scroll Top",
        keys: "Ctrl+Home",
    },
    ShortcutEntry {
        action: "Scroll Bottom",
        keys: "Ctrl+End",
    },
];

const ZOOM: &[ShortcutEntry] = &[
    ShortcutEntry {
        action: "Zoom In",
        keys: "Ctrl+=",
    },
    ShortcutEntry {
        action: "Zoom Out",
        keys: "Ctrl+-",
    },
    ShortcutEntry {
        action: "Zoom Reset",
        keys: "Ctrl+0",
    },
];

const WORKSPACES: &[ShortcutEntry] = &[
    ShortcutEntry {
        action: "Next Workspace",
        keys: "Ctrl+Alt+Right",
    },
    ShortcutEntry {
        action: "Previous Workspace",
        keys: "Ctrl+Alt+Left",
    },
    ShortcutEntry {
        action: "Toggle Sidebar",
        keys: "Ctrl+B",
    },
    ShortcutEntry {
        action: "Settings",
        keys: "Ctrl+,",
    },
];

// ---------------------------------------------------------------------------
// Colors (dark theme)
// ---------------------------------------------------------------------------

const TEXT_COLOR: Color = Color::from_rgb(0.85, 0.85, 0.85);
const HEADER_COLOR: Color = Color::from_rgb(0.95, 0.95, 0.95);
const KEY_COLOR: Color = Color::from_rgb(0.6, 0.7, 0.8);
const SEPARATOR_COLOR: Color = Color::from_rgb(0.2, 0.2, 0.25);

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns all shortcut categories.
pub fn shortcut_categories() -> Vec<ShortcutCategory> {
    vec![
        ShortcutCategory {
            name: "Tabs",
            entries: TABS,
        },
        ShortcutCategory {
            name: "Split Panes",
            entries: SPLIT_PANES,
        },
        ShortcutCategory {
            name: "Clipboard",
            entries: CLIPBOARD,
        },
        ShortcutCategory {
            name: "Scrollback",
            entries: SCROLLBACK,
        },
        ShortcutCategory {
            name: "Zoom",
            entries: ZOOM,
        },
        ShortcutCategory {
            name: "Workspaces",
            entries: WORKSPACES,
        },
    ]
}

/// Renders the shortcuts tab content as a scrollable list of categories.
pub fn view_shortcuts_tab<'a, M: 'a>() -> Element<'a, M> {
    let categories = shortcut_categories();
    let mut content = column![].spacing(12).padding(Padding::from([8, 16]));

    for (i, cat) in categories.iter().enumerate() {
        // Category header
        let header = text(cat.name).size(14).color(HEADER_COLOR);
        content = content.push(header);

        // Shortcut entries
        for entry in cat.entries {
            let action_label = text(entry.action).size(13).color(TEXT_COLOR);

            let key_badge = text(entry.keys).size(12).color(KEY_COLOR);

            let entry_row = row![action_label, Space::new().width(Length::Fill), key_badge]
                .align_y(iced::Alignment::Center)
                .padding(Padding::from([2, 0]));

            content = content.push(entry_row);
        }

        // Separator between categories (not after the last one)
        if i + 1 < categories.len() {
            content = content.push(Space::new().height(4));
            content = content.push(
                rule::horizontal(1).style(move |_theme| rule::Style {
                    color: SEPARATOR_COLOR,
                    radius: 0.0.into(),
                    fill_mode: rule::FillMode::Full,
                    snap: true,
                }),
            );
            content = content.push(Space::new().height(4));
        }
    }

    let scroll = scrollable(content).width(Length::Fill).height(Length::Fill);

    container(scroll)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortcut_categories_not_empty() {
        let cats = shortcut_categories();
        assert!(!cats.is_empty());
        for cat in &cats {
            assert!(!cat.name.is_empty());
            assert!(!cat.entries.is_empty());
        }
    }

    #[test]
    fn test_all_entries_have_content() {
        for cat in shortcut_categories() {
            for entry in cat.entries {
                assert!(!entry.action.is_empty());
                assert!(!entry.keys.is_empty());
            }
        }
    }
}
