# Bell Burst Suppression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Suppress rapid-fire bell notifications from the same terminal (e.g., during Claude Code `/batch`) so only the first bell and a "settled" notification after quiet are heard.

**Architecture:** Extend the existing pure-function debounce system in `notifications.rs` with burst detection. Track per-terminal `last_bell_ms` and `bell_burst_suppressed` count in `app.rs`. On each `ToastTick`, check if any burst has gone quiet and fire a single "settled" notification.

**Tech Stack:** Rust, iced framework (existing notification infrastructure)

---

### Task 1: Add burst detection pure functions to `notifications.rs`

**Files:**
- Modify: `src-tauri/native/iced-shell/src/notifications.rs:1-101`

- [ ] **Step 1: Write failing tests for `is_burst_active`**

Add these tests after the existing test module (line 104+):

```rust
#[test]
fn test_burst_not_active_when_no_prior_sound() {
    assert!(!is_burst_active(5_000, None));
}

#[test]
fn test_burst_active_when_sound_within_window() {
    assert!(is_burst_active(20_000, Some(5_000))); // 15s ago, within 30s
}

#[test]
fn test_burst_not_active_when_sound_outside_window() {
    assert!(!is_burst_active(35_000, Some(0))); // 35s ago, outside 30s
}

#[test]
fn test_burst_active_at_boundary() {
    // At exactly 29_999ms after last sound, still in burst
    assert!(is_burst_active(BURST_WINDOW_MS - 1, Some(0)));
}

#[test]
fn test_burst_not_active_at_boundary() {
    // At exactly BURST_WINDOW_MS, no longer in burst
    assert!(!is_burst_active(BURST_WINDOW_MS, Some(0)));
}

#[test]
fn test_burst_active_clock_rollback() {
    // now < last_sound → saturating_sub=0 < BURST_WINDOW_MS → burst active (safe: suppresses)
    assert!(is_burst_active(500, Some(1_000)));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p iced-shell --lib notifications::tests::test_burst`
Expected: compile errors — `is_burst_active` and `BURST_WINDOW_MS` don't exist yet.

- [ ] **Step 3: Add constants and `is_burst_active` function**

Add after line 8 (`WINDOW_ATTENTION_DEBOUNCE_MS`):

```rust
/// Burst detection window: if the last played sound was within this window,
/// subsequent bells are suppressed (burst mode).
pub const BURST_WINDOW_MS: u64 = 30_000;

/// Quiet detection: after this many ms with no new bells, a burst is
/// considered "settled" and one final notification fires.
pub const BURST_QUIET_MS: u64 = 10_000;
```

Add after `bell_attention_is_critical` (line 94), before `is_within_debounce_window`:

```rust
/// Pure helper: returns true when a terminal is in bell-burst mode.
///
/// A burst is active when the last sound that actually played for this
/// terminal was within [`BURST_WINDOW_MS`].
pub fn is_burst_active(now_ms: u64, last_sound_played_ms: Option<u64>) -> bool {
    is_within_debounce_window(last_sound_played_ms, now_ms, BURST_WINDOW_MS)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p iced-shell --lib notifications::tests::test_burst`
Expected: all 5 new tests PASS.

- [ ] **Step 5: Write failing tests for `is_burst_quiet`**

```rust
#[test]
fn test_burst_quiet_when_no_bells_suppressed() {
    // suppressed_count=0 → never quiet (nothing to settle)
    assert!(!is_burst_quiet(20_000, Some(5_000), 0));
}

#[test]
fn test_burst_quiet_when_bells_suppressed_and_enough_silence() {
    // Last bell was 15s ago, 3 suppressed → quiet
    assert!(is_burst_quiet(20_000, Some(5_000), 3));
}

#[test]
fn test_burst_not_quiet_when_recent_bell() {
    // Last bell was 5s ago, 3 suppressed → not quiet yet
    assert!(!is_burst_quiet(10_000, Some(5_000), 3));
}

#[test]
fn test_burst_quiet_at_boundary() {
    // Exactly BURST_QUIET_MS after last bell → quiet
    assert!(is_burst_quiet(BURST_QUIET_MS, Some(0), 1));
}

#[test]
fn test_burst_not_quiet_just_before_boundary() {
    assert!(!is_burst_quiet(BURST_QUIET_MS - 1, Some(0), 1));
}

#[test]
fn test_burst_quiet_no_last_bell() {
    // No last_bell_ms recorded → not quiet
    assert!(!is_burst_quiet(20_000, None, 5));
}

#[test]
fn test_burst_quiet_clock_rollback() {
    // now < last_bell → saturating_sub=0 < BURST_QUIET_MS → not quiet (safe: no premature settle)
    assert!(!is_burst_quiet(500, Some(1_000), 5));
}
```

- [ ] **Step 6: Run tests to verify they fail**

Run: `cargo test -p iced-shell --lib notifications::tests::test_burst_quiet`
Expected: compile error — `is_burst_quiet` doesn't exist.

- [ ] **Step 7: Add `is_burst_quiet` function**

Add right after `is_burst_active`:

```rust
/// Pure helper: returns true when a bell burst has gone quiet.
///
/// A burst is "quiet" when at least one bell was suppressed and no new
/// bell has arrived for [`BURST_QUIET_MS`].
pub fn is_burst_quiet(now_ms: u64, last_bell_ms: Option<u64>, suppressed_count: u32) -> bool {
    if suppressed_count == 0 {
        return false;
    }
    match last_bell_ms {
        Some(last) => now_ms.saturating_sub(last) >= BURST_QUIET_MS,
        None => false,
    }
}
```

- [ ] **Step 8: Run all notification tests**

Run: `cargo test -p iced-shell --lib notifications::tests`
Expected: all tests PASS (existing + 13 new).

- [ ] **Step 9: Commit**

```
git add src-tauri/native/iced-shell/src/notifications.rs
git commit -m "feat: add bell burst detection pure functions"
```

---

### Task 2: Add burst state fields to `App` and wire up Bell handler

**Files:**
- Modify: `src-tauri/native/iced-shell/src/app.rs`

- [ ] **Step 1: Add burst state fields to `App` struct**

After `last_attention_request_ms` (line 354), add:

```rust
    /// Most recent bell timestamp per terminal (regardless of whether sound played).
    last_bell_ms: HashMap<String, u64>,
    /// Count of bells suppressed during current burst per terminal.
    bell_burst_suppressed: HashMap<String, u32>,
```

- [ ] **Step 2: Initialize fields in `App::new()`**

After `last_attention_request_ms: None,` (around line 527), add:

```rust
            last_bell_ms: HashMap::new(),
            bell_burst_suppressed: HashMap::new(),
```

- [ ] **Step 3: Clear fields in `clear_runtime_state()`**

After `self.last_attention_request_ms = None;` (around line 1354), add:

```rust
        self.last_bell_ms.clear();
        self.bell_burst_suppressed.clear();
```

- [ ] **Step 4: Clean up fields in terminal close paths**

In `DeleteWorkspaceDecision::Delete` (around line 6232), after `self.last_terminal_sound_ms.remove(&terminal_id);` add:

```rust
                    self.last_bell_ms.remove(&terminal_id);
                    self.bell_burst_suppressed.remove(&terminal_id);
```

In `close_terminal_immediate` (around line 7208), after `self.last_terminal_sound_ms.remove(session_id);` add:

```rust
        self.last_bell_ms.remove(session_id);
        self.bell_burst_suppressed.remove(session_id);
```

- [ ] **Step 5: Modify Bell handler to check burst state**

Replace the Bell handler (lines 1518-1527):

```rust
            Message::DaemonEvent(DaemonEventMsg::Bell { session_id }) => {
                self.notifications.record_bell(&session_id);
                let now_ms = Self::now_ms();
                self.last_bell_ms.insert(session_id.clone(), now_ms);

                let last_sound_ms = self.last_terminal_sound_ms.get(&session_id).copied();
                let burst_active = notifications::is_burst_active(now_ms, last_sound_ms);

                if burst_active {
                    *self.bell_burst_suppressed.entry(session_id.clone()).or_insert(0) += 1;
                    log::debug!(
                        "Bell from session {} (burst-suppressed, {} total)",
                        session_id,
                        self.bell_burst_suppressed.get(&session_id).unwrap_or(&0)
                    );
                } else {
                    self.bell_burst_suppressed.remove(&session_id);
                    let is_focused = self.active_focused() == Some(session_id.as_str());
                    if !is_focused {
                        self.enqueue_bell_toast(&session_id);
                    }
                    self.play_notification_sound_if_allowed(&session_id);
                    log::debug!("Bell from session {}", session_id);
                    return self.request_window_attention_if_allowed();
                }
            }
```

- [ ] **Step 6: Run `cargo check -p iced-shell`**

Expected: compiles cleanly.

- [ ] **Step 7: Commit**

```
git add src-tauri/native/iced-shell/src/app.rs
git commit -m "feat: add burst state fields and suppress bells during burst"
```

---

### Task 3: Add "settled" notification on burst quiet

**Files:**
- Modify: `src-tauri/native/iced-shell/src/app.rs`

- [ ] **Step 1: Add `check_burst_quiet` method**

Add after `request_window_attention_if_allowed` (around line 1314):

```rust
    /// Scan for terminals whose bell burst has gone quiet and fire a
    /// single "settled" notification for each.  Returns a combined Task
    /// so that window-attention requests (if any) are dispatched.
    fn check_burst_quiet(&mut self, now_ms: u64) -> Task<Message> {
        // Extract last_bell_ms ref before iterating bell_burst_suppressed
        // to avoid borrowing self twice in the closure.
        let last_bell_ms = &self.last_bell_ms;
        let quiet_terminals: Vec<(String, u32)> = self
            .bell_burst_suppressed
            .iter()
            .filter(|(tid, &count)| {
                notifications::is_burst_quiet(
                    now_ms,
                    last_bell_ms.get(*tid).copied(),
                    count,
                )
            })
            .map(|(tid, &count)| (tid.clone(), count))
            .collect();

        let mut tasks = Vec::new();
        for (terminal_id, count) in quiet_terminals {
            self.bell_burst_suppressed.remove(&terminal_id);
            self.last_bell_ms.remove(&terminal_id);

            let is_focused = self.active_focused() == Some(terminal_id.as_str());
            if !is_focused {
                let title = "Activity Settled".to_string();
                let message = if let Some(term) = self.terminals.get(&terminal_id) {
                    let workspace = term
                        .workspace_id
                        .as_deref()
                        .and_then(|wid| self.workspaces.get(wid))
                        .map(|ws| ws.name.as_str())
                        .unwrap_or("Unknown workspace");
                    format!(
                        "{} in {} ({} bells suppressed)",
                        term.tab_label(),
                        workspace,
                        count
                    )
                } else {
                    format!("Terminal {} ({} bells suppressed)", terminal_id, count)
                };
                self.enqueue_toast_for_terminal(title, message, &terminal_id);
            }

            self.play_notification_sound_if_allowed(&terminal_id);
            tasks.push(self.request_window_attention_if_allowed());
        }

        Task::batch(tasks)
    }
```

- [ ] **Step 2: Wire into `ToastTick` handler**

Replace the ToastTick handler (line 3211-3214):

```rust
            Message::ToastTick => {
                let now_ms = Self::now_ms();
                let _ = prune_expired_toasts(self.toasts.as_mut(), now_ms);
                return self.check_burst_quiet(now_ms);
            }
```

- [ ] **Step 3: Run `cargo check -p iced-shell`**

Expected: compiles cleanly.

- [ ] **Step 4: Run all iced-shell tests**

Run: `cargo test -p iced-shell`
Expected: all tests PASS.

- [ ] **Step 5: Commit**

```
git add src-tauri/native/iced-shell/src/app.rs
git commit -m "feat: fire settled notification when bell burst goes quiet"
```

---

### Task 4: Add changelog fragment

**Files:**
- Create: `changelog/unreleased/bell-burst-suppression.md`

- [ ] **Step 1: Create changelog fragment**

```markdown
### Added

- **Bell burst suppression** — Rapid-fire bell notifications from the same terminal (e.g., during Claude Code `/batch`) are now suppressed after the first bell. A single "Activity Settled" notification fires once the burst goes quiet (10s of silence), preventing notification spam while still confirming when batch work completes.
```

- [ ] **Step 2: Commit**

```
git add changelog/unreleased/bell-burst-suppression.md
git commit -m "docs: add changelog fragment for bell burst suppression"
```
