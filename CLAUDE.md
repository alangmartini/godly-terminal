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

# Production build (daemon is built automatically via beforeBundleCommand)
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

## Bug Fix Workflow

When the user pastes a bug report or describes a bug:

1. **Write a test suite first** that reproduces the bug. The tests must fail, confirming the bug exists.
2. **Run the test suite** to verify the tests actually fail as expected (red phase).
3. **Fix the bug** by modifying the source code.
4. **Run the test suite again** and loop until all tests pass (green phase).
5. Continue with the standard verification requirements below (full build + all tests).

Do NOT skip the reproduction step. The test must fail before you start fixing.

## Feature Development Workflow

When adding a new feature:

1. **Implement the feature** in the source code.
2. **Write an E2E test suite** covering the feature's key user-facing behaviors.
3. **Run the E2E tests** (`npm run test:e2e`) and loop until all tests pass.
4. Continue with the standard verification requirements below (full build + all tests).

Do NOT consider a feature complete without an accompanying E2E test suite.

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

### Adding auto-save triggers

Inject `State<Arc<AutoSaveManager>>` and call `auto_save.mark_dirty()` after state mutations.

### Terminal state flow

User input → `terminalService.writeToTerminal()` → IPC → DaemonClient → named pipe → daemon → PTY
Shell output → daemon reader thread → named pipe → DaemonBridge → `terminal-output` event → `TerminalPane.terminal.write()`
