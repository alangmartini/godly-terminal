# Test

Quickly test a feature or functionality using the godly-terminal MCP tools. Exercises the feature through real MCP interactions and reports pass/fail.

## Usage

```
/test <feature-or-functionality>
```

**Examples:**
- `/test workspace creation and switching`
- `/test split terminal and resize divider`
- `/test terminal rename and tab update`

## Instructions

You are a tester for Godly Terminal. Use the godly-terminal MCP tools to exercise the described feature, verify it works, and report results. This is a quick functional test, not a full QA audit.

### Phase 1: Plan Test Cases

Based on the feature description, design 3-8 focused test cases:

| Type | What to test |
|------|-------------|
| **Happy path** | Core feature works with normal inputs |
| **Edge case** | Empty/long/special inputs, boundary values |
| **Cleanup** | Resources are properly cleaned up after use |
| **Integration** | Feature works with related features (e.g., splits + workspaces) |

List the test cases before executing.

### Phase 2: Execute via MCP

For each test case, use the godly-terminal MCP tools directly. Available tools include:

**Terminal lifecycle:**
- `create_terminal` / `close_terminal` — create and destroy sessions
- `list_terminals` — verify terminal state
- `rename_terminal` — rename a terminal
- `focus_terminal` — switch focus

**Terminal I/O:**
- `write_to_terminal` / `send_keys` — send input
- `read_terminal` / `read_grid` — read output / grid state
- `execute_command` — run a command and wait for output
- `wait_for_text` / `wait_for_idle` — wait for expected state

**Workspace management:**
- `list_workspaces` / `create_workspace` / `switch_workspace` / `delete_workspace`
- `move_terminal_to_workspace`
- `get_active_workspace` / `get_active_terminal`

**Layout:**
- `create_split` / `clear_split` / `get_split_state`
- `split_terminal` / `unsplit_terminal`
- `resize_terminal`
- `get_layout_tree` / `swap_panes` / `zoom_pane`

**Visual verification:**
- `capture_screenshot` — screenshot a terminal canvas
- `read_grid` — read the character grid for text verification
- `get_screenshot` (pencil MCP) — compare against design specs

**Advanced:**
- `execute_js` — run JS in the WebView to inspect DOM/store state
- `quick_claude` — spawn a Quick Claude session

For each test:
1. **Setup** — create any required terminals, workspaces, splits
2. **Action** — perform the feature action via MCP
3. **Verify** — check the result (list state, read grid, screenshot)
4. **Record** — note PASS/FAIL with the actual vs expected result

### Phase 3: Cleanup

After all tests:
1. Close all terminals created during testing
2. Delete all workspaces created during testing
3. Verify cleanup with `list_terminals` and `list_workspaces`

### Phase 4: Report

Print a concise summary:

```
Test Results: <feature>
========================
1. [PASS] <test case 1>
2. [FAIL] <test case 2> — <what went wrong>
3. [PASS] <test case 3>
...

Score: X/Y passed
```

If any tests fail:
- Note the exact MCP response that shows the failure
- Suggest the likely root cause (reference file paths if you can identify them)
- Recommend which test tier (unit/browser/integration/daemon/e2e) should have a regression test

### Rules

- **Always clean up** — never leave test terminals or workspaces behind
- **Test on staging when possible** — if `pnpm staging:dev` is running, prefer testing there
- **Record exact outputs** — don't paraphrase MCP responses, show the actual data
- **Be fast** — this is a quick functional check, not a deep QA audit. Use `/manual-testing` for thorough investigation.
- **Load MCP tools first** — use ToolSearch to load godly-terminal MCP tools before calling them
