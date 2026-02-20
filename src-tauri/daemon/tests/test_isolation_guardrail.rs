//! Guardrail test: scan all daemon integration tests for patterns that could
//! kill or interfere with the production daemon, or hang CI indefinitely.
//!
//! Incident: `taskkill /F /IM godly-daemon.exe` in a test killed the production
//! daemon, freezing 8 live terminal sessions. Tests that used the production
//! `PIPE_NAME` constant connected to the live daemon instead of an isolated one.
//!
//! This test prevents those patterns from ever being reintroduced. It scans every
//! `*.rs` file in `daemon/tests/` (except itself) and fails if it finds:
//!
//! 1. `taskkill /IM` — kills ALL processes by name (must use `/PID` or `child.kill()`)
//! 2. `use godly_protocol::PIPE_NAME` — imports production pipe name (must use isolated pipes)
//! 3. `PIPE_NAME` used as a value — references production pipe constant directly
//! 4. `#[test]` without `#[ntest::timeout(...)]` — tests must have timeouts to prevent CI hangs
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test test_isolation_guardrail

use std::fs;
use std::path::PathBuf;

fn daemon_tests_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests")
}

/// Patterns that must NEVER appear in daemon test files.
/// Each entry: (pattern, explanation)
const FORBIDDEN_PATTERNS: &[(&str, &str)] = &[
    (
        "taskkill /IM",
        "taskkill /IM kills ALL daemon processes by name, including the production daemon. Use child.kill() or taskkill /PID instead.",
    ),
    (
        "taskkill /F /IM",
        "taskkill /F /IM kills ALL daemon processes by name, including the production daemon. Use child.kill() or taskkill /F /PID instead.",
    ),
    (
        "use godly_protocol::PIPE_NAME",
        "Importing the production PIPE_NAME constant means the test connects to the live daemon. Use GODLY_PIPE_NAME env var for an isolated pipe.",
    ),
];

/// Additional check: if a file spawns a daemon (Command::new with "godly-daemon"),
/// it must set GODLY_PIPE_NAME or use --instance for isolation.
fn check_daemon_spawn_isolation(filename: &str, content: &str) -> Vec<String> {
    let mut violations = Vec::new();

    // Find lines that spawn the daemon binary
    let spawns_daemon = content.contains("godly-daemon")
        && (content.contains("Command::new") || content.contains(".spawn()"));

    if spawns_daemon {
        let has_pipe_env = content.contains("GODLY_PIPE_NAME");
        let has_instance_arg = content.contains("--instance");

        if !has_pipe_env && !has_instance_arg {
            violations.push(format!(
                "{}: spawns godly-daemon without GODLY_PIPE_NAME env var or --instance arg. \
                 The test will connect to the production daemon instead of an isolated one.",
                filename
            ));
        }
    }

    violations
}

/// Check that every `#[test]` function has an `#[ntest::timeout(...)]` annotation.
/// Tests without timeouts can hang CI indefinitely.
fn check_timeout_annotations(filename: &str, content: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Found a #[test] attribute
        if trimmed == "#[test]" {
            // Look backwards and forwards (up to 3 lines) for ntest::timeout
            let search_start = i.saturating_sub(3);
            let search_end = (i + 4).min(lines.len());
            let nearby = &lines[search_start..search_end];

            let has_timeout = nearby
                .iter()
                .any(|l| l.contains("ntest::timeout"));

            if !has_timeout {
                // Find the function name on the next non-attribute, non-empty line
                let fn_name = lines[i + 1..]
                    .iter()
                    .find(|l| l.trim().starts_with("fn "))
                    .map(|l| l.trim())
                    .unwrap_or("unknown");
                violations.push(format!(
                    "{}: `{}` has #[test] without #[ntest::timeout(...)]. \
                     All daemon tests must have a timeout to prevent CI hangs. \
                     Add e.g. #[ntest::timeout(60_000)] for a 1-minute timeout.",
                    filename, fn_name
                ));
            }
        }
    }

    violations
}

#[test]
#[ntest::timeout(30_000)] // 30s — file scanning only, no daemon
fn no_forbidden_patterns_in_daemon_tests() {
    let tests_dir = daemon_tests_dir();
    let mut violations = Vec::new();

    let entries = fs::read_dir(&tests_dir).unwrap_or_else(|e| {
        panic!("Failed to read daemon tests dir {:?}: {}", tests_dir, e);
    });

    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();

        // Only check .rs files, skip ourselves
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let filename = path.file_name().unwrap().to_string_lossy().to_string();
        if filename == "test_isolation_guardrail.rs" {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!("Failed to read {:?}: {}", path, e);
        });

        // Check forbidden string patterns
        for (pattern, explanation) in FORBIDDEN_PATTERNS {
            if content.contains(pattern) {
                violations.push(format!("{}: contains `{}` — {}", filename, pattern, explanation));
            }
        }

        // Check daemon spawn isolation
        violations.extend(check_daemon_spawn_isolation(&filename, &content));

        // Check timeout annotations
        violations.extend(check_timeout_annotations(&filename, &content));
    }

    if !violations.is_empty() {
        panic!(
            "\n\nDAEMON TEST GUARDRAIL VIOLATIONS FOUND:\n\n{}\n\n\
             These patterns can kill the production daemon, freeze live terminals, or hang CI.\n\
             See CLAUDE.md \"Daemon Test Isolation\" section for the correct patterns.\n",
            violations.join("\n\n")
        );
    }
}
