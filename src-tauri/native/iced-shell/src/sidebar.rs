use iced::widget::{button, column, container, mouse_area, row, rule, scrollable, text, Space};
use iced::{Border, Color, Element, Length, Padding};

use crate::theme::{
    ACCENT, ACCENT_HOVER, BG_SECONDARY, BG_TERTIARY, BORDER, DANGER, TEXT_ACTIVE, TEXT_PRIMARY,
    TEXT_SECONDARY,
};
use crate::workspace_state::WorkspaceInfo;

/// Width of the sidebar in logical pixels.
pub const SIDEBAR_WIDTH: f32 = 220.0;

/// Sidebar-level actions emitted to the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarAction {
    SelectWorkspace(String),
    ToggleWorkspaceContext(String),
    RenameWorkspace(String),
    DeleteWorkspace(String),
    OpenWorkspaceInExplorer(String),
    ToggleWorkspaceWorktreeMode(String),
    MoveWorkspaceUp(String),
    MoveWorkspaceDown(String),
    NewWorkspace,
    ToggleSettings,
}

/// Renders the workspace sidebar as a vertical list.
///
/// - fixed header ("WORKSPACES") with compact controls
/// - scrollable workspace list
/// - contextual actions shown for right-clicked workspace
pub fn view_sidebar<'a, M: Clone + 'a>(
    workspaces: &'a [WorkspaceInfo],
    active_id: Option<&str>,
    context_menu_workspace_id: Option<&str>,
    on_action: impl Fn(SidebarAction) -> M + 'a,
) -> Element<'a, M> {
    let mut items = column![].spacing(2);

    for (idx, ws) in workspaces.iter().enumerate() {
        let is_active = active_id == Some(ws.id.as_str());

        let row_text = if is_active { ACCENT_HOVER } else { TEXT_PRIMARY };

        let name_label = text(&ws.name).size(13).color(row_text);

        let terminal_count = ws.layout.leaf_count();
        let badge_bg = if is_active { ACCENT } else { BG_TERTIARY };
        let badge_text = if is_active {
            BG_SECONDARY
        } else {
            TEXT_SECONDARY
        };

        let badge = container(
            text(format!("{}", terminal_count))
                .size(11)
                .color(badge_text),
        )
        .padding(Padding::from([1, 7]))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(badge_bg)),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 10.0.into(),
            },
            ..container::Style::default()
        });

        let worktree_indicator_bg = if ws.worktree_mode {
            ACCENT_HOVER
        } else {
            Color::from_rgba(TEXT_SECONDARY.r, TEXT_SECONDARY.g, TEXT_SECONDARY.b, 0.25)
        };
        let worktree_indicator = container(Space::new().width(Length::Fixed(6.0)).height(Length::Fixed(6.0)))
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(worktree_indicator_bg)),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 99.0.into(),
                },
                ..container::Style::default()
            });

        let item_content = row![
            worktree_indicator,
            name_label,
            Space::new().width(Length::Fill),
            badge
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        let select_id = ws.id.clone();
        let row_btn = button(item_content)
            .on_press(on_action(SidebarAction::SelectWorkspace(select_id)))
            .padding(Padding::from([7, 10]))
            .width(Length::Fill)
            .style(move |_theme, status| {
                let bg = if is_active {
                    Color::from_rgba(ACCENT_HOVER.r, ACCENT_HOVER.g, ACCENT_HOVER.b, 0.08)
                } else {
                    match status {
                        button::Status::Hovered | button::Status::Pressed => BG_TERTIARY,
                        _ => Color::TRANSPARENT,
                    }
                };

                let border = if is_active {
                    Border {
                        color: ACCENT_HOVER,
                        width: 1.0,
                        radius: 4.0.into(),
                    }
                } else {
                    Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: 4.0.into(),
                    }
                };

                button::Style {
                    background: Some(iced::Background::Color(bg)),
                    text_color: row_text,
                    border,
                    ..button::Style::default()
                }
            });

        let context_id = ws.id.clone();
        let ws_row = mouse_area(row_btn)
            .on_right_press(on_action(SidebarAction::ToggleWorkspaceContext(context_id)));

        items = items.push(ws_row);

        let show_context = context_menu_workspace_id == Some(ws.id.as_str());
        if show_context {
            let rename_id = ws.id.clone();
            let open_id = ws.id.clone();
            let worktree_id = ws.id.clone();
            let move_up_id = ws.id.clone();
            let move_down_id = ws.id.clone();
            let delete_id = ws.id.clone();

            let rename_btn = button(text("Rename").size(12).color(TEXT_PRIMARY))
                .on_press(on_action(SidebarAction::RenameWorkspace(rename_id)))
                .padding(Padding::from([5, 8]))
                .width(Length::Fill)
                .style(|_theme, status| {
                    let bg = match status {
                        button::Status::Hovered | button::Status::Pressed => BG_TERTIARY,
                        _ => Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        text_color: TEXT_PRIMARY,
                        border: Border::default(),
                        ..button::Style::default()
                    }
                });

            let open_btn = button(text("Open in Explorer").size(12).color(TEXT_PRIMARY))
                .on_press(on_action(SidebarAction::OpenWorkspaceInExplorer(open_id)))
                .padding(Padding::from([5, 8]))
                .width(Length::Fill)
                .style(|_theme, status| {
                    let bg = match status {
                        button::Status::Hovered | button::Status::Pressed => BG_TERTIARY,
                        _ => Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        text_color: TEXT_PRIMARY,
                        border: Border::default(),
                        ..button::Style::default()
                    }
                });

            let worktree_label = if ws.worktree_mode {
                "Disable Worktree Mode"
            } else {
                "Enable Worktree Mode"
            };
            let worktree_btn = button(text(worktree_label).size(12).color(TEXT_PRIMARY))
                .on_press(on_action(SidebarAction::ToggleWorkspaceWorktreeMode(worktree_id)))
                .padding(Padding::from([5, 8]))
                .width(Length::Fill)
                .style(|_theme, status| {
                    let bg = match status {
                        button::Status::Hovered | button::Status::Pressed => BG_TERTIARY,
                        _ => Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        text_color: TEXT_PRIMARY,
                        border: Border::default(),
                        ..button::Style::default()
                    }
                });

            let move_up_btn = if idx > 0 {
                button(text("Move Up").size(12).color(TEXT_PRIMARY))
                    .on_press(on_action(SidebarAction::MoveWorkspaceUp(move_up_id)))
                    .padding(Padding::from([5, 8]))
                    .width(Length::Fill)
                    .style(|_theme, status| {
                        let bg = match status {
                            button::Status::Hovered | button::Status::Pressed => BG_TERTIARY,
                            _ => Color::TRANSPARENT,
                        };
                        button::Style {
                            background: Some(iced::Background::Color(bg)),
                            text_color: TEXT_PRIMARY,
                            border: Border::default(),
                            ..button::Style::default()
                        }
                    })
            } else {
                button(text("Move Up").size(12).color(TEXT_SECONDARY))
                    .padding(Padding::from([5, 8]))
                    .width(Length::Fill)
                    .style(|_theme, _status| button::Style::default())
            };

            let move_down_btn = if idx + 1 < workspaces.len() {
                button(text("Move Down").size(12).color(TEXT_PRIMARY))
                    .on_press(on_action(SidebarAction::MoveWorkspaceDown(move_down_id)))
                    .padding(Padding::from([5, 8]))
                    .width(Length::Fill)
                    .style(|_theme, status| {
                        let bg = match status {
                            button::Status::Hovered | button::Status::Pressed => BG_TERTIARY,
                            _ => Color::TRANSPARENT,
                        };
                        button::Style {
                            background: Some(iced::Background::Color(bg)),
                            text_color: TEXT_PRIMARY,
                            border: Border::default(),
                            ..button::Style::default()
                        }
                    })
            } else {
                button(text("Move Down").size(12).color(TEXT_SECONDARY))
                    .padding(Padding::from([5, 8]))
                    .width(Length::Fill)
                    .style(|_theme, _status| button::Style::default())
            };

            let delete_btn = button(text("Delete").size(12).color(DANGER))
                .on_press(on_action(SidebarAction::DeleteWorkspace(delete_id)))
                .padding(Padding::from([5, 8]))
                .width(Length::Fill)
                .style(|_theme, status| {
                    let bg = match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            Color::from_rgba(DANGER.r, DANGER.g, DANGER.b, 0.20)
                        }
                        _ => Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        text_color: DANGER,
                        border: Border::default(),
                        ..button::Style::default()
                    }
                });

            let menu = container(column![
                rename_btn,
                open_btn,
                worktree_btn,
                move_up_btn,
                move_down_btn,
                delete_btn
            ])
            .padding(Padding::from([4, 4]))
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(BG_SECONDARY)),
                border: Border {
                    color: BORDER,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..container::Style::default()
            });

            items = items.push(container(menu).padding(Padding::from([0, 10])));
        }
    }

    let workspaces_badge = container(
        text(format!("{}", workspaces.len()))
            .size(10)
            .color(TEXT_SECONDARY),
    )
    .padding(Padding::from([1, 7]))
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(BG_TERTIARY)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 10.0.into(),
        },
        ..container::Style::default()
    });

    let settings_btn = button(text("\u{2699}").size(13).color(TEXT_PRIMARY))
        .on_press(on_action(SidebarAction::ToggleSettings))
        .padding(Padding::from([2, 7]))
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => BG_TERTIARY,
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(iced::Background::Color(bg)),
                text_color: TEXT_ACTIVE,
                border: Border {
                    color: BORDER,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..button::Style::default()
            }
        });

    let new_btn = button(text("+").size(14).color(TEXT_PRIMARY))
        .on_press(on_action(SidebarAction::NewWorkspace))
        .padding(Padding::from([1, 7]))
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => BG_TERTIARY,
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(iced::Background::Color(bg)),
                text_color: TEXT_ACTIVE,
                border: Border {
                    color: BORDER,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..button::Style::default()
            }
        });

    let header = container(
        row![
            text("WORKSPACES").size(11).color(TEXT_SECONDARY),
            Space::new().width(Length::Fill),
            workspaces_badge,
            settings_btn,
            new_btn,
        ]
        .align_y(iced::Alignment::Center)
        .spacing(6),
    )
    .padding(Padding::from([10, 10]))
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(BG_SECONDARY)),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    });

    let scrollable_list = scrollable(
        container(items)
            .padding(Padding::from([8, 8]))
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    let sidebar_content = container(column![header, scrollable_list])
        .width(Length::Fixed(SIDEBAR_WIDTH - 1.0)) // Reserve 1px for divider
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(BG_SECONDARY)),
            ..container::Style::default()
        });

    let divider = rule::vertical(1).style(move |_theme| rule::Style {
        color: BORDER,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    });

    row![sidebar_content, divider]
        .width(Length::Fixed(SIDEBAR_WIDTH))
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::split_pane::LayoutNode;

    fn test_workspace(id: &str, name: &str) -> WorkspaceInfo {
        WorkspaceInfo {
            id: id.to_string(),
            name: name.to_string(),
            folder_path: "C:\\".to_string(),
            worktree_mode: false,
            layout: LayoutNode::Leaf {
                terminal_id: "t1".into(),
            },
            focused_terminal: "t1".to_string(),
        }
    }

    #[derive(Clone, Debug)]
    enum TestMsg {
        Action(SidebarAction),
    }

    #[test]
    fn test_view_sidebar_empty() {
        // Should not panic with empty workspace list
        let _el: Element<'_, TestMsg> =
            view_sidebar(&[], None, None, |action| TestMsg::Action(action));
    }

    #[test]
    fn test_view_sidebar_single() {
        let workspaces = vec![test_workspace("w1", "Workspace 1")];
        let _el: Element<'_, TestMsg> = view_sidebar(&workspaces, Some("w1"), None, |action| {
            TestMsg::Action(action)
        });
    }

    #[test]
    fn test_view_sidebar_multiple() {
        let workspaces = vec![
            test_workspace("w1", "Workspace 1"),
            test_workspace("w2", "Workspace 2"),
            test_workspace("w3", "Workspace 3"),
        ];
        let _el: Element<'_, TestMsg> = view_sidebar(&workspaces, Some("w2"), Some("w2"), |action| {
            TestMsg::Action(action)
        });
    }
}
