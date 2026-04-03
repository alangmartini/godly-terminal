use godly_protocol::types::RichGridData;
use godly_terminal_surface::GridPos;

/// Tracks a text selection via mouse drag.
#[derive(Debug, Clone)]
pub struct SelectionState {
    anchor: GridPos,
    end: GridPos,
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
    pub fn start(&mut self, pos: GridPos) {
        self.anchor = pos;
        self.end = pos;
        self.active = true;
    }

    pub fn update(&mut self, pos: GridPos) {
        self.end = pos;
    }

    pub fn finish(&mut self) {
        self.active = false;
    }

    pub fn adjust_for_scroll(&mut self, delta: isize) {
        self.anchor.row = (self.anchor.row as isize + delta).max(0) as usize;
        self.end.row = (self.end.row as isize + delta).max(0) as usize;
    }

    pub fn clear(&mut self) {
        self.anchor = GridPos { row: 0, col: 0 };
        self.end = GridPos { row: 0, col: 0 };
        self.active = false;
    }

    pub fn normalized(&self) -> (GridPos, GridPos) {
        if self.anchor.row < self.end.row
            || (self.anchor.row == self.end.row && self.anchor.col <= self.end.col)
        {
            (self.anchor, self.end)
        } else {
            (self.end, self.anchor)
        }
    }

    pub fn is_selected(&self, row: usize, col: usize) -> bool {
        let (start, end) = self.normalized();
        if row < start.row || row > end.row {
            return false;
        }
        if start.row == end.row {
            return col >= start.col && col <= end.col;
        }
        if row == start.row {
            col >= start.col
        } else if row == end.row {
            col <= end.col
        } else {
            true
        }
    }

    pub fn has_selection(&self) -> bool {
        self.anchor != self.end
    }

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
            lines.push(line.trim_end().to_string());
        }
        lines.join("\n")
    }
}
