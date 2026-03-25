/// Expand tmux format strings like `#{pane_id}`, `#{session_name}`, etc.
///
/// Supported variables:
/// - `#{pane_id}` — the tmux pane ID (e.g., %0)
/// - `#{session_name}` — the tmux session name
/// - `#{pane_width}` — terminal width in columns
/// - `#{pane_height}` — terminal height in rows
/// - `#{pane_index}` — pane index within session
/// - `#{pane_active}` — "1" if active, "0" if not
/// - `#{window_index}` — always 0 (we map sessions 1:1 with windows)
pub fn expand(format: &str, vars: &FormatVars) -> String {
    let mut result = format.to_string();

    result = result.replace("#{pane_id}", &vars.pane_id);
    result = result.replace("#{session_name}", &vars.session_name);
    result = result.replace("#{pane_width}", &vars.pane_width.to_string());
    result = result.replace("#{pane_height}", &vars.pane_height.to_string());
    result = result.replace("#{pane_index}", &vars.pane_index.to_string());
    result = result.replace(
        "#{pane_active}",
        if vars.pane_active { "1" } else { "0" },
    );
    result = result.replace("#{window_index}", &vars.window_index.to_string());

    result
}

/// Default tmux `list-panes` output line format.
pub fn default_list_panes_line(vars: &FormatVars) -> String {
    let active_str = if vars.pane_active { " (active)" } else { "" };
    format!(
        "{}: [{}x{}] [history 0/0, 0 bytes] {}{}",
        vars.pane_index, vars.pane_width, vars.pane_height, vars.pane_id, active_str
    )
}

/// Variables available for tmux format string expansion.
#[derive(Debug, Default)]
pub struct FormatVars {
    pub pane_id: String,
    pub session_name: String,
    pub pane_width: u32,
    pub pane_height: u32,
    pub pane_index: u32,
    pub pane_active: bool,
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
            ..Default::default()
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
    fn expand_pane_index() {
        let vars = FormatVars {
            pane_index: 3,
            ..Default::default()
        };
        assert_eq!(expand("#{pane_index}", &vars), "3");
    }

    #[test]
    fn expand_pane_active() {
        let active = FormatVars {
            pane_active: true,
            ..Default::default()
        };
        assert_eq!(expand("#{pane_active}", &active), "1");

        let inactive = FormatVars {
            pane_active: false,
            ..Default::default()
        };
        assert_eq!(expand("#{pane_active}", &inactive), "0");
    }

    #[test]
    fn expand_no_vars() {
        let vars = FormatVars::default();
        assert_eq!(expand("plain text", &vars), "plain text");
    }

    #[test]
    fn default_list_panes_active() {
        let vars = FormatVars {
            pane_id: "%0".to_string(),
            pane_width: 80,
            pane_height: 24,
            pane_index: 0,
            pane_active: true,
            ..Default::default()
        };
        assert_eq!(
            default_list_panes_line(&vars),
            "0: [80x24] [history 0/0, 0 bytes] %0 (active)"
        );
    }

    #[test]
    fn default_list_panes_inactive() {
        let vars = FormatVars {
            pane_id: "%1".to_string(),
            pane_width: 120,
            pane_height: 30,
            pane_index: 1,
            pane_active: false,
            ..Default::default()
        };
        assert_eq!(
            default_list_panes_line(&vars),
            "1: [120x30] [history 0/0, 0 bytes] %1"
        );
    }
}
