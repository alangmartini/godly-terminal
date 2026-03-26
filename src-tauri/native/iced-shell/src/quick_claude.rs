use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use godly_app_adapter::commands;
use godly_app_adapter::daemon_client::NativeDaemonClient;

/// How the Quick Claude session is isolated from the main repo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IsolationMode {
    /// No isolation — run in the workspace folder directly.
    None,
    /// Create a git worktree (detached HEAD).
    Worktree,
    /// Full git clone from origin's default branch.
    Clone,
}

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
    /// Clone the repository for isolated work. If not a git repo, has no
    /// remote, or clone fails, gracefully falls back (returns Ok).
    CreateClone {
        agent_index: usize,
        repo_folder: String,
    },
    /// Handle the Claude Code workspace trust prompt if it appears.
    /// Polls the terminal grid for trust prompt indicators and sends Enter to accept.
    /// Non-blocking: returns Ok silently if no trust prompt appears within timeout
    /// or if Claude has moved past startup (producing working output).
    HandleTrustPromptIfNeeded { agent_index: usize, timeout_ms: u64 },
}

impl LaunchStep {
    /// Human-readable label for display in the loading overlay.
    pub fn label(&self) -> &'static str {
        match self {
            LaunchStep::CreateWorktree { .. } => "Creating worktree",
            LaunchStep::CreateClone { .. } => "Cloning repository",
            LaunchStep::CreateTerminal { .. } => "Starting terminal",
            LaunchStep::WaitIdle { .. } => "Waiting for shell",
            LaunchStep::RunCommand { .. } => "Launching Claude",
            LaunchStep::WaitReady { .. } => "Waiting for Claude",
            LaunchStep::WaitForClaudeReady { .. } => "Waiting for Claude",
            LaunchStep::SendEnter { .. } => "Sending input",
            LaunchStep::SendPrompt { .. } => "Sending prompt",
            LaunchStep::WaitForEcho { .. } => "Confirming input",
            LaunchStep::Delay { .. } => "Preparing",
            LaunchStep::HandleTrustPromptIfNeeded { .. } => "Finalizing setup",
        }
    }
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
            command: format!("claude --resume {}", session_id),
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
    isolation: IsolationMode,
    claude_session_id: Option<&str>,
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
    // Pre-assign Claude session ID for resume support
    if let Some(sid) = claude_session_id {
        cmd.push_str(&format!(" --session-id {}", sid));
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
        // Collapse newlines/carriage-returns into spaces — embedded newlines in
        // the PTY write buffer are interpreted as Enter keypresses by the shell,
        // splitting the command across multiple input lines.
        let oneline = effective_prompt
            .replace("\r\n", " ")
            .replace('\r', " ")
            .replace('\n', " ");
        // Shell-escape: wrap in single quotes, escape embedded single quotes
        // PowerShell escapes single quotes by doubling them: 'isn''t' -> isn't
        let escaped = oneline.replace('\'', "''");
        cmd.push_str(&format!(" '{}'", escaped));
    }

    let mut steps = Vec::new();
    for i in 0..num_agents {
        match isolation {
            IsolationMode::Worktree => {
                if let Some(folder) = cwd {
                    steps.push(LaunchStep::CreateWorktree {
                        agent_index: i,
                        repo_folder: folder.to_string(),
                    });
                }
            }
            IsolationMode::Clone => {
                if let Some(folder) = cwd {
                    steps.push(LaunchStep::CreateClone {
                        agent_index: i,
                        repo_folder: folder.to_string(),
                    });
                }
            }
            IsolationMode::None => {}
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
        steps.push(LaunchStep::HandleTrustPromptIfNeeded {
            agent_index: i,
            timeout_ms: 20_000,
        });
    }
    steps
}

/// State for a running Quick Claude launch.
#[derive(Debug, Clone)]
pub struct LaunchState {
    /// Unique identifier for this launch instance. Used to route step
    /// completions when multiple launches target the same workspace.
    pub launch_id: String,
    pub preset_name: String,
    pub steps: Vec<LaunchStep>,
    pub current_step: usize,
    pub agent_terminal_ids: Vec<Option<String>>,
    pub workspace_id: String,
    pub is_new_workspace: bool,
    pub completed: bool,
    pub error: Option<String>,
    /// Whether this launch uses clone mode (true) or worktree mode (false).
    pub is_clone: bool,
    /// Worktree path created by a `CreateWorktree` step, applied to the next
    /// terminal that gets created.
    pub pending_worktree_path: Option<String>,
    /// ID of the QuickClaudeSessionRecord, for updating CWD/terminal_id after async steps.
    pub session_record_id: Option<String>,
    /// Placeholder terminal IDs created before the real terminal.
    /// Used by the UI to detect which panes should show the launch progress overlay.
    pub placeholder_ids: HashSet<String>,
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
            launch_id: uuid::Uuid::new_v4().to_string(),
            preset_name,
            steps,
            current_step: 0,
            agent_terminal_ids: vec![None; num_agents],
            workspace_id,
            is_new_workspace,
            completed: false,
            error: None,
            is_clone: false,
            pending_worktree_path: None,
            session_record_id: None,
            placeholder_ids: HashSet::new(),
        }
    }

    pub fn total_steps(&self) -> usize {
        self.steps.len()
    }

    /// Human-readable label of the current launch step.
    pub fn current_step_label(&self) -> &'static str {
        self.steps
            .get(self.current_step)
            .map(|s| s.label())
            .unwrap_or("Launching")
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
        LaunchStep::WaitIdle {
            agent_index,
            idle_ms,
        } => {
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
        LaunchStep::CreateClone { repo_folder, .. } => {
            if !crate::git_worktree::is_git_repo(&repo_folder) {
                log::warn!("Not a git repo, skipping clone: {repo_folder}");
                return Ok(StepResult::Ok);
            }
            let dir_name = crate::git_worktree::generate_worktree_dir_name();
            match crate::git_worktree::create_clone(&repo_folder, &dir_name) {
                Ok(clone_path) => Ok(StepResult::WorktreeCreated {
                    worktree_path: clone_path,
                }),
                Err(e) => {
                    log::warn!("Clone failed, falling back to main branch: {e}");
                    Ok(StepResult::Ok)
                }
            }
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
        LaunchStep::HandleTrustPromptIfNeeded {
            agent_index,
            timeout_ms,
        } => {
            let session_id = resolve_session_id(&agent_terminal_ids, agent_index)?;
            handle_trust_prompt_if_needed(&client, &session_id, timeout_ms)?;
            Ok(StepResult::Ok)
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

        let plain = commands::get_plain_grid(client, session_id)?;
        let hash = simple_rows_hash(&plain.rows);

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

        let plain = commands::get_plain_grid(client, session_id)?;
        let text = plain.rows.join("\n");
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

        let plain = commands::get_plain_grid(client, session_id)?;
        let text = plain.rows.join("\n");
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

        let plain = commands::get_plain_grid(client, session_id)?;
        let text = plain.rows.join("\n");

        // Handle trust prompt if present and not skipped
        if !skip_trust && !trust_handled && has_trust_prompt_text(&text) {
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
        if has_prompt && (skip_trust || trust_handled || !has_trust_prompt_text(&text)) {
            return Ok(());
        }

        std::thread::sleep(poll_interval);
    }
}

/// Check if the grid text contains trust prompt indicators.
/// Matches both old ("Do you trust the files") and current
/// ("I trust this folder") Claude Code wording.
fn has_trust_prompt_text(text: &str) -> bool {
    text.contains("I trust this folder")
        || text.contains("Do you trust the files")
        || text.contains("Do you trust")
}

/// Non-blocking trust prompt handler for CLI-arg launches.
///
/// When Claude Code is launched with a prompt as a CLI positional argument,
/// the trust prompt blocks processing. This function detects and dismisses it.
/// If no trust prompt appears within the timeout, returns Ok (the project may
/// already be trusted or Claude has moved past startup).
fn handle_trust_prompt_if_needed(
    client: &NativeDaemonClient,
    session_id: &str,
    timeout_ms: u64,
) -> Result<(), String> {
    let timeout = Duration::from_millis(timeout_ms);
    let poll_interval = Duration::from_millis(300);
    let start = std::time::Instant::now();
    let max_accept_attempts = 5;
    let mut accept_attempts = 0;

    loop {
        if start.elapsed() > timeout {
            if accept_attempts > 0 {
                log::warn!(
                    "Quick Claude: trust prompt still visible after {} accept attempts and timeout",
                    accept_attempts,
                );
            }
            return Ok(());
        }

        let plain = commands::get_plain_grid(client, session_id)?;
        let text = plain.rows.join("\n");

        log::debug!(
            "Quick Claude: grid text (first 200 chars): {:?}",
            text.chars().take(200).collect::<String>()
        );

        // Trust prompt detected — accept it
        if has_trust_prompt_text(&text) {
            accept_attempts += 1;
            if accept_attempts == 1 {
                log::info!("Quick Claude: trust prompt detected, waiting for input handler to stabilize");
                // First detection: wait for Claude Code's TUI input handler to
                // fully initialize before sending the keypress. Without this
                // delay the \r often arrives before the prompt is ready to
                // accept input and gets silently swallowed.
                std::thread::sleep(Duration::from_millis(800));
            }
            log::info!(
                "Quick Claude: sending Enter to accept trust prompt (attempt {}/{})",
                accept_attempts, max_accept_attempts,
            );
            commands::write_to_terminal(client, session_id, b"\r")?;

            if accept_attempts >= max_accept_attempts {
                log::warn!("Quick Claude: exhausted accept attempts, giving up");
                return Ok(());
            }

            // Wait for the prompt to process the keypress before re-checking
            std::thread::sleep(Duration::from_millis(1_500));
            continue;
        }

        // If we previously sent Enter and the trust prompt is now gone, success
        if accept_attempts > 0 {
            log::info!(
                "Quick Claude: trust prompt dismissed after {} attempt(s)",
                accept_attempts,
            );
            return Ok(());
        }

        // Early exit: if Claude is genuinely past startup (not on the trust
        // prompt screen), the trust prompt was never shown.
        // NOTE: Do NOT use `text.contains("Claude Code")` here — the trust
        // prompt screen itself shows "Claude Code'll be able to read, edit,
        // and execute files here." which causes a false positive.
        let on_trust_screen = text.contains("safety check")
            || text.contains("Accessing workspace")
            || text.contains("Esc to cancel");
        let past_startup = !on_trust_screen
            && (text.contains("Task(")
                || text.lines().rev().any(|l| {
                    let t = l.trim();
                    t == ">" || t == "> "
                }));
        if past_startup && start.elapsed() > Duration::from_millis(3_000) {
            return Ok(());
        }

        std::thread::sleep(poll_interval);
    }
}

/// Simple hash of plain-text rows for change detection.
fn simple_rows_hash(rows: &[String]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for row in rows {
        row.hash(&mut hasher);
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
        assert!(matches!(
            steps[0],
            LaunchStep::CreateTerminal { agent_index: 0, .. }
        ));
        assert!(matches!(
            steps[1],
            LaunchStep::WaitIdle { agent_index: 0, .. }
        ));
        match &steps[2] {
            LaunchStep::RunCommand {
                agent_index,
                command,
            } => {
                assert_eq!(*agent_index, 0);
                assert!(command.contains("--resume"));
                assert!(command.contains("abc-123"));
            }
            _ => panic!("Expected RunCommand"),
        }
        assert!(matches!(
            steps[3],
            LaunchStep::WaitReady { agent_index: 0, .. }
        ));
    }

    #[test]
    fn resume_steps_uses_correct_cli_syntax() {
        let steps = resume_launch_steps("abc-123-def", Some("/test/dir"));
        match &steps[2] {
            LaunchStep::RunCommand { command, .. } => {
                // Must use `claude --resume <id>`, NOT `claude --resume --session-id <id>`
                assert_eq!(command, "claude --resume abc-123-def");
                assert!(
                    !command.contains("--session-id"),
                    "should not use --session-id flag"
                );
            }
            _ => panic!("Expected RunCommand"),
        }
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
        let steps = default_launch_steps(
            1,
            "",
            "sonnet",
            "default",
            None,
            &[],
            IsolationMode::None,
            None,
        );
        // CreateTerminal, WaitIdle, RunCommand, HandleTrustPromptIfNeeded
        assert_eq!(steps.len(), 4);
        assert!(matches!(
            steps[0],
            LaunchStep::CreateTerminal { agent_index: 0, .. }
        ));
        assert!(matches!(
            steps[1],
            LaunchStep::WaitIdle { agent_index: 0, .. }
        ));
        assert!(matches!(
            steps[2],
            LaunchStep::RunCommand { agent_index: 0, .. }
        ));
        assert!(matches!(
            steps[3],
            LaunchStep::HandleTrustPromptIfNeeded { agent_index: 0, .. }
        ));
    }

    #[test]
    fn default_steps_single_with_prompt() {
        let steps = default_launch_steps(
            1,
            "build the app",
            "sonnet",
            "default",
            None,
            &[],
            IsolationMode::None,
            None,
        );
        // Prompt is now a CLI arg — 4 steps, prompt embedded in command
        assert_eq!(steps.len(), 4);
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            assert!(command.contains("'build the app'"));
        } else {
            panic!("Expected RunCommand");
        }
    }

    #[test]
    fn default_steps_prompt_with_single_quotes() {
        let steps = default_launch_steps(
            1,
            "fix it's broken",
            "sonnet",
            "auto",
            None,
            &[],
            IsolationMode::None,
            None,
        );
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            // PowerShell escapes single quotes by doubling them
            assert!(command.contains("'fix it''s broken'"), "got: {command}");
        } else {
            panic!("Expected RunCommand");
        }
    }

    #[test]
    fn default_steps_prompt_with_embedded_newlines() {
        let prompt = "Lets implement it :)\n\n   ⎿  Error\nclaude stuff";
        let steps = default_launch_steps(
            1,
            prompt,
            "sonnet",
            "auto",
            None,
            &[],
            IsolationMode::None,
            None,
        );
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            // Newlines must be collapsed to spaces so the command stays on one PTY line
            assert!(
                !command.contains('\n'),
                "command must not contain newlines: {command}"
            );
            assert!(
                !command.contains('\r'),
                "command must not contain carriage returns: {command}"
            );
            assert!(command.contains("Lets implement it :)"), "got: {command}");
        } else {
            panic!("Expected RunCommand");
        }
    }

    #[test]
    fn default_steps_prompt_with_double_quotes() {
        let steps = default_launch_steps(
            1,
            "what is \"the issue\"",
            "sonnet",
            "auto",
            None,
            &[],
            IsolationMode::None,
            None,
        );
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
        let steps = default_launch_steps(
            4,
            "test",
            "sonnet",
            "default",
            None,
            &[],
            IsolationMode::None,
            None,
        );
        // Each agent: 4 steps. 4 agents = 16
        assert_eq!(steps.len(), 16);
    }

    #[test]
    fn default_steps_with_model_and_mode() {
        let steps =
            default_launch_steps(1, "", "opus", "plan", None, &[], IsolationMode::None, None);
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            assert_eq!(command, "claude --model opus --permission-mode plan");
        } else {
            panic!("Expected RunCommand at index 2");
        }
    }

    #[test]
    fn default_steps_auto_mode() {
        let steps =
            default_launch_steps(1, "", "haiku", "auto", None, &[], IsolationMode::None, None);
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            assert_eq!(
                command,
                "claude --model haiku --dangerously-skip-permissions"
            );
        } else {
            panic!("Expected RunCommand at index 2");
        }
    }

    #[test]
    fn default_steps_default_mode_no_extra_flag() {
        let steps = default_launch_steps(
            1,
            "",
            "sonnet",
            "default",
            None,
            &[],
            IsolationMode::None,
            None,
        );
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            assert_eq!(command, "claude --model sonnet");
        } else {
            panic!("Expected RunCommand at index 2");
        }
    }

    #[test]
    fn default_steps_propagates_cwd() {
        let steps = default_launch_steps(
            1,
            "",
            "sonnet",
            "default",
            Some("/my/project"),
            &[],
            IsolationMode::None,
            None,
        );
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
        let steps = default_launch_steps(
            1,
            "fix this bug",
            "sonnet",
            "auto",
            None,
            &images,
            IsolationMode::None,
            None,
        );
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
        let with_images = default_launch_steps(
            1,
            "hello",
            "sonnet",
            "default",
            None,
            &[],
            IsolationMode::None,
            None,
        );
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
            1,
            "hello",
            "sonnet",
            "auto",
            Some("/my/project"),
            &[],
            IsolationMode::Worktree,
            None,
        );
        // CreateWorktree, CreateTerminal, WaitIdle, RunCommand, HandleTrustPromptIfNeeded = 5 steps
        assert_eq!(steps.len(), 5);
        assert!(matches!(
            steps[0],
            LaunchStep::CreateWorktree { agent_index: 0, .. }
        ));
        assert!(matches!(
            steps[1],
            LaunchStep::CreateTerminal { agent_index: 0, .. }
        ));
    }

    #[test]
    fn default_steps_worktree_no_cwd_skips_worktree_step() {
        let steps = default_launch_steps(
            1,
            "hello",
            "sonnet",
            "auto",
            None,
            &[],
            IsolationMode::Worktree,
            None,
        );
        // No CWD → no CreateWorktree, just normal 4 steps
        assert_eq!(steps.len(), 4);
        assert!(matches!(
            steps[0],
            LaunchStep::CreateTerminal { agent_index: 0, .. }
        ));
    }

    #[test]
    fn default_steps_worktree_false_no_worktree_step() {
        let steps = default_launch_steps(
            1,
            "hello",
            "sonnet",
            "auto",
            Some("/my/project"),
            &[],
            IsolationMode::None,
            None,
        );
        // IsolationMode::None → no CreateWorktree, just normal 4 steps
        assert_eq!(steps.len(), 4);
        assert!(matches!(
            steps[0],
            LaunchStep::CreateTerminal { agent_index: 0, .. }
        ));
    }

    #[test]
    fn default_steps_includes_session_id_when_provided() {
        let steps = default_launch_steps(
            1,
            "test",
            "sonnet",
            "default",
            None,
            &[],
            IsolationMode::None,
            Some("my-uuid-123"),
        );
        match &steps[2] {
            LaunchStep::RunCommand { command, .. } => {
                assert!(command.contains("--session-id my-uuid-123"));
            }
            _ => panic!("Expected RunCommand"),
        }
    }

    #[test]
    fn default_steps_no_session_id_when_none() {
        let steps = default_launch_steps(
            1,
            "test",
            "sonnet",
            "default",
            None,
            &[],
            IsolationMode::None,
            None,
        );
        match &steps[2] {
            LaunchStep::RunCommand { command, .. } => {
                assert!(!command.contains("--session-id"));
            }
            _ => panic!("Expected RunCommand"),
        }
    }

    #[test]
    fn default_steps_clone_mode_inserts_create_clone_step() {
        let steps = default_launch_steps(
            1,
            "hello",
            "sonnet",
            "auto",
            Some("/my/project"),
            &[],
            IsolationMode::Clone,
            None,
        );
        // CreateClone, CreateTerminal, WaitIdle, RunCommand, HandleTrustPromptIfNeeded = 5 steps
        assert_eq!(steps.len(), 5);
        assert!(matches!(
            steps[0],
            LaunchStep::CreateClone { agent_index: 0, .. }
        ));
        assert!(matches!(
            steps[1],
            LaunchStep::CreateTerminal { agent_index: 0, .. }
        ));
    }

    #[test]
    fn default_steps_clone_mode_no_cwd_skips_clone_step() {
        let steps = default_launch_steps(
            1,
            "hello",
            "sonnet",
            "auto",
            None,
            &[],
            IsolationMode::Clone,
            None,
        );
        // No CWD → no CreateClone, just normal 4 steps
        assert_eq!(steps.len(), 4);
        assert!(matches!(
            steps[0],
            LaunchStep::CreateTerminal { agent_index: 0, .. }
        ));
    }

    #[test]
    fn has_trust_prompt_detects_current_wording() {
        assert!(has_trust_prompt_text(
            "  > 1. Yes, I trust this folder\n  2. No, exit"
        ));
    }

    #[test]
    fn has_trust_prompt_detects_old_wording() {
        assert!(has_trust_prompt_text(
            "Do you trust the files in this folder?"
        ));
    }

    #[test]
    fn has_trust_prompt_negative() {
        assert!(!has_trust_prompt_text("Claude Code v1.0.0\nLoading..."));
    }

    #[test]
    fn default_steps_trust_timeout_is_20s() {
        let steps = default_launch_steps(
            1,
            "test",
            "sonnet",
            "default",
            None,
            &[],
            IsolationMode::None,
            None,
        );
        match &steps[3] {
            LaunchStep::HandleTrustPromptIfNeeded { timeout_ms, .. } => {
                assert_eq!(*timeout_ms, 20_000);
            }
            _ => panic!("Expected HandleTrustPromptIfNeeded"),
        }
    }
}
