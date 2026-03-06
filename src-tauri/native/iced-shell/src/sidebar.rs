use std::f32::consts::TAU;

use iced::widget::{
    button, canvas, column, container, mouse_area, row, rule, scrollable, text, Space,
};
use iced::{Border, Color, Element, Length, Padding, Point, Rectangle, Renderer, Size, Theme};

use crate::theme::{
    ACCENT, ACCENT_HOVER, BG_SECONDARY, BG_TERTIARY, BORDER, DANGER, TEXT_ACTIVE, TEXT_PRIMARY,
    TEXT_SECONDARY,
};
use crate::workspace_state::WorkspaceInfo;

/// Width of the sidebar in logical pixels.
pub const SIDEBAR_WIDTH: f32 = 220.0;
/// Minimum user-resizable sidebar width in logical pixels.
pub const SIDEBAR_MIN_WIDTH: f32 = 180.0;
/// Maximum user-resizable sidebar width in logical pixels.
pub const SIDEBAR_MAX_WIDTH: f32 = 420.0;
/// Resize handle width in logical pixels.
pub const SIDEBAR_RESIZE_HANDLE_WIDTH: f32 = 6.0;
const HEADER_ICON_SIZE: f32 = 12.0;

/// Clamp a sidebar width into the supported resize range.
pub fn clamp_sidebar_width(width: f32) -> f32 {
    width.clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidebarHeaderIconKind {
    Settings,
    NewWorkspace,
}

#[derive(Debug, Clone, Copy)]
struct SidebarHeaderIcon {
    kind: SidebarHeaderIconKind,
    color: Color,
}

impl SidebarHeaderIcon {
    fn settings(color: Color) -> Self {
        Self {
            kind: SidebarHeaderIconKind::Settings,
            color,
        }
    }

    fn new_workspace(color: Color) -> Self {
        Self {
            kind: SidebarHeaderIconKind::NewWorkspace,
            color,
        }
    }
}

impl<Message> canvas::Program<Message> for SidebarHeaderIcon {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let size = bounds.size();
        let center = Point::new(size.width * 0.5, size.height * 0.5);

        match self.kind {
            SidebarHeaderIconKind::Settings => {
                let ring_radius = size.width.min(size.height) * 0.28;
                let ring = canvas::Path::circle(center, ring_radius);
                let ring_stroke = canvas::Stroke::default()
                    .with_color(self.color)
                    .with_width(1.2);
                frame.stroke(&ring, ring_stroke);

                for index in 0..8 {
                    let angle = index as f32 * TAU / 8.0;
                    let (sin, cos) = angle.sin_cos();
                    let inner = Point::new(
                        center.x + cos * (ring_radius + 0.6),
                        center.y + sin * (ring_radius + 0.6),
                    );
                    let outer = Point::new(
                        center.x + cos * (ring_radius + 2.4),
                        center.y + sin * (ring_radius + 2.4),
                    );
                    let spoke = canvas::Path::line(inner, outer);
                    frame.stroke(&spoke, ring_stroke);
                }

                let hub = canvas::Path::circle(center, ring_radius * 0.34);
                frame.fill(&hub, self.color);
            }
            SidebarHeaderIconKind::NewWorkspace => {
                let box_size = size.width.min(size.height) * 0.72;
                let top_left = Point::new(center.x - box_size * 0.5, center.y - box_size * 0.5);
                let outline = canvas::Path::rectangle(top_left, Size::new(box_size, box_size));
                let box_stroke = canvas::Stroke::default()
                    .with_color(self.color)
                    .with_width(1.1);
                frame.stroke(&outline, box_stroke);

                let arm = box_size * 0.24;
                let vertical = canvas::Path::line(
                    Point::new(center.x, center.y - arm),
                    Point::new(center.x, center.y + arm),
                );
                let horizontal = canvas::Path::line(
                    Point::new(center.x - arm, center.y),
                    Point::new(center.x + arm, center.y),
                );
                let plus_stroke = canvas::Stroke::default()
                    .with_color(self.color)
                    .with_width(1.4)
                    .with_line_cap(canvas::LineCap::Round);
                frame.stroke(&vertical, plus_stroke);
                frame.stroke(&horizontal, plus_stroke);
            }
        }

        vec![frame.into_geometry()]
    }
}

fn header_action_button_style(status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
        _ => Color::TRANSPARENT,
    };

    button::Style {
        background: Some(iced::Background::Color(bg)),
        text_color: TEXT_ACTIVE(),
        border: Border {
            color: BORDER(),
            width: 1.0,
            radius: 4.0.into(),
        },
        ..button::Style::default()
    }
}

/// Sidebar-level actions emitted to the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarAction {
    SelectWorkspace(String),
    WorkspaceDragHover(String),
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

/// Workspace-level signals used by the sidebar rendering.
pub trait SidebarWorkspaceSignals {
    /// Returns the workspace ID whose context menu should be visible.
    fn context_menu_workspace_id(&self) -> Option<&str>;

    /// Returns true when a workspace has a pending notification.
    fn has_workspace_notification(&self, _workspace_id: &str) -> bool {
        false
    }

    /// Returns an optional AI mode icon string for a workspace.
    fn workspace_ai_mode_icon(&self, _workspace_id: &str) -> Option<&'static str> {
        None
    }
}

impl<'a> SidebarWorkspaceSignals for Option<&'a str> {
    fn context_menu_workspace_id(&self) -> Option<&str> {
        *self
    }
}

impl<'a, F> SidebarWorkspaceSignals for (Option<&'a str>, F)
where
    F: for<'b> Fn(&'b str) -> bool,
{
    fn context_menu_workspace_id(&self) -> Option<&str> {
        self.0
    }

    fn has_workspace_notification(&self, workspace_id: &str) -> bool {
        (self.1)(workspace_id)
    }
}

impl<'a, F, G> SidebarWorkspaceSignals for (Option<&'a str>, F, G)
where
    F: for<'b> Fn(&'b str) -> bool,
    G: for<'b> Fn(&'b str) -> Option<&'static str>,
{
    fn context_menu_workspace_id(&self) -> Option<&str> {
        self.0
    }

    fn has_workspace_notification(&self, workspace_id: &str) -> bool {
        (self.1)(workspace_id)
    }

    fn workspace_ai_mode_icon(&self, workspace_id: &str) -> Option<&'static str> {
        (self.2)(workspace_id)
    }
}

/// Renders the workspace sidebar as a vertical list.
///
/// - fixed header ("WORKSPACES") with compact controls
/// - scrollable workspace list
/// - contextual actions shown for right-clicked workspace
pub fn view_sidebar<'a, M: Clone + 'a, S: SidebarWorkspaceSignals>(
    workspaces: &'a [WorkspaceInfo],
    active_id: Option<&str>,
    workspace_signals: S,
    sidebar_width: f32,
    on_action: impl Fn(SidebarAction) -> M + 'a,
    on_resize_start: M,
    on_resize_end: M,
) -> Element<'a, M> {
    let sidebar_width = sidebar_width.max(0.0);
    let resize_handle_width = sidebar_width.min(SIDEBAR_RESIZE_HANDLE_WIDTH);
    let sidebar_content_width = (sidebar_width - resize_handle_width).max(0.0);
    let context_menu_workspace_id = workspace_signals.context_menu_workspace_id();
    let mut items = column![].spacing(2);

    for (idx, ws) in workspaces.iter().enumerate() {
        let is_active = active_id == Some(ws.id.as_str());
        let has_workspace_notification =
            workspace_signals.has_workspace_notification(ws.id.as_str());

        let row_text = if is_active {
            ACCENT_HOVER()
        } else {
            TEXT_PRIMARY()
        };

        let name_label = text(&ws.name).size(13).color(row_text);

        let terminal_count = ws.layout.leaf_count();
        let badge_bg = if is_active { ACCENT() } else { BG_TERTIARY() };
        let badge_text = if is_active {
            BG_SECONDARY()
        } else {
            TEXT_SECONDARY()
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
            ACCENT_HOVER()
        } else {
            Color::from_rgba(TEXT_SECONDARY().r, TEXT_SECONDARY().g, TEXT_SECONDARY().b, 0.25)
        };
        let worktree_indicator = container(
            Space::new()
                .width(Length::Fixed(6.0))
                .height(Length::Fixed(6.0)),
        )
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(worktree_indicator_bg)),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 99.0.into(),
            },
            ..container::Style::default()
        });

        let notification_indicator_bg = if is_active { ACCENT_HOVER() } else { DANGER() };
        let notification_indicator_border = if is_active { BORDER() } else { BG_SECONDARY() };
        let notification_indicator = container(
            Space::new()
                .width(Length::Fixed(8.0))
                .height(Length::Fixed(8.0)),
        )
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(notification_indicator_bg)),
            border: Border {
                color: notification_indicator_border,
                width: 1.0,
                radius: 99.0.into(),
            },
            ..container::Style::default()
        });

        let ai_mode_icon = workspace_signals.workspace_ai_mode_icon(ws.id.as_str());
        let mut item_content = row![
            worktree_indicator,
            name_label,
        ];
        if let Some(icon) = ai_mode_icon {
            item_content = item_content.push(text(icon).size(11));
        }
        item_content = item_content.push(Space::new().width(Length::Fill));
        if has_workspace_notification {
            item_content = item_content.push(notification_indicator);
        }
        let item_content = item_content
            .push(badge)
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
                    Color::from_rgba(ACCENT_HOVER().r, ACCENT_HOVER().g, ACCENT_HOVER().b, 0.08)
                } else {
                    match status {
                        button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                        _ => Color::TRANSPARENT,
                    }
                };

                let border = if is_active {
                    Border {
                        color: ACCENT_HOVER(),
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

        let hover_id = ws.id.clone();
        let context_id = ws.id.clone();
        let ws_row = mouse_area(row_btn)
            .on_enter(on_action(SidebarAction::WorkspaceDragHover(hover_id)))
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

            let rename_btn = button(text("Rename").size(12).color(TEXT_PRIMARY()))
                .on_press(on_action(SidebarAction::RenameWorkspace(rename_id)))
                .padding(Padding::from([5, 8]))
                .width(Length::Fill)
                .style(|_theme, status| {
                    let bg = match status {
                        button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                        _ => Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        text_color: TEXT_PRIMARY(),
                        border: Border::default(),
                        ..button::Style::default()
                    }
                });

            let open_btn = button(text("Open in Explorer").size(12).color(TEXT_PRIMARY()))
                .on_press(on_action(SidebarAction::OpenWorkspaceInExplorer(open_id)))
                .padding(Padding::from([5, 8]))
                .width(Length::Fill)
                .style(|_theme, status| {
                    let bg = match status {
                        button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                        _ => Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        text_color: TEXT_PRIMARY(),
                        border: Border::default(),
                        ..button::Style::default()
                    }
                });

            let worktree_label = if ws.worktree_mode {
                "Disable Worktree Mode"
            } else {
                "Enable Worktree Mode"
            };
            let worktree_btn = button(text(worktree_label).size(12).color(TEXT_PRIMARY()))
                .on_press(on_action(SidebarAction::ToggleWorkspaceWorktreeMode(
                    worktree_id,
                )))
                .padding(Padding::from([5, 8]))
                .width(Length::Fill)
                .style(|_theme, status| {
                    let bg = match status {
                        button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                        _ => Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        text_color: TEXT_PRIMARY(),
                        border: Border::default(),
                        ..button::Style::default()
                    }
                });

            let move_up_btn = if idx > 0 {
                button(text("Move Up").size(12).color(TEXT_PRIMARY()))
                    .on_press(on_action(SidebarAction::MoveWorkspaceUp(move_up_id)))
                    .padding(Padding::from([5, 8]))
                    .width(Length::Fill)
                    .style(|_theme, status| {
                        let bg = match status {
                            button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                            _ => Color::TRANSPARENT,
                        };
                        button::Style {
                            background: Some(iced::Background::Color(bg)),
                            text_color: TEXT_PRIMARY(),
                            border: Border::default(),
                            ..button::Style::default()
                        }
                    })
            } else {
                button(text("Move Up").size(12).color(TEXT_SECONDARY()))
                    .padding(Padding::from([5, 8]))
                    .width(Length::Fill)
                    .style(|_theme, _status| button::Style::default())
            };

            let move_down_btn = if idx + 1 < workspaces.len() {
                button(text("Move Down").size(12).color(TEXT_PRIMARY()))
                    .on_press(on_action(SidebarAction::MoveWorkspaceDown(move_down_id)))
                    .padding(Padding::from([5, 8]))
                    .width(Length::Fill)
                    .style(|_theme, status| {
                        let bg = match status {
                            button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                            _ => Color::TRANSPARENT,
                        };
                        button::Style {
                            background: Some(iced::Background::Color(bg)),
                            text_color: TEXT_PRIMARY(),
                            border: Border::default(),
                            ..button::Style::default()
                        }
                    })
            } else {
                button(text("Move Down").size(12).color(TEXT_SECONDARY()))
                    .padding(Padding::from([5, 8]))
                    .width(Length::Fill)
                    .style(|_theme, _status| button::Style::default())
            };

            let delete_btn = button(text("Delete").size(12).color(DANGER()))
                .on_press(on_action(SidebarAction::DeleteWorkspace(delete_id)))
                .padding(Padding::from([5, 8]))
                .width(Length::Fill)
                .style(|_theme, status| {
                    let bg = match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            Color::from_rgba(DANGER().r, DANGER().g, DANGER().b, 0.20)
                        }
                        _ => Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        text_color: DANGER(),
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
                background: Some(iced::Background::Color(BG_SECONDARY())),
                border: Border {
                    color: BORDER(),
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
            .color(TEXT_SECONDARY()),
    )
    .padding(Padding::from([1, 7]))
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(BG_TERTIARY())),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 10.0.into(),
        },
        ..container::Style::default()
    });

    let settings_btn = button(
        canvas(SidebarHeaderIcon::settings(TEXT_PRIMARY()))
            .width(Length::Fixed(HEADER_ICON_SIZE))
            .height(Length::Fixed(HEADER_ICON_SIZE)),
    )
    .on_press(on_action(SidebarAction::ToggleSettings))
    .padding(Padding::from([2, 7]))
    .style(|_theme, status| header_action_button_style(status));

    let new_btn = button(
        canvas(SidebarHeaderIcon::new_workspace(TEXT_PRIMARY()))
            .width(Length::Fixed(HEADER_ICON_SIZE))
            .height(Length::Fixed(HEADER_ICON_SIZE)),
    )
    .on_press(on_action(SidebarAction::NewWorkspace))
    .padding(Padding::from([1, 7]))
    .style(|_theme, status| header_action_button_style(status));

    let header = container(
        row![
            text("WORKSPACES").size(11).color(TEXT_SECONDARY()),
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
        background: Some(iced::Background::Color(BG_SECONDARY())),
        border: Border {
            color: BORDER(),
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
        .width(Length::Fixed(sidebar_content_width))
        .height(Length::Fill)
        .clip(true)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(BG_SECONDARY())),
            ..container::Style::default()
        });

    let divider = rule::vertical(1).style(move |_theme| rule::Style {
        color: BORDER(),
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    });

    let resize_handle = mouse_area(
        container(divider)
            .width(Length::Fixed(resize_handle_width))
            .height(Length::Fill),
    )
    .on_press(on_resize_start)
    .on_release(on_resize_end);

    row![sidebar_content, resize_handle]
        .width(Length::Fixed(sidebar_width))
        .height(Length::Fill)
        .clip(true)
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
        Action,
        ResizeStart,
        ResizeEnd,
    }

    #[test]
    fn test_view_sidebar_empty() {
        // Should not panic with empty workspace list
        let _el: Element<'_, TestMsg> = view_sidebar(
            &[],
            None,
            None,
            SIDEBAR_WIDTH,
            |_action| TestMsg::Action,
            TestMsg::ResizeStart,
            TestMsg::ResizeEnd,
        );
    }

    #[test]
    fn test_view_sidebar_single() {
        let workspaces = vec![test_workspace("w1", "Workspace 1")];
        let _el: Element<'_, TestMsg> = view_sidebar(
            &workspaces,
            Some("w1"),
            None,
            SIDEBAR_WIDTH,
            |_action| TestMsg::Action,
            TestMsg::ResizeStart,
            TestMsg::ResizeEnd,
        );
    }

    #[test]
    fn test_view_sidebar_multiple() {
        let workspaces = vec![
            test_workspace("w1", "Workspace 1"),
            test_workspace("w2", "Workspace 2"),
            test_workspace("w3", "Workspace 3"),
        ];
        let _el: Element<'_, TestMsg> = view_sidebar(
            &workspaces,
            Some("w2"),
            Some("w2"),
            SIDEBAR_WIDTH,
            |_action| TestMsg::Action,
            TestMsg::ResizeStart,
            TestMsg::ResizeEnd,
        );
    }

    #[test]
    fn test_view_sidebar_with_notification_signal() {
        fn has_notification(workspace_id: &str) -> bool {
            workspace_id == "w2"
        }

        let workspaces = vec![
            test_workspace("w1", "Workspace 1"),
            test_workspace("w2", "Workspace 2"),
        ];
        let _el: Element<'_, TestMsg> = view_sidebar(
            &workspaces,
            Some("w1"),
            (Some("w2"), has_notification as fn(&str) -> bool),
            SIDEBAR_WIDTH,
            |_action| TestMsg::Action,
            TestMsg::ResizeStart,
            TestMsg::ResizeEnd,
        );
    }

    #[test]
    fn test_sidebar_action_workspace_drag_hover_equality() {
        assert_eq!(
            SidebarAction::WorkspaceDragHover("w1".to_string()),
            SidebarAction::WorkspaceDragHover("w1".to_string())
        );
    }

    #[test]
    fn test_clamp_sidebar_width_bounds() {
        assert_eq!(clamp_sidebar_width(120.0), SIDEBAR_MIN_WIDTH);
        assert_eq!(clamp_sidebar_width(800.0), SIDEBAR_MAX_WIDTH);
        assert_eq!(clamp_sidebar_width(260.0), 260.0);
    }

    #[test]
    fn test_view_sidebar_allows_animation_widths_below_resize_minimum() {
        let workspaces = vec![test_workspace("w1", "Workspace 1")];
        let _el: Element<'_, TestMsg> = view_sidebar(
            &workspaces,
            Some("w1"),
            None,
            72.0,
            |_action| TestMsg::Action,
            TestMsg::ResizeStart,
            TestMsg::ResizeEnd,
        );
    }
}
