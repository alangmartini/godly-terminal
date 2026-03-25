//! tmux format string expansion.
//!
//! Supports `#{variable}` syntax for format strings like `-F "#{pane_id}"`.

use std::collections::HashMap;

/// Expand tmux format variables in a format string.
///
/// Replaces `#{key}` with the corresponding value from `vars`.
/// Unknown variables are replaced with empty strings.
pub fn expand_format(format: &str, vars: &HashMap<&str, String>) -> String {
    let mut result = String::with_capacity(format.len());
    let mut chars = format.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '#' && chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut key = String::new();
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                key.push(c);
            }
            if let Some(val) = vars.get(key.as_str()) {
                result.push_str(val);
            }
            // Unknown vars expand to empty string
        } else {
            result.push(ch);
        }
    }

    result
}

/// Build the default format string output for list-panes.
///
/// Default tmux list-panes output format:
/// `0: [80x24] [history 0/0, 0 bytes] %0 (active)`
pub fn default_list_panes_line(
    index: usize,
    cols: u16,
    rows: u16,
    pane_id: &str,
    active: bool,
) -> String {
    let active_str = if active { " (active)" } else { "" };
    format!(
        "{}: [{}x{}] [history 0/0, 0 bytes] {}{}",
        index, cols, rows, pane_id, active_str
    )
}

/// Build the common format variables for a pane.
pub fn pane_format_vars<'a>(
    pane_id: &'a str,
    pane_index: usize,
    cols: u16,
    rows: u16,
    active: bool,
    session: &'a str,
) -> HashMap<&'a str, String> {
    let mut vars = HashMap::new();
    vars.insert("pane_id", pane_id.to_string());
    vars.insert("pane_index", pane_index.to_string());
    vars.insert("pane_width", cols.to_string());
    vars.insert("pane_height", rows.to_string());
    vars.insert("pane_active", if active { "1" } else { "0" }.to_string());
    vars.insert("session_name", session.to_string());
    vars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_simple_variable() {
        let mut vars = HashMap::new();
        vars.insert("pane_id", "%0".to_string());
        assert_eq!(expand_format("#{pane_id}", &vars), "%0");
    }

    #[test]
    fn expand_multiple_variables() {
        let mut vars = HashMap::new();
        vars.insert("pane_id", "%1".to_string());
        vars.insert("pane_width", "80".to_string());
        vars.insert("pane_height", "24".to_string());
        assert_eq!(
            expand_format("#{pane_id}: #{pane_width}x#{pane_height}", &vars),
            "%1: 80x24"
        );
    }

    #[test]
    fn expand_unknown_variable() {
        let vars = HashMap::new();
        assert_eq!(expand_format("#{unknown}", &vars), "");
    }

    #[test]
    fn expand_no_variables() {
        let vars = HashMap::new();
        assert_eq!(expand_format("plain text", &vars), "plain text");
    }

    #[test]
    fn expand_mixed_text_and_variables() {
        let mut vars = HashMap::new();
        vars.insert("pane_id", "%2".to_string());
        assert_eq!(expand_format("pane=#{pane_id}!", &vars), "pane=%2!");
    }

    #[test]
    fn default_list_panes_active() {
        let line = default_list_panes_line(0, 80, 24, "%0", true);
        assert_eq!(line, "0: [80x24] [history 0/0, 0 bytes] %0 (active)");
    }

    #[test]
    fn default_list_panes_inactive() {
        let line = default_list_panes_line(1, 120, 30, "%1", false);
        assert_eq!(line, "1: [120x30] [history 0/0, 0 bytes] %1");
    }

    #[test]
    fn pane_format_vars_builds_map() {
        let vars = pane_format_vars("%0", 0, 80, 24, true, "main");
        assert_eq!(vars.get("pane_id"), Some(&"%0".to_string()));
        assert_eq!(vars.get("pane_width"), Some(&"80".to_string()));
        assert_eq!(vars.get("pane_height"), Some(&"24".to_string()));
        assert_eq!(vars.get("pane_active"), Some(&"1".to_string()));
        assert_eq!(vars.get("session_name"), Some(&"main".to_string()));
    }
}
