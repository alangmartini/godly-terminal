//! Minimal tmux CLI argument parser.
//!
//! tmux uses a POSIX-style CLI: `tmux [flags] command [command-args...]`.
//! We parse just enough to dispatch to the right subcommand handler.

/// Parsed top-level tmux invocation.
pub struct TmuxArgs {
    /// The subcommand name (e.g. "send-keys", "split-window").
    pub command: String,
    /// All arguments after the subcommand.
    pub args: Vec<String>,
}

/// Parse `tmux [-flags] <command> [args...]` from raw process args.
///
/// Skips argv[0] (the binary name). Recognizes top-level flags like `-S`
/// (socket path — ignored, we use pipes) before the subcommand.
pub fn parse(mut args: Vec<String>) -> Result<TmuxArgs, String> {
    if !args.is_empty() {
        args.remove(0);
    }

    while let Some(first) = args.first() {
        if first.starts_with('-') {
            let flag = args.remove(0);
            match flag.as_str() {
                // Flags that take a value argument
                "-S" | "-L" | "-f" => {
                    if !args.is_empty() {
                        args.remove(0);
                    }
                }
                _ => {}
            }
        } else {
            break;
        }
    }

    if args.is_empty() {
        return Err("no command specified".to_string());
    }

    let command = args.remove(0);
    Ok(TmuxArgs {
        command,
        args,
    })
}

/// Parse option flags from a command's argument list.
///
/// Returns (options, positional_args).
/// `flag_opts` are flags that take no value (e.g. "-p", "-l").
/// `value_opts` are flags that consume the next arg (e.g. "-t", "-F").
pub fn parse_command_args(
    args: &[String],
    flag_opts: &[&str],
    value_opts: &[&str],
) -> (std::collections::HashMap<String, String>, Vec<String>) {
    let mut opts = std::collections::HashMap::new();
    let mut positional = Vec::new();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        if flag_opts.contains(&arg.as_str()) {
            opts.insert(arg.clone(), String::new());
            i += 1;
        } else if value_opts.contains(&arg.as_str()) {
            if i + 1 < args.len() {
                opts.insert(arg.clone(), args[i + 1].clone());
                i += 2;
            } else {
                // Missing value — treat as positional (tmux would error)
                positional.push(arg.clone());
                i += 1;
            }
        } else {
            positional.push(arg.clone());
            i += 1;
        }
    }

    (opts, positional)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_command() {
        let args = vec![
            "tmux".to_string(),
            "send-keys".to_string(),
            "hello".to_string(),
        ];
        let parsed = parse(args).unwrap();
        assert_eq!(parsed.command, "send-keys");
        assert_eq!(parsed.args, vec!["hello"]);
    }

    #[test]
    fn parse_with_top_level_flags() {
        let args = vec![
            "tmux".to_string(),
            "-S".to_string(),
            "/tmp/tmux.sock".to_string(),
            "list-sessions".to_string(),
        ];
        let parsed = parse(args).unwrap();
        assert_eq!(parsed.command, "list-sessions");
        assert!(parsed.args.is_empty());
    }

    #[test]
    fn parse_no_command_returns_error() {
        let args = vec!["tmux".to_string()];
        assert!(parse(args).is_err());
    }

    #[test]
    fn parse_command_args_mixed() {
        let args = vec![
            "-t".to_string(),
            "%0".to_string(),
            "-l".to_string(),
            "hello".to_string(),
            "world".to_string(),
        ];
        let (opts, positional) = parse_command_args(&args, &["-l"], &["-t"]);
        assert_eq!(opts.get("-t").unwrap(), "%0");
        assert!(opts.contains_key("-l"));
        assert_eq!(positional, vec!["hello", "world"]);
    }
}
