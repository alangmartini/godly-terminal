# Quick Claude: Instant Idea Capture

## Status: Implemented

## Problem

The workflow of opening a new terminal with worktree, starting Claude Code, and typing a prompt was too slow for rapid idea capture. Each step required waiting for the previous to complete, making it impossible to fire off multiple ideas in quick succession.

## Solution

Added "Quick Claude" — a fire-and-forget feature with 3 entry points:

### 1. Keyboard Shortcut (`Ctrl+Shift+Q`)
- Opens a dialog with a textarea for the prompt and optional branch name
- Dialog closes immediately on Ctrl+Enter
- Terminal + worktree + Claude Code + prompt delivery happen in a background thread
- Toast notification appears when the session is ready

### 2. MCP Tool (`quick_claude`)
- Parameters: `workspace_id`, `prompt`, `branch_name?`, `skip_fetch?`
- Returns immediately with `terminal_id` and `worktree_branch`
- Multiple calls can be fired in rapid succession from a central Claude session
- Each spawns its own background thread, no blocking

### 3. Central Claude Dispatcher
- Use the MCP tool from any Claude session (no extra code needed)
- Say "new idea: fix the scrollback bug" and Claude calls `quick_claude`

## Key Optimization: Skip Fetch

Added `create_worktree_with_options(..., skip_fetch: bool)` to `worktree.rs`. When `skip_fetch=true` (default for Quick Claude), worktrees branch from local HEAD instead of fetching from origin first. This saves 100-500ms of network latency per worktree creation.

## Background Task Flow

1. Wait for shell to be idle (500ms idle threshold, 5s timeout)
2. Write `claude -dangerously-skip-permissions\r`
3. Wait for Claude to be idle (2000ms idle threshold, 60s timeout)
4. Write the user's prompt
5. Emit `quick-claude-ready` Tauri event (triggers toast notification)

## Files Changed

- `src-tauri/src/worktree.rs` — `create_worktree_with_options` with `skip_fetch`
- `src-tauri/protocol/src/mcp_messages.rs` — `QuickClaude` variant
- `src-tauri/src/commands/terminal.rs` — `quick_claude` command + background task
- `src-tauri/src/mcp_server/handler.rs` — MCP handler for `QuickClaude`
- `src-tauri/mcp/src/tools.rs` — Tool definition + dispatch
- `src-tauri/mcp/src/daemon_direct.rs` — App-only fallback
- `src-tauri/src/lib.rs` — Register command
- `src/state/keybinding-store.ts` — `tabs.quickClaude` action
- `src/components/dialogs.ts` — `showQuickClaudeDialog()`
- `src/components/App.ts` — Shortcut handler + toast listener
