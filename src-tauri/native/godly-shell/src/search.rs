//! Find-in-terminal search feature.

use godly_protocol::types::RichGridData;

/// A match location in the grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub row: usize,
    pub col_start: usize,
    pub col_end: usize,
}

/// Search state for the find-in-terminal feature.
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub active: bool,
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub current_index: usize,
    pub regex_mode: bool,
}

impl SearchState {
    pub fn open(&mut self) {
        self.active = true;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.query.clear();
        self.matches.clear();
        self.current_index = 0;
    }

    pub fn set_query(&mut self, query: String, grid: Option<&RichGridData>) {
        self.query = query;
        self.matches.clear();
        self.current_index = 0;
        if let Some(grid) = grid {
            self.matches = find_matches(&self.query, grid, self.regex_mode);
        }
    }

    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_index = (self.current_index + 1) % self.matches.len();
        }
    }

    pub fn prev_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_index = if self.current_index == 0 {
                self.matches.len() - 1
            } else {
                self.current_index - 1
            };
        }
    }

    pub fn toggle_regex(&mut self, grid: Option<&RichGridData>) {
        self.regex_mode = !self.regex_mode;
        self.matches.clear();
        self.current_index = 0;
        if let Some(grid) = grid {
            self.matches = find_matches(&self.query, grid, self.regex_mode);
        }
    }

    pub fn current_match(&self) -> Option<&SearchMatch> {
        self.matches.get(self.current_index)
    }

    pub fn match_info(&self) -> String {
        if self.matches.is_empty() {
            if self.query.is_empty() { String::new() } else { "No matches".to_string() }
        } else {
            format!("{}/{}", self.current_index + 1, self.matches.len())
        }
    }
}

/// Find all matches of a query in the grid data.
pub fn find_matches(query: &str, grid: &RichGridData, regex_mode: bool) -> Vec<SearchMatch> {
    if query.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let query_lower = query.to_lowercase();

    for (row_idx, row) in grid.rows.iter().enumerate() {
        let line: String = row.cells.iter().map(|c| c.content.as_str()).collect();

        if regex_mode {
            let mut start = 0;
            while let Some(pos) = line[start..].find(query) {
                let col_start = start + pos;
                let col_end = col_start + query.len() - 1;
                matches.push(SearchMatch { row: row_idx, col_start, col_end });
                start = col_start + 1;
            }
        } else {
            let line_lower = line.to_lowercase();
            let mut start = 0;
            while let Some(pos) = line_lower[start..].find(&query_lower) {
                let col_start = start + pos;
                let col_end = col_start + query.len() - 1;
                matches.push(SearchMatch { row: row_idx, col_start, col_end });
                start = col_start + 1;
            }
        }
    }

    matches
}
