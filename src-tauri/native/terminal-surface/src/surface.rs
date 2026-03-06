use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Font, Pixels, Point, Rectangle, Renderer, Size, Theme};

use godly_protocol::types::RichGridData;

use crate::colors::{brighten_color, dim_color, parse_color};
use crate::font_metrics::FontMetrics;

/// Grid position for selection rendering (local to avoid cross-crate dependency).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridPos {
    pub row: usize,
    pub col: usize,
}

/// Default terminal foreground (light gray).
pub const DEFAULT_FG: Color = Color {
    r: 0.8,
    g: 0.8,
    b: 0.8,
    a: 1.0,
};

/// Default terminal background (near-black).
pub const DEFAULT_BG: Color = Color {
    r: 0.07,
    g: 0.07,
    b: 0.10,
    a: 1.0,
};

/// Minimal internal state for the Canvas `Program`.
///
/// Grid data and font metrics live on [`TerminalCanvas`] itself so that the
/// parent widget can update them directly. This struct exists only to satisfy
/// the `Program::State` associated type.
#[derive(Debug, Default)]
pub struct TerminalCanvasState;

/// Terminal canvas program that renders a RichGridData snapshot.
///
/// Grid data is borrowed for the duration of the render, avoiding a full clone
/// of `RichGridData` per pane per frame. Font metrics and selection state are
/// cheap `Copy` types carried by value.
pub struct TerminalCanvas<'a> {
    /// Borrowed grid data from the daemon (avoids cloning per frame).
    pub grid: Option<&'a RichGridData>,
    /// Font metrics for cell sizing.
    pub metrics: FontMetrics,
    /// Optional selection range (start, end) in reading order.
    /// When set, draws a semi-transparent blue overlay on selected cells.
    pub selection: Option<(GridPos, GridPos)>,
}

impl Default for TerminalCanvas<'_> {
    fn default() -> Self {
        Self {
            grid: None,
            metrics: FontMetrics::default(),
            selection: None,
        }
    }
}

impl<'a> TerminalCanvas<'a> {
    /// Create a new terminal canvas with the given font metrics.
    pub fn new(metrics: FontMetrics) -> Self {
        Self {
            grid: None,
            metrics,
            selection: None,
        }
    }
}

impl<Message> canvas::Program<Message> for TerminalCanvas<'_> {
    type State = TerminalCanvasState;

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Fill background
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), DEFAULT_BG);

        let grid = match &self.grid {
            Some(g) => g,
            None => return vec![frame.into_geometry()],
        };

        let cell_w = self.metrics.cell_width;
        let cell_h = self.metrics.cell_height;
        let font_size = self.metrics.font_size;
        let monospace = Font::MONOSPACE;

        // Draw each cell
        for (row_idx, row) in grid.rows.iter().enumerate() {
            let y = row_idx as f32 * cell_h;

            for (col_idx, cell) in row.cells.iter().enumerate() {
                // Skip wide continuation cells (drawn by the wide char's cell)
                if cell.wide_continuation {
                    continue;
                }

                let x = col_idx as f32 * cell_w;
                let char_width = if cell.wide { 2.0 } else { 1.0 };

                // Resolve colors, handling inverse attribute
                let (mut fg, bg) = if cell.inverse {
                    (
                        parse_color(&cell.bg, DEFAULT_BG),
                        parse_color(&cell.fg, DEFAULT_FG),
                    )
                } else {
                    (
                        parse_color(&cell.fg, DEFAULT_FG),
                        parse_color(&cell.bg, DEFAULT_BG),
                    )
                };

                if cell.dim {
                    fg = dim_color(fg);
                }

                // Bold brightening (most terminals render bold as brighter)
                if cell.bold {
                    fg = brighten_color(fg);
                }

                // Draw background (only if non-default to avoid overdraw)
                if bg.r != DEFAULT_BG.r || bg.g != DEFAULT_BG.g || bg.b != DEFAULT_BG.b {
                    frame.fill_rectangle(
                        Point::new(x, y),
                        Size::new(cell_w * char_width, cell_h),
                        bg,
                    );
                }

                // Determine font variant based on cell attributes
                let cell_font = match (cell.bold, cell.italic) {
                    (false, false) => monospace,
                    (true, false) => Font {
                        weight: iced::font::Weight::Bold,
                        ..monospace
                    },
                    (false, true) => Font {
                        style: iced::font::Style::Italic,
                        ..monospace
                    },
                    (true, true) => Font {
                        weight: iced::font::Weight::Bold,
                        style: iced::font::Style::Italic,
                        ..monospace
                    },
                };

                // Draw text content
                if !cell.content.is_empty() && cell.content != " " {
                    let text = canvas::Text {
                        content: cell.content.clone(),
                        position: Point::new(x, y),
                        color: fg,
                        size: Pixels(font_size),
                        font: cell_font,
                        ..canvas::Text::default()
                    };
                    frame.fill_text(text);
                }

                // Draw underline
                if cell.underline {
                    let underline_y = y + cell_h - 1.5;
                    let path = canvas::Path::line(
                        Point::new(x, underline_y),
                        Point::new(x + cell_w * char_width, underline_y),
                    );
                    frame.stroke(
                        &path,
                        canvas::Stroke::default().with_color(fg).with_width(1.0),
                    );
                }
            }
        }

        // Draw selection overlay
        if let Some((start, end)) = &self.selection {
            let selection_color = Color::from_rgba(0.2, 0.4, 0.8, 0.3); // semi-transparent blue
            for row in start.row..=end.row {
                if row >= grid.rows.len() {
                    break;
                }
                let y = row as f32 * cell_h;
                let col_start = if row == start.row { start.col } else { 0 };
                let col_end = if row == end.row {
                    end.col
                } else {
                    grid.rows[row].cells.len().saturating_sub(1)
                };
                let x = col_start as f32 * cell_w;
                let width = ((col_end - col_start + 1) as f32) * cell_w;
                frame.fill_rectangle(Point::new(x, y), Size::new(width, cell_h), selection_color);
            }
        }

        // Draw cursor
        if !grid.cursor_hidden {
            let cursor_x = grid.cursor.col as f32 * cell_w;
            let cursor_y = grid.cursor.row as f32 * cell_h;

            let cursor_path =
                canvas::Path::rectangle(Point::new(cursor_x, cursor_y), Size::new(cell_w, cell_h));
            frame.stroke(
                &cursor_path,
                canvas::Stroke::default()
                    .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.8))
                    .with_width(1.5),
            );
        }

        vec![frame.into_geometry()]
    }
}
