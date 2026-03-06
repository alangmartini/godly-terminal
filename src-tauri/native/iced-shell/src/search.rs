use godly_protocol::types::RichGridData;
use iced::widget::{button, container, row, text, text_input, Space};
use iced::{Background, Border, Element, Length, Padding};

use crate::theme;

/// A match location in the grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub row: usize,
    pub col_start: usize,
    pub col_end: usize,
}

/// Search state for the find-in-terminal feature.
#[derive(Debug, Clone)]
pub struct SearchState {
    pub active: bool,
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub current_index: usize,
    pub regex_mode: bool,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            active: false,
            query: String::new(),
            matches: Vec::new(),
            current_index: 0,
            regex_mode: false,
        }
    }
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
}

/// Find all matches of a query in the grid data.
///
/// Case-insensitive by default. If regex_mode is true, treat the query
/// as case-sensitive (simple differentiation without a regex crate dep).
pub fn find_matches(query: &str, grid: &RichGridData, regex_mode: bool) -> Vec<SearchMatch> {
    if query.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let query_lower = query.to_lowercase();

    for (row_idx, row) in grid.rows.iter().enumerate() {
        let line: String = row.cells.iter().map(|c| c.content.as_str()).collect();

        if regex_mode {
            // Case-sensitive search in regex mode.
            let mut start = 0;
            while let Some(pos) = line[start..].find(query) {
                let col_start = start + pos;
                let col_end = col_start + query.len() - 1;
                matches.push(SearchMatch {
                    row: row_idx,
                    col_start,
                    col_end,
                });
                start = col_start + 1;
            }
        } else {
            let line_lower = line.to_lowercase();
            let mut start = 0;
            while let Some(pos) = line_lower[start..].find(&query_lower) {
                let col_start = start + pos;
                let col_end = col_start + query.len() - 1;
                matches.push(SearchMatch {
                    row: row_idx,
                    col_start,
                    col_end,
                });
                start = col_start + 1;
            }
        }
    }

    matches
}

/// Render the search bar overlay.
pub fn view_search_bar<'a, M: Clone + 'a>(
    state: &SearchState,
    on_query_changed: impl Fn(String) -> M + 'a,
    on_next: M,
    on_prev: M,
    on_close: M,
    on_toggle_regex: M,
) -> Element<'a, M> {
    let match_info = if state.matches.is_empty() {
        if state.query.is_empty() {
            String::new()
        } else {
            "No matches".to_string()
        }
    } else {
        format!("{}/{}", state.current_index + 1, state.matches.len())
    };

    let input = text_input("Search...", &state.query)
        .on_input(on_query_changed)
        .size(13)
        .width(Length::Fixed(200.0))
        .padding(Padding::from([4, 8]));

    let info = text(match_info).size(12).color(theme::TEXT_SECONDARY());

    let regex_label = if state.regex_mode { ".*" } else { "Aa" };
    let regex_btn = button(text(regex_label).size(12))
        .on_press(on_toggle_regex)
        .padding(Padding::from([3, 8]));

    let prev_btn = button(text("\u{25B2}").size(10))
        .on_press(on_prev)
        .padding(Padding::from([3, 6]));

    let next_btn = button(text("\u{25BC}").size(10))
        .on_press(on_next)
        .padding(Padding::from([3, 6]));

    let close_btn = button(text("\u{2715}").size(12))
        .on_press(on_close)
        .padding(Padding::from([3, 6]));

    let bar = container(
        row![
            input,
            info,
            Space::new().width(4.0),
            regex_btn,
            prev_btn,
            next_btn,
            close_btn
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding(Padding::from([6, 10]))
    .style(|_theme| container::Style {
        background: Some(Background::Color(theme::BG_SECONDARY())),
        border: Border {
            color: theme::BORDER(),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    });

    bar.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use godly_protocol::types::{CursorState, GridDimensions, RichGridCell, RichGridRow};

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
    fn find_single_match() {
        let grid = make_grid(&["hello world"]);
        let matches = find_matches("world", &grid, false);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].row, 0);
        assert_eq!(matches[0].col_start, 6);
        assert_eq!(matches[0].col_end, 10);
    }

    #[test]
    fn find_case_insensitive() {
        let grid = make_grid(&["Hello World"]);
        let matches = find_matches("hello", &grid, false);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn find_multiple_matches() {
        let grid = make_grid(&["foo bar foo baz foo"]);
        let matches = find_matches("foo", &grid, false);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn find_across_rows() {
        let grid = make_grid(&["hello", "hello"]);
        let matches = find_matches("hello", &grid, false);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].row, 0);
        assert_eq!(matches[1].row, 1);
    }

    #[test]
    fn find_empty_query_returns_empty() {
        let grid = make_grid(&["hello"]);
        let matches = find_matches("", &grid, false);
        assert!(matches.is_empty());
    }

    #[test]
    fn find_no_match() {
        let grid = make_grid(&["hello world"]);
        let matches = find_matches("xyz", &grid, false);
        assert!(matches.is_empty());
    }

    #[test]
    fn regex_mode_case_sensitive() {
        let grid = make_grid(&["Hello hello"]);
        let matches = find_matches("Hello", &grid, true);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].col_start, 0);
    }

    #[test]
    fn search_state_open_close() {
        let mut state = SearchState::default();
        assert!(!state.active);
        state.open();
        assert!(state.active);
        state.close();
        assert!(!state.active);
        assert!(state.query.is_empty());
    }

    #[test]
    fn search_state_next_prev_wrap() {
        let mut state = SearchState::default();
        let grid = make_grid(&["aa aa aa"]);
        state.open();
        state.set_query("aa".to_string(), Some(&grid));
        assert_eq!(state.matches.len(), 3);
        assert_eq!(state.current_index, 0);
        state.next_match();
        assert_eq!(state.current_index, 1);
        state.next_match();
        assert_eq!(state.current_index, 2);
        state.next_match();
        assert_eq!(state.current_index, 0); // wrap
        state.prev_match();
        assert_eq!(state.current_index, 2); // wrap back
    }
}
