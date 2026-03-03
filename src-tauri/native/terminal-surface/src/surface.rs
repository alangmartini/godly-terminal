use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Font, Pixels, Point, Rectangle, Renderer, Size, Theme};

use godly_protocol::types::RichGridData;

use crate::colors::{dim_color, parse_color};

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

/// Terminal canvas state holding the current grid snapshot and font metrics.
pub struct TerminalCanvasState {
    /// Current grid data from the daemon.
    pub grid: Option<RichGridData>,
    /// Width of a single cell in pixels.
    pub cell_width: f32,
    /// Height of a single cell in pixels.
    pub cell_height: f32,
    /// Font size in pixels.
    pub font_size: f32,
}

impl Default for TerminalCanvasState {
    fn default() -> Self {
        Self {
            grid: None,
            cell_width: 9.0,
            cell_height: 18.0,
            font_size: 14.0,
        }
    }
}

/// Terminal canvas program that renders a RichGridData snapshot.
///
/// Uses Iced's Canvas widget with the `Program` trait. Drawing is done
/// via `Frame::fill_rectangle()` for backgrounds and `Frame::fill_text()`
/// for cell content.
pub struct TerminalCanvas;

impl<Message> canvas::Program<Message> for TerminalCanvas {
    type State = TerminalCanvasState;

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Fill background
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), DEFAULT_BG);

        let grid = match &state.grid {
            Some(g) => g,
            None => return vec![frame.into_geometry()],
        };

        let cell_w = state.cell_width;
        let cell_h = state.cell_height;
        let font_size = state.font_size;
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

                // Draw background (only if non-default to avoid overdraw)
                if bg.r != DEFAULT_BG.r || bg.g != DEFAULT_BG.g || bg.b != DEFAULT_BG.b {
                    frame.fill_rectangle(
                        Point::new(x, y),
                        Size::new(cell_w * char_width, cell_h),
                        bg,
                    );
                }

                // Draw text content
                if !cell.content.is_empty() && cell.content != " " {
                    let text = canvas::Text {
                        content: cell.content.clone(),
                        position: Point::new(x, y),
                        color: fg,
                        size: Pixels(font_size),
                        font: monospace,
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

        // Draw cursor
        if !grid.cursor_hidden {
            let cursor_x = grid.cursor.col as f32 * cell_w;
            let cursor_y = grid.cursor.row as f32 * cell_h;

            let cursor_path = canvas::Path::rectangle(
                Point::new(cursor_x, cursor_y),
                Size::new(cell_w, cell_h),
            );
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
