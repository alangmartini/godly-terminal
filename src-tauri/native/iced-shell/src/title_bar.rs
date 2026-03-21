use iced::widget::{button, canvas, container, mouse_area, row, text};
use iced::{Border, Color, Element, Font, Length, Padding, Point, Rectangle, Renderer, Size, Theme};

use crate::theme::{DANGER, GHOST_HOVER, TEXT_SECONDARY, TITLE_BAR_BG};

/// Height of the custom title bar in logical pixels.
pub const TITLE_BAR_HEIGHT: f32 = 34.0;

/// Small terminal icon drawn via canvas.
struct TerminalIcon {
    color: Color,
}

impl<Message> canvas::Program<Message> for TerminalIcon {
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
        let cx = size.width * 0.5;
        let cy = size.height * 0.5;
        let s = size.width.min(size.height) * 0.36;

        // Monitor outline
        let outline = canvas::Path::rectangle(
            Point::new(cx - s, cy - s * 0.8),
            Size::new(s * 2.0, s * 1.6),
        );
        let stroke = canvas::Stroke::default()
            .with_color(self.color)
            .with_width(1.2);
        frame.stroke(&outline, stroke);

        // Prompt caret ">"
        let caret = canvas::Path::new(|b| {
            b.move_to(Point::new(cx - s * 0.45, cy - s * 0.25));
            b.line_to(Point::new(cx - s * 0.1, cy + s * 0.05));
            b.line_to(Point::new(cx - s * 0.45, cy + s * 0.35));
        });
        let caret_stroke = canvas::Stroke::default()
            .with_color(self.color)
            .with_width(1.3)
            .with_line_cap(canvas::LineCap::Round)
            .with_line_join(canvas::LineJoin::Round);
        frame.stroke(&caret, caret_stroke);

        // Cursor line
        let cursor_line = canvas::Path::line(
            Point::new(cx + s * 0.05, cy + s * 0.35),
            Point::new(cx + s * 0.5, cy + s * 0.35),
        );
        frame.stroke(&cursor_line, stroke);

        vec![frame.into_geometry()]
    }
}

/// Renders the custom window title bar with drag area and control buttons.
pub fn view_title_bar<'a, M: Clone + 'a>(
    title: String,
    font: Font,
    on_drag: M,
    on_minimize: M,
    on_maximize: M,
    on_close: M,
) -> Element<'a, M> {
    let icon = canvas(TerminalIcon {
        color: TEXT_SECONDARY(),
    })
    .width(Length::Fixed(16.0))
    .height(Length::Fixed(16.0));

    let title_text = text(title)
        .size(12.5)
        .color(TEXT_SECONDARY())
        .font(font);

    let title_content = row![
        container(icon).padding(Padding { top: 0.0, right: 6.0, bottom: 0.0, left: 8.0 }),
        title_text,
    ]
    .align_y(iced::Alignment::Center);

    let drag_area = mouse_area(
        container(title_content)
            .height(Length::Fixed(TITLE_BAR_HEIGHT))
            .center_y(Length::Fixed(TITLE_BAR_HEIGHT)),
    )
    .on_press(on_drag);

    let minimize_btn = window_control_button("\u{2013}", GHOST_HOVER(), on_minimize); // –
    let maximize_btn = window_control_button("\u{25A1}", GHOST_HOVER(), on_maximize); // □
    let close_btn = window_control_button("\u{00D7}", DANGER(), on_close); // ×

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
            background: Some(iced::Background::Color(TITLE_BAR_BG())),
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
            .color(TEXT_SECONDARY())
            .center(),
    )
    .on_press(on_press)
    .padding(Padding::from([0, 16]))
    .height(Length::Fixed(TITLE_BAR_HEIGHT))
    .style(move |_theme, status| {
        let bg_color = match status {
            button::Status::Hovered | button::Status::Pressed => hover_bg,
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(iced::Background::Color(bg_color)),
            text_color: TEXT_SECONDARY(),
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
            Font::default(),
            Msg::Drag,
            Msg::Min,
            Msg::Max,
            Msg::Close,
        );
    }
}
