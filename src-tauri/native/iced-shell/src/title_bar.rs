use iced::widget::{button, canvas, column, container, mouse_area, row, rule, text};
use iced::{
    Border, Color, Element, Font, Length, Padding, Point, Rectangle, Renderer, Size, Theme,
};

use crate::theme::{BORDER, DANGER, GHOST_HOVER, TEXT_SECONDARY, TITLE_BAR_BG};

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

// ---------------------------------------------------------------------------
// Canvas-drawn window control icons (minimize, maximize, close).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum WindowControlKind {
    Minimize,
    Maximize,
    Close,
}

struct WindowControlIcon {
    kind: WindowControlKind,
    color: Color,
}

impl<Message> canvas::Program<Message> for WindowControlIcon {
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
        let w = bounds.size().width;
        let h = bounds.size().height;
        let cx = w * 0.5;
        let cy = h * 0.5;

        let stroke = canvas::Stroke::default()
            .with_color(self.color)
            .with_width(1.0)
            .with_line_cap(canvas::LineCap::Round)
            .with_line_join(canvas::LineJoin::Round);

        match self.kind {
            WindowControlKind::Minimize => {
                // Horizontal line
                let s = w.min(h) * 0.35;
                let line = canvas::Path::line(Point::new(cx - s, cy), Point::new(cx + s, cy));
                frame.stroke(&line, stroke);
            }
            WindowControlKind::Maximize => {
                // Square outline
                let s = w.min(h) * 0.28;
                let rect = canvas::Path::rectangle(
                    Point::new(cx - s, cy - s),
                    Size::new(s * 2.0, s * 2.0),
                );
                frame.stroke(&rect, stroke);
            }
            WindowControlKind::Close => {
                // X shape
                let s = w.min(h) * 0.28;
                let line1 =
                    canvas::Path::line(Point::new(cx - s, cy - s), Point::new(cx + s, cy + s));
                let line2 =
                    canvas::Path::line(Point::new(cx + s, cy - s), Point::new(cx - s, cy + s));
                frame.stroke(&line1, stroke);
                frame.stroke(&line2, stroke);
            }
        }

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

    let title_text = text(title).size(12.5).font(font).color(TEXT_SECONDARY());

    let title_content = row![
        container(icon).padding(Padding {
            top: 0.0,
            right: 6.0,
            bottom: 0.0,
            left: 8.0
        }),
        title_text,
    ]
    .align_y(iced::Alignment::Center);

    let drag_area = mouse_area(
        container(title_content)
            .height(Length::Fixed(TITLE_BAR_HEIGHT))
            .center_y(Length::Fixed(TITLE_BAR_HEIGHT)),
    )
    .on_press(on_drag);

    let minimize_btn =
        window_control_icon_button(WindowControlKind::Minimize, GHOST_HOVER(), on_minimize);
    let maximize_btn =
        window_control_icon_button(WindowControlKind::Maximize, GHOST_HOVER(), on_maximize);
    let close_btn = window_control_icon_button(WindowControlKind::Close, DANGER(), on_close);

    let controls = row![minimize_btn, maximize_btn, close_btn].spacing(0);

    let content = row![container(drag_area).width(Length::Fill), controls,]
        .align_y(iced::Alignment::Center)
        .height(Length::Fixed(TITLE_BAR_HEIGHT));

    let bar = container(content)
        .width(Length::Fill)
        .height(Length::Fixed(TITLE_BAR_HEIGHT))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(TITLE_BAR_BG())),
            ..container::Style::default()
        });

    let title_sep_color = {
        let b = BORDER();
        Color::from_rgba(b.r, b.g, b.b, 0.55)
    };
    let separator = rule::horizontal(1).style(move |_theme| rule::Style {
        color: title_sep_color,
        radius: 0.0.into(),
        fill_mode: rule::FillMode::Full,
        snap: true,
    });

    column![bar, separator].width(Length::Fill).into()
}

fn window_control_icon_button<'a, M: Clone + 'a>(
    kind: WindowControlKind,
    hover_bg: Color,
    on_press: M,
) -> Element<'a, M> {
    let is_close = matches!(kind, WindowControlKind::Close);
    let icon_color = TEXT_SECONDARY();
    let icon = canvas(WindowControlIcon {
        kind,
        color: icon_color,
    })
    .width(Length::Fixed(11.0))
    .height(Length::Fixed(11.0));

    button(
        container(icon)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .on_press(on_press)
    .padding(Padding::from([0, 16]))
    .height(Length::Fixed(TITLE_BAR_HEIGHT))
    .width(Length::Fixed(46.0))
    .style(move |_theme, status| {
        let (bg_color, border) = match status {
            button::Status::Hovered | button::Status::Pressed => {
                if is_close {
                    // Windows-standard red close button hover
                    (
                        Color::from_rgb(0.77, 0.17, 0.11),
                        Border {
                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.06),
                            width: 1.0,
                            radius: 5.0.into(),
                        },
                    )
                } else {
                    (
                        hover_bg,
                        Border {
                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.06),
                            width: 1.0,
                            radius: 5.0.into(),
                        },
                    )
                }
            }
            _ => (Color::TRANSPARENT, Border::default()),
        };
        button::Style {
            background: Some(iced::Background::Color(bg_color)),
            text_color: icon_color,
            border,
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
