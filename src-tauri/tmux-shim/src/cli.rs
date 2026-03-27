use std::collections::HashMap;

/// Early-return on `Err`, printing the error to stderr and returning exit code 1.
/// Use in functions that return `i32` (exit codes).
macro_rules! tmux_try {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => {
                eprintln!("tmux: {}", e);
                return 1;
            }
        }
    };
    ($expr:expr, $msg:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => {
                eprintln!("tmux: {}: {}", $msg, e);
                return 1;
            }
        }
    };
}

/// Parsed tmux CLI arguments.
///
/// tmux uses single-dash single-letter flags:
///   - Boolean flags: `-d`, `-h`, `-v`, `-P`
///   - Value flags: `-s name`, `-t target`, `-F format`, `-c dir`, `-x W`, `-y H`
#[derive(Debug, Default)]
pub struct TmuxArgs {
    /// Boolean flags that were present (e.g., 'd', 'h', 'v', 'P')
    pub flags: Vec<char>,
    /// Key-value options (e.g., 's' -> "my-session", 't' -> "target")
    pub options: HashMap<char, String>,
    /// Positional arguments after all flags
    pub positional: Vec<String>,
}

impl TmuxArgs {
    /// Flags that take a value argument.
    const VALUE_FLAGS: &[char] = &['s', 't', 'F', 'c', 'x', 'y', 'f', 'n', 'e', 'l', 'T'];

    /// Parse tmux-style arguments from a slice of strings.
    pub fn parse(args: &[String]) -> Self {
        let mut result = TmuxArgs::default();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];

            if arg.starts_with('-') && arg.len() >= 2 && arg.as_bytes()[1] != b'-' {
                // Single-dash flag(s): could be `-d` or `-dt target` (combined)
                let chars: Vec<char> = arg[1..].chars().collect();
                let mut j = 0;

                while j < chars.len() {
                    let ch = chars[j];

                    if Self::VALUE_FLAGS.contains(&ch) {
                        // This flag takes a value
                        let remaining: String = chars[j + 1..].iter().collect();
                        if !remaining.is_empty() {
                            // Value is concatenated: `-sMySession`
                            result.options.insert(ch, remaining);
                        } else if i + 1 < args.len() {
                            // Value is the next argument: `-s MySession`
                            i += 1;
                            result.options.insert(ch, args[i].clone());
                        }
                        // Either way, we consumed the rest of this flag group
                        break;
                    } else {
                        result.flags.push(ch);
                    }
                    j += 1;
                }
            } else {
                result.positional.push(arg.clone());
            }

            i += 1;
        }

        result
    }

    /// Check if a boolean flag is set.
    pub fn has_flag(&self, flag: char) -> bool {
        self.flags.contains(&flag)
    }

    /// Get the value of an option flag.
    pub fn get_option(&self, key: char) -> Option<&str> {
        self.options.get(&key).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn parse_empty() {
        let parsed = TmuxArgs::parse(&[]);
        assert!(parsed.flags.is_empty());
        assert!(parsed.options.is_empty());
        assert!(parsed.positional.is_empty());
    }

    #[test]
    fn parse_boolean_flags() {
        let parsed = TmuxArgs::parse(&args(&["-d", "-P"]));
        assert!(parsed.has_flag('d'));
        assert!(parsed.has_flag('P'));
        assert!(!parsed.has_flag('v'));
    }

    #[test]
    fn parse_combined_boolean_flags() {
        let parsed = TmuxArgs::parse(&args(&["-dP"]));
        assert!(parsed.has_flag('d'));
        assert!(parsed.has_flag('P'));
    }

    #[test]
    fn parse_value_flag_separate() {
        let parsed = TmuxArgs::parse(&args(&["-s", "my-session"]));
        assert_eq!(parsed.get_option('s'), Some("my-session"));
    }

    #[test]
    fn parse_value_flag_concatenated() {
        let parsed = TmuxArgs::parse(&args(&["-smy-session"]));
        assert_eq!(parsed.get_option('s'), Some("my-session"));
    }

    #[test]
    fn parse_combined_boolean_then_value() {
        let parsed = TmuxArgs::parse(&args(&["-ds", "my-session"]));
        assert!(parsed.has_flag('d'));
        assert_eq!(parsed.get_option('s'), Some("my-session"));
    }

    #[test]
    fn parse_format_flag() {
        let parsed = TmuxArgs::parse(&args(&["-P", "-F", "#{pane_id}"]));
        assert!(parsed.has_flag('P'));
        assert_eq!(parsed.get_option('F'), Some("#{pane_id}"));
    }

    #[test]
    fn parse_multiple_value_flags() {
        let parsed = TmuxArgs::parse(&args(&["-s", "sess", "-t", "target", "-c", "/tmp"]));
        assert_eq!(parsed.get_option('s'), Some("sess"));
        assert_eq!(parsed.get_option('t'), Some("target"));
        assert_eq!(parsed.get_option('c'), Some("/tmp"));
    }

    #[test]
    fn parse_dimensions() {
        let parsed = TmuxArgs::parse(&args(&["-x", "120", "-y", "40"]));
        assert_eq!(parsed.get_option('x'), Some("120"));
        assert_eq!(parsed.get_option('y'), Some("40"));
    }

    #[test]
    fn parse_positional_args() {
        let parsed = TmuxArgs::parse(&args(&["-d", "bash", "--login"]));
        assert!(parsed.has_flag('d'));
        assert_eq!(parsed.positional, vec!["bash", "--login"]);
    }

    #[test]
    fn parse_mixed_everything() {
        let parsed = TmuxArgs::parse(&args(&[
            "-dP",
            "-s",
            "work",
            "-F",
            "#{pane_id}",
            "-c",
            "/home/user",
            "bash",
        ]));
        assert!(parsed.has_flag('d'));
        assert!(parsed.has_flag('P'));
        assert_eq!(parsed.get_option('s'), Some("work"));
        assert_eq!(parsed.get_option('F'), Some("#{pane_id}"));
        assert_eq!(parsed.get_option('c'), Some("/home/user"));
        assert_eq!(parsed.positional, vec!["bash"]);
    }
}
