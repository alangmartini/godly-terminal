# Batch-Friendly Clone Option in Quick Claude

## Problem

Quick Claude sessions run inside git worktrees (`godly-wt-*`). Claude Code's `/batch` command needs to create its own worktrees for parallel work units, but git worktrees-of-worktrees are problematic. The user needs a way to launch Claude Code into a **full git clone** instead of a worktree, so `/batch` can freely manage its own worktrees.

## Solution

Add a "Full clone (batch-friendly)" checkbox to the Quick Claude dialog. When checked, the launch sequence performs `git clone` instead of `git worktree add`, producing a complete `.git` directory that `/batch` can work with. The clone lives in the same `%APPDATA%/com.godly.terminal/worktrees/` directory and follows the same lifecycle (tracked for cleanup on terminal close).

## Design

### 1. Isolation Mode Enum

Replace the boolean `use_worktree` parameter with a three-state enum:

```rust
// In quick_claude.rs
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

### 2. New Launch Step: `CreateClone`

```rust
LaunchStep::CreateClone {
    agent_index: usize,
    repo_folder: String,
}
```

Mirrors `CreateWorktree` semantics. Returns `StepResult::WorktreeCreated { worktree_path }` — downstream handling (override CWD, update session record) is identical.

### 3. `git_worktree.rs` — New Functions

```rust
/// Get the remote URL for a repository (defaults to "origin").
pub fn get_remote_url(repo_root: &str) -> Result<String, String>;

/// Get the default branch name from the remote.
pub fn get_default_branch(repo_root: &str) -> Result<String, String>;

/// Clone a repository from its origin remote.
///
/// Creates the clone at `%APPDATA%/com.godly.terminal/worktrees/<dir_name>/`.
/// Returns the absolute path to the clone directory.
pub fn create_clone(repo_folder: &str, dir_name: &str) -> Result<String, String>;
```

Implementation:
1. `find_repo_root(repo_folder)` to get the canonical root.
2. `get_remote_url(root)` → `git -C <root> remote get-url origin`. If no remote exists, return `Err` — the `CreateClone` step falls back gracefully (returns `StepResult::Ok`, logs warning), same pattern as `CreateWorktree`.
3. `get_default_branch(root)` → `git -C <root> symbolic-ref refs/remotes/origin/HEAD` → parse `refs/remotes/origin/<branch>` → extract `<branch>`. Fallback chain: try `main`, then `master`. Avoid `git remote show origin` (makes a network call, can hang).
4. `git clone <url> --branch <default_branch> <worktrees_base_dir/dir_name>`. No `--single-branch` — `/batch` may need to reference other branches when creating worktrees.

**Error handling:** Clone uses whatever authentication is already configured for the remote (SSH agent, credential helper, etc.). If `git clone` fails for any reason (auth, network, no remote), the `CreateClone` step falls back gracefully just like `CreateWorktree` does — returns `StepResult::Ok` and logs a warning. The terminal launches in the original workspace folder instead.

### 4. Data Model Changes

**`QuickClaudePreferences`** — add field:
```rust
#[serde(default)]
pub batch_clone_mode: bool,  // default: false
```

**`QuickClaudeDialogState`** — add field:
```rust
pub batch_clone_mode: bool,
```

**`QuickClaudeSessionRecord`** — add field:
```rust
#[serde(default)]
pub is_clone: bool,  // default: false — distinguishes clone from worktree for cleanup
```

### 5. UI Changes

Add a third checkbox in the Quick Claude dialog checkbox row:

```
☐ Open in main branch (no worktree)    ☐ Auto-suggest branch name    ☐ Full clone (batch-friendly)
```

**Mutual exclusion:** When "Full clone" is checked:
- "Open in main branch" is force-unchecked and visually grayed out (clone always gets the default branch, making main_branch_mode redundant).
- "Auto-suggest branch name" is force-unchecked and grayed out (no branch to suggest).

When "Open in main branch" is checked:
- "Full clone" is force-unchecked (they serve different purposes).

### 6. `default_launch_steps` Signature Change

```rust
pub fn default_launch_steps(
    num_agents: usize,
    prompt: &str,
    model: &str,
    mode: &str,
    cwd: Option<&str>,
    image_paths: &[String],
    isolation: IsolationMode,  // was: use_worktree: bool
    claude_session_id: Option<&str>,
) -> Vec<LaunchStep>
```

Step insertion logic:
- `IsolationMode::None` → no extra step (current `main_branch_mode` behavior).
- `IsolationMode::Worktree` → insert `CreateWorktree` step (current worktree behavior).
- `IsolationMode::Clone` → insert `CreateClone` step.

### 7. app.rs Handler Changes

**Launch path** (around line 2635):
```rust
let isolation = if dlg.batch_clone_mode {
    IsolationMode::Clone
} else if dlg.main_branch_mode {
    IsolationMode::None
} else {
    IsolationMode::Worktree
};
```

**Step execution** (`execute_step`):
Add a `LaunchStep::CreateClone` arm that calls `git_worktree::create_clone()` and returns `StepResult::WorktreeCreated { worktree_path }`.

**Step result handling** (`handle_launch_step_result`):
The existing `WorktreeCreated` handler at line ~7496 extracts the agent_index by matching the current step against `LaunchStep::CreateWorktree`. This match must be extended to also match `LaunchStep::CreateClone { agent_index, .. }`, otherwise the `pending_worktree_path` will never be set for clone steps. Simplest fix: extract agent_index from either variant in the same match arm.

**Session record**: Set `is_clone: dlg.batch_clone_mode` when creating the record.

### 8. Cleanup

The existing cleanup path in `app.rs` (`WorktreeCloseConfirmed` at ~line 3406) calls `git_worktree::remove_worktree()`, which tries `git worktree remove --force` first, then falls back to `rm -rf` + `git worktree prune`. For clones, `git worktree remove` will fail (the clone is not a registered worktree), triggering the `rm -rf` fallback — so **the existing cleanup accidentally works** for clones without code changes.

However, to make this explicit and avoid the misleading confirm dialog text:

**`TerminalState`** (`terminal_state.rs`) — add field:
```rust
#[serde(default)]
pub is_clone: bool,
```

**`PersistedAppState`** (`session_persistence.rs`) — change `terminal_worktree_paths: HashMap<String, String>` to include clone flag, or add a parallel `terminal_clone_paths: HashSet<String>`.

Simpler approach: keep the existing `terminal_worktree_paths` as-is (it stores the path), add `terminal_clone_ids: HashSet<String>` to mark which terminals are clones. On cleanup:
- If terminal_id is in `terminal_clone_ids`: just `rm -rf` (skip git worktree remove).
- Otherwise: use existing `remove_worktree()`.

**Confirm dialog** (`confirm_dialog.rs`, `view_worktree_close_confirm`): check `is_clone` and adjust wording — "Remove Clone" / "Keep Clone" instead of "Remove Worktree" / "Keep Worktree".

### 9. Message Variants

Add to the `Message` enum:
```rust
QuickClaudeDialogBatchCloneToggled(bool),
```

Handle in `update()`: toggle `batch_clone_mode`, force-uncheck conflicting options.

## Files Modified

| File | Change |
|------|--------|
| `quick_claude.rs` | Add `IsolationMode` enum, `CreateClone` step, update `default_launch_steps` signature |
| `quick_claude_dialog.rs` | Add `batch_clone_mode` to preferences/state, render checkbox, mutual exclusion logic |
| `quick_claude_sessions.rs` | Add `is_clone: bool` field to `QuickClaudeSessionRecord` |
| `git_worktree.rs` | Add `get_remote_url()`, `get_default_branch()`, `create_clone()` functions |
| `app.rs` | Add `QuickClaudeDialogBatchCloneToggled` message, compute `IsolationMode`, handle `CreateClone` step, extend `handle_launch_step_result` match for `CreateClone`, pass `is_clone` to session record and terminal state |
| `terminal_state.rs` | Add `is_clone: bool` field |
| `session_persistence.rs` | Add `terminal_clone_ids: HashSet<String>` for persistence across restarts |
| `confirm_dialog.rs` | Adjust "Remove Worktree"/"Keep Worktree" text when `is_clone` is true |

## Testing

- Unit tests for `create_clone()`, `get_remote_url()`, `get_default_branch()` in `git_worktree.rs`.
- Unit tests for `IsolationMode::Clone` step insertion in `default_launch_steps`.
- **Update existing tests**: 12+ tests in `quick_claude.rs` that pass `use_worktree: bool` must be updated to use `IsolationMode`.
- Unit test for mutual exclusion logic (batch_clone_mode ↔ main_branch_mode).
- Unit test for `is_clone` field serde backward compatibility (old records without `is_clone` deserialize with `false`).
- E2E: launch Quick Claude with clone mode → verify `/batch` can create worktrees inside the clone.
