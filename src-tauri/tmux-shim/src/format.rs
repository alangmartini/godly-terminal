/// Expand tmux format strings like `#{pane_id}`, `#{session_name}`, etc.
///
/// Supported variables:
/// - `#{pane_id}` — the tmux pane ID (e.g., %0)
/// - `#{session_name}` — the tmux session name
/// - `#{pane_width}` — terminal width in columns
/// - `#{pane_height}` — terminal height in rows
/// - `#{window_index}` — always 0 (we map sessions 1:1 with windows)
pub fn expand(format: &str, vars: &FormatVars) -> String {
    let mut result = format.to_string();

    result = result.replace("#{pane_id}", &vars.pane_id);
    result = result.replace("#{session_name}", &vars.session_name);
    result = result.replace("#{pane_width}", &vars.pane_width.to_string());
    result = result.replace("#{pane_height}", &vars.pane_height.to_string());
    result = result.replace("#{window_index}", &vars.window_index.to_string());

    result
}

/// Variables available for tmux format string expansion.
#[derive(Debug, Default)]
pub struct FormatVars {
    pub pane_id: String,
    pub session_name: String,
    pub pane_width: u32,
    pub pane_height: u32,
    pub window_index: u32,
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
        // This is what Claude Code typically passes: -F '#{pane_id}'
        let vars = FormatVars {
            pane_id: "%5".to_string(),
            ..Default::default()
        };
        assert_eq!(expand("#{pane_id}", &vars), "%5");
    }
}
