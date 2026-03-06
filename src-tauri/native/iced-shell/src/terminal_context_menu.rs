use iced::widget::{button, column, container, mouse_area, text};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::theme::{
    BG_SECONDARY, BG_TERTIARY, BORDER, BACKDROP, RADIUS_MD, SHADOW_COLOR, TEXT_PRIMARY,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermCtxAction {
    Copy,
    CopyClean,
    Paste,
    SelectAll,
    Clear,
    SplitRight,
    SplitDown,
}

impl TermCtxAction {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::CopyClean => "Copy (Clean)",
            Self::Paste => "Paste",
            Self::SelectAll => "Select All",
            Self::Clear => "Clear",
            Self::SplitRight => "Split Right",
            Self::SplitDown => "Split Down",
        }
    }

    pub fn all() -> &'static [TermCtxAction] {
        &[
            Self::Copy,
            Self::CopyClean,
            Self::Paste,
            Self::SelectAll,
            Self::Clear,
            Self::SplitRight,
            Self::SplitDown,
        ]
    }
}

/// Build a terminal context menu overlay at (x, y) pixel position.
///
/// `on_action` maps a chosen action to the caller's message type.
/// `on_close` is emitted when the user clicks the backdrop to dismiss.
pub fn view_terminal_context_menu<'a, M: Clone + 'a>(
    x: f32,
    y: f32,
    on_action: impl Fn(TermCtxAction) -> M + 'a,
    on_close: M,
) -> Element<'a, M> {
    let action_btn = |action: &TermCtxAction, msg: M| -> Element<'a, M> {
        button(text(action.label()).size(12).color(TEXT_PRIMARY()))
            .on_press(msg)
            .padding(Padding::from([5, 8]))
            .width(Length::Fill)
            .style(|_theme, status| {
                let bg = match status {
                    button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                    _ => Color::TRANSPARENT,
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: TEXT_PRIMARY(),
                    border: Border::default(),
                    ..button::Style::default()
                }
            })
            .into()
    };

    let mut items: Vec<Element<'a, M>> = Vec::new();
    for action in TermCtxAction::all() {
        // Add a visual separator before SplitRight
        if *action == TermCtxAction::SplitRight {
            items.push(
                container(iced::widget::Space::new().width(Length::Fill).height(1.0))
                    .width(Length::Fill)
                    .style(|_theme| container::Style {
                        background: Some(Background::Color(BORDER())),
                        ..container::Style::default()
                    })
                    .into(),
            );
        }
        items.push(action_btn(action, on_action(action.clone())));
    }

    let menu = container(column(items).spacing(2))
        .padding(Padding::from([8, 6]))
        .width(Length::Fixed(200.0))
        .style(|_theme| container::Style {
            background: Some(Background::Color(BG_SECONDARY())),
            border: Border {
                color: BORDER(),
                width: 1.0,
                radius: RADIUS_MD.into(),
            },
            shadow: Shadow {
                color: SHADOW_COLOR,
                offset: Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..container::Style::default()
        });

    // Position the menu using padding from the top-left corner.
    let positioned = container(menu)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: y,
            right: 0.0,
            bottom: 0.0,
            left: x,
        });

    // Full-screen backdrop catches dismiss clicks.
    let close = on_close;
    mouse_area(
        container(positioned)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(Background::Color(BACKDROP())),
                ..container::Style::default()
            }),
    )
    .on_press(close)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_actions_have_labels() {
        for action in TermCtxAction::all() {
            assert!(!action.label().is_empty());
        }
    }

    #[test]
    fn action_count() {
        assert_eq!(TermCtxAction::all().len(), 7);
    }
}
