use std::sync::Arc;
use std::time::Duration;

use godly_app_adapter::commands;
use godly_app_adapter::daemon_client::NativeDaemonClient;

/// A single step in a Quick Claude launch sequence.
#[derive(Debug, Clone)]
pub enum LaunchStep {
    /// Create a terminal session via daemon.
    CreateTerminal {
        agent_index: usize,
        cwd: Option<String>,
    },
    /// Wait until terminal output stabilizes for the given duration.
    WaitIdle { agent_index: usize, idle_ms: u64 },
    /// Write a command + carriage return to the terminal.
    RunCommand { agent_index: usize, command: String },
    /// Wait for marker text in terminal output, with timeout.
    WaitReady {
        agent_index: usize,
        marker: String,
        timeout_ms: u64,
    },
    /// Adaptive wait: handles trust prompt if it appears, then waits for
    /// Claude's input prompt. Replaces the old WaitReady("trust") + SendEnter
    /// + WaitReady(">") sequence which broke when the trust prompt was absent.
    WaitForClaudeReady {
        agent_index: usize,
        /// Skip trust prompt detection (true when --dangerously-skip-permissions).
        skip_trust: bool,
        timeout_ms: u64,
    },
    /// Send Enter key (carriage return).
    SendEnter { agent_index: usize },
    /// Send the prompt text to the terminal (without carriage return).
    SendPrompt { agent_index: usize, prompt: String },
    /// Wait for echoed text in terminal grid, confirming TUI is reading stdin.
    WaitForEcho {
        agent_index: usize,
        text_prefix: String,
        timeout_ms: u64,
    },
    /// Sleep for N milliseconds.
    Delay { ms: u64 },
    /// Create a git worktree for isolated work. If not a git repo or
    /// creation fails, gracefully falls back (returns Ok).
    CreateWorktree {
        agent_index: usize,
        repo_folder: String,
    },
}

/// Build launch steps for resuming an existing Claude session.
pub fn resume_launch_steps(session_id: &str, cwd: Option<&str>) -> Vec<LaunchStep> {
    vec![
        LaunchStep::CreateTerminal {
            agent_index: 0,
            cwd: cwd.map(|s| s.to_string()),
        },
        LaunchStep::WaitIdle {
            agent_index: 0,
            idle_ms: 2000,
        },
        LaunchStep::RunCommand {
            agent_index: 0,
            command: format!("claude --resume --session-id {}", session_id),
        },
        LaunchStep::WaitReady {
            agent_index: 0,
            marker: ">".to_string(),
            timeout_ms: 30000,
        },
    ]
}

/// Build the default launch sequence for a preset with N agents.
///
/// The prompt is passed as a CLI positional argument to `claude`, avoiding
/// the fragile interactive prompt detection that broke because Claude Code's
/// TUI renders a hint bar below the `>` input prompt.
pub fn default_launch_steps(
    num_agents: usize,
    prompt: &str,
    model: &str,
    mode: &str,
    cwd: Option<&str>,
    image_paths: &[String],
    use_worktree: bool,
) -> Vec<LaunchStep> {
    let mut cmd = "claude".to_string();
    // Add model flag (always, since we default to sonnet)
    cmd.push_str(&format!(" --model {}", model));
    // Add mode flag
    match mode {
        "plan" => cmd.push_str(" --permission-mode plan"),
        "auto" => cmd.push_str(" --dangerously-skip-permissions"),
        _ => {} // "default" — no flag
    }
    // Build effective prompt: append image references if any.
    // IMPORTANT: keep everything on ONE line — embedded newlines in the command
    // string cause PowerShell on Windows to misparse the multi-line input via PTY.
    let effective_prompt = if image_paths.is_empty() {
        prompt.to_string()
    } else {
        let paths = image_paths.join(", ");
        format!(
            "{} [Attached images - please read these files to view them: {}]",
            prompt, paths
        )
    };
    // Pass prompt as CLI positional argument
    if !effective_prompt.is_empty() {
        // Shell-escape: wrap in single quotes, escape embedded single quotes
        // PowerShell escapes single quotes by doubling them: 'isn''t' -> isn't
        let escaped = effective_prompt.replace('\'', "''");
        cmd.push_str(&format!(" '{}'", escaped));
    }

    let mut steps = Vec::new();
    for i in 0..num_agents {
        if use_worktree {
            if let Some(folder) = cwd {
                steps.push(LaunchStep::CreateWorktree {
                    agent_index: i,
                    repo_folder: folder.to_string(),
                });
            }
        }
        steps.push(LaunchStep::CreateTerminal {
            agent_index: i,
            cwd: cwd.map(|s| s.to_string()),
        });
        steps.push(LaunchStep::WaitIdle {
            agent_index: i,
            idle_ms: 2000,
        });
        steps.push(LaunchStep::RunCommand {
            agent_index: i,
            command: cmd.clone(),
        });
    }
    steps
}

/// State for a running Quick Claude launch.
#[derive(Debug, Clone)]
pub struct LaunchState {
    pub preset_name: String,
    pub steps: Vec<LaunchStep>,
    pub current_step: usize,
    pub agent_terminal_ids: Vec<Option<String>>,
    pub workspace_id: String,
    pub is_new_workspace: bool,
    pub completed: bool,
    pub error: Option<String>,
    /// Worktree path created by a `CreateWorktree` step, applied to the next
    /// terminal that gets created.
    pub pending_worktree_path: Option<String>,
}

impl LaunchState {
    pub fn new(
        preset_name: String,
        steps: Vec<LaunchStep>,
        num_agents: usize,
        workspace_id: String,
        is_new_workspace: bool,
    ) -> Self {
        Self {
            preset_name,
            steps,
            current_step: 0,
            agent_terminal_ids: vec![None; num_agents],
            workspace_id,
            is_new_workspace,
            completed: false,
            error: None,
            pending_worktree_path: None,
        }
    }

    pub fn total_steps(&self) -> usize {
        self.steps.len()
    }
}

/// Execute a single launch step. Returns the terminal ID if the step was CreateTerminal.
pub fn execute_step(
    client: Arc<NativeDaemonClient>,
    step: LaunchStep,
    agent_terminal_ids: Vec<Option<String>>,
    rows: u16,
    cols: u16,
) -> Result<StepResult, String> {
    match step {
        LaunchStep::CreateTerminal { cwd, .. } => {
            let session_id = uuid::Uuid::new_v4().to_string();
            commands::create_terminal(
                &client,
                &session_id,
                godly_protocol::ShellType::Windows,
                cwd.as_deref(),
                rows,
                cols,
            )?;
            Ok(StepResult::TerminalCreated(session_id))
        }
        LaunchStep::WaitIdle { agent_index, idle_ms } => {
            let session_id = resolve_session_id(&agent_terminal_ids, agent_index)?;
            wait_for_idle(&client, &session_id, idle_ms)?;
            Ok(StepResult::Ok)
        }
        LaunchStep::RunCommand {
            agent_index,
            command,
        } => {
            let session_id = resolve_session_id(&agent_terminal_ids, agent_index)?;
            let data = format!("{}\r", command);
            commands::write_to_terminal(&client, &session_id, data.as_bytes())?;
            Ok(StepResult::Ok)
        }
        LaunchStep::WaitReady {
            agent_index,
            marker,
            timeout_ms,
        } => {
            let session_id = resolve_session_id(&agent_terminal_ids, agent_index)?;
            wait_for_marker(&client, &session_id, &marker, timeout_ms)?;
            Ok(StepResult::Ok)
        }
        LaunchStep::WaitForClaudeReady {
            agent_index,
            skip_trust,
            timeout_ms,
        } => {
            let session_id = resolve_session_id(&agent_terminal_ids, agent_index)?;
            wait_for_claude_ready(&client, &session_id, skip_trust, timeout_ms)?;
            Ok(StepResult::Ok)
        }
        LaunchStep::SendEnter { agent_index } => {
            let session_id = resolve_session_id(&agent_terminal_ids, agent_index)?;
            commands::write_to_terminal(&client, &session_id, b"\r")?;
            Ok(StepResult::Ok)
        }
        LaunchStep::SendPrompt {
            agent_index,
            prompt,
        } => {
            let session_id = resolve_session_id(&agent_terminal_ids, agent_index)?;
            // Write prompt text WITHOUT carriage return to avoid ink paste detection.
            // A separate SendEnter step follows after WaitForEcho confirms the TUI read the text.
            commands::write_to_terminal(&client, &session_id, prompt.as_bytes())?;
            Ok(StepResult::Ok)
        }
        LaunchStep::WaitForEcho {
            agent_index,
            text_prefix,
            timeout_ms,
        } => {
            let session_id = resolve_session_id(&agent_terminal_ids, agent_index)?;
            wait_for_echo(&client, &session_id, &text_prefix, timeout_ms)?;
            Ok(StepResult::Ok)
        }
        LaunchStep::Delay { ms } => {
            std::thread::sleep(Duration::from_millis(ms));
            Ok(StepResult::Ok)
        }
        LaunchStep::CreateWorktree { repo_folder, .. } => {
            if !crate::git_worktree::is_git_repo(&repo_folder) {
                log::warn!("Not a git repo, skipping worktree creation: {repo_folder}");
                return Ok(StepResult::Ok);
            }
            let repo_root = match crate::git_worktree::find_repo_root(&repo_folder) {
                Ok(root) => root,
                Err(e) => {
                    log::warn!("Failed to find repo root, skipping worktree: {e}");
                    return Ok(StepResult::Ok);
                }
            };
            let dir_name = crate::git_worktree::generate_worktree_dir_name();
            match crate::git_worktree::create_worktree(&repo_root, &dir_name) {
                Ok(worktree_path) => Ok(StepResult::WorktreeCreated { worktree_path }),
                Err(e) => {
                    log::warn!("Worktree creation failed, falling back to main branch: {e}");
                    Ok(StepResult::Ok)
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum StepResult {
    Ok,
    TerminalCreated(String),
    WorktreeCreated { worktree_path: String },
}

fn resolve_session_id(
    agent_terminal_ids: &[Option<String>],
    agent_index: usize,
) -> Result<String, String> {
    agent_terminal_ids
        .get(agent_index)
        .and_then(|opt| opt.clone())
        .ok_or_else(|| format!("Agent {} has no terminal yet", agent_index))
}

/// Poll the grid snapshot until output stabilizes (no changes for `idle_ms`).
fn wait_for_idle(
    client: &NativeDaemonClient,
    session_id: &str,
    idle_ms: u64,
) -> Result<(), String> {
    let idle_duration = Duration::from_millis(idle_ms);
    let timeout = Duration::from_secs(60);
    let poll_interval = Duration::from_millis(200);
    let start = std::time::Instant::now();

    let mut last_snapshot_hash: Option<u64> = None;
    let mut last_change = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            // Timeout is not fatal for idle wait; proceed anyway
            return Ok(());
        }

        let grid = commands::get_grid_snapshot(client, session_id)?;
        let hash = simple_grid_hash(&grid);

        if last_snapshot_hash == Some(hash) {
            if last_change.elapsed() >= idle_duration {
                return Ok(());
            }
        } else {
            last_snapshot_hash = Some(hash);
            last_change = std::time::Instant::now();
        }

        std::thread::sleep(poll_interval);
    }
}

/// Poll the grid snapshot until the marker text appears.
fn wait_for_marker(
    client: &NativeDaemonClient,
    session_id: &str,
    marker: &str,
    timeout_ms: u64,
) -> Result<(), String> {
    let timeout = Duration::from_millis(timeout_ms);
    let poll_interval = Duration::from_millis(300);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            return Err(format!(
                "Timed out waiting for marker '{}' after {}ms",
                marker, timeout_ms
            ));
        }

        let grid = commands::get_grid_snapshot(client, session_id)?;
        let text = grid_to_text(&grid);
        if text.contains(marker) {
            return Ok(());
        }

        std::thread::sleep(poll_interval);
    }
}

/// Wait for echoed text to appear in the grid, confirming the TUI is reading stdin.
/// Timeout is non-fatal: we log a warning and proceed (the Enter key will be sent anyway).
fn wait_for_echo(
    client: &NativeDaemonClient,
    session_id: &str,
    text_prefix: &str,
    timeout_ms: u64,
) -> Result<(), String> {
    let timeout = Duration::from_millis(timeout_ms);
    let poll_interval = Duration::from_millis(250);
    let start = std::time::Instant::now();
    // Use first 40 chars as search prefix (grid may wrap long text)
    let search_text: String = text_prefix.chars().take(40).collect();

    loop {
        if start.elapsed() > timeout {
            log::warn!(
                "Echo detection timed out after {}ms, sending Enter anyway",
                timeout_ms
            );
            return Ok(());
        }

        let grid = commands::get_grid_snapshot(client, session_id)?;
        let text = grid_to_text(&grid);
        if text.contains(&search_text) {
            // Small buffer to ensure TUI read loop is stable
            std::thread::sleep(Duration::from_millis(100));
            return Ok(());
        }

        std::thread::sleep(poll_interval);
    }
}

/// Adaptive wait for Claude Code to be ready for input.
///
/// Handles two scenarios:
/// 1. Trust prompt appears ("Do you trust...") — sends Enter to accept, then
///    waits for Claude's input prompt.
/// 2. No trust prompt (auto mode or already-trusted dir) — waits directly
///    for Claude's input prompt.
///
/// Uses ">" on its own line as the ready marker (Claude Code's input prompt).
fn wait_for_claude_ready(
    client: &NativeDaemonClient,
    session_id: &str,
    skip_trust: bool,
    timeout_ms: u64,
) -> Result<(), String> {
    let timeout = Duration::from_millis(timeout_ms);
    let poll_interval = Duration::from_millis(300);
    let start = std::time::Instant::now();
    let mut trust_handled = false;

    loop {
        if start.elapsed() > timeout {
            return Err(format!(
                "Timed out waiting for Claude to be ready after {}ms",
                timeout_ms
            ));
        }

        let grid = commands::get_grid_snapshot(client, session_id)?;
        let text = grid_to_text(&grid);

        // Handle trust prompt if present and not skipped
        if !skip_trust && !trust_handled && text.contains("Do you trust") {
            commands::write_to_terminal(client, session_id, b"\r")?;
            trust_handled = true;
            std::thread::sleep(poll_interval);
            continue;
        }

        // Check for Claude's input prompt: ">" as the last non-empty line.
        // Only matches Claude's actual input prompt, not arbitrary lines ending with ">".
        let has_prompt = text
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(|line| {
                let trimmed = line.trim();
                trimmed == ">" || trimmed == "> "
            })
            .unwrap_or(false);
        if has_prompt && (skip_trust || trust_handled || !text.contains("Do you trust")) {
            return Ok(());
        }

        std::thread::sleep(poll_interval);
    }
}

/// Extract all text content from a RichGridData snapshot.
fn grid_to_text(grid: &godly_protocol::types::RichGridData) -> String {
    let mut text = String::new();
    for row in &grid.rows {
        for cell in &row.cells {
            text.push_str(&cell.content);
        }
        text.push('\n');
    }
    text
}

/// Simple hash of grid content for change detection.
fn simple_grid_hash(grid: &godly_protocol::types::RichGridData) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for row in &grid.rows {
        for cell in &row.cells {
            cell.content.hash(&mut hasher);
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_steps_has_four_steps() {
        let steps = resume_launch_steps("abc-123", Some("/test/dir"));
        assert_eq!(steps.len(), 4);
        assert!(matches!(steps[0], LaunchStep::CreateTerminal { agent_index: 0, .. }));
        assert!(matches!(steps[1], LaunchStep::WaitIdle { agent_index: 0, .. }));
        match &steps[2] {
            LaunchStep::RunCommand { agent_index, command } => {
                assert_eq!(*agent_index, 0);
                assert!(command.contains("--resume"));
                assert!(command.contains("abc-123"));
            }
            _ => panic!("Expected RunCommand"),
        }
        assert!(matches!(steps[3], LaunchStep::WaitReady { agent_index: 0, .. }));
    }

    #[test]
    fn resume_steps_propagates_cwd() {
        let steps = resume_launch_steps("abc-123", Some("/my/project"));
        if let LaunchStep::CreateTerminal { cwd, .. } = &steps[0] {
            assert_eq!(cwd.as_deref(), Some("/my/project"));
        } else {
            panic!("Expected CreateTerminal");
        }
    }

    #[test]
    fn default_steps_single_no_prompt() {
        let steps = default_launch_steps(1, "", "sonnet", "default", None, &[], false);
        // CreateTerminal, WaitIdle, RunCommand
        assert_eq!(steps.len(), 3);
        assert!(matches!(steps[0], LaunchStep::CreateTerminal { agent_index: 0, .. }));
        assert!(matches!(steps[1], LaunchStep::WaitIdle { agent_index: 0, .. }));
        assert!(matches!(steps[2], LaunchStep::RunCommand { agent_index: 0, .. }));
    }

    #[test]
    fn default_steps_single_with_prompt() {
        let steps = default_launch_steps(1, "build the app", "sonnet", "default", None, &[], false);
        // Prompt is now a CLI arg — same 3 steps, prompt embedded in command
        assert_eq!(steps.len(), 3);
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            assert!(command.contains("'build the app'"));
        } else {
            panic!("Expected RunCommand");
        }
    }

    #[test]
    fn default_steps_prompt_with_single_quotes() {
        let steps = default_launch_steps(1, "fix it's broken", "sonnet", "auto", None, &[], false);
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            // PowerShell escapes single quotes by doubling them
            assert!(command.contains("'fix it''s broken'"), "got: {command}");
        } else {
            panic!("Expected RunCommand");
        }
    }

    #[test]
    fn default_steps_prompt_with_double_quotes() {
        let steps = default_launch_steps(1, "what is \"the issue\"", "sonnet", "auto", None, &[], false);
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            // Double quotes inside single-quoted strings are literal (no escaping needed)
            assert!(
                command.contains("'what is \"the issue\"'"),
                "got: {command}"
            );
        } else {
            panic!("Expected RunCommand");
        }
    }

    #[test]
    fn default_steps_grid_2x2() {
        let steps = default_launch_steps(4, "test", "sonnet", "default", None, &[], false);
        // Each agent: 3 steps. 4 agents = 12
        assert_eq!(steps.len(), 12);
    }

    #[test]
    fn default_steps_with_model_and_mode() {
        let steps = default_launch_steps(1, "", "opus", "plan", None, &[], false);
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            assert_eq!(command, "claude --model opus --permission-mode plan");
        } else {
            panic!("Expected RunCommand at index 2");
        }
    }

    #[test]
    fn default_steps_auto_mode() {
        let steps = default_launch_steps(1, "", "haiku", "auto", None, &[], false);
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            assert_eq!(command, "claude --model haiku --dangerously-skip-permissions");
        } else {
            panic!("Expected RunCommand at index 2");
        }
    }

    #[test]
    fn default_steps_default_mode_no_extra_flag() {
        let steps = default_launch_steps(1, "", "sonnet", "default", None, &[], false);
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            assert_eq!(command, "claude --model sonnet");
        } else {
            panic!("Expected RunCommand at index 2");
        }
    }

    #[test]
    fn default_steps_propagates_cwd() {
        let steps = default_launch_steps(1, "", "sonnet", "default", Some("/my/project"), &[], false);
        if let LaunchStep::CreateTerminal { cwd, .. } = &steps[0] {
            assert_eq!(cwd.as_deref(), Some("/my/project"));
        } else {
            panic!("Expected CreateTerminal");
        }
    }

    #[test]
    fn default_steps_with_image_paths() {
        let images = vec![
            "C:/tmp/godly-clipboard/clipboard-123.png".to_string(),
            "C:/Users/test/screenshot.jpg".to_string(),
        ];
        let steps = default_launch_steps(1, "fix this bug", "sonnet", "auto", None, &images, false);
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            assert!(command.contains("fix this bug"));
            assert!(command.contains("[Attached images"));
            assert!(command.contains("clipboard-123.png"));
            assert!(command.contains("screenshot.jpg"));
        } else {
            panic!("Expected RunCommand");
        }
    }

    #[test]
    fn default_steps_empty_images_unchanged() {
        let with_images = default_launch_steps(1, "hello", "sonnet", "default", None, &[], false);
        if let LaunchStep::RunCommand { command, .. } = &with_images[2] {
            assert!(!command.contains("[Attached images"));
            assert!(command.contains("'hello'"));
        } else {
            panic!("Expected RunCommand");
        }
    }

    #[test]
    fn resolve_session_id_missing() {
        let ids = vec![None, Some("abc".into())];
        assert!(resolve_session_id(&ids, 0).is_err());
        assert_eq!(resolve_session_id(&ids, 1).unwrap(), "abc");
        assert!(resolve_session_id(&ids, 5).is_err());
    }

    #[test]
    fn default_steps_with_worktree_inserts_create_worktree_step() {
        let steps = default_launch_steps(
            1, "hello", "sonnet", "auto", Some("/my/project"), &[], true,
        );
        // CreateWorktree, CreateTerminal, WaitIdle, RunCommand = 4 steps
        assert_eq!(steps.len(), 4);
        assert!(matches!(steps[0], LaunchStep::CreateWorktree { agent_index: 0, .. }));
        assert!(matches!(steps[1], LaunchStep::CreateTerminal { agent_index: 0, .. }));
    }

    #[test]
    fn default_steps_worktree_no_cwd_skips_worktree_step() {
        let steps = default_launch_steps(1, "hello", "sonnet", "auto", None, &[], true);
        // No CWD → no CreateWorktree, just normal 3 steps
        assert_eq!(steps.len(), 3);
        assert!(matches!(steps[0], LaunchStep::CreateTerminal { agent_index: 0, .. }));
    }

    #[test]
    fn default_steps_worktree_false_no_worktree_step() {
        let steps = default_launch_steps(
            1, "hello", "sonnet", "auto", Some("/my/project"), &[], false,
        );
        // use_worktree=false → no CreateWorktree
        assert_eq!(steps.len(), 3);
        assert!(matches!(steps[0], LaunchStep::CreateTerminal { agent_index: 0, .. }));
    }
}
