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

## Issue Investigation Tracking

For every issue you work on, create or update `docs/<issue-slug>.md` to track attempts and outcomes. Check `docs/` first to avoid repeating failed approaches from previous sessions.

## Project-Specific Workflow Notes

These extend the global CLAUDE.md workflows (bug fix, feature development):

- **Bug fixes**: Write a full test **suite** (not a single test) to reproduce the bug.
- **Features**: Write **E2E tests** (`npm run test:e2e`), not just unit tests.

## Parallel Agent Workflow

- **Task claiming**: Create `current_tasks/<branch-name>.md` with scope and files. Check for overlap first. Remove when PR merges.
- **Branch naming**: `wt-<scope>` (e.g., `wt-fix-scrollback`, `wt-feat-search`).
- **Stay in sync**: Rebase from master before opening a PR. If another agent merges first, rebase on top.
- **Scope boundaries**: Each agent owns one clearly scoped task. Avoid modifying the same files as another active agent.

## Output Hygiene

- **Run targeted tests first**: `cargo test -p godly-daemon` before the full suite.
- **Summarize failures**: State the root cause concisely, don't paste full stack traces.
- **Avoid verbose flags**: No `--verbose` or `--nocapture` unless actively debugging a specific test.
- **Incremental verification**: `cargo check` before `cargo test`. One crate before all crates.

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

## Architecture Overview

Godly Terminal is a Windows terminal application built with Tauri 2.0, featuring workspaces and tmux-style session persistence via a background daemon.

### Stack
- **Frontend**: TypeScript + vanilla DOM + Canvas2D renderer (backed by godly-vt)
- **Backend**: Rust + Tauri 2.0 (GUI client) + godly-daemon (background PTY manager)
- **Terminal engine**: godly-vt (forked from vt100-rust, SIMD VT parser with scrollback)
- **Build**: Vite (frontend) + Cargo workspace (backend)

### Rendering Pipeline

The daemon owns all terminal state via godly-vt parsers. The frontend is a pure display layer:

```
Shell output → daemon PTY reader → ring buffer + godly-vt parser
                                          │
                              ┌───────────┘
                              ▼
Frontend: terminal-output event → fetch RichGridData snapshot via IPC
                                          │
                              ┌───────────┘
                              ▼
              TerminalRenderer.render(snapshot) → Canvas2D paint
```

Key design: **no terminal parsing happens in the frontend**. The daemon's godly-vt parser is the single source of truth for grid state, cursor position, colors, scrollback, etc.

### Daemon Architecture

```
┌─────────────┐     Named Pipe IPC      ┌─────────────────┐
│  Tauri App   │◄──────────────────────►│  godly-daemon    │
│  (GUI client)│  connect/disconnect     │  (background)    │
│              │  at will                │                  │
│  DaemonClient│                        │  PTY Sessions    │
│  Bridge      │                        │  Ring Buffers    │
└─────────────┘                         │  godly-vt Parsers│
     │                                  └─────────────────┘
     │ Tauri events                           │
     ▼                                        │ portable-pty
  Frontend                                    ▼
  (Canvas2D renderer)                    Shell processes
                                         (survive app close)
```

### Workspace Crate Structure

```
src-tauri/
  Cargo.toml              ← workspace root
  protocol/               ← shared message types (godly-protocol)
    src/lib.rs, messages.rs, frame.rs, types.rs
  daemon/                 ← background daemon binary (godly-daemon)
    src/main.rs, server.rs, session.rs, pid.rs
  godly-vt/               ← terminal state engine (forked from vt100-rust)
    src/lib.rs, grid.rs, screen.rs, parser.rs
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
- `get_grid_snapshot` - Fetch RichGridData from daemon's godly-vt parser
- `get_grid_dimensions` / `get_grid_text` - Query grid state
- `set_scrollback` - Set scrollback viewport offset
- `reconnect_sessions` / `attach_session` - Reconnect to live daemon sessions on restart
- `detach_all_sessions` - Detach on window close (sessions keep running)
- `create_workspace` / `delete_workspace` - Workspace management
- `save_layout` / `load_layout` - Persistence
- `save_scrollback` / `load_scrollback` - Terminal history

Backend emits events to frontend (via DaemonBridge):
- `terminal-output` - PTY output data (triggers grid snapshot fetch)
- `terminal-closed` - Process exit
- `process-changed` - Shell process name updates

### State Management

**Frontend** (`src/state/store.ts`): Observable store with `subscribe()` pattern. Components call store methods, store notifies all subscribers.

**Backend** (`src-tauri/src/state/`): Thread-safe state using `RwLock<HashMap>`. Holds workspaces, terminals, and session metadata (shell_type, cwd for persistence).

### Session Lifecycle

1. **Create**: App sends `CreateSession` + `Attach` to daemon via named pipe
2. **Running**: Daemon owns PTY + godly-vt parser, streams output events to attached client
3. **App close**: App sends `Detach` for all sessions, saves layout
4. **App reopen**: Loads layout, checks daemon for live sessions via `ListSessions`
5. **Reattach**: If session alive → `Attach` (ring buffer replays missed output into godly-vt)
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
└── TerminalPane.ts      - Canvas2D terminal pane (delegates to TerminalRenderer)
    └── TerminalRenderer.ts - Canvas2D rendering of godly-vt grid snapshots
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

## Keyboard Shortcuts

All keyboard shortcuts defined in `DEFAULT_SHORTCUTS` (`src/state/keybinding-store.ts`) must be displayed in the Settings dialog (`src/components/SettingsDialog.ts`). When adding a new shortcut category, add it to the `categories` array in `renderShortcuts()` so it appears in the UI.

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
Shell output → daemon reader thread → ring buffer + godly-vt parser → named pipe → DaemonBridge → `terminal-output` event → `TerminalPane.fetchAndRenderSnapshot()` → Canvas2D paint

## MCP Testing

See [docs/mcp-testing.md](docs/mcp-testing.md) for the full MCP test procedure and known gaps.
