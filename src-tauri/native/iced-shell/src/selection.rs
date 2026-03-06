use godly_protocol::types::RichGridData;

/// Grid coordinate (row, column).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridPos {
    pub row: usize,
    pub col: usize,
}

/// Tracks a text selection via mouse drag.
#[derive(Debug, Clone)]
pub struct SelectionState {
    /// Where the mouse was pressed (anchor point).
    anchor: GridPos,
    /// Current drag position (end point).
    end: GridPos,
    /// Whether a selection is currently active (mouse held down).
    pub active: bool,
}

impl Default for SelectionState {
    fn default() -> Self {
        Self {
            anchor: GridPos { row: 0, col: 0 },
            end: GridPos { row: 0, col: 0 },
            active: false,
        }
    }
}

impl SelectionState {
    /// Begin a new selection at the given anchor point.
    pub fn start(&mut self, pos: GridPos) {
        self.anchor = pos;
        self.end = pos;
        self.active = true;
    }

    /// Update the end position during a drag.
    pub fn update(&mut self, pos: GridPos) {
        self.end = pos;
    }

    /// Finish the selection (mouse released). Keeps anchor/end for highlighting.
    pub fn finish(&mut self) {
        self.active = false;
    }

    /// Reset to no selection.
    pub fn clear(&mut self) {
        self.anchor = GridPos { row: 0, col: 0 };
        self.end = GridPos { row: 0, col: 0 };
        self.active = false;
    }

    /// Return (start, end) in reading order (top-left to bottom-right).
    ///
    /// If anchor is after end, they are swapped so start <= end in reading order.
    pub fn normalized(&self) -> (GridPos, GridPos) {
        if self.anchor.row < self.end.row
            || (self.anchor.row == self.end.row && self.anchor.col <= self.end.col)
        {
            (self.anchor, self.end)
        } else {
            (self.end, self.anchor)
        }
    }

    /// Check if a cell at (row, col) falls within the selection range.
    ///
    /// For multi-row selections:
    /// - First row: from start.col to end of line
    /// - Middle rows: fully selected
    /// - Last row: from column 0 to end.col
    pub fn is_selected(&self, row: usize, col: usize) -> bool {
        let (start, end) = self.normalized();

        // Outside the row range entirely.
        if row < start.row || row > end.row {
            return false;
        }

        // Single-row selection.
        if start.row == end.row {
            return col >= start.col && col <= end.col;
        }

        // Multi-row selection.
        if row == start.row {
            // First row: from start.col to end of line.
            col >= start.col
        } else if row == end.row {
            // Last row: from column 0 to end.col.
            col <= end.col
        } else {
            // Middle rows: fully selected.
            true
        }
    }

    /// Extract selected text from the grid data.
    ///
    /// Joins rows with newlines. Trailing spaces are trimmed from each row.
    pub fn selected_text(&self, grid: &RichGridData) -> String {
        let (start, end) = self.normalized();
        let mut lines = Vec::new();

        for row in start.row..=end.row {
            if row >= grid.rows.len() {
                break;
            }

            let grid_row = &grid.rows[row];
            let col_start = if row == start.row { start.col } else { 0 };
            let col_end = if row == end.row {
                end.col
            } else {
                grid_row.cells.len().saturating_sub(1)
            };

            let mut line = String::new();
            for col in col_start..=col_end {
                if col < grid_row.cells.len() {
                    line.push_str(&grid_row.cells[col].content);
                }
            }

            // Trim trailing spaces from each row.
            let trimmed = line.trim_end().to_string();
            lines.push(trimmed);
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use godly_protocol::types::{CursorState, GridDimensions, RichGridCell, RichGridRow};

    /// Helper: build a RichGridData with the given text lines.
    fn make_grid(lines: &[&str]) -> RichGridData {
        let rows = lines
            .iter()
            .map(|line| {
                let cells = line
                    .chars()
                    .map(|ch| RichGridCell {
                        content: ch.to_string(),
                        fg: "default".into(),
                        bg: "default".into(),
                        bold: false,
                        dim: false,
                        italic: false,
                        underline: false,
                        inverse: false,
                        wide: false,
                        wide_continuation: false,
                    })
                    .collect();
                RichGridRow {
                    cells,
                    wrapped: false,
                }
            })
            .collect::<Vec<_>>();

        let num_rows = rows.len();
        let num_cols = lines.first().map(|l| l.len()).unwrap_or(0);

        RichGridData {
            rows,
            cursor: CursorState { row: 0, col: 0 },
            dimensions: GridDimensions {
                rows: num_rows as u16,
                cols: num_cols as u16,
            },
            alternate_screen: false,
            cursor_hidden: false,
            title: String::new(),
            scrollback_offset: 0,
            total_scrollback: 0,
        }
    }

    #[test]
    fn test_start_sets_anchor_and_active() {
        let mut sel = SelectionState::default();
        assert!(!sel.active);

        sel.start(GridPos { row: 3, col: 5 });
        assert!(sel.active);
        assert_eq!(sel.anchor, GridPos { row: 3, col: 5 });
        assert_eq!(sel.end, GridPos { row: 3, col: 5 });
    }

    #[test]
    fn test_update_changes_end_position() {
        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 0, col: 0 });
        sel.update(GridPos { row: 2, col: 10 });
        assert_eq!(sel.end, GridPos { row: 2, col: 10 });
        // Anchor unchanged.
        assert_eq!(sel.anchor, GridPos { row: 0, col: 0 });
    }

    #[test]
    fn test_finish_clears_active_but_preserves_positions() {
        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 1, col: 2 });
        sel.update(GridPos { row: 3, col: 4 });
        sel.finish();

        assert!(!sel.active);
        assert_eq!(sel.anchor, GridPos { row: 1, col: 2 });
        assert_eq!(sel.end, GridPos { row: 3, col: 4 });
    }

    #[test]
    fn test_clear_resets_everything() {
        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 5, col: 10 });
        sel.update(GridPos { row: 8, col: 3 });
        sel.clear();

        assert!(!sel.active);
        assert_eq!(sel.anchor, GridPos { row: 0, col: 0 });
        assert_eq!(sel.end, GridPos { row: 0, col: 0 });
    }

    #[test]
    fn test_normalized_forward_selection() {
        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 1, col: 5 });
        sel.update(GridPos { row: 3, col: 10 });

        let (start, end) = sel.normalized();
        assert_eq!(start, GridPos { row: 1, col: 5 });
        assert_eq!(end, GridPos { row: 3, col: 10 });
    }

    #[test]
    fn test_normalized_backward_selection() {
        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 3, col: 10 });
        sel.update(GridPos { row: 1, col: 5 });

        let (start, end) = sel.normalized();
        assert_eq!(start, GridPos { row: 1, col: 5 });
        assert_eq!(end, GridPos { row: 3, col: 10 });
    }

    #[test]
    fn test_normalized_same_row_backward() {
        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 2, col: 8 });
        sel.update(GridPos { row: 2, col: 3 });

        let (start, end) = sel.normalized();
        assert_eq!(start, GridPos { row: 2, col: 3 });
        assert_eq!(end, GridPos { row: 2, col: 8 });
    }

    #[test]
    fn test_is_selected_single_row() {
        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 2, col: 3 });
        sel.update(GridPos { row: 2, col: 7 });

        // Within range.
        assert!(sel.is_selected(2, 3));
        assert!(sel.is_selected(2, 5));
        assert!(sel.is_selected(2, 7));

        // Outside range.
        assert!(!sel.is_selected(2, 2));
        assert!(!sel.is_selected(2, 8));
        assert!(!sel.is_selected(1, 5));
        assert!(!sel.is_selected(3, 5));
    }

    #[test]
    fn test_is_selected_multi_row() {
        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 1, col: 5 });
        sel.update(GridPos { row: 3, col: 3 });

        // First row: col >= 5 is selected.
        assert!(!sel.is_selected(1, 4));
        assert!(sel.is_selected(1, 5));
        assert!(sel.is_selected(1, 50));

        // Middle row (row 2): fully selected.
        assert!(sel.is_selected(2, 0));
        assert!(sel.is_selected(2, 100));

        // Last row: col <= 3 is selected.
        assert!(sel.is_selected(3, 0));
        assert!(sel.is_selected(3, 3));
        assert!(!sel.is_selected(3, 4));

        // Outside row range.
        assert!(!sel.is_selected(0, 5));
        assert!(!sel.is_selected(4, 0));
    }

    #[test]
    fn test_is_selected_cell_outside_returns_false() {
        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 5, col: 10 });
        sel.update(GridPos { row: 5, col: 15 });

        assert!(!sel.is_selected(0, 0));
        assert!(!sel.is_selected(4, 12));
        assert!(!sel.is_selected(6, 12));
        assert!(!sel.is_selected(5, 9));
        assert!(!sel.is_selected(5, 16));
    }

    #[test]
    fn test_selected_text_single_row() {
        let grid = make_grid(&["Hello, world!", "Second line  "]);

        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 0, col: 0 });
        sel.update(GridPos { row: 0, col: 4 });

        assert_eq!(sel.selected_text(&grid), "Hello");
    }

    #[test]
    fn test_selected_text_multi_row() {
        let grid = make_grid(&["Hello, world!", "Second line  ", "Third line   "]);

        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 0, col: 7 });
        sel.update(GridPos { row: 2, col: 4 });

        let text = sel.selected_text(&grid);
        // Row 0: from col 7 to end -> "world!" (trimmed)
        // Row 1: full row -> "Second line" (trimmed)
        // Row 2: from col 0 to 4 -> "Third"
        assert_eq!(text, "world!\nSecond line\nThird");
    }

    #[test]
    fn test_selected_text_trims_trailing_spaces() {
        let grid = make_grid(&["abc   ", "def   "]);

        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 0, col: 0 });
        sel.update(GridPos { row: 1, col: 5 });

        let text = sel.selected_text(&grid);
        assert_eq!(text, "abc\ndef");
    }

    #[test]
    fn test_selected_text_backward_selection() {
        let grid = make_grid(&["Hello, world!"]);

        let mut sel = SelectionState::default();
        // Select backward: anchor after end.
        sel.start(GridPos { row: 0, col: 7 });
        sel.update(GridPos { row: 0, col: 0 });

        assert_eq!(sel.selected_text(&grid), "Hello, w");
    }

    #[test]
    fn test_selected_text_empty_grid() {
        let grid = make_grid(&[]);
        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 0, col: 0 });
        sel.update(GridPos { row: 0, col: 5 });

        assert_eq!(sel.selected_text(&grid), "");
    }

    #[test]
    fn test_default_is_no_selection() {
        let sel = SelectionState::default();
        assert!(!sel.active);
        assert_eq!(sel.anchor, GridPos { row: 0, col: 0 });
        assert_eq!(sel.end, GridPos { row: 0, col: 0 });
    }
}
