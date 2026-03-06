use iced::widget::{canvas, Space};
use iced::{mouse, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

/// Scrollbar dimensions.
const TRACK_WIDTH: f32 = 8.0;
pub const MIN_THUMB_HEIGHT: f32 = 20.0;
const TRACK_MARGIN: f32 = 2.0;

/// Computed scrollbar thumb position and size.
pub struct ScrollbarMetrics {
    pub thumb_y: f32,
    pub thumb_height: f32,
    pub track_height: f32,
}

pub fn compute_metrics(
    total_lines: usize,
    visible_lines: usize,
    scroll_offset: usize,
    track_height: f32,
) -> ScrollbarMetrics {
    if total_lines == 0 || total_lines <= visible_lines {
        return ScrollbarMetrics {
            thumb_y: 0.0,
            thumb_height: track_height,
            track_height,
        };
    }

    let ratio = visible_lines as f32 / total_lines as f32;
    let thumb_height = (track_height * ratio).max(MIN_THUMB_HEIGHT);
    let scrollable = track_height - thumb_height;
    let max_offset = total_lines.saturating_sub(visible_lines);
    let progress = if max_offset > 0 {
        scroll_offset as f32 / max_offset as f32
    } else {
        0.0
    };
    // scroll_offset 0 = bottom (most recent), so invert
    let thumb_y = scrollable * (1.0 - progress);

    ScrollbarMetrics {
        thumb_y,
        thumb_height,
        track_height,
    }
}

struct ScrollbarCanvas {
    metrics: ScrollbarMetrics,
}

impl canvas::Program<()> for ScrollbarCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Track background
        let track = canvas::Path::rectangle(
            Point::new(TRACK_MARGIN, 0.0),
            Size::new(TRACK_WIDTH - TRACK_MARGIN * 2.0, bounds.height),
        );
        frame.fill(&track, Color::from_rgba(0.2, 0.2, 0.2, 0.3));

        // Thumb
        let thumb_color = Color::from_rgba(0.4, 0.4, 0.4, 0.5);
        let w = TRACK_WIDTH - TRACK_MARGIN * 2.0;
        let h = self.metrics.thumb_height;
        let y = self.metrics.thumb_y;
        let thumb = canvas::Path::rectangle(Point::new(TRACK_MARGIN, y), Size::new(w, h));
        frame.fill(&thumb, thumb_color);

        vec![frame.into_geometry()]
    }
}

/// Render the scrollbar as an Element. Returns zero-width space if not needed.
pub fn view_scrollbar<'a>(
    total_lines: usize,
    visible_lines: usize,
    scroll_offset: usize,
    track_height: f32,
) -> Element<'a, ()> {
    let show = total_lines > visible_lines;

    if !show {
        return Space::new().width(0.0).height(0.0).into();
    }

    let metrics = compute_metrics(total_lines, visible_lines, scroll_offset, track_height);

    canvas(ScrollbarCanvas { metrics })
        .width(Length::Fixed(TRACK_WIDTH))
        .height(Length::Fixed(track_height))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_when_all_visible() {
        let m = compute_metrics(10, 20, 0, 400.0);
        assert_eq!(m.thumb_height, 400.0);
        assert_eq!(m.thumb_y, 0.0);
    }

    #[test]
    fn metrics_proportional_thumb() {
        let m = compute_metrics(100, 25, 0, 400.0);
        assert_eq!(m.thumb_height, 100.0); // 25% of 400
    }

    #[test]
    fn metrics_min_thumb_height() {
        let m = compute_metrics(10000, 10, 0, 400.0);
        assert_eq!(m.thumb_height, MIN_THUMB_HEIGHT);
    }

    #[test]
    fn metrics_scroll_at_bottom() {
        let m = compute_metrics(100, 25, 0, 400.0);
        // offset 0 = bottom, thumb should be at bottom
        let expected_y = 400.0 - 100.0; // track - thumb
        assert!((m.thumb_y - expected_y).abs() < 0.01);
    }

    #[test]
    fn metrics_scroll_at_top() {
        let m = compute_metrics(100, 25, 75, 400.0);
        // offset 75 = top of scrollback
        assert!(m.thumb_y < 1.0); // thumb near top
    }

    #[test]
    fn metrics_zero_total() {
        let m = compute_metrics(0, 25, 0, 400.0);
        assert_eq!(m.thumb_height, 400.0);
    }
}
