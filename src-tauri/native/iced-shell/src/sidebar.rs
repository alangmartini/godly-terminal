use iced::widget::{button, column, container, row, rule, scrollable, text};
use iced::{Border, Color, Element, Length, Padding};

use crate::workspace_state::WorkspaceInfo;

/// Width of the sidebar in logical pixels.
pub const SIDEBAR_WIDTH: f32 = 200.0;

// Colors (match tab_bar.rs style)
const SIDEBAR_BG: Color = Color::from_rgb(0.06, 0.06, 0.08);
const ACTIVE_WS_BG: Color = Color::from_rgb(0.2, 0.2, 0.25);
const INACTIVE_WS_BG: Color = Color::from_rgb(0.10, 0.10, 0.13);
const WS_TEXT_COLOR: Color = Color::from_rgb(0.85, 0.85, 0.85);
const BADGE_COLOR: Color = Color::from_rgb(0.5, 0.5, 0.55);
const DIVIDER_COLOR: Color = Color::from_rgb(0.2, 0.2, 0.25);

/// Renders the workspace sidebar as a vertical list.
///
/// Shows each workspace as a row with its name on the left and a terminal
/// count badge on the right. The active workspace is highlighted with a
/// brighter background. A "+" button at the bottom creates new workspaces.
/// A vertical divider line separates the sidebar from the main content area.
pub fn view_sidebar<'a, M: Clone + 'a>(
    workspaces: &'a [WorkspaceInfo],
    active_id: Option<&str>,
    on_workspace_click: impl Fn(String) -> M + 'a,
    on_new_workspace: M,
) -> Element<'a, M> {
    let mut items = column![].spacing(1);

    for ws in workspaces {
        let is_active = active_id == Some(ws.id.as_str());
        let bg = if is_active {
            ACTIVE_WS_BG
        } else {
            INACTIVE_WS_BG
        };

        let name_label = text(&ws.name).size(13).color(WS_TEXT_COLOR);

        let terminal_count = ws.layout.leaf_count();
        let badge = text(format!("{}", terminal_count))
            .size(11)
            .color(BADGE_COLOR);

        let item_content = row![name_label, badge]
            .spacing(4)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill);

        let ws_id = ws.id.clone();
        let ws_btn = button(item_content)
            .on_press(on_workspace_click(ws_id))
            .padding(Padding::from([6, 10]))
            .width(Length::Fill)
            .style(move |_theme, _status| button::Style {
                background: Some(iced::Background::Color(bg)),
                text_color: WS_TEXT_COLOR,
                border: Border::default(),
                ..button::Style::default()
            });

        items = items.push(ws_btn);
    }

    // "+" button to add new workspace
    let new_btn = button(text("+").size(14).color(WS_TEXT_COLOR))
        .on_press(on_new_workspace)
        .padding(Padding::from([6, 10]))
        .width(Length::Fill)
        .style(|_theme, status| {
            let bg_color = match status {
                button::Status::Hovered | button::Status::Pressed => ACTIVE_WS_BG,
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(iced::Background::Color(bg_color)),
                text_color: WS_TEXT_COLOR,
                border: Border::default(),
                ..button::Style::default()
            }
        });

    items = items.push(new_btn);

    let scrollable_list = scrollable(items).width(Length::Fill).height(Length::Fill);

    let sidebar_content = container(scrollable_list)
        .width(Length::Fixed(SIDEBAR_WIDTH - 1.0)) // Reserve 1px for divider
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(SIDEBAR_BG)),
            ..container::Style::default()
        });

    let divider = rule::vertical(1).style(move |_theme| rule::Style {
        color: DIVIDER_COLOR,
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
            layout: LayoutNode::Leaf {
                terminal_id: "t1".into(),
            },
            focused_terminal: "t1".to_string(),
        }
    }

    #[derive(Clone, Debug)]
    enum TestMsg {
        Click(String),
        New,
    }

    #[test]
    fn test_view_sidebar_empty() {
        // Should not panic with empty workspace list
        let _el: Element<'_, TestMsg> =
            view_sidebar(&[], None, |id| TestMsg::Click(id), TestMsg::New);
    }

    #[test]
    fn test_view_sidebar_single() {
        let workspaces = vec![test_workspace("w1", "Workspace 1")];
        let _el: Element<'_, TestMsg> = view_sidebar(
            &workspaces,
            Some("w1"),
            |id| TestMsg::Click(id),
            TestMsg::New,
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
            |id| TestMsg::Click(id),
            TestMsg::New,
        );
    }
}
