//! Minimal tmux CLI argument parser.
//!
//! Parses tmux-style command lines like:
//!   tmux split-window -h -t %0 -c /some/dir
//!   tmux select-pane -t %1
//!   tmux list-panes -t main -F "#{pane_id}"

use std::collections::HashMap;

/// Parsed tmux command with subcommand and flags.
#[derive(Debug)]
pub struct TmuxArgs {
    pub subcommand: String,
    /// Flag -> value mappings (e.g., "-t" -> "%0")
    pub flags: HashMap<String, String>,
    /// Boolean flags that don't take values (e.g., "-h", "-v", "-P")
    pub bool_flags: Vec<String>,
    /// Remaining positional arguments
    pub positional: Vec<String>,
}

/// Flags that take a value argument (the next arg).
const VALUE_FLAGS: &[&str] = &["-t", "-F", "-c", "-s", "-n", "-x", "-y"];

/// Flags that are boolean (no value).
const BOOL_FLAGS: &[&str] = &["-h", "-v", "-P", "-d", "-l", "-a"];

impl TmuxArgs {
    /// Parse args from an iterator (typically `std::env::args().skip(1)`).
    pub fn parse<I: Iterator<Item = String>>(mut args: I) -> Result<Self, String> {
        let subcommand = args.next().ok_or_else(|| "No subcommand provided".to_string())?;

        let mut flags = HashMap::new();
        let mut bool_flags = Vec::new();
        let mut positional = Vec::new();

        let mut iter = args.peekable();
        while let Some(arg) = iter.next() {
            if arg.starts_with('-') {
                if BOOL_FLAGS.contains(&arg.as_str()) {
                    bool_flags.push(arg);
                } else if VALUE_FLAGS.contains(&arg.as_str()) {
                    let value = iter.next().ok_or_else(|| format!("Flag {} requires a value", arg))?;
                    flags.insert(arg, value);
                } else {
                    // Unknown flag — treat as boolean
                    bool_flags.push(arg);
                }
            } else {
                positional.push(arg);
            }
        }

        Ok(Self {
            subcommand,
            flags,
            bool_flags,
            positional,
        })
    }

    /// Get the value of a flag like `-t`.
    pub fn flag(&self, name: &str) -> Option<&str> {
        self.flags.get(name).map(|s| s.as_str())
    }

    /// Check if a boolean flag is present.
    pub fn has_flag(&self, name: &str) -> bool {
        self.bool_flags.iter().any(|f| f == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> TmuxArgs {
        TmuxArgs::parse(args.iter().map(|s| s.to_string())).unwrap()
    }

    #[test]
    fn parse_split_window_horizontal() {
        let args = parse(&["split-window", "-h", "-t", "%0", "-c", "/tmp"]);
        assert_eq!(args.subcommand, "split-window");
        assert!(args.has_flag("-h"));
        assert!(!args.has_flag("-v"));
        assert_eq!(args.flag("-t"), Some("%0"));
        assert_eq!(args.flag("-c"), Some("/tmp"));
    }

    #[test]
    fn parse_split_window_vertical_default() {
        let args = parse(&["split-window", "-v"]);
        assert_eq!(args.subcommand, "split-window");
        assert!(args.has_flag("-v"));
        assert!(!args.has_flag("-h"));
    }

    #[test]
    fn parse_split_window_with_print() {
        let args = parse(&["split-window", "-h", "-P", "-F", "#{pane_id}"]);
        assert!(args.has_flag("-P"));
        assert_eq!(args.flag("-F"), Some("#{pane_id}"));
    }

    #[test]
    fn parse_select_pane() {
        let args = parse(&["select-pane", "-t", "%1"]);
        assert_eq!(args.subcommand, "select-pane");
        assert_eq!(args.flag("-t"), Some("%1"));
    }

    #[test]
    fn parse_list_panes() {
        let args = parse(&["list-panes", "-t", "main", "-F", "#{pane_id}"]);
        assert_eq!(args.subcommand, "list-panes");
        assert_eq!(args.flag("-t"), Some("main"));
        assert_eq!(args.flag("-F"), Some("#{pane_id}"));
    }

    #[test]
    fn parse_kill_pane() {
        let args = parse(&["kill-pane", "-t", "%2"]);
        assert_eq!(args.subcommand, "kill-pane");
        assert_eq!(args.flag("-t"), Some("%2"));
    }

    #[test]
    fn parse_no_subcommand_errors() {
        let result = TmuxArgs::parse(std::iter::empty());
        assert!(result.is_err());
    }

    #[test]
    fn parse_flag_missing_value_errors() {
        let result = TmuxArgs::parse(["split-window", "-t"].iter().map(|s| s.to_string()));
        assert!(result.is_err());
    }
}
