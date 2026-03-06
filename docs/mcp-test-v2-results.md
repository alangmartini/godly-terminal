# MCP Orchestrator Test V2 — Post-Fix Results

**Date**: 2026-03-03
**Binary**: godly-mcp BUILD 25 (new), Godly Terminal app (old — NOT rebuilt)
**Test**: Spawn 3 Claude Code agents, test write/execute/focus/cleanup

---

## Critical Finding

**The handler fixes (PRs #536, #537, #538) are in `src-tauri/src/mcp_server/handler.rs` which compiles into the Tauri app binary (`godly-terminal.exe`), NOT the MCP binary (`godly-mcp.exe`).** We only rebuilt godly-mcp. The running Godly Terminal app still has the old handler code.

This means NONE of the following fixes are active yet:
- Fire-and-forget for `write_to_terminal` (Bug #3 fix)
- `poll_idle` for `create_terminal` command (Bug #6 fix)
- Worktree cleanup on `close_terminal` (Bug #7 fix)
- Auto-focus gating with `focus` parameter (PR #538)

**To activate these fixes: rebuild and reinstall the full Godly Terminal app (or staging build).**

---

## Test Results

### What's fixed (MCP binary side)

| Test | Result | Notes |
|------|--------|-------|
| `quick_claude` returns immediately | PASS | All 3 agents spawned, returned terminal IDs + worktree info |
| `quick_claude` creates worktrees | PASS | 3 worktrees created with correct branch names |
| No duplicate Agent workspace | PASS | Single Agent workspace `95085625-...` shared by all 3 |
| `close_terminal` works | PASS | All 4 test terminals closed |
| `read_grid` works | PASS | Correct grid content returned |
| `list_terminals` / `list_workspaces` | PASS | Correct data |
| `wait_for_text` | PASS | Correctly times out when text absent |
| `focus` parameter accepted | PASS | No parse errors (but not acted on — app side) |

### What's still broken (needs Tauri app rebuild)

| Test | Result | Bug | Root Cause |
|------|--------|-----|------------|
| `write_to_terminal` | FAIL — timeout | #3 | Old handler still uses `daemon.send_request()` |
| `execute_command` | FAIL — timeout | #4 | Same |
| `quick_claude` prompt delivery | FAIL — never arrives | #2 | Background thread depends on write channel |
| `create_terminal` command | FAIL — runs twice | #6 | Old handler writes immediately, no `poll_idle` |
| Worktree cleanup on close | FAIL — dirs remain | #7 | Old handler has no cleanup logic |
| Auto-focus default off | FAIL — focus stolen | PR #538 | Old handler doesn't read `focus` param |

### New findings

| # | Severity | Finding |
|---|----------|---------|
| N1 | **LOW** | Agent workspace not visible in `list_workspaces` — 3 terminals reference workspace `95085625-...` but it doesn't appear in workspace list. Might be a frontend-only workspace not persisted. |
| N2 | **INFO** | `write_to_terminal` still delivers text despite timeout error (same as before — false negative) |
| N3 | **INFO** | `quick_claude_background` fails silently when prompt delivery fails — no error surfaced to caller |
| N4 | **INFO** | Tab names in Agent workspace show "Claude Code" (from screenshot) instead of the worktree branch names. The terminal `name` field does have the correct branch name — frontend may be using process name instead. |

---

## Action Required

**Rebuild and install Godly Terminal (staging)** to activate all fixes:

```bash
pnpm staging:build && pnpm staging:install
```

This rebuilds the full Tauri app including:
- `src-tauri/src/mcp_server/handler.rs` (fire-and-forget, poll_idle, worktree cleanup, focus gating)
- `src-tauri/src/commands/terminal.rs` (poll_idle visibility change)

After install, restart Godly Terminal (Staging) and rerun these tests.

---

## Architecture Lesson

The MCP integration has **two binaries**:

```
godly-mcp.exe (MCP server)          godly-terminal.exe (Tauri app)
  - Parses MCP tool calls             - Handles MCP requests
  - Converts to McpRequest             - Routes to daemon
  - Sends via MCP pipe                 - Returns McpResponse
  - Returns JSON to Claude Code        - Contains handler.rs logic
```

Changes to **tool definitions, argument parsing** → rebuild `godly-mcp`
Changes to **request handling, daemon interaction, focus, cleanup** → rebuild `godly-terminal`
Changes to **protocol messages** → rebuild both
