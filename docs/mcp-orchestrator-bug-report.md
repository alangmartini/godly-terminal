# MCP Orchestrator Bug Report

**Date**: 2026-03-03
**Tester**: Claude Code (Opus 4.6) via godly-terminal MCP
**Test scenario**: Spawn 3 Claude Code agents via `quick_claude` to work on different lanes of the Iced+wgpu migration plan, monitoring each MCP command for correctness.

## Test Environment

- Godly Terminal (production build)
- godly-terminal MCP server (godly-mcp)
- Workspace: `godly-terminal` (ID: `46f65210-facf-4476-bcbc-dd86141a330d`)
- 3 agent terminals spawned via `quick_claude`

---

## Bug Summary

| # | Severity | Tool | Bug | Status |
|---|----------|------|-----|--------|
| 1 | ~~CRITICAL~~ | `quick_claude` | `workspace_id` parameter ignored — terminals created in wrong workspace | **By Design** (see analysis) |
| 2 | **CRITICAL** | `quick_claude` | Prompt never delivered to Claude Code session | Root-caused (depends on Bug #3) |
| 3 | **CRITICAL** | `write_to_terminal` | Always times out with "timed out waiting on channel" | **Root-caused** — fix in PR #534 |
| 4 | **CRITICAL** | `execute_command` | Always times out with "timed out waiting on channel" | **Root-caused** — fix in PR #534 |
| 5 | **HIGH** | `write_to_terminal` | Write IS delivered to terminal despite timeout error (false negative) | **Root-caused** — same as Bug #3 |
| 6 | **MEDIUM** | `create_terminal` | `command` parameter executes the command twice (duplicate) | Needs investigation |
| 7 | **MEDIUM** | `quick_claude` / `close_terminal` | Worktrees never cleaned up after terminal close (46 accumulated) | Fix in PR #534 |
| 8 | **HIGH** | `quick_claude` | Creates duplicate "Agent" workspace with wrong folder path | Confirmed |

---

## Detailed Findings

### Bug 1: `quick_claude` ignores `workspace_id` parameter — **BY DESIGN**

**Reproduction:**
```
quick_claude(workspace_id="46f65210-...", prompt="...", branch_name="test-mcp-agent-a-contract")
```

**Expected:** Terminal created in workspace `46f65210-...` (godly-terminal)
**Actual:** Terminal created in workspace `95085625-...` (Agent/typesense)

**Evidence:** All 3 `quick_claude` calls with `workspace_id=46f65210-...` resulted in terminals in `95085625-...`. Confirmed via `list_terminals` and `get_workspace_details`.

**Also:** `create_terminal(workspace_id="46f65210-...")` exhibited the same behavior — terminal appeared in `95085625-...`.

**Root cause (confirmed — by design):** This is intentional behavior per `src-tauri/src/mcp_server/handler.rs:12-54`. The `ensure_mcp_workspace` function routes ALL MCP-created terminals to a dedicated "Agent" workspace. This design prevents a WebView2 broadcast storm that caused crashes under heavy output (issue #204). The `workspace_id` parameter is only used for `folder_path` resolution (CWD and worktree operations), not for terminal display placement.

The handler comments explain (lines 217-219, 435-437):
> "MCP terminals are displayed in the Agent workspace (avoids WebView2 broadcast storm — issue #204), but we use the *original* workspace_id to resolve folder_path for CWD and worktree operations."

**Verdict:** Not a bug. However, the MCP tool descriptions for `create_terminal` and `quick_claude` should be clearer that `workspace_id` controls the git repo / CWD source, not the visual workspace where the terminal tab appears. All MCP terminals always appear in the "Agent" workspace tab.

---

### Bug 2: `quick_claude` prompt never delivered

**Reproduction:**
```
quick_claude(
  workspace_id="46f65210-...",
  prompt="You are Agent A working on Lane A: Contract Freeze...",
  branch_name="test-mcp-agent-a-contract"
)
```

**Expected:** Claude Code starts and receives the prompt text
**Actual:** Claude Code starts (v2.1.63, `--dangerously-skip-permissions` mode) but sits at empty `>` prompt. Prompt text never appears.

**Evidence:** `read_grid` on all 3 terminals shows Claude Code at empty `>` prompt. `wait_for_text` with 15s timeout found no trace of prompt content in any terminal.

**Impact:** `quick_claude` is fundamentally non-functional — it starts Claude Code but never sends the task. The "Fire multiple calls in rapid succession for quick idea capture" promise in the tool description is completely broken.

**Root cause hypothesis:** `quick_claude` likely depends on `write_to_terminal` to deliver the prompt after Claude Code is ready, which leads to Bug #3.

---

### Bug 3: `write_to_terminal` always times out

**Reproduction:**
```
write_to_terminal(terminal_id="ef8c9975-...", data="Hello\n")
write_to_terminal(terminal_id="64bcd6fa-...", data="hi\n")
write_to_terminal(terminal_id="5f05ed0f-...", data="echo test\n")  # own terminal
```

**Expected:** Text written, success returned
**Actual:** `Error: Failed to receive response: timed out waiting on channel`

**Evidence:** Tested on 4 different terminals including the caller's own. All returned the same timeout error.

**HOWEVER** (see Bug #5): The text IS actually written to the terminal. The write side works but the response/acknowledgment channel is broken.

**Impact:** Any tool depending on write acknowledgment (including `quick_claude` prompt delivery, `execute_command`) is broken.

**Root cause (confirmed):** The error "Failed to receive response: timed out waiting on channel" originates from `src-tauri/src/daemon_client/client.rs:609` — `recv_timeout(Duration::from_secs(15))`. The MCP handler's `WriteToTerminal` at `handler.rs:1100` uses `daemon.send_request()` which goes through the bridge I/O thread. Under load with multiple terminals producing output, the bridge is congested and the 15-second timeout is hit.

The Tauri command handler for the same operation (`src-tauri/src/commands/terminal.rs:196-215`) correctly uses `daemon.send_fire_and_forget(&request)` instead of `daemon.send_request()`, with the comment: "Fire-and-forget: don't block the Tauri thread pool waiting for the daemon's Ok response. Blocking here caused ~2s input lag under rapid keystrokes."

**Fix:** Change the MCP handler to use `send_fire_and_forget()` for `WriteToTerminal`, matching the Tauri command handler behavior. This also fixes Bugs #2, #4, and #5.

---

### Bug 4: `execute_command` always times out

**Reproduction:**
```
execute_command(terminal_id="ef8c9975-...", command="Hello?", timeout_ms=15000)
execute_command(terminal_id="5f05ed0f-...", command="echo test", timeout_ms=10000)
```

**Expected:** Command executed, output returned
**Actual:** `Error: Failed to receive response: timed out waiting on channel`

**Impact:** Cannot programmatically run commands in terminals and read output. This is the primary tool for agent orchestration.

---

### Bug 5: Writes arrive despite timeout error (false negative)

**Reproduction:**
After Bug #3/#4 errors on own terminal, `read_grid` showed:
```
> echo MCP_WRITE_TEST_OK
  echo MCP_WRITE_TEST_OK
  echo MCP_EXEC_TEST
  echo MCP_EXEC_TEST
```

**Expected:** Either (a) write succeeds and returns success, or (b) write fails and text doesn't appear
**Actual:** Write fails with timeout error BUT text appears in the terminal prompt

**Impact:** False error reporting. The MCP server tells the caller the operation failed when it actually succeeded. This also means each failed write is actually polluting the terminal with unintended input.

---

### Bug 6: `create_terminal` command runs twice

**Reproduction:**
```
create_terminal(workspace_id="46f65210-...", command="echo MCP_CREATE_TEST_OK")
```

**Expected:** `echo MCP_CREATE_TEST_OK` runs once
**Actual:** `read_grid` shows the command and its output appeared TWICE:
```
PS ...> echo MCP_CREATE_TEST_OK
MCP_CREATE_TEST_OK
PS ...> echo MCP_CREATE_TEST_OK
MCP_CREATE_TEST_OK
PS ...>
```

**Impact:** Commands with side effects (file writes, git operations, installs) would execute twice.

**Status:** Needs investigation by the fix agent. Likely the `command` parameter is being written to the terminal both during session creation (via shell args or initial write) and again by the MCP handler after the terminal is ready.

---

### Bug 7: Worktree accumulation (no cleanup)

**Evidence:** `ls` of worktree directory shows 46 worktree directories dating back to Feb 9. Includes:
- 3 worktrees from this test session (still present after `close_terminal`)
- Numerous old worktrees from previous sessions
- Some with increasingly long recursive names (e.g., `wt-s-sse-navigation-navigation-sse-navigation`)

**Expected:** Worktrees cleaned up when terminal is closed, or at least when terminal is explicitly closed via `close_terminal` MCP call.
**Actual:** Worktrees persist indefinitely.

**Impact:** Disk space leak. Each worktree is a full git checkout. With 46 accumulated, this could be many GB.

---

### Bug 8: Duplicate workspace creation

**Reproduction:** `quick_claude(workspace_id="46f65210-...")` called 3 times.

**Before test:** 3 workspaces (Agent, typesense, godly-terminal)
**After test:** 4 workspaces — a NEW "Agent" workspace (`95085625-...`) was created pointing to typesense folder

**Impact:** Workspace pollution. Multiple "Agent" workspaces with identical names are confusing.

---

## Tools That Work Correctly

| Tool | Status | Notes |
|------|--------|-------|
| `list_terminals` | OK | Returns correct data |
| `list_workspaces` | OK | Returns correct data |
| `get_workspace_details` | OK | Returns correct data |
| `get_current_terminal` | OK | Correct terminal + workspace ID |
| `read_terminal` | OK | Returns terminal output (ANSI stripped when requested) |
| `read_grid` | OK | Returns clean grid with cursor position |
| `rename_terminal` | OK | Name persists |
| `focus_terminal` | OK | Switches focus |
| `close_terminal` | OK | Terminals close (but worktrees remain) |
| `wait_for_idle` | OK | Correctly detects idle state |
| `wait_for_text` | OK | Correctly times out when text absent |
| `capture_screenshot` | OK | Saves PNG (but blank if terminal not visible) |
| `export_terminal_info` | OK | Returns correct metadata |
| `send_keys` | OK | Correctly rejects invalid keys, sends special keys |

---

## Root Cause Analysis

The bugs cluster into three categories:

### Category A: Write/Response Channel Timeout (Bugs 3, 4, 5, 2) — ROOT-CAUSED

All write operations (`write_to_terminal`, `execute_command`) fail with a channel timeout. The writes DO reach the terminal (confirmed by `read_grid`), but the acknowledgment never comes back. This is the **root cause of Bug #2** — `quick_claude` can't deliver prompts because it depends on the write channel.

**Confirmed root cause:** The MCP handler's `WriteToTerminal` (at `handler.rs:1100`) uses `daemon.send_request()`, which is a synchronous request-response call through the bridge I/O thread. The bridge is a single-threaded I/O loop that handles ALL pipe reads/writes — snapshot requests queue behind streaming output events. Under load with multiple terminals producing output, the bridge becomes congested and the 15-second `recv_timeout` at `client.rs:609` is hit.

The Tauri command handler for the identical operation (`commands/terminal.rs:196-215`) was already fixed to use `daemon.send_fire_and_forget()` with the comment: "Fire-and-forget: don't block the Tauri thread pool waiting for the daemon's Ok response." The MCP handler was never updated to match.

**Fix:** Use `send_fire_and_forget()` in the MCP handler for `WriteToTerminal`, matching the Tauri command. This eliminates the timeout for Bugs #3/#4/#5 and unblocks prompt delivery for Bug #2.

### Category B: Workspace Routing (Bugs 1, 8) — BY DESIGN (partially)

**Bug #1 is by design.** The `ensure_mcp_workspace` function (`handler.rs:12-54`) intentionally routes all MCP terminals to a dedicated "Agent" workspace to prevent a WebView2 broadcast storm that caused crashes under heavy output (issue #204). The `workspace_id` parameter is only used for `folder_path` resolution (CWD and worktree source), not visual placement. The tool descriptions should be updated to clarify this.

**Bug #8 (duplicate workspace creation)** is a real bug — the deduplication logic should find the existing "Agent" workspace instead of creating a new one.

### Category C: Cleanup and Duplication (Bugs 6, 7) — NEEDS INVESTIGATION

**Bug #6 (command duplication):** Needs investigation by the fix agent. Likely the `command` parameter is being written to the terminal both during initial shell setup and again by the MCP handler's post-creation write logic.

**Bug #7 (worktree accumulation):** Worktrees are created via `git worktree add` when terminals spawn with worktree mode, but `close_terminal` never runs `git worktree remove`. Fix: add worktree cleanup to the terminal close path.

---

## Recommendations

### Priority 1 (Blocking — makes orchestrator unusable) — FIX IN PR #534
1. **Fix write channel timeout (Bugs 3/4/5/2)** — Change MCP handler's `WriteToTerminal` from `daemon.send_request()` to `daemon.send_fire_and_forget()`, matching the Tauri command handler. This single fix unblocks `quick_claude` prompt delivery too.

### Priority 2 (High — breaks multi-workspace workflows) — FIX IN PR #534
2. **Fix duplicate command execution (Bug 6)** in `create_terminal` — command should run exactly once. Needs investigation.
3. **Add worktree cleanup (Bug 7)** — clean up worktrees when terminals are closed via MCP.

### Priority 3 (Medium — quality of life)
4. **Deduplicate workspace creation (Bug 8)** — `quick_claude` should not create duplicate "Agent" workspaces.
5. **Clarify tool descriptions (Bug 1)** — Update `create_terminal` and `quick_claude` tool descriptions to clarify that `workspace_id` controls folder_path/CWD source, not visual workspace placement. All MCP terminals appear in the "Agent" workspace tab.

### Priority 4 (Nice to have)
6. **Screenshot of non-visible terminals** — `capture_screenshot` returns blank for terminals not currently displayed. Consider falling back to a grid-based text rendering.
