# Quick Claude Resume Worktree Tracking

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix Quick Claude resume to use correct CLI syntax, track worktree CWDs in session records, and switch the resume tab to use session records as the data source so worktree conversations are discoverable and resume in the correct directory.

**Architecture:** Four changes: (1) fix `--resume` command syntax, (2) add `cwd` field to `QuickClaudeSessionRecord` and persist it at launch time with a pre-assigned `claude_session_id`, (3) replace JSONL-based `discover_sessions()` with session-record-based loading, (4) use stored CWD and workspace for resume.

**Tech Stack:** Rust (iced-shell crate), serde JSON persistence

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src-tauri/native/iced-shell/src/quick_claude.rs` | Modify | Fix resume command, add `session_record_id` to LaunchState, accept `claude_session_id` in `default_launch_steps` |
| `src-tauri/native/iced-shell/src/quick_claude_sessions.rs` | Modify | Add `cwd` field to session record |
| `src-tauri/native/iced-shell/src/quick_claude_dialog.rs` | Modify | Add `cwd`/`workspace_id` to `ClaudeSession`, replace `discover_sessions()`, remove dead code |
| `src-tauri/native/iced-shell/src/app.rs` | Modify | Wire up session recording at launch, use stored CWD for resume |

---

### Task 1: Fix resume command syntax

**Files:**
- Modify: `src-tauri/native/iced-shell/src/quick_claude.rs:55-74`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn resume_steps_uses_correct_cli_syntax() {
    let steps = resume_launch_steps("abc-123-def", Some("/test/dir"));
    match &steps[2] {
        LaunchStep::RunCommand { command, .. } => {
            // Must use `claude --resume <id>`, NOT `claude --resume --session-id <id>`
            assert_eq!(command, "claude --resume abc-123-def");
            assert!(!command.contains("--session-id"), "should not use --session-id flag");
        }
        _ => panic!("Expected RunCommand"),
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p iced-shell resume_steps_uses_correct_cli_syntax`
Expected: FAIL — current command is `"claude --resume --session-id abc-123-def"`

- [ ] **Step 3: Fix the command format**

In `src-tauri/native/iced-shell/src/quick_claude.rs:67`, change:
```rust
// OLD:
command: format!("claude --resume --session-id {}", session_id),
// NEW:
command: format!("claude --resume {}", session_id),
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p iced-shell resume_steps_uses_correct_cli_syntax`
Expected: PASS

- [ ] **Step 5: Commit**

```
fix: use correct `claude --resume <id>` CLI syntax
```

---

### Task 2: Add `cwd` field to `QuickClaudeSessionRecord` and `session_record_id` to `LaunchState`

**Files:**
- Modify: `src-tauri/native/iced-shell/src/quick_claude_sessions.rs:14-25, 134-146`
- Modify: `src-tauri/native/iced-shell/src/quick_claude.rs:148-180` (LaunchState)

- [ ] **Step 1: Write failing tests**

In `quick_claude_sessions.rs` tests:
```rust
#[test]
fn test_session_record_cwd_roundtrip() {
    let mut record = make_record("s-cwd", "t-cwd", SessionStatus::Running);
    record.cwd = Some("/worktree/path".to_string());
    let json = serde_json::to_string(&record).expect("serialize");
    let decoded: QuickClaudeSessionRecord = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.cwd, Some("/worktree/path".to_string()));
}

#[test]
fn test_old_records_without_cwd_deserialize() {
    let json = r#"{"id":"s-1","prompt":"test","terminal_id":"t-1","workspace_id":"ws-1","branch":"main","model":"opus","mode":"code","status":"Running","launched_at":"2026-03-20T12:00:00Z","claude_session_id":null}"#;
    let decoded: QuickClaudeSessionRecord = serde_json::from_str(json).expect("old records should deserialize");
    assert_eq!(decoded.cwd, None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p iced-shell test_session_record_cwd`
Expected: FAIL — `cwd` field doesn't exist yet

- [ ] **Step 3: Add `cwd` field to struct and test helper**

In `QuickClaudeSessionRecord` (line 24), add after `claude_session_id`:
```rust
#[serde(default)]
pub cwd: Option<String>,
```

In `make_record()` test helper (line 134), add `cwd: None`.

- [ ] **Step 4: Add `session_record_id` to `LaunchState`**

In `src-tauri/native/iced-shell/src/quick_claude.rs`, add to `LaunchState` struct (after `pending_worktree_path`):
```rust
/// ID of the QuickClaudeSessionRecord, for updating CWD/terminal_id after async steps.
pub session_record_id: Option<String>,
```

Initialize it to `None` in `LaunchState::new()`.

- [ ] **Step 5: Run tests**

Run: `cargo test -p iced-shell test_session_record_cwd && cargo check -p iced-shell`
Expected: PASS

- [ ] **Step 6: Commit**

```
feat: add cwd field to QuickClaudeSessionRecord

Stores the working directory (worktree path or workspace folder) at
launch time. Uses serde(default) for backwards compatibility.
Also adds session_record_id to LaunchState for post-launch updates.
```

---

### Task 3: Pre-assign Claude session ID at launch and record session

**Files:**
- Modify: `src-tauri/native/iced-shell/src/quick_claude.rs:82-144` (default_launch_steps)
- Modify: `src-tauri/native/iced-shell/src/app.rs:2560-2646` (launch handler)

**Key insight:** To make resume work, we need a known Claude Code session ID. The cleanest way is to pre-generate a UUID and pass `--session-id <uuid>` when launching Claude Code. Then we store that UUID in the session record's `claude_session_id`.

- [ ] **Step 1: Add `claude_session_id` parameter to `default_launch_steps`**

Change the function signature to accept an optional session ID:
```rust
pub fn default_launch_steps(
    num_agents: usize,
    prompt: &str,
    model: &str,
    mode: &str,
    cwd: Option<&str>,
    image_paths: &[String],
    use_worktree: bool,
    claude_session_id: Option<&str>,  // NEW
) -> Vec<LaunchStep> {
```

After building the `cmd` string and before the `for` loop (around line 118), append:
```rust
if let Some(sid) = claude_session_id {
    cmd.push_str(&format!(" --session-id {}", sid));
}
```

- [ ] **Step 2: Update call site in `app.rs` to generate and pass session ID**

In the `QuickClaudeDialogLaunch` handler (app.rs around line 2630):
```rust
let claude_session_id = uuid::Uuid::new_v4().to_string();
let steps = crate::quick_claude::default_launch_steps(
    num_agents, prompt, &model, &mode, cwd.as_deref(), &image_paths, use_worktree,
    Some(&claude_session_id),  // NEW
);
```

After creating the launch state, record the session:
```rust
let record_id = uuid::Uuid::new_v4().to_string();
let session_record = crate::quick_claude_sessions::QuickClaudeSessionRecord {
    id: record_id.clone(),
    prompt: prompt.to_string(),
    terminal_id: String::new(), // filled after TerminalCreated
    workspace_id: ws_id.clone(),
    branch: String::new(),
    model: model.clone(),
    mode: mode.clone(),
    status: crate::quick_claude_sessions::SessionStatus::Running,
    launched_at: crate::quick_claude_sessions::now_iso8601(),
    claude_session_id: Some(claude_session_id),
    cwd: cwd.clone(),
};
let _ = crate::quick_claude_sessions::add_session(session_record);
launch_state.session_record_id = Some(record_id);
```

- [ ] **Step 3: Update existing tests that call `default_launch_steps`**

Add `None` as the last argument to all existing test calls of `default_launch_steps`.

- [ ] **Step 4: Write test for session ID in command**

```rust
#[test]
fn default_steps_includes_session_id_when_provided() {
    let steps = default_launch_steps(1, "test", "sonnet", "default", None, &[], false, Some("my-uuid-123"));
    match &steps[2] {
        LaunchStep::RunCommand { command, .. } => {
            assert!(command.contains("--session-id my-uuid-123"));
        }
        _ => panic!("Expected RunCommand"),
    }
}

#[test]
fn default_steps_no_session_id_when_none() {
    let steps = default_launch_steps(1, "test", "sonnet", "default", None, &[], false, None);
    match &steps[2] {
        LaunchStep::RunCommand { command, .. } => {
            assert!(!command.contains("--session-id"));
        }
        _ => panic!("Expected RunCommand"),
    }
}
```

- [ ] **Step 5: Build and test**

Run: `cargo test -p iced-shell && cargo check -p iced-shell`
Expected: PASS

- [ ] **Step 6: Commit**

```
feat: pre-assign Claude session ID at launch and record session

Generate a UUID for --session-id when launching Claude Code so the
session ID is known upfront and stored in QuickClaudeSessionRecord.
This enables reliable resume by session ID.
```

---

### Task 4: Update session record after async steps (worktree, terminal creation)

**Files:**
- Modify: `src-tauri/native/iced-shell/src/app.rs:7454-7509` (handle_launch_step_result)

- [ ] **Step 1: Update CWD when worktree is created**

In `handle_launch_step_result` (app.rs around line 7465), after `launch.pending_worktree_path = Some(worktree_path.clone())`, add:

```rust
if let Some(ref record_id) = launch.session_record_id {
    let rid = record_id.clone();
    let wt = worktree_path.clone();
    // Fire-and-forget: update session record CWD
    let mut sessions = crate::quick_claude_sessions::load_sessions();
    if let Some(rec) = sessions.iter_mut().find(|s| s.id == rid) {
        rec.cwd = Some(wt);
    }
    let _ = crate::quick_claude_sessions::save_sessions(&sessions);
}
```

- [ ] **Step 2: Update terminal_id when terminal is created**

In the `TerminalCreated` handler (around line 7489-7509), after `launch.agent_terminal_ids[agent_index] = Some(session_id.clone())`, add:

```rust
if let Some(ref record_id) = launch.session_record_id {
    let rid = record_id.clone();
    let tid = session_id.clone();
    let mut sessions = crate::quick_claude_sessions::load_sessions();
    if let Some(rec) = sessions.iter_mut().find(|s| s.id == rid) {
        rec.terminal_id = tid;
    }
    let _ = crate::quick_claude_sessions::save_sessions(&sessions);
}
```

- [ ] **Step 3: Build check**

Run: `cargo check -p iced-shell`
Expected: PASS

- [ ] **Step 4: Commit**

```
feat: update session record after worktree and terminal creation

When a worktree is created asynchronously, update the session record's
CWD to the worktree path. When a terminal is created, store its ID
in the record for stale-session cleanup.
```

---

### Task 5: Switch resume tab to session records and add fields to `ClaudeSession`

**Files:**
- Modify: `src-tauri/native/iced-shell/src/quick_claude_dialog.rs:25-32, 1250-1307, 1309-1368, 1370-1385, 1583, 1699-1743`
- Modify: `src-tauri/native/iced-shell/src/app.rs:2435-2439`

- [ ] **Step 1: Add `cwd` and `workspace_id` to `ClaudeSession`**

In `ClaudeSession` struct (line 25):
```rust
pub struct ClaudeSession {
    pub session_id: String,
    pub first_message: String,
    pub model: String,
    pub timestamp: String,
    pub branch: String,
    pub file_path: String,
    pub cwd: Option<String>,       // NEW
    pub workspace_id: String,      // NEW
}
```

Update test fixture at line 1583 to include `cwd: None, workspace_id: String::new()`.

- [ ] **Step 2: Replace `discover_sessions()` body**

Replace the entire function body. Change signature to take no arguments since session records are global:

```rust
/// Load recent Quick Claude sessions from the session record file.
pub fn discover_sessions() -> Vec<ClaudeSession> {
    let records = crate::quick_claude_sessions::load_sessions();

    records
        .into_iter()
        .rev() // most recent first
        .filter_map(|r| {
            // Only include sessions with a known Claude session ID
            let claude_sid = r.claude_session_id?;
            Some(ClaudeSession {
                session_id: claude_sid,
                first_message: r.prompt,
                model: r.model,
                timestamp: format_relative_time_from_iso(&r.launched_at),
                branch: r.branch,
                file_path: String::new(),
                cwd: r.cwd,
                workspace_id: r.workspace_id,
            })
        })
        .take(20)
        .collect()
}
```

- [ ] **Step 3: Add `format_relative_time_from_iso` helper**

Convert ISO 8601 timestamps to relative time strings. Reuse the same formatting logic as `format_relative_time` but parse from ISO string:

```rust
fn format_relative_time_from_iso(iso: &str) -> String {
    // Parse "YYYY-MM-DDTHH:MM:SSZ" to epoch seconds
    if iso.len() < 19 { return iso.to_string(); }
    let year: u64 = iso[0..4].parse().unwrap_or(0);
    let month: u64 = iso[5..7].parse().unwrap_or(0);
    let day: u64 = iso[8..10].parse().unwrap_or(0);
    let hour: u64 = iso[11..13].parse().unwrap_or(0);
    let min: u64 = iso[14..16].parse().unwrap_or(0);
    let sec: u64 = iso[17..19].parse().unwrap_or(0);
    if year == 0 || month == 0 || day == 0 { return iso.to_string(); }

    // Inverse of days_to_ymd: convert to days since epoch
    let a = if month <= 2 { 1u64 } else { 0 };
    let y = year - a;
    let m = month + 12 * a - 3;
    let days = y * 365 + y / 4 - y / 100 + y / 400
        + (153 * m + 2) / 5 + day - 1 - 719468;
    let total_secs = days * 86400 + hour * 3600 + min * 60 + sec;

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let elapsed = now_secs.saturating_sub(total_secs);
    format_elapsed_secs(elapsed)
}

fn format_elapsed_secs(secs: u64) -> String {
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else if secs < 604800 {
        format!("{}d ago", secs / 86400)
    } else {
        format!("{}w ago", secs / 604800)
    }
}
```

Refactor existing `format_relative_time` to use `format_elapsed_secs`:
```rust
fn format_relative_time(time: std::time::SystemTime) -> String {
    let secs = time.elapsed().unwrap_or_default().as_secs();
    format_elapsed_secs(secs)
}
```

This way the existing tests for `format_relative_time` keep working.

- [ ] **Step 4: Update call site in `app.rs`**

Change the sessions discovery call (app.rs line 2435-2439) to remove the workspace folder argument:

```rust
let sessions_task = iced::Task::perform(
    async move {
        crate::quick_claude_dialog::discover_sessions()
    },
    Message::QuickClaudeDialogSessionsLoaded,
);
```

- [ ] **Step 5: Remove dead code**

Remove `parse_session_jsonl()` (lines 1309-1368) — no longer called.

- [ ] **Step 6: Update/rename tests**

- Rename `discover_sessions_no_workspace_returns_empty` to `discover_sessions_no_records_returns_empty` and remove the argument.
- Rename `discover_sessions_nonexistent_path_returns_empty` similarly or delete it (both test the same thing now).

- [ ] **Step 7: Build and test**

Run: `cargo test -p iced-shell && cargo check -p iced-shell`
Expected: PASS

- [ ] **Step 8: Commit**

```
feat: switch resume tab to QuickClaudeSessionRecord data source

Replace JSONL-based session discovery with session record loading.
Only sessions with a known claude_session_id appear in the resume
tab. Worktree conversations are now visible since session records
store the CWD regardless of project directory.
```

---

### Task 6: Use stored CWD and workspace when resuming

**Files:**
- Modify: `src-tauri/native/iced-shell/src/app.rs:2751-2812` (QuickClaudeDialogResume handler)

- [ ] **Step 1: Use session's stored CWD instead of selected workspace**

Replace the CWD resolution block (lines 2768-2774):

```rust
// Use session's stored CWD if it still exists on disk,
// falling back to selected workspace folder.
let cwd = session.cwd.clone()
    .filter(|p| std::path::Path::new(p).exists())
    .or_else(|| {
        dlg.selected_workspace_id
            .as_ref()
            .and_then(|id| self.workspaces.get(id))
            .map(|ws| ws.folder_path.clone())
            .filter(|p| !p.is_empty())
    });
```

- [ ] **Step 2: Open in original workspace instead of creating new one**

Replace the workspace creation block (lines 2781-2798). Reuse the session's original workspace if it still exists:

```rust
let placeholder_id = uuid::Uuid::new_v4().to_string();
let rows = self.calculate_rows();
let cols = self.calculate_cols();

let (ws_id, is_new_ws) = if self.workspaces.get(&session.workspace_id).is_some() {
    (session.workspace_id.clone(), false)
} else {
    let id = uuid::Uuid::new_v4().to_string();
    let snippet: String = session.first_message.chars().take(30).collect();
    let ws_name = if snippet.is_empty() {
        "Quick Claude (Resume)".to_string()
    } else {
        format!("QC: {}", snippet.trim())
    };
    self.workspaces.add(id.clone(), ws_name, placeholder_id.clone());
    self.next_workspace_num += 1;
    (id, true)
};

self.terminals.add_to_workspace(
    placeholder_id.clone(),
    rows,
    cols,
    ws_id.clone(),
);
```

Update the `LaunchState` creation to use the resolved `ws_id` and remove the hardcoded workspace name.

- [ ] **Step 3: Build and test**

Run: `cargo check -p iced-shell`
Expected: PASS

- [ ] **Step 4: Commit**

```
fix: resume Quick Claude in original workspace with stored CWD

Instead of creating a new workspace, reuse the original. Use the
stored CWD from the session record (worktree path) with fallback
to workspace folder if the path no longer exists on disk.
```

---

### Task 7: Wire up stale session cleanup

**Files:**
- Modify: `src-tauri/native/iced-shell/src/app.rs` (QuickClaudeDialogOpen handler, around line 2435)

- [ ] **Step 1: Call `cleanup_stale_sessions` when dialog opens**

Before the `discover_sessions` call, run cleanup with current live terminal IDs:

```rust
let live_ids: Vec<String> = self.terminals.all_ids().map(|s| s.to_string()).collect();
let _ = crate::quick_claude_sessions::cleanup_stale_sessions(&live_ids);
```

- [ ] **Step 2: Build and test**

Run: `cargo check -p iced-shell`
Expected: PASS

- [ ] **Step 3: Commit**

```
fix: wire up stale session cleanup on Quick Claude dialog open

Marks sessions with dead terminal IDs as Completed and trims to
50 entries when the dialog opens.
```
