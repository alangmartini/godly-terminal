use iced::widget::{column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Padding};

use crate::theme::{ACCENT, BG_PRIMARY, BORDER, TEXT_ACTIVE, TEXT_PRIMARY};

/// A single shortcut entry.
#[derive(Clone, Copy)]
pub struct ShortcutEntry {
    pub action: &'static str,
    pub keys: &'static str,
}

/// A category of shortcuts.
#[derive(Clone, Copy)]
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
        action: "Recent Tab in Workspace",
        keys: "Hold Ctrl+Tab",
    },
    ShortcutEntry {
        action: "Recent Tab in Workspace (Reverse)",
        keys: "Hold Ctrl+Shift+Tab",
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

const CATEGORIES: &[ShortcutCategory] = &[
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
];

const SECTION_RADIUS: f32 = 6.0;
const KEY_BADGE_RADIUS: f32 = 5.0;
const SECTION_SPACING: f32 = 10.0;
const ENTRY_SPACING: f32 = 8.0;
const ROW_SPACING: f32 = 10.0;

fn tint(color: Color, alpha: f32) -> Color {
    Color::from_rgba(color.r, color.g, color.b, alpha)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns all shortcut categories.
pub fn shortcut_categories() -> Vec<ShortcutCategory> {
    CATEGORIES.to_vec()
}

/// Renders the shortcuts tab content as a scrollable list of categories.
pub fn view_shortcuts_tab<'a, M: 'a>() -> Element<'a, M> {
    let mut content = column![]
        .spacing(SECTION_SPACING)
        .padding(Padding::from([2, 2]));

    for cat in CATEGORIES {
        let mut entries = column![].spacing(ENTRY_SPACING).width(Length::Fill);

        for entry in cat.entries {
            let key_badge = container(text(entry.keys).size(12).color(ACCENT))
                .padding(Padding::from([3, 8]))
                .style(|_theme| container::Style {
                    background: Some(Background::Color(tint(BG_PRIMARY, 0.7))),
                    border: Border {
                        color: BORDER,
                        width: 1.0,
                        radius: KEY_BADGE_RADIUS.into(),
                    },
                    ..container::Style::default()
                });

            let entry_row = row![
                text(entry.action)
                    .size(13)
                    .color(TEXT_PRIMARY)
                    .width(Length::Fill),
                key_badge
            ]
            .align_y(iced::Alignment::Center)
            .spacing(ROW_SPACING)
            .padding(Padding::from([2, 0]))
            .width(Length::Fill);

            entries = entries.push(entry_row);
        }

        let section = container(
            column![text(cat.name).size(14).color(TEXT_ACTIVE), entries]
                .spacing(8)
                .width(Length::Fill),
        )
        .padding(Padding::from([10, 12]))
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(tint(BG_PRIMARY, 0.4))),
            border: Border {
                color: BORDER,
                width: 1.0,
                radius: SECTION_RADIUS.into(),
            },
            ..container::Style::default()
        });

        content = content.push(section);
    }

    let scroll = scrollable(content)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .spacing(2),
        ))
        .spacing(8)
        .width(Length::Fill)
        .height(Length::Fill);

    container(scroll)
        .padding(Padding::from([0, 2]))
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

    #[test]
    fn test_tab_shortcuts_describe_workspace_recent_tab_popup_semantics() {
        let tabs = shortcut_categories()
            .into_iter()
            .find(|category| category.name == "Tabs")
            .expect("tabs category should exist");

        assert_eq!(tabs.entries[2].action, "Recent Tab in Workspace");
        assert_eq!(tabs.entries[2].keys, "Hold Ctrl+Tab");
        assert_eq!(tabs.entries[3].action, "Recent Tab in Workspace (Reverse)");
        assert_eq!(tabs.entries[3].keys, "Hold Ctrl+Shift+Tab");
    }
}
