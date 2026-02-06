# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Development Commands

```bash
# Install dependencies
npm install

# Development mode (starts Vite + Tauri)
npm run tauri dev

# Production build
npm run tauri build

# TypeScript check only
npx tsc --noEmit

# Rust check only
cd src-tauri && cargo check

# Run Rust tests
cd src-tauri && cargo test

# Run TypeScript tests
npm test
```

## Git Workflow

Always commit all staged and unstaged changes when making a commit. Do not leave uncommitted changes behind.

## Verification Requirements

**IMPORTANT**: After making any code changes, always verify the project builds and tests pass before considering work complete. Loop until all checks pass:

1. **Run all tests**:
   ```bash
   cd src-tauri && cargo test
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

Godly Terminal is a Windows terminal application built with Tauri 2.0, featuring workspaces and tmux-style persistence.

### Stack
- **Frontend**: TypeScript + vanilla DOM + xterm.js
- **Backend**: Rust + Tauri 2.0 + portable-pty
- **Build**: Vite (frontend) + Cargo (backend)

### Frontend-Backend Communication

All terminal and workspace operations use Tauri IPC commands defined in `src-tauri/src/commands/`. Frontend services (`src/services/`) wrap `invoke()` calls.

Key IPC commands:
- `create_terminal` / `close_terminal` - PTY session lifecycle
- `write_to_terminal` / `resize_terminal` - PTY I/O
- `create_workspace` / `delete_workspace` - Workspace management
- `save_layout` / `load_layout` - Persistence
- `save_scrollback` / `load_scrollback` - Terminal history

Backend emits events to frontend:
- `terminal-output` - PTY output data
- `terminal-closed` - Process exit
- `process-changed` - Shell process name updates

### State Management

**Frontend** (`src/state/store.ts`): Observable store with `subscribe()` pattern. Components call store methods, store notifies all subscribers.

**Backend** (`src-tauri/src/state/`): Thread-safe state using `RwLock<HashMap>`. Holds workspaces, terminals, and PTY sessions.

### PTY Management

`src-tauri/src/pty/manager.rs` handles pseudo-terminal sessions:
1. Creates PTY via `portable_pty`
2. Spawns shell (PowerShell or WSL with distribution selection)
3. Reader thread captures output and emits events
4. `ProcessMonitor` detects process exit

WSL paths are converted from Windows format (e.g., `C:\Users\...` → `/mnt/c/Users/...`) in `src-tauri/src/utils/path.rs`.

### Persistence

Three persistence mechanisms in `src-tauri/src/persistence/`:
- **layout.rs** - Workspace/terminal metadata saved on exit
- **scrollback.rs** - Terminal buffer content per-session (5MB limit)
- **autosave.rs** - Background thread saves every 30s if dirty

Data stored via `tauri-plugin-store` in app data directory.

### Component Structure

```
App.ts           - Root: manages layout, keyboard shortcuts, state subscription
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

### Adding auto-save triggers

Inject `State<Arc<AutoSaveManager>>` and call `auto_save.mark_dirty()` after state mutations.

### Terminal state flow

User input → `terminalService.writeToTerminal()` → IPC → `PtySession.write()` → shell
Shell output → reader thread → `terminal-output` event → `TerminalPane.terminal.write()`
