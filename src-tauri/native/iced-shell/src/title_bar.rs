use iced::widget::{button, container, mouse_area, row, text};
use iced::{Border, Color, Element, Length, Padding};

use crate::theme::TEXT_SECONDARY;

/// Height of the custom title bar in logical pixels.
pub const TITLE_BAR_HEIGHT: f32 = 30.0;

const TITLE_BAR_BG: Color = Color::from_rgb(0.06, 0.06, 0.08);
const CONTROL_HOVER_BG: Color = Color::from_rgb(0.18, 0.18, 0.22);
const CLOSE_HOVER_BG: Color = Color::from_rgb(0.75, 0.15, 0.15);
const CONTROL_TEXT_COLOR: Color = Color::from_rgb(0.70, 0.70, 0.75);

/// Renders the custom window title bar with drag area and control buttons.
pub fn view_title_bar<'a, M: Clone + 'a>(
    title: String,
    on_drag: M,
    on_minimize: M,
    on_maximize: M,
    on_close: M,
) -> Element<'a, M> {
    let title_text = text(title)
        .size(12)
        .color(TEXT_SECONDARY());

    let drag_area = mouse_area(
        container(title_text)
            .padding(Padding::from([0, 12]))
            .height(Length::Fixed(TITLE_BAR_HEIGHT))
            .center_y(Length::Fixed(TITLE_BAR_HEIGHT)),
    )
    .on_press(on_drag);

    let minimize_btn = window_control_button("\u{2013}", CONTROL_HOVER_BG, on_minimize); // –
    let maximize_btn = window_control_button("\u{25A1}", CONTROL_HOVER_BG, on_maximize); // □
    let close_btn = window_control_button("\u{00D7}", CLOSE_HOVER_BG, on_close); // ×

    let controls = row![minimize_btn, maximize_btn, close_btn].spacing(0);

    let content = row![
        container(drag_area).width(Length::Fill),
        controls,
    ]
    .align_y(iced::Alignment::Center)
    .height(Length::Fixed(TITLE_BAR_HEIGHT));

    container(content)
        .width(Length::Fill)
        .height(Length::Fixed(TITLE_BAR_HEIGHT))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(TITLE_BAR_BG)),
            ..container::Style::default()
        })
        .into()
}

fn window_control_button<'a, M: Clone + 'a>(
    label: &'static str,
    hover_bg: Color,
    on_press: M,
) -> Element<'a, M> {
    button(
        text(label)
            .size(14)
            .color(CONTROL_TEXT_COLOR)
            .center(),
    )
    .on_press(on_press)
    .padding(Padding::from([0, 14]))
    .height(Length::Fixed(TITLE_BAR_HEIGHT))
    .style(move |_theme, status| {
        let bg_color = match status {
            button::Status::Hovered | button::Status::Pressed => hover_bg,
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(iced::Background::Color(bg_color)),
            text_color: CONTROL_TEXT_COLOR,
            border: Border::default(),
            ..button::Style::default()
        }
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    enum Msg {
        Drag,
        Min,
        Max,
        Close,
    }

    #[test]
    fn title_bar_renders_without_panic() {
        let _ = view_title_bar(
            "pwsh — Godly Terminal".to_string(),
            Msg::Drag,
            Msg::Min,
            Msg::Max,
            Msg::Close,
        );
    }
}
