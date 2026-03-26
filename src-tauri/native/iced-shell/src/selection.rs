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

    /// Adjust selection coordinates when the viewport scrolls.
    ///
    /// `delta` is the change in scrollback offset (positive = scrolled up into
    /// history, content moved down in viewport). Both anchor and end shift by
    /// `delta` so the selection stays on the same content.
    pub fn adjust_for_scroll(&mut self, delta: isize) {
        self.anchor.row = (self.anchor.row as isize + delta).max(0) as usize;
        self.end.row = (self.end.row as isize + delta).max(0) as usize;
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

    /// Extract selected text with ANSI escape sequences stripped.
    pub fn selected_text_clean(&self, grid: &RichGridData) -> String {
        let raw = self.selected_text(grid);
        strip_ansi_escapes(&raw)
    }
}

/// Strip ANSI escape sequences from text.
///
/// Removes CSI (ESC[...X), OSC (ESC]...BEL/ST), and single-char escapes.
/// Preserves printable characters, newlines, carriage returns, and tabs.
fn strip_ansi_escapes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if (c as u32) >= 0x40 && (c as u32) <= 0x7E {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(&c) = chars.peek() {
                        if c == '\x07' {
                            chars.next();
                            break;
                        }
                        if c == '\x1b' {
                            chars.next();
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                        chars.next();
                    }
                }
                _ => {
                    chars.next();
                }
            }
        } else if ch >= '\x20' || ch == '\n' || ch == '\r' || ch == '\t' {
            result.push(ch);
        }
    }
    result
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
            cursor: CursorState {
                row: 0,
                col: 0,
                cursor_style: Default::default(),
            },
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

    #[test]
    fn test_strip_ansi_csi() {
        assert_eq!(
            strip_ansi_escapes("hello \x1b[31mworld\x1b[0m"),
            "hello world"
        );
    }

    #[test]
    fn test_strip_ansi_osc() {
        assert_eq!(
            strip_ansi_escapes("before\x1b]0;title\x07after"),
            "beforeafter"
        );
    }

    #[test]
    fn test_strip_preserves_newlines() {
        assert_eq!(strip_ansi_escapes("line1\nline2\n"), "line1\nline2\n");
    }

    #[test]
    fn test_strip_no_escapes() {
        assert_eq!(strip_ansi_escapes("plain text"), "plain text");
    }

    #[test]
    fn test_strip_control_chars() {
        assert_eq!(strip_ansi_escapes("hello\x01\x02world"), "helloworld");
    }

    #[test]
    fn test_selected_text_clean() {
        let mut grid = make_grid(&["hello world"]);
        grid.rows[0].cells[5].content = "\x1b[31m ".to_string();
        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 0, col: 0 });
        sel.update(GridPos { row: 0, col: 10 });
        let clean = sel.selected_text_clean(&grid);
        assert!(!clean.contains('\x1b'));
        assert!(clean.contains("hello"));
        assert!(clean.contains("world"));
    }

    /// Helper: build a RichGridData with the given text lines and scrollback metadata.
    fn make_grid_with_scrollback(
        lines: &[&str],
        scrollback_offset: usize,
        total_scrollback: usize,
    ) -> RichGridData {
        let mut grid = make_grid(lines);
        grid.scrollback_offset = scrollback_offset;
        grid.total_scrollback = total_scrollback;
        grid
    }

    // -----------------------------------------------------------------------
    // Bug #755: Selection anchor not fixed when scrolling during drag
    // -----------------------------------------------------------------------
    //
    // When a user selects text and then scrolls, the selection coordinates
    // are viewport-relative and don't account for scrollback offset changes.
    // After scrolling, the same viewport row shows different content, but
    // the selection still indexes into the viewport rows — returning the
    // wrong text.

    #[test]
    fn test_selection_text_stable_after_scroll() {
        // Bug #755: selected_text() must return the originally-selected
        // content even after the viewport scrolls.
        //
        // Scenario:
        // 1. Viewport shows rows A-E at scrollback_offset=0
        // 2. User selects rows 1-2 ("line B", "line C")
        // 3. User scrolls up 2 lines → scrollback_offset=2
        // 4. scroll_active() calls adjust_for_scroll(2)
        // 5. Viewport now shows [hist X, hist Y, line A, line B, line C]
        // 6. selected_text() should return "line B\nline C" (rows 3-4)

        let grid_before =
            make_grid_with_scrollback(&["line A", "line B", "line C", "line D", "line E"], 0, 10);

        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 1, col: 0 });
        sel.update(GridPos { row: 2, col: 5 });
        assert_eq!(sel.selected_text(&grid_before), "line B\nline C");

        // Scroll up 2 lines — scroll_active() adjusts selection.
        sel.adjust_for_scroll(2);

        let grid_after =
            make_grid_with_scrollback(&["hist X", "hist Y", "line A", "line B", "line C"], 2, 10);

        let text = sel.selected_text(&grid_after);
        assert_eq!(
            text, "line B\nline C",
            "Bug #755: selected_text() should return original content after scroll"
        );
    }

    #[test]
    fn test_selection_highlight_follows_content_after_scroll() {
        // Bug #755: is_selected() must track content, not viewport position.
        //
        // Select rows 2-3, scroll up 2 → content shifts to rows 4-5.
        // adjust_for_scroll(2) should move the selection to match.

        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 2, col: 0 });
        sel.update(GridPos { row: 3, col: 5 });

        assert!(sel.is_selected(2, 0), "row 2 selected before scroll");
        assert!(sel.is_selected(3, 0), "row 3 selected before scroll");
        assert!(!sel.is_selected(4, 0), "row 4 not selected before scroll");

        // Scroll up 2 — content at viewport row 2 moves to row 4.
        sel.adjust_for_scroll(2);

        assert!(
            sel.is_selected(4, 0),
            "Bug #755: After scroll up 2, row 4 should be selected (content moved there)"
        );
        assert!(
            sel.is_selected(5, 0),
            "Bug #755: After scroll up 2, row 5 should be selected (content moved there)"
        );
        assert!(
            !sel.is_selected(2, 0),
            "Bug #755: Row 2 now has different content and should not be selected"
        );
    }

    #[test]
    fn test_active_drag_extends_selection_via_scroll() {
        // Bug #755: During active drag, scrolling should extend selection.
        //
        // 1. Click row 4
        // 2. Scroll up 3 → adjust_for_scroll(3) shifts anchor to row 7
        // 3. Drag to row 0
        // 4. Selection spans rows 0-7 (8 rows of content)

        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 4, col: 0 });

        // Scroll up 3 — anchor shifts from row 4 to row 7.
        sel.adjust_for_scroll(3);

        // User drags to top of viewport.
        sel.update(GridPos { row: 0, col: 0 });

        let (start, end) = sel.normalized();
        assert_eq!(start.row, 0);
        assert_eq!(
            end.row, 7,
            "Bug #755: After scroll up 3 during drag from row 4, anchor should be at row 7"
        );
    }

    #[test]
    fn test_copy_after_scroll_returns_correct_content() {
        // Bug #755: After selecting and scrolling, Ctrl+C must copy the
        // originally-selected text, not whatever is now at those viewport rows.

        // 6-row viewport.
        let grid_before = make_grid_with_scrollback(
            &["alpha", "bravo", "charlie", "delta", "echo", "foxtrot"],
            0,
            10,
        );

        let mut sel = SelectionState::default();
        sel.start(GridPos { row: 1, col: 0 });
        sel.update(GridPos { row: 2, col: 6 });
        sel.finish();
        assert_eq!(sel.selected_text(&grid_before), "bravo\ncharlie");

        // Scroll up 2 — adjust_for_scroll(2) shifts selection to rows 3-4.
        sel.adjust_for_scroll(2);

        let grid_after = make_grid_with_scrollback(
            &["hist 2", "hist 1", "alpha", "bravo", "charlie", "delta"],
            2,
            10,
        );

        let copied = sel.selected_text(&grid_after);
        assert_eq!(
            copied, "bravo\ncharlie",
            "Bug #755: Copy after scroll should return originally-selected content"
        );
    }
}
