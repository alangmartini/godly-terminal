use iced::widget::{column, container, text};
use iced::{Background, Border, Color, Element, Padding};

/// Render a small transparent performance overlay.
pub fn view_perf_overlay<'a, M: 'a>(
    fps: f32,
    frame_ms: f32,
    render_ms: f32,
    renderer_type: &str,
    terminal_count: usize,
    dropped_frames: u64,
    phase_ms: (f32, f32, f32, f32),
    cells_rendered: u32,
    histogram: &[u32; 5],
) -> Element<'a, M> {
    let green = Color::from_rgba(0.0, 1.0, 0.0, 0.9);
    let dim_green = Color::from_rgba(0.0, 0.8, 0.0, 0.6);
    let red = Color::from_rgba(1.0, 0.2, 0.2, 0.9);

    let dropped_color = if dropped_frames > 0 { red } else { green };

    let content = column![
        text(format!("FPS: {:.0}", fps)).size(11).color(green),
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
        text("── Render Phases ──").size(10).color(dim_green),
        text(format!(
            "BG: {:.2}ms | Glyph: {:.2}ms",
            phase_ms.0, phase_ms.1
        ))
        .size(10)
        .color(green),
        text(format!(
            "Cursor: {:.2}ms | Select: {:.2}ms",
            phase_ms.2, phase_ms.3
        ))
        .size(10)
        .color(green),
        text(format!("Cells: {}", cells_rendered))
            .size(10)
            .color(green),
        text("── Frame Health ──").size(10).color(dim_green),
        text(format!("Dropped: {}", dropped_frames))
            .size(10)
            .color(dropped_color),
        text(format!(
            "<8: {} | 8-16: {} | 16-33: {} | 33-100: {} | >100: {}",
            histogram[0], histogram[1], histogram[2], histogram[3], histogram[4]
        ))
        .size(10)
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
        let histogram = [100, 50, 10, 2, 0];
        let _el: Element<'_, TestMsg> = view_perf_overlay(
            60.0,
            16.6,
            0.5,
            "Canvas",
            5,
            3,
            (0.10, 0.25, 0.01, 0.02),
            1024,
            &histogram,
        );
    }
}
