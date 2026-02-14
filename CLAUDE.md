# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Development Commands

```bash
# Install dependencies
npm install

# Build the daemon (required before first dev run)
npm run build:daemon

# Development mode (starts Vite + Tauri)
npm run tauri dev

# Production build (daemon must be built first)
npm run build:daemon:release
npm run tauri build

# TypeScript check only
npx tsc --noEmit

# Rust check only (all workspace members)
cd src-tauri && cargo check --workspace

# Run Rust tests (all workspace members)
cd src-tauri && cargo test --workspace

# Run TypeScript tests
npm test
```

## Git Workflow

Always commit all staged and unstaged changes when making a commit. Do not leave uncommitted changes behind.

Never add "Generated with Claude Code" or any similar attribution message to commits, PRs, or any other output.

### PR Policy

- **Features (`feat:`) and bug fixes (`fix:`)**: Create a feature branch, open a PR to master, and wait for merge.
- **Documentation (`docs:`), chores (`chore:`), style (`style:`), and other minor changes**: Commit and push directly to master — no PR needed.

## Debugging Principles

- **Never mask errors.** Don't add retry loops, fallback handlers, or auto-recovery that hides the root cause of a crash or failure. If something crashes, the priority is understanding WHY — not papering over it so the user doesn't notice.
- **Preserve crash evidence.** Logs must survive process restarts. Never truncate logs on startup. Use append mode and rotate old logs so the previous run's crash info is always available for post-mortem.

## Bug Fix Workflow

When the user pastes a bug report or describes a bug:

1. **Write a test suite first** that reproduces the bug. The tests must fail, confirming the bug exists.
2. **Run the test suite** to verify the tests actually fail as expected (red phase).
3. **Fix the bug** by modifying the source code.
4. **Run the test suite again** and loop until all tests pass (green phase).
5. Continue with the standard verification requirements below (full build + all tests).

Do NOT skip the reproduction step. The test must fail before you start fixing.

### Test Quality Standards

- Tests must be **specific** enough that passing them means the bug is actually fixed, not just that something changed.
- Each test should assert the **exact expected behavior**, not just "no error."
- Regression tests should include the original bug trigger as a comment (e.g., `// Bug: scrollback was truncated when buffer exceeded 5MB`).

## Feature Development Workflow

When adding a new feature:

1. **Implement the feature** in the source code.
2. **Write an E2E test suite** covering the feature's key user-facing behaviors.
3. **Run the E2E tests** (`npm run test:e2e`) and loop until all tests pass.
4. Continue with the standard verification requirements below (full build + all tests).

Do NOT consider a feature complete without an accompanying E2E test suite.

## Parallel Agent Workflow

When multiple Claude instances work simultaneously in worktrees:

### Task Claiming

Before starting work, create a file `current_tasks/<branch-name>.md` describing the task scope and files likely to be modified. Check existing files in `current_tasks/` first to avoid overlap. Remove the file when the PR is merged.

### Branch Naming

Worktree branches should use descriptive names: `wt-<scope>` (e.g., `wt-fix-scrollback`, `wt-feat-search`). Avoid generic names like `wt-abc123`.

### Staying in Sync

Pull and rebase from master before opening a PR. If another agent's PR merges first, rebase on top of it before pushing.

### Scope Boundaries

Each agent should own a clearly scoped task. Avoid modifying the same files as another active agent. If overlap is unavoidable, coordinate via smaller, more frequent commits and PRs.

### Task Scoping

Each agent should receive a single, well-defined task when launched. Good tasks have clear boundaries:
- "Implement search in the terminal pane" (one feature, known files)
- "Write tests for the ring buffer module" (one module, test-only changes)
- "Refactor daemon session cleanup logic" (one concern, contained scope)

Avoid giving one agent a broad task like "improve the codebase" — it will collide with other agents. The narrower the task, the fewer merge conflicts.

## Output Hygiene

Rules to keep context windows clean during long agent sessions:

- **Run targeted tests first**: When working on a specific crate or module, run just that crate's tests (`cargo test -p godly-daemon`) before the full suite.
- **Summarize failures**: When tests fail, identify the root cause and state it concisely rather than pasting full stack traces.
- **Avoid verbose flags**: Don't use `--verbose`, `--nocapture`, or similar flags unless actively debugging a specific test.
- **Incremental verification**: Check compilation (`cargo check`) before running tests. Check one crate before all crates.

### Clean Test Output

Tests should produce minimal, parseable output — not walls of text that pollute the context window.

- **Minimal on success**: A passing test suite should print a summary line (e.g., `45 passed, 0 failed`), not per-test details. Use the default output level; avoid `--show-output` or `--nocapture` for routine runs.
- **Structured on failure**: Failed tests should print a single-line error identifier (e.g., `FAIL: test_ring_buffer_overflow — expected 1024 bytes, got 0`) followed by the relevant assertion, not the entire backtrace.
- **Log to files, not stdout**: When debugging requires verbose output, redirect to a file (`cargo test 2> test-debug.log`) and read the file selectively rather than flooding the terminal.
- **Grep-friendly format**: Error messages should be self-contained on one line so they can be found with `grep FAIL` or `grep ERROR`. Avoid multi-line error formatting in test harnesses.
- **Pre-compute summaries**: When running large test suites, use summary flags (`--format terse` for Rust, `--reporter=dot` for JS) to get aggregate pass/fail counts without per-test noise.

## Verification Requirements

**IMPORTANT**: After making any code changes, always verify the project builds and tests pass before considering work complete. Loop until all checks pass:

1. **Run all tests**:
   ```bash
   cd src-tauri && cargo test -p godly-protocol && cargo test -p godly-daemon && cargo test -p godly-terminal
   npm test
   ```

2. **Verify production build**:
   ```bash
   npm run build
   ```

3. If any step fails, fix the issues and repeat until everything passes.

This catches:
- TypeScript compilation errors
- Rust compilation errors
- Test failures
- Configuration errors (tauri.conf.json, etc.)

## Architecture Overview

Godly Terminal is a Windows terminal application built with Tauri 2.0, featuring workspaces and tmux-style session persistence via a background daemon.

### Stack
- **Frontend**: TypeScript + vanilla DOM + xterm.js
- **Backend**: Rust + Tauri 2.0 (GUI client) + godly-daemon (background PTY manager)
- **Build**: Vite (frontend) + Cargo workspace (backend)

### Daemon Architecture

```
┌─────────────┐     Named Pipe IPC      ┌─────────────────┐
│  Tauri App   │◄──────────────────────►│  godly-daemon    │
│  (GUI client)│  connect/disconnect     │  (background)    │
│              │  at will                │                  │
│  DaemonClient│                        │  PTY Sessions    │
│  Bridge      │                        │  Ring Buffers    │
└─────────────┘                         └─────────────────┘
     │                                        │
     │ Tauri events                           │ portable-pty
     ▼                                        ▼
  Frontend                               Shell processes
  (unchanged)                            (survive app close)
```

### Workspace Crate Structure

```
src-tauri/
  Cargo.toml              ← workspace root
  protocol/               ← shared message types (godly-protocol)
    src/lib.rs, messages.rs, frame.rs, types.rs
  daemon/                 ← background daemon binary (godly-daemon)
    src/main.rs, server.rs, session.rs, pid.rs
  src/                    ← Tauri app
    daemon_client/        ← IPC client + event bridge
      mod.rs, client.rs, bridge.rs
    commands/             ← Tauri IPC command handlers
    state/                ← App state (workspaces, terminals, session metadata)
    persistence/          ← Layout, scrollback, autosave
    pty/                  ← Process monitor (queries daemon for PIDs)
```

### Frontend-Backend Communication

All terminal and workspace operations use Tauri IPC commands defined in `src-tauri/src/commands/`. Frontend services (`src/services/`) wrap `invoke()` calls. Terminal commands proxy through the daemon via named pipe IPC.

Key IPC commands:
- `create_terminal` / `close_terminal` - Creates/closes daemon session + attaches
- `write_to_terminal` / `resize_terminal` - Proxied to daemon session
- `reconnect_sessions` / `attach_session` - Reconnect to live daemon sessions on restart
- `detach_all_sessions` - Detach on window close (sessions keep running)
- `create_workspace` / `delete_workspace` - Workspace management
- `save_layout` / `load_layout` - Persistence
- `save_scrollback` / `load_scrollback` - Terminal history

Backend emits events to frontend (via DaemonBridge):
- `terminal-output` - PTY output data
- `terminal-closed` - Process exit
- `process-changed` - Shell process name updates

### State Management

**Frontend** (`src/state/store.ts`): Observable store with `subscribe()` pattern. Components call store methods, store notifies all subscribers.

**Backend** (`src-tauri/src/state/`): Thread-safe state using `RwLock<HashMap>`. Holds workspaces, terminals, and session metadata (shell_type, cwd for persistence).

### Session Lifecycle

1. **Create**: App sends `CreateSession` + `Attach` to daemon via named pipe
2. **Running**: Daemon owns PTY, streams output to attached client
3. **App close**: App sends `Detach` for all sessions, saves layout
4. **App reopen**: Loads layout, checks daemon for live sessions via `ListSessions`
5. **Reattach**: If session alive → `Attach` (ring buffer replays missed output)
6. **Fallback**: If session dead → create fresh terminal with saved CWD + load scrollback
7. **Idle**: Daemon self-terminates after 5min with no sessions and no clients

### Persistence

Three persistence mechanisms in `src-tauri/src/persistence/`:
- **layout.rs** - Workspace/terminal metadata saved on exit (reads from session_metadata)
- **scrollback.rs** - Terminal buffer content per-session (5MB limit)
- **autosave.rs** - Background thread saves every 30s if dirty

Data stored via `tauri-plugin-store` in app data directory.

### Component Structure

```
App.ts           - Root: manages layout, keyboard shortcuts, reconnection logic
├── WorkspaceSidebar.ts  - Workspace list, new workspace dialog, drop target
├── TabBar.ts            - Terminal tabs with drag-drop reordering
└── TerminalPane.ts      - xterm.js wrapper with scrollback save/load
```

### Shell Types

`ShellType` enum supports:
- `Windows` - PowerShell with `-NoLogo`
- `Wsl { distribution }` - WSL with optional distro selection

## Daemon Test Isolation (CRITICAL)

**Tests must NEVER interfere with the production daemon.** A test that kills or connects to the production daemon will freeze all live terminal sessions.

### Required isolation rules for `daemon/tests/*.rs`:

1. **Use isolated pipe names** — every test must create its own unique pipe via `GODLY_PIPE_NAME` env var or `--instance` CLI arg. NEVER import or use the production `PIPE_NAME` constant from `godly_protocol`.
2. **Kill by PID, not by name** — NEVER use `taskkill /F /IM godly-daemon.exe` (kills ALL daemon processes). Use `child.kill()` for child-process daemons or `taskkill /F /PID <pid>` for detached daemons.
3. **Use `GODLY_NO_DETACH=1`** — keeps the test daemon as a child process so `child.kill()` works for cleanup.
4. **Pattern to follow** — see `handler_starvation.rs` or `memory_stress.rs` for the `DaemonFixture` pattern with proper isolation.

### Guardrail test

`daemon/tests/test_isolation_guardrail.rs` automatically scans all daemon test files for violations of these rules. It runs as part of the normal test suite and will fail if any test file:
- Uses `taskkill /IM` (process-name kill)
- Imports the production `PIPE_NAME` constant
- Spawns a daemon without `GODLY_PIPE_NAME` or `--instance` isolation

## Key Patterns

### Adding a new Tauri command

1. Add function in `src-tauri/src/commands/` with `#[tauri::command]`
2. Register in `lib.rs` `invoke_handler`
3. Add TypeScript wrapper in `src/services/`

### Adding a new daemon command

1. Add variant to `Request` and `Response` in `protocol/src/messages.rs`
2. Handle in `daemon/src/server.rs` `handle_request()`
3. Add client method in `src/daemon_client/client.rs`
4. Add Tauri command wrapper in `src/commands/terminal.rs`

### Modifying godly-mcp

When changing any code in `src-tauri/mcp/`, bump the `BUILD` constant in `src-tauri/mcp/src/main.rs` so the log shows which binary is running. The log line `=== godly-mcp starting === build=N` makes it easy to confirm a rebuilt binary is actually in use.

### Adding auto-save triggers

Inject `State<Arc<AutoSaveManager>>` and call `auto_save.mark_dirty()` after state mutations.

### Terminal state flow

User input → `terminalService.writeToTerminal()` → IPC → DaemonClient → named pipe → daemon → PTY
Shell output → daemon reader thread → named pipe → DaemonBridge → `terminal-output` event → `TerminalPane.terminal.write()`

## MCP Testing Procedure

When asked to test the godly-terminal MCP, use the MCP tools directly from Claude Code. The MCP binary (`godly-mcp`) exposes 15 tools via JSON-RPC over stdio, proxied through the Tauri app via named pipe IPC.

### Test Sequence

Run these tests in order. Each phase builds on the previous one. Clean up all test artifacts (terminals, workspaces, worktrees) when done.

**Phase 1 — Read-only queries (no side effects):**
1. `get_current_terminal` → expect `{id, name, process_name, workspace_id}`
2. `list_terminals` → expect array of terminal objects
3. `list_workspaces` → expect array of workspace objects
4. `get_notification_status` (no params) → expect `{enabled, source: "global"}`

**Phase 2 — Notifications:**
5. `notify` with `message` → expect `{success: true}`, verify chime plays
6. `set_notification_enabled` with `terminal_id` + `enabled: false` → expect success
7. `get_notification_status` with `terminal_id` → expect `{enabled: false, source: "terminal"}`
8. `set_notification_enabled` with `terminal_id` + `enabled: true` → re-enable
9. Repeat steps 6-8 for `workspace_id` instead of `terminal_id`

**Phase 3 — Terminal CRUD:**
10. `create_terminal` (basic, just `workspace_id`) → expect `{id, success: true}`
11. `create_terminal` with `cwd` param → expect success
12. `create_terminal` with `command` param → expect success, then `read_terminal` to verify command output appears
13. `create_terminal` with `worktree: true` → expect `{id, worktree_path, worktree_branch}`
14. `create_terminal` with `worktree_name` → expect custom branch name in response
15. `rename_terminal` → rename a test terminal, verify via `list_terminals`
16. `focus_terminal` → expect success (visual confirmation needed — see gaps)
17. `write_to_terminal` → send `echo "MARKER"`, then `read_terminal` to verify
18. `read_terminal` with `mode: "tail"` → expect terminal content
19. `read_terminal` with `mode: "head"` → expect terminal content
20. `read_terminal` with `mode: "full"` → expect terminal content
21. `read_terminal` with `filename` param → expect file written to disk
22. `close_terminal` → close all test terminals

**Phase 4 — Workspace operations:**
23. `create_workspace` with `name` + `folder_path` → expect `{id, success: true}`
24. `switch_workspace` to new workspace → expect success
25. `move_terminal_to_workspace` → move a terminal, verify via `list_terminals`
26. `switch_workspace` back to original → expect success
27. Clean up: close test terminals, remove worktrees via git CLI

**Phase 5 — Error handling:**
28. `write_to_terminal` with invalid ID → expect error (daemon validates: "Session not found")
29. `read_terminal` with invalid ID → expect error (daemon validates: "Session not found")
30. `close_terminal` with invalid ID → **BUG: returns `{success: true}` silently**
31. `switch_workspace` with invalid ID → **BUG: returns `{success: true}` silently**
32. `rename_terminal` with invalid ID → **BUG: returns `{success: true}` silently**
33. `focus_terminal` with invalid ID → **BUG: returns `{success: true}` silently**
34. `move_terminal_to_workspace` with invalid ID → **BUG: returns `{success: true}` silently**

Note: Operations routed through the daemon (`write_to_terminal`, `read_terminal`) properly validate IDs.
Operations handled by Tauri app state (`close`, `switch`, `rename`, `focus`, `move`) silently succeed with invalid IDs — they need validation added.

### Cleanup Checklist

After testing, ensure:
- [ ] All test terminals are closed
- [ ] All test worktrees are removed (`git worktree remove` + `git branch -d`)
- [ ] Test workspace still exists (no `delete_workspace` tool — manual cleanup needed)

### Known Gaps (cannot test via MCP alone)

| Gap | Description | Suggested MCP Tool |
|-----|-------------|-------------------|
| No `delete_workspace` | Can create but not delete workspaces; leaves orphans | `delete_workspace` |
| No `delete_worktree` | Worktrees from `create_terminal` need manual git cleanup | `delete_worktree` or auto-cleanup on `close_terminal` |
| No `get_active_workspace` | Cannot verify `switch_workspace` actually changed the UI | `get_active_workspace` |
| No `get_active_terminal` | Cannot verify `focus_terminal` actually switched the tab | `get_active_terminal` |
| No plain-text `read_terminal` | Output contains raw ANSI escapes, hard to parse programmatically | Add `strip_ansi: true` param to `read_terminal` |
| No `get_terminal_cwd` | Cannot verify `cwd` param on `create_terminal` worked | `get_terminal_cwd` or include cwd in terminal info |
| No `resize_terminal` via MCP | The daemon supports resize but MCP doesn't expose it | `resize_terminal` |
| Silent success on invalid IDs | `close`, `switch_workspace`, `rename`, `focus`, `move` return `{success: true}` for nonexistent IDs | Add ID validation in Tauri MCP handler before dispatching |
| No error case testing docs | Error format inconsistent between daemon-routed and Tauri-routed tools | Standardize error responses across all tools |
