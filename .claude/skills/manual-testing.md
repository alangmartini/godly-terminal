# Manual Testing

Manually test a Godly Terminal feature via MCP tools, document findings, and create a GitHub issue with all improvement points.

## Usage

```
/manual-testing <feature-name-or-description>
```

## Instructions

You are a QA tester for Godly Terminal. Your job is to exercise a feature through the MCP tools (godly-terminal MCP server), observe actual behavior, compare it to expected behavior, and file a detailed GitHub issue with every problem you find.

### Phase 1: Understand the Feature

1. **Search the codebase** for all files related to the feature. Use Explore agent or Grep/Glob to find:
   - Backend implementation (Rust: `src-tauri/`)
   - Frontend implementation (TypeScript: `src/`)
   - MCP tool definitions (`src-tauri/mcp/src/tools.rs`)
   - MCP handler (`src-tauri/src/mcp_server/handler.rs`)
   - Related Tauri commands (`src-tauri/src/commands/`)
   - Tests (if any)

2. **Read the relevant code** to understand:
   - What the feature is supposed to do (happy path)
   - What edge cases exist
   - What fallback behavior is defined
   - What the MCP interface looks like

3. **Document the expected behavior** before testing. Write it down so you can compare against actual results.

### Phase 2: Design Test Cases

Create a test matrix covering:

| Category | Examples |
|----------|---------|
| **Happy path** | Normal use with valid inputs |
| **Edge cases** | Empty input, very long input, special characters, unicode |
| **Error paths** | Invalid IDs, missing required params, conflicting params |
| **Boundary conditions** | Min/max values, off-by-one, timeout boundaries |
| **Integration** | Feature combined with other features (e.g., worktrees + workspaces) |
| **Concurrency** | Rapid-fire calls, parallel operations |

Aim for 5-15 test cases depending on feature complexity.

### Phase 3: Execute Tests via MCP

For each test case:

1. **Call the MCP tool** with the test inputs using the godly-terminal MCP tools
2. **Record the actual result** — exact response JSON, branch names, error messages, etc.
3. **Compare** actual vs expected
4. **Classify** the result:
   - **PASS** — behavior matches expectations
   - **FAIL** — behavior is wrong or broken
   - **DEGRADED** — works but poorly (e.g., garbage output, slow, confusing UX)
   - **UNEXPECTED** — behavior not covered by docs/code (could be good or bad)

**Important rules:**
- Test via MCP tools only (this simulates how Claude Code and other AI tools interact with the terminal)
- Clean up after each test (close terminals, remove worktrees) to avoid polluting the user's environment
- If a test creates resources, track them and clean up in a final cleanup phase
- Record exact outputs — don't paraphrase. Copy the actual JSON responses.

### Phase 3b: Visual / UX Testing

When the feature has any UI or visual component (dialogs, tabs, terminal rendering, sidebar changes, etc.), take screenshots and analyze the appearance and UX.

1. **Take screenshots** using the `read_grid` MCP tool for terminal-rendered content, or ask the user to focus the relevant part of the app and use the system screenshot capabilities. If there's a design file (`.pen`), use pencil MCP tools to compare against the design spec.

2. **Evaluate the visual output** for each relevant state of the feature:
   - Does the UI match the expected design / layout?
   - Are labels, text, and icons legible and properly aligned?
   - Is spacing consistent (padding, margins, gaps)?
   - Do interactive elements (buttons, inputs, dropdowns) look clickable/focusable?
   - Are loading/disabled/error states visually distinct?
   - Does it look good at different terminal sizes? (use `resize_terminal` to test)

3. **Evaluate the UX flow**:
   - Is the feature discoverable? Can a user find it without reading docs?
   - Is the number of steps/clicks reasonable for the task?
   - Are error messages helpful and actionable (not just "Failed")?
   - Is there proper feedback for async operations (loading indicators, success confirmation)?
   - Are destructive actions guarded with confirmation?
   - Is keyboard navigation supported where expected?

4. **Record visual findings** with concrete descriptions:
   - BAD: "The dialog looks off"
   - GOOD: "The 'AI Suggest' button is hidden when model isn't loaded, with no indication AI suggestions exist or how to enable them"

5. **Classify visual/UX issues** using the same severity scale (P0-P3), noting them as `[UX]` or `[Visual]` in the issue.

**Note:** If the feature is purely backend/MCP with no UI, skip this phase and note "No visual component — Phase 3b skipped" in the output.

### Phase 4: Analyze Findings

After all tests complete, analyze the results:

1. **Group failures** by root cause (don't file 5 issues for the same bug)
2. **Assess severity** for each problem:
   - **P0 (Critical)**: Feature is fundamentally broken, core path doesn't work
   - **P1 (Major)**: Important use case fails, bad UX that confuses users
   - **P2 (Minor)**: Edge case fails, cosmetic issues, minor inconveniences
   - **P3 (Nit)**: Suggestions, nice-to-haves, code quality observations
3. **Identify patterns** — is the feature over-engineered? Under-tested? Missing error handling?
4. **Note any positive findings** too — what works well?

### Phase 5: Create GitHub Issue

Create a single GitHub issue (not one per finding) with this structure:

```markdown
## Summary

<1-2 sentence overview of the feature and testing verdict>

## Test Results

| # | Test Case | Input | Expected | Actual | Verdict |
|---|-----------|-------|----------|--------|---------|
| 1 | Happy path - basic | ... | ... | ... | PASS/FAIL |
| 2 | Edge case - empty | ... | ... | ... | FAIL |
...

**Score: X/Y passed** (Z degraded)

## Problems Found

### P0: <critical issue title>
- **Symptom**: What happened
- **Expected**: What should happen
- **Root cause**: Why (from code reading)
- **Affected code**: File paths and line numbers

### P1: <major issue title>
...

## Visual / UX Issues

### [UX] P1: <issue title>
- **What**: Description of the UX problem
- **Why it matters**: Impact on user experience
- **Suggestion**: How to improve it

### [Visual] P2: <issue title>
...

## What Works Well

- <positive finding 1>
- <positive finding 2>

## Improvement Suggestions

- [ ] <actionable suggestion 1>
- [ ] <actionable suggestion 2>
...

## Affected Files

- `path/to/file1.rs` — description
- `path/to/file2.ts` — description
```

Label the issue with `bug` if core functionality is broken, `enhancement` if it mostly works but needs improvement.

### Phase 6: Cleanup

1. **Close all test terminals** created during testing
2. **Remove all test worktrees** created during testing
3. **Verify** no resources were left behind by listing terminals/workspaces

### Output

After all phases, print a summary:

```
Manual Testing Complete
=======================
Feature tested:    <name>
Test cases:        <total>
Passed:            <count>
Failed:            <count>
Degraded:          <count>
Issues created:    #<number>
Resources cleaned: <count> terminals, <count> worktrees
```

### Tips

- When testing MCP tools, use `list_terminals` and `list_workspaces` to verify state changes
- Use `execute_command` to run verification commands inside test terminals (e.g., `git branch` to check branch name)
- Use `read_grid` to capture terminal screen state for visual verification of terminal-rendered UI
- Use `resize_terminal` to test responsive behavior at different sizes
- For features with a `.pen` design file, use pencil MCP tools (`get_screenshot`, `batch_get`) to compare implementation against the design spec
- If a feature requires the Tauri app to be running (not just the daemon), note which tests are MCP-only vs require the full app
- Search for existing issues before creating a new one: `gh issue list --search "<keywords>" --state all`

### Advanced Testing Tools

When MCP-only testing is insufficient (e.g., you need to test drag-and-drop, keyboard shortcuts, visual layout, or mouse interactions), use the **hybrid testing framework**:

#### JS Bridge (godly-terminal MCP)
- **`execute_js`** — Run JavaScript in the WebView to inspect DOM state, read the store, get element positions, check CSS classes, or dispatch synthetic events. Examples:
  - `return window.__STORE__.getState().splitViews` — query split state
  - `return document.querySelector('.split-divider')?.getBoundingClientRect()` — get element position for PyAutoGUI
  - `return document.querySelector('.terminal-pane')?.className` — check CSS classes
- **`capture_screenshot`** — Capture a terminal canvas as a PNG file. Pass `terminal_id` for a specific terminal, or omit for the first visible canvas.
- **`create_split` / `clear_split` / `get_split_state`** — Programmatic split view control via MCP.

#### PyAutoGUI MCP (OS-level automation)
When the `pyautogui-mcp` server is registered, you have real mouse/keyboard control:
- **`screenshot`** — Full-screen or region screenshot (see the actual window)
- **`click` / `drag` / `drag_from_to`** — Real mouse interactions (resize dividers, drag tabs)
- **`press_key`** — App-level keyboard shortcuts (e.g., `ctrl+\` for split)
- **`focus_window` / `get_window_rect`** — Window management

#### Hybrid Pattern (recommended for UI testing)
1. Use `execute_js` to get element positions (reliable, CSS-selector-based)
2. Use `get_window_rect` to get the window offset
3. Use PyAutoGUI `click`/`drag` at the computed screen coordinates
4. Use `capture_screenshot` or PyAutoGUI `screenshot` to verify the result
5. Use `execute_js` to verify state changes in the store

#### Testing on Staging
Prefer running tests on the Godly Staging instance (`npm run staging:dev`) to avoid disrupting the production app. The staging instance uses `GODLY_INSTANCE=staging` for full isolation.
