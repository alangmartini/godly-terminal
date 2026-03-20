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

For each test case, use the godly-terminal MCP tools to observe and verify state. Available tools are **read-only** — you can inspect state but cannot create, modify, or delete resources.

**Terminal inspection:**
- `get_current_terminal` / `get_active_terminal` — get current/active terminal info
- `list_terminals` — list all terminals
- `read_terminal` / `read_grid` — read terminal output / grid state
- `export_terminal_info` — export detailed terminal info

**Workspace inspection:**
- `list_workspaces` / `get_active_workspace` / `get_workspace_details` — query workspace state
- `get_workspace_modes` — check workspace modes

**Layout inspection:**
- `get_split_state` / `get_layout_tree` — query split and layout state
- `get_tab_order` — check tab ordering

**Visual verification:**
- `capture_screenshot` — screenshot a terminal canvas
- `read_grid` — read the character grid for text verification

**State queries:**
- `ui_query` / `ui_wait` — query UI state and wait for conditions
- `wait_for_text` / `wait_for_idle` — wait for expected terminal state

**App info:**
- `get_app_info` / `get_active_theme` / `get_font_size` / `list_themes` — app configuration
- `get_notification_status` / `get_notification_config` / `list_mute_patterns` — notification state
- `list_available_shells` / `get_default_shell` — shell configuration

For each test:
1. **Setup** — ask the user to set up the required state (create terminals, workspaces, splits) since MCP tools are read-only
2. **Action** — ask the user to perform the feature action in the app
3. **Verify** — check the result using read-only MCP tools (list state, read grid, screenshot)
4. **Record** — note PASS/FAIL with the actual vs expected result

### Phase 3: Cleanup

After all tests:
1. Verify no unexpected resources remain using `list_terminals` and `list_workspaces`
2. If test resources need cleanup, ask the user to close/delete them manually (MCP tools are read-only)

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

- **Always verify cleanup** — check that no unexpected terminals or workspaces remain; ask the user to clean up if needed
- **Record exact outputs** — don't paraphrase MCP responses, show the actual data
- **Be fast** — this is a quick functional check, not a deep QA audit. Use `/manual-testing` for thorough investigation.
- **Load MCP tools first** — use ToolSearch to load godly-terminal MCP tools before calling them
