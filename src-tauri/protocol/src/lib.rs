pub mod ansi;
pub mod frame;
pub mod mcp_messages;
pub mod messages;
pub mod types;

pub use frame::{read_message, write_message};
pub use mcp_messages::{McpRequest, McpResponse, McpTerminalInfo, McpWorkspaceInfo};
pub use messages::{DaemonMessage, Event, Request, Response};
pub use types::{SessionInfo, ShellType};

/// Default named pipe path used by both daemon and client
pub const PIPE_NAME: &str = r"\\.\pipe\godly-terminal-daemon";

/// Named pipe path for MCP communication (Tauri app <-> godly-mcp binary)
pub const MCP_PIPE_NAME: &str = r"\\.\pipe\godly-terminal-mcp";

/// Get a suffix derived from the GODLY_INSTANCE env var (e.g. "-test").
/// Returns empty string when unset, so production paths are unchanged.
pub fn instance_suffix() -> String {
    std::env::var("GODLY_INSTANCE")
        .ok()
        .filter(|name| !name.is_empty())
        .map(|name| format!("-{}", name))
        .unwrap_or_default()
}

/// Get the daemon pipe name, allowing override via GODLY_PIPE_NAME env var.
/// Falls back to the default pipe name with an optional instance suffix.
pub fn pipe_name() -> String {
    std::env::var("GODLY_PIPE_NAME")
        .unwrap_or_else(|_| format!(r"\\.\pipe\godly-terminal-daemon{}", instance_suffix()))
}

/// Get the MCP pipe name, allowing override via GODLY_MCP_PIPE_NAME env var.
/// Falls back to the default MCP pipe name with an optional instance suffix.
pub fn mcp_pipe_name() -> String {
    std::env::var("GODLY_MCP_PIPE_NAME")
        .unwrap_or_else(|_| format!(r"\\.\pipe\godly-terminal-mcp{}", instance_suffix()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run a check in a child process with isolated env vars.
    /// Uses a temp file to pass the result back (stdout is polluted by the
    /// test harness in the subprocess).
    fn run_in_subprocess(env_vars: &[(&str, &str)], check: &str) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let result_file = std::env::temp_dir().join(format!(
            "godly-protocol-test-{}-{}-{}.txt",
            check,
            std::process::id(),
            id
        ));
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.arg("--ignored")
            .arg("--exact")
            .arg("tests::subprocess_harness");
        cmd.env("__SUBPROCESS_CHECK", check);
        cmd.env("__SUBPROCESS_RESULT_FILE", &result_file);
        // Clear instance-related vars so the subprocess starts clean
        cmd.env_remove("GODLY_INSTANCE");
        cmd.env_remove("GODLY_PIPE_NAME");
        cmd.env_remove("GODLY_MCP_PIPE_NAME");
        for (k, v) in env_vars {
            cmd.env(k, v);
        }
        let output = cmd.output().expect("failed to run subprocess");
        assert!(
            output.status.success(),
            "subprocess failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result = std::fs::read_to_string(&result_file)
            .unwrap_or_else(|e| panic!("failed to read result file {:?}: {}", result_file, e));
        let _ = std::fs::remove_file(&result_file);
        result
    }

    /// Internal harness — run via subprocess only, ignored by normal test runner.
    #[test]
    #[ignore]
    fn subprocess_harness() {
        let check = std::env::var("__SUBPROCESS_CHECK").unwrap_or_default();
        let result_file = std::env::var("__SUBPROCESS_RESULT_FILE").unwrap();
        let result = match check.as_str() {
            "instance_suffix" => instance_suffix(),
            "pipe_name" => pipe_name(),
            "mcp_pipe_name" => mcp_pipe_name(),
            _ => panic!("unknown check: {}", check),
        };
        std::fs::write(result_file, &result).expect("failed to write result file");
    }

    #[test]
    fn instance_suffix_empty_when_unset() {
        let result = run_in_subprocess(&[], "instance_suffix");
        assert_eq!(result, "");
    }

    #[test]
    fn instance_suffix_empty_string_treated_as_unset() {
        // Empty GODLY_INSTANCE should behave like unset, not produce a lone "-"
        let result = run_in_subprocess(&[("GODLY_INSTANCE", "")], "instance_suffix");
        assert_eq!(result, "");
    }

    #[test]
    fn instance_suffix_returns_dash_prefixed_name() {
        let result = run_in_subprocess(&[("GODLY_INSTANCE", "test")], "instance_suffix");
        assert_eq!(result, "-test");
    }

    #[test]
    fn pipe_name_default_without_instance() {
        let result = run_in_subprocess(&[], "pipe_name");
        assert_eq!(result, r"\\.\pipe\godly-terminal-daemon");
    }

    #[test]
    fn pipe_name_with_instance_suffix() {
        let result = run_in_subprocess(&[("GODLY_INSTANCE", "test")], "pipe_name");
        assert_eq!(result, r"\\.\pipe\godly-terminal-daemon-test");
    }

    #[test]
    fn pipe_name_explicit_override_takes_precedence() {
        // GODLY_PIPE_NAME should override even when GODLY_INSTANCE is set
        let result = run_in_subprocess(
            &[
                ("GODLY_INSTANCE", "test"),
                ("GODLY_PIPE_NAME", r"\\.\pipe\custom"),
            ],
            "pipe_name",
        );
        assert_eq!(result, r"\\.\pipe\custom");
    }

    #[test]
    fn mcp_pipe_name_default_without_instance() {
        let result = run_in_subprocess(&[], "mcp_pipe_name");
        assert_eq!(result, r"\\.\pipe\godly-terminal-mcp");
    }

    #[test]
    fn mcp_pipe_name_with_instance_suffix() {
        let result = run_in_subprocess(&[("GODLY_INSTANCE", "test")], "mcp_pipe_name");
        assert_eq!(result, r"\\.\pipe\godly-terminal-mcp-test");
    }

    #[test]
    fn mcp_pipe_name_explicit_override_takes_precedence() {
        let result = run_in_subprocess(
            &[
                ("GODLY_INSTANCE", "test"),
                ("GODLY_MCP_PIPE_NAME", r"\\.\pipe\custom-mcp"),
            ],
            "mcp_pipe_name",
        );
        assert_eq!(result, r"\\.\pipe\custom-mcp");
    }
}
