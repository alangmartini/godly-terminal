use std::sync::Arc;
use std::time::Duration;

use godly_app_adapter::commands;
use godly_app_adapter::daemon_client::NativeDaemonClient;

/// A single step in a Quick Claude launch sequence.
#[derive(Debug, Clone)]
pub enum LaunchStep {
    /// Create a terminal session via daemon.
    CreateTerminal { agent_index: usize },
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
    /// Send Enter key (carriage return).
    SendEnter { agent_index: usize },
    /// Send the preset prompt text + carriage return.
    SendPrompt { agent_index: usize, prompt: String },
    /// Sleep for N milliseconds.
    Delay { ms: u64 },
}

/// Build launch steps for resuming an existing Claude session.
pub fn resume_launch_steps(session_id: &str) -> Vec<LaunchStep> {
    vec![
        LaunchStep::CreateTerminal { agent_index: 0 },
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
pub fn default_launch_steps(num_agents: usize, prompt: &str, model: &str, mode: &str) -> Vec<LaunchStep> {
    let mut cmd = "claude".to_string();
    // Add model flag (always, since we default to sonnet)
    cmd.push_str(&format!(" --model {}", model));
    // Add mode flag
    match mode {
        "plan" => cmd.push_str(" --plan"),
        "auto" => cmd.push_str(" --dangerously-skip-permissions"),
        _ => {} // "default" — no flag
    }

    let mut steps = Vec::new();
    for i in 0..num_agents {
        steps.push(LaunchStep::CreateTerminal { agent_index: i });
        steps.push(LaunchStep::WaitIdle {
            agent_index: i,
            idle_ms: 2000,
        });
        steps.push(LaunchStep::RunCommand {
            agent_index: i,
            command: cmd.clone(),
        });
        steps.push(LaunchStep::WaitReady {
            agent_index: i,
            marker: "trust".to_string(),
            timeout_ms: 30000,
        });
        steps.push(LaunchStep::SendEnter { agent_index: i });
        steps.push(LaunchStep::WaitReady {
            agent_index: i,
            marker: ">".to_string(),
            timeout_ms: 15000,
        });
        if !prompt.is_empty() {
            steps.push(LaunchStep::SendPrompt {
                agent_index: i,
                prompt: prompt.to_string(),
            });
        }
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
    pub completed: bool,
    pub error: Option<String>,
}

impl LaunchState {
    pub fn new(preset_name: String, steps: Vec<LaunchStep>, num_agents: usize, workspace_id: String) -> Self {
        Self {
            preset_name,
            steps,
            current_step: 0,
            agent_terminal_ids: vec![None; num_agents],
            workspace_id,
            completed: false,
            error: None,
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
        LaunchStep::CreateTerminal { .. } => {
            let session_id = uuid::Uuid::new_v4().to_string();
            commands::create_terminal(
                &client,
                &session_id,
                godly_protocol::ShellType::Windows,
                None,
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
            let data = format!("{}\r", prompt);
            commands::write_to_terminal(&client, &session_id, data.as_bytes())?;
            Ok(StepResult::Ok)
        }
        LaunchStep::Delay { ms } => {
            std::thread::sleep(Duration::from_millis(ms));
            Ok(StepResult::Ok)
        }
    }
}

#[derive(Debug, Clone)]
pub enum StepResult {
    Ok,
    TerminalCreated(String),
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
        let steps = resume_launch_steps("abc-123");
        assert_eq!(steps.len(), 4);
        assert!(matches!(steps[0], LaunchStep::CreateTerminal { agent_index: 0 }));
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
    fn default_steps_single_no_prompt() {
        let steps = default_launch_steps(1, "", "sonnet", "default");
        // CreateTerminal, WaitIdle, RunCommand, WaitReady(trust), SendEnter, WaitReady(>)
        assert_eq!(steps.len(), 6);
        assert!(matches!(steps[0], LaunchStep::CreateTerminal { agent_index: 0 }));
        assert!(matches!(steps[2], LaunchStep::RunCommand { agent_index: 0, .. }));
    }

    #[test]
    fn default_steps_single_with_prompt() {
        let steps = default_launch_steps(1, "build the app", "sonnet", "default");
        // 6 base steps + 1 SendPrompt
        assert_eq!(steps.len(), 7);
        assert!(matches!(steps[6], LaunchStep::SendPrompt { agent_index: 0, .. }));
    }

    #[test]
    fn default_steps_grid_2x2() {
        let steps = default_launch_steps(4, "test", "sonnet", "default");
        // Each agent: 7 steps (with prompt). 4 agents = 28
        assert_eq!(steps.len(), 28);
    }

    #[test]
    fn default_steps_with_model_and_mode() {
        let steps = default_launch_steps(1, "", "opus", "plan");
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            assert_eq!(command, "claude --model opus --plan");
        } else {
            panic!("Expected RunCommand at index 2");
        }
    }

    #[test]
    fn default_steps_auto_mode() {
        let steps = default_launch_steps(1, "", "haiku", "auto");
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            assert_eq!(command, "claude --model haiku --dangerously-skip-permissions");
        } else {
            panic!("Expected RunCommand at index 2");
        }
    }

    #[test]
    fn default_steps_default_mode_no_extra_flag() {
        let steps = default_launch_steps(1, "", "sonnet", "default");
        if let LaunchStep::RunCommand { command, .. } = &steps[2] {
            assert_eq!(command, "claude --model sonnet");
        } else {
            panic!("Expected RunCommand at index 2");
        }
    }

    #[test]
    fn resolve_session_id_missing() {
        let ids = vec![None, Some("abc".into())];
        assert!(resolve_session_id(&ids, 0).is_err());
        assert_eq!(resolve_session_id(&ids, 1).unwrap(), "abc");
        assert!(resolve_session_id(&ids, 5).is_err());
    }
}
