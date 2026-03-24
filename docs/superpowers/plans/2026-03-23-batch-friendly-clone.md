# Batch-Friendly Clone Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "Full clone (batch-friendly)" checkbox to the Quick Claude dialog that performs `git clone` instead of `git worktree add`, giving Claude Code's `/batch` command a real `.git` directory to create its own worktrees in.

**Architecture:** New `IsolationMode` enum replaces `use_worktree: bool`. A `CreateClone` launch step shells out to `git clone`. The clone lives in the same `%APPDATA%/com.godly.terminal/worktrees/` directory and reuses the existing `WorktreeCreated` result handling. An `is_clone` flag flows through `TerminalInfo` and `PersistedSessionState` to differentiate cleanup (rm-rf vs git-worktree-remove) and dialog text.

**Tech Stack:** Rust (iced), git CLI, serde JSON persistence

**Spec:** `docs/superpowers/specs/2026-03-23-batch-friendly-clone-design.md`

---

### Task 1: Add `create_clone` and helpers to `git_worktree.rs`

**Files:**
- Modify: `src-tauri/native/iced-shell/src/git_worktree.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn get_remote_url_on_nonexistent_dir() {
    assert!(get_remote_url("/nonexistent/path").is_err());
}

#[test]
fn get_default_branch_on_nonexistent_dir() {
    assert!(get_default_branch("/nonexistent/path").is_err());
}

#[test]
fn create_clone_on_nonexistent_dir() {
    assert!(create_clone("/nonexistent/path", "test-clone").is_err());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test -p iced-shell git_worktree::tests --no-run 2>&1 | head -10`
Expected: compile error — functions don't exist yet.

- [ ] **Step 3: Implement `get_remote_url`**

```rust
/// Get the remote URL for a repository (defaults to "origin").
pub fn get_remote_url(repo_root: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["-C", repo_root, "remote", "get-url", "origin"])
        .output()
        .map_err(|e| format!("Failed to run git: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("No remote 'origin': {stderr}"));
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() {
        return Err("git remote get-url returned empty URL".to_string());
    }
    Ok(url)
}
```

- [ ] **Step 4: Implement `get_default_branch`**

```rust
/// Get the default branch name from the remote.
///
/// Tries `git symbolic-ref refs/remotes/origin/HEAD` first, then falls
/// back to checking if `main` or `master` exists locally.
pub fn get_default_branch(repo_root: &str) -> Result<String, String> {
    // Try symbolic-ref first (works when origin/HEAD is set).
    let output = Command::new("git")
        .args(["-C", repo_root, "symbolic-ref", "refs/remotes/origin/HEAD"])
        .output()
        .map_err(|e| format!("Failed to run git: {e}"))?;

    if output.status.success() {
        let full_ref = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // Parse "refs/remotes/origin/main" → "main"
        if let Some(branch) = full_ref.strip_prefix("refs/remotes/origin/") {
            if !branch.is_empty() {
                return Ok(branch.to_string());
            }
        }
    }

    // Fallback: check if main or master branch exists
    for candidate in &["main", "master"] {
        let check = Command::new("git")
            .args(["-C", repo_root, "rev-parse", "--verify", &format!("refs/remotes/origin/{candidate}")])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if check.map(|s| s.success()).unwrap_or(false) {
            return Ok(candidate.to_string());
        }
    }

    Err("Could not determine default branch".to_string())
}
```

- [ ] **Step 5: Implement `create_clone`**

```rust
/// Clone a repository from its origin remote.
///
/// Creates the clone at `%APPDATA%/com.godly.terminal/worktrees/<dir_name>/`.
/// Returns the absolute path to the clone directory.
pub fn create_clone(repo_folder: &str, dir_name: &str) -> Result<String, String> {
    let repo_root = find_repo_root(repo_folder)?;
    let url = get_remote_url(&repo_root)?;
    let branch = get_default_branch(&repo_root)?;

    let worktrees_dir = worktrees_base_dir()?;
    let clone_path = worktrees_dir.join(dir_name);
    let clone_path_str = clone_path
        .to_str()
        .ok_or_else(|| "Clone path contains invalid characters".to_string())?
        .to_string();

    if !worktrees_dir.exists() {
        std::fs::create_dir_all(&worktrees_dir)
            .map_err(|e| format!("Failed to create worktrees directory: {e}"))?;
    }

    let output = Command::new("git")
        .args(["clone", &url, "--branch", &branch, &clone_path_str])
        .output()
        .map_err(|e| format!("Failed to run git clone: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git clone failed: {stderr}"));
    }

    Ok(clone_path_str)
}
```

- [ ] **Step 6: Add `remove_clone` function**

```rust
/// Remove a cloned repository directory.
///
/// Unlike worktrees, clones have no git bookkeeping — just rm -rf.
pub fn remove_clone(clone_path: &str) -> Result<(), String> {
    let path = Path::new(clone_path);
    if path.exists() {
        std::fs::remove_dir_all(path)
            .map_err(|e| format!("Failed to remove clone directory: {e}"))?;
    }
    Ok(())
}
```

Add test for `remove_clone`:

```rust
#[test]
fn remove_clone_nonexistent_is_ok() {
    assert!(remove_clone("/nonexistent/path/that/does/not/exist").is_ok());
}
```

- [ ] **Step 7: Run tests**

Run: `cd src-tauri && cargo test -p iced-shell git_worktree::tests -- --nocapture`
Expected: all PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/native/iced-shell/src/git_worktree.rs
git commit -m "feat: add create_clone, get_remote_url, get_default_branch to git_worktree"
```

---

### Task 2: Add `IsolationMode` enum and `CreateClone` step to `quick_claude.rs`

**Files:**
- Modify: `src-tauri/native/iced-shell/src/quick_claude.rs`

**Note:** Task 1 must be completed first — `create_clone` must exist before `execute_step` can call it.

- [ ] **Step 1: Write failing test for IsolationMode::Clone step insertion**

```rust
// Add to existing tests module in quick_claude.rs
#[test]
fn default_steps_clone_mode_inserts_create_clone_step() {
    let steps = default_launch_steps(
        1, "hello", "sonnet", "auto", Some("/my/project"), &[], IsolationMode::Clone, None,
    );
    // CreateClone, CreateTerminal, WaitIdle, RunCommand = 4 steps
    assert_eq!(steps.len(), 4);
    assert!(matches!(steps[0], LaunchStep::CreateClone { agent_index: 0, .. }));
    assert!(matches!(steps[1], LaunchStep::CreateTerminal { agent_index: 0, .. }));
}

#[test]
fn default_steps_clone_mode_no_cwd_skips_clone_step() {
    let steps = default_launch_steps(1, "hello", "sonnet", "auto", None, &[], IsolationMode::Clone, None);
    assert_eq!(steps.len(), 3);
    assert!(matches!(steps[0], LaunchStep::CreateTerminal { agent_index: 0, .. }));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test -p iced-shell quick_claude::tests::default_steps_clone --no-run 2>&1 | head -20`
Expected: compilation error — `IsolationMode` does not exist yet.

- [ ] **Step 3: Add `IsolationMode` enum, `CreateClone` variant, `is_clone` on `LaunchState`, update signature**

Add at top of `quick_claude.rs` (after the imports):

```rust
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
```

Add `CreateClone` to the `LaunchStep` enum:

```rust
/// Clone the repository for isolated work. If not a git repo, has no
/// remote, or clone fails, gracefully falls back (returns Ok).
CreateClone {
    agent_index: usize,
    repo_folder: String,
},
```

Add `is_clone: bool` field to `LaunchState`:

```rust
/// Whether this launch uses clone mode (true) or worktree mode (false).
pub is_clone: bool,
```

Initialize it as `false` in `LaunchState::new()`.

Change `default_launch_steps` signature — replace `use_worktree: bool` with `isolation: IsolationMode`. Update the step insertion logic:

```rust
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
    // ... rest unchanged
}
```

- [ ] **Step 4: Update all existing tests to use `IsolationMode`**

Replace every `use_worktree: bool` argument in existing tests:
- `false` → `IsolationMode::None`
- `true` → `IsolationMode::Worktree`

Affected tests (12+): `default_steps_single_no_prompt`, `default_steps_single_with_prompt`, `default_steps_prompt_with_single_quotes`, `default_steps_prompt_with_double_quotes`, `default_steps_grid_2x2`, `default_steps_with_model_and_mode`, `default_steps_auto_mode`, `default_steps_default_mode_no_extra_flag`, `default_steps_propagates_cwd`, `default_steps_with_image_paths`, `default_steps_empty_images_unchanged`, `default_steps_with_worktree_inserts_create_worktree_step`, `default_steps_worktree_no_cwd_skips_worktree_step`, `default_steps_worktree_false_no_worktree_step`, `default_steps_includes_session_id_when_provided`, `default_steps_no_session_id_when_none`.

- [ ] **Step 5: Add `CreateClone` arm to `execute_step`**

In the `execute_step` function, add a match arm for `CreateClone`:

```rust
LaunchStep::CreateClone { repo_folder, .. } => {
    if !crate::git_worktree::is_git_repo(&repo_folder) {
        log::warn!("Not a git repo, skipping clone: {repo_folder}");
        return Ok(StepResult::Ok);
    }
    let dir_name = crate::git_worktree::generate_worktree_dir_name();
    match crate::git_worktree::create_clone(&repo_folder, &dir_name) {
        Ok(clone_path) => Ok(StepResult::WorktreeCreated { worktree_path: clone_path }),
        Err(e) => {
            log::warn!("Clone failed, falling back to main branch: {e}");
            Ok(StepResult::Ok)
        }
    }
}
```

Note: `create_clone` calls `find_repo_root` internally, so we pass `repo_folder` directly — no double `find_repo_root` call.

- [ ] **Step 6: Update the preset launch path in `app.rs`**

At `app.rs` line ~2362, there is a second call to `default_launch_steps` for preset-based launches that passes `use_worktree: false`. Update it to pass `IsolationMode::None`:

```rust
let steps = crate::quick_claude::default_launch_steps(
    num_agents, &preset.prompt_template, "sonnet", "default", cwd.as_deref(), &[], crate::quick_claude::IsolationMode::None, None,
);
```

- [ ] **Step 7: Run all tests to verify they pass**

Run: `cd src-tauri && cargo test -p iced-shell quick_claude::tests -- --nocapture`
Expected: all tests PASS (new clone tests + updated existing tests).

- [ ] **Step 8: Commit**

```bash
git add src-tauri/native/iced-shell/src/quick_claude.rs src-tauri/native/iced-shell/src/app.rs
git commit -m "feat: add IsolationMode enum and CreateClone launch step"
```

---

### Task 3: Add `is_clone` to data models and session persistence

**Files:**
- Modify: `src-tauri/native/iced-shell/src/quick_claude_sessions.rs`
- Modify: `src-tauri/native/iced-shell/src/terminal_state.rs`
- Modify: `src-tauri/native/iced-shell/src/session_persistence.rs`

- [ ] **Step 1: Write failing test for backward compat in `quick_claude_sessions.rs`**

```rust
// In quick_claude_sessions.rs tests
#[test]
fn test_old_records_without_is_clone_deserialize() {
    let json = r#"{"id":"s-1","prompt":"test","terminal_id":"t-1","workspace_id":"ws-1","branch":"main","model":"opus","mode":"code","status":"Running","launched_at":"2026-03-20T12:00:00Z","claude_session_id":null}"#;
    let decoded: QuickClaudeSessionRecord = serde_json::from_str(json).expect("old records should deserialize");
    assert!(!decoded.is_clone);
}

#[test]
fn test_is_clone_roundtrip() {
    let mut record = make_record("s-clone", "t-clone", SessionStatus::Running);
    record.is_clone = true;
    let json = serde_json::to_string(&record).expect("serialize");
    let decoded: QuickClaudeSessionRecord = serde_json::from_str(&json).expect("deserialize");
    assert!(decoded.is_clone);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Expected: compile error — `is_clone` field doesn't exist.

- [ ] **Step 3: Add `is_clone` to `QuickClaudeSessionRecord`**

In `quick_claude_sessions.rs`, add after the `cwd` field:

```rust
#[serde(default)]
pub is_clone: bool,
```

Update `make_record` test helper to include `is_clone: false`.

- [ ] **Step 4: Add `is_clone` to `TerminalInfo`**

In `terminal_state.rs`, add after the `worktree_path` field (line 28):

```rust
/// Whether this terminal's worktree_path points to a clone (true) or a git worktree (false).
pub is_clone: bool,
```

Update the `TerminalInfo` constructor (the place at line ~367 where `worktree_path: None` is set) to also set `is_clone: false`.

Add a setter method alongside `set_worktree_path`:

```rust
pub fn set_clone_flag(&mut self, id: &str, is_clone: bool) {
    if let Some(term) = self.terminals.get_mut(id) {
        term.is_clone = is_clone;
    }
}
```

- [ ] **Step 5: Add `terminal_clone_ids` to persistence structs**

In `session_persistence.rs`, add to `PersistedSessionState` (after `terminal_worktree_paths`):

```rust
/// Terminal IDs whose worktree_path is actually a clone (not a git worktree).
#[serde(default)]
pub terminal_clone_ids: std::collections::HashSet<String>,
```

Add the same field to `MergedSessionState`:

```rust
pub terminal_clone_ids: std::collections::HashSet<String>,
```

Update the merge logic (~line 348) to also merge `terminal_clone_ids`:

```rust
let terminal_clone_ids: std::collections::HashSet<String> = persisted
    .terminal_clone_ids
    .into_iter()
    .filter(|id| live_ids.contains(id))
    .collect();
```

And include it in the returned `MergedSessionState`.

Update all test struct constructions that create `PersistedSessionState` to include `terminal_clone_ids: std::collections::HashSet::new()`.

- [ ] **Step 6: Run tests**

Run: `cd src-tauri && cargo test -p iced-shell -- --nocapture 2>&1 | tail -5`
Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/native/iced-shell/src/quick_claude_sessions.rs src-tauri/native/iced-shell/src/terminal_state.rs src-tauri/native/iced-shell/src/session_persistence.rs
git commit -m "feat: add is_clone flag to session records, terminal state, and persistence"
```

---

### Task 4: Add `batch_clone_mode` to dialog state and preferences

**Files:**
- Modify: `src-tauri/native/iced-shell/src/quick_claude_dialog.rs`

- [ ] **Step 1: Write failing test for preferences**

```rust
#[test]
fn test_preferences_batch_clone_default() {
    let prefs = QuickClaudePreferences::default();
    assert!(!prefs.batch_clone_mode);
}

#[test]
fn test_preferences_batch_clone_roundtrip() {
    let prefs = QuickClaudePreferences {
        batch_clone_mode: true,
        ..Default::default()
    };
    let json = serde_json::to_string(&prefs).unwrap();
    let decoded: QuickClaudePreferences = serde_json::from_str(&json).unwrap();
    assert!(decoded.batch_clone_mode);
}

#[test]
fn test_old_prefs_without_batch_clone_deserialize() {
    let json = r#"{"selected_model":"sonnet","selected_mode":"auto","selected_ai_tool":"Claude Code","selected_workspace_id":null,"auto_suggest_branch":true,"main_branch_mode":false}"#;
    let decoded: QuickClaudePreferences = serde_json::from_str(json).unwrap();
    assert!(!decoded.batch_clone_mode);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Expected: compile error — `batch_clone_mode` doesn't exist.

- [ ] **Step 3: Add `batch_clone_mode` to `QuickClaudePreferences`**

```rust
#[serde(default)]
pub batch_clone_mode: bool,
```

In `Default` impl, add `batch_clone_mode: false`.

- [ ] **Step 4: Add `batch_clone_mode` to `QuickClaudeDialogState`**

Add the field:
```rust
pub batch_clone_mode: bool,
```

Initialize from prefs in `QuickClaudeDialogState::new()`:
```rust
batch_clone_mode: prefs.batch_clone_mode,
```

Include in `to_preferences()`:
```rust
batch_clone_mode: self.batch_clone_mode,
```

- [ ] **Step 5: Render the checkbox in the dialog UI**

In the `render_new_prompt_tab` function (around line 840, after the `auto_suggest_btn`):

```rust
let batch_clone_indicator = if state.batch_clone_mode { "\u{2611}" } else { "\u{2610}" };
let batch_clone_color = if state.batch_clone_mode { accent } else { text_primary };
let batch_clone_btn = button(
    row![
        text(batch_clone_indicator).size(14).color(accent),
        text("Full clone (batch-friendly)")
            .size(12)
            .color(batch_clone_color)
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center),
)
.on_press(on_batch_clone_toggled(!state.batch_clone_mode))
.padding(Padding::from([4, 8]))
.style(move |_theme, _status| button::Style {
    background: None,
    border: Border::default(),
    ..button::Style::default()
});
```

Add `batch_clone_btn` to the `checkbox_row`:
```rust
let checkbox_row = row![main_branch_btn, auto_suggest_btn, batch_clone_btn].spacing(12);
```

The `on_batch_clone_toggled` callback needs to be added as a parameter to `render_new_prompt_tab` (same pattern as `on_main_branch_toggled` and `on_auto_suggest_toggled`). Also add it to `view_quick_claude_dialog`'s signature and pass it through.

When `batch_clone_mode` is true, gray out the other two checkboxes by using `text_secondary` color instead of `text_primary`/`accent`, and don't attach `.on_press()` so they're visually disabled. Apply the same pattern in reverse: when `main_branch_mode` is true, gray out the batch clone checkbox.

- [ ] **Step 6: Run tests**

Run: `cd src-tauri && cargo test -p iced-shell quick_claude_dialog::tests -- --nocapture`
Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/native/iced-shell/src/quick_claude_dialog.rs
git commit -m "feat: add batch_clone_mode checkbox to Quick Claude dialog"
```

---

### Task 5: Wire up `app.rs` — message, launch, step handling, cleanup

**Files:**
- Modify: `src-tauri/native/iced-shell/src/app.rs`
- Modify: `src-tauri/native/iced-shell/src/confirm_dialog.rs`

- [ ] **Step 1: Add message variant**

In the `Message` enum (around line 779, after `QuickClaudeDialogAutoSuggestToggled`):

```rust
QuickClaudeDialogBatchCloneToggled(bool),
```

- [ ] **Step 2: Handle the toggle message**

In the `update()` match (find where `QuickClaudeDialogMainBranchToggled` is handled, around line 2535):

```rust
Message::QuickClaudeDialogBatchCloneToggled(val) => {
    if let Some(ref mut dlg) = self.quick_claude_dialog {
        dlg.batch_clone_mode = val;
        if val {
            // Mutual exclusion: disable main_branch_mode and auto_suggest
            dlg.main_branch_mode = false;
            dlg.auto_suggest_branch = false;
        }
    }
}
```

Also update `QuickClaudeDialogMainBranchToggled` handler to uncheck batch_clone_mode:

```rust
Message::QuickClaudeDialogMainBranchToggled(val) => {
    if let Some(ref mut dlg) = self.quick_claude_dialog {
        dlg.main_branch_mode = val;
        if val {
            dlg.batch_clone_mode = false;
        }
    }
}
```

- [ ] **Step 3: Update launch path to compute `IsolationMode`**

At line ~2635, replace:
```rust
let use_worktree = !dlg.main_branch_mode;
```

With:
```rust
let isolation = if dlg.batch_clone_mode {
    crate::quick_claude::IsolationMode::Clone
} else if dlg.main_branch_mode {
    crate::quick_claude::IsolationMode::None
} else {
    crate::quick_claude::IsolationMode::Worktree
};
```

Update the `default_launch_steps` call to pass `isolation` instead of `use_worktree`.

- [ ] **Step 4: Set `is_clone` on session record**

At line ~2653, in the session record construction, add:
```rust
is_clone: dlg.batch_clone_mode,
```

- [ ] **Step 5: Update `handle_launch_step_result` to match `CreateClone`**

At line ~7496, where the agent_index is extracted, extend the match:

```rust
let agent_idx = launch.steps.get(si).and_then(|step| {
    match step {
        crate::quick_claude::LaunchStep::CreateWorktree { agent_index, .. } => Some(*agent_index),
        crate::quick_claude::LaunchStep::CreateClone { agent_index, .. } => Some(*agent_index),
        _ => None,
    }
});
```

- [ ] **Step 6: Set `is_clone` on terminal state when worktree path is applied**

Find where `set_worktree_path` is called for the newly created terminal (around the `WorktreeCreated` handling in `handle_launch_step_result` or `Message::WorktreeCreated`). After setting the worktree path, also check if this was a clone:

In the `Message::WorktreeCreated` handler at line 3402-3404, we need to also propagate `is_clone`. The simplest approach: check the session record.

Actually, the better approach: store `is_clone` on `LaunchState` itself. Add a field:
```rust
pub is_clone: bool,
```

Set it when constructing `LaunchState` (from `dlg.batch_clone_mode`). Then in the `WorktreeCreated` step result handler, after `launch.pending_worktree_path = Some(worktree_path.clone())`, also read `launch.is_clone`.

When the `TerminalCreated` step applies the pending worktree path, also call `self.terminals.set_clone_flag(&session_id, launch.is_clone)`.

- [ ] **Step 7: Update cleanup in `WorktreeCloseConfirmed`**

At line 3406-3428, the cleanup currently calls `remove_worktree`. Branch on `is_clone`:

```rust
Message::WorktreeCloseConfirmed { session_id } => {
    self.worktree_close_pending = None;
    let worktree_path = self.terminals.get(&session_id)
        .and_then(|t| t.worktree_path.clone());
    let is_clone = self.terminals.get(&session_id)
        .map(|t| t.is_clone)
        .unwrap_or(false);
    let repo_root = self.workspaces.active()
        .map(|ws| ws.folder_path.clone());
    let close_task = self.close_terminal_immediate(&session_id);
    if let Some(wt_path) = worktree_path {
        let sid = session_id.clone();
        let remove_task = if is_clone {
            Task::perform(
                async move {
                    let (tx, rx) = futures_channel::oneshot::channel::<Result<(), String>>();
                    std::thread::spawn(move || {
                        let result = crate::git_worktree::remove_clone(&wt_path);
                        let _ = tx.send(result);
                    });
                    rx.await.unwrap_or_else(|_| Err("Background thread panicked".into()))
                },
                move |result| Message::WorktreeRemoved { session_id: sid, result },
            )
        } else if let Some(root) = repo_root {
            Task::perform(
                async move {
                    let (tx, rx) = futures_channel::oneshot::channel::<Result<(), String>>();
                    std::thread::spawn(move || {
                        let result = crate::git_worktree::remove_worktree(&root, &wt_path);
                        let _ = tx.send(result);
                    });
                    rx.await.unwrap_or_else(|_| Err("Background thread panicked".into()))
                },
                move |result| Message::WorktreeRemoved { session_id: sid, result },
            )
        } else {
            Task::none()
        };
        return Task::batch([close_task, remove_task]);
    }
    return close_task;
}
```

- [ ] **Step 8: Update confirm dialog text**

In `confirm_dialog.rs`, update `view_worktree_close_confirm` signature to accept `is_clone: bool`:

```rust
pub fn view_worktree_close_confirm<'a, M: Clone + 'a>(
    worktree_path: &'a str,
    is_clone: bool,
    on_remove: M,
    on_keep: M,
    on_cancel: M,
) -> Element<'a, M>
```

Change the text strings based on `is_clone`:
- Title: `"Close Clone Terminal?"` vs `"Close Worktree Terminal?"`
- Body: `"This terminal uses a git clone at:"` vs `"This terminal uses a git worktree at:"`
- Prompt: `"Remove the clone from disk, or keep it for later?"` vs `"Remove the worktree..."`
- Buttons: `"Keep Clone"` / `"Remove Clone"` vs `"Keep Worktree"` / `"Remove Worktree"`

Update the call site in `app.rs` (line ~4094) to pass `is_clone`:

```rust
let is_clone = self.terminals.get(pending_id)
    .map(|t| t.is_clone)
    .unwrap_or(false);
crate::confirm_dialog::view_worktree_close_confirm(
    worktree_path,
    is_clone,
    Message::WorktreeCloseConfirmed { session_id: pid1 },
    Message::WorktreeCloseKeep { session_id: pid2 },
    Message::WorktreeCloseCancelled,
)
```

- [ ] **Step 9: Pass the toggle callback to the dialog render function**

Find where `render_new_prompt_tab` is called and pass the new callback closure:
```rust
|val| Message::QuickClaudeDialogBatchCloneToggled(val)
```

This follows the same pattern as `on_main_branch_toggled` and `on_auto_suggest_toggled`.

- [ ] **Step 10: Update session persistence save/restore**

In the save path (where `terminal_worktree_paths` is populated), also populate `terminal_clone_ids` for terminals where `is_clone == true`.

In the restore path (where worktree paths are applied to terminals after restart), also restore the `is_clone` flag from `terminal_clone_ids`.

- [ ] **Step 11: Run full test suite**

Run: `cd src-tauri && cargo test -p iced-shell -- --nocapture 2>&1 | tail -10`
Expected: all PASS.

- [ ] **Step 12: Commit**

```bash
git add src-tauri/native/iced-shell/src/app.rs src-tauri/native/iced-shell/src/confirm_dialog.rs
git commit -m "feat: wire up batch clone mode in app — launch, cleanup, confirm dialog"
```

---

### Task 6: Changelog fragment and PR

**Files:**
- Create: `changelog/unreleased/batch-friendly-clone.md`

- [ ] **Step 1: Create changelog fragment**

```markdown
### Added
- **Full clone mode** - Quick Claude dialog now has a "Full clone (batch-friendly)" checkbox that clones the repo instead of creating a worktree, enabling Claude Code's `/batch` command to work properly
```

- [ ] **Step 2: Commit and push**

```bash
git add changelog/unreleased/batch-friendly-clone.md
git commit -m "docs: add changelog fragment for batch-friendly clone"
git push -u origin HEAD
```

- [ ] **Step 3: Create PR**

Use the git-workflow-manager agent to open a PR referencing the relevant issue.
