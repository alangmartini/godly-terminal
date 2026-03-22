use iced::widget::{column, container, text};
use iced::{Background, Border, Color, Element, Padding};

/// Render a small transparent performance overlay.
pub fn view_perf_overlay<'a, M: 'a>(
    fps: f32,
    frame_ms: f32,
    render_ms: f32,
    renderer_type: &str,
    terminal_count: usize,
) -> Element<'a, M> {
    let green = Color::from_rgba(0.0, 1.0, 0.0, 0.9);
    let content = column![
        text(format!("FPS: {:.0}", fps))
            .size(11)
            .color(green),
        text(format!("Frame: {:.1}ms", frame_ms))
            .size(11)
            .color(green),
        text(format!("Render: {:.2}ms", render_ms))
            .size(11)
            .color(green),
        text(format!("Renderer: {}", renderer_type))
            .size(11)
            .color(green),
        text(format!("Terminals: {}", terminal_count))
            .size(11)
            .color(green),
    ]
    .spacing(2);

    container(content)
        .padding(Padding::from([6, 10]))
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.65))),
            border: Border {
                color: Color::from_rgba(0.0, 1.0, 0.0, 0.3),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    enum TestMsg {}

    #[test]
    fn perf_overlay_renders() {
        let _el: Element<'_, TestMsg> = view_perf_overlay(60.0, 16.6, 0.5, "Canvas", 5);
    }
}
