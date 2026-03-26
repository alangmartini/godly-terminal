use std::collections::HashMap;

/// Variables available for tmux format string expansion.
#[derive(Debug, Default)]
pub struct FormatVars {
    pub pane_id: String,
    pub session_name: String,
    pub pane_width: u32,
    pub pane_height: u32,
    pub window_index: u32,
}

/// Expand tmux format strings using the fixed `FormatVars` struct.
///
/// Supported: `#{pane_id}`, `#{session_name}`, `#{pane_width}`,
/// `#{pane_height}`, `#{window_index}`.
pub fn expand(format: &str, vars: &FormatVars) -> String {
    let mut result = format.to_string();
    result = result.replace("#{pane_id}", &vars.pane_id);
    result = result.replace("#{session_name}", &vars.session_name);
    result = result.replace("#{pane_width}", &vars.pane_width.to_string());
    result = result.replace("#{pane_height}", &vars.pane_height.to_string());
    result = result.replace("#{window_index}", &vars.window_index.to_string());
    result
}

/// Expand tmux format variables using a HashMap.
///
/// Replaces `#{key}` with the corresponding value from `vars`.
/// Unknown variables expand to empty strings.
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
        } else {
            result.push(ch);
        }
    }

    result
}

/// Build common format variables for a pane.
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
    vars.insert("window_index", "0".to_string());
    vars
}

/// Default tmux `list-panes` output format.
///
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_pane_id() {
        let vars = FormatVars {
            pane_id: "%0".to_string(),
            ..Default::default()
        };
        assert_eq!(expand("#{pane_id}", &vars), "%0");
    }

    #[test]
    fn expand_session_name() {
        let vars = FormatVars {
            session_name: "work".to_string(),
            ..Default::default()
        };
        assert_eq!(expand("#{session_name}", &vars), "work");
    }

    #[test]
    fn expand_multiple_vars() {
        let vars = FormatVars {
            pane_id: "%3".to_string(),
            session_name: "dev".to_string(),
            pane_width: 120,
            pane_height: 40,
            window_index: 0,
        };
        let result = expand("#{session_name}:#{window_index}.#{pane_id}", &vars);
        assert_eq!(result, "dev:0.%3");
    }

    #[test]
    fn expand_dimensions() {
        let vars = FormatVars {
            pane_width: 200,
            pane_height: 50,
            ..Default::default()
        };
        assert_eq!(expand("#{pane_width}x#{pane_height}", &vars), "200x50");
    }

    #[test]
    fn expand_no_vars() {
        let vars = FormatVars::default();
        assert_eq!(expand("plain text", &vars), "plain text");
    }

    #[test]
    fn expand_just_pane_id_format() {
        let vars = FormatVars {
            pane_id: "%5".to_string(),
            ..Default::default()
        };
        assert_eq!(expand("#{pane_id}", &vars), "%5");
    }

    // ── HashMap-based expand_format tests ──

    #[test]
    fn expand_format_simple_variable() {
        let mut vars = HashMap::new();
        vars.insert("pane_id", "%0".to_string());
        assert_eq!(expand_format("#{pane_id}", &vars), "%0");
    }

    #[test]
    fn expand_format_multiple_variables() {
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
    fn expand_format_unknown_variable() {
        let vars = HashMap::new();
        assert_eq!(expand_format("#{unknown}", &vars), "");
    }

    #[test]
    fn expand_format_mixed_text() {
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
