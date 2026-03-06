# Architecture Details

## Rendering Pipeline

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

## Daemon Architecture

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

## Workspace Crate Structure

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

## Frontend-Backend Communication

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

## State Management

**Frontend** (`src/state/store.ts`): Observable store with `subscribe()` pattern. Components call store methods, store notifies all subscribers.

**Backend** (`src-tauri/src/state/`): Thread-safe state using `RwLock<HashMap>`. Holds workspaces, terminals, and session metadata (shell_type, cwd for persistence).

## Session Lifecycle

1. **Create**: App sends `CreateSession` + `Attach` to daemon via named pipe
2. **Running**: Daemon owns PTY + godly-vt parser, streams output events to attached client
3. **App close**: App sends `Detach` for all sessions, saves layout
4. **App reopen**: Loads layout, checks daemon for live sessions via `ListSessions`
5. **Reattach**: If session alive → `Attach` (ring buffer replays missed output into godly-vt)
6. **Fallback**: If session dead → create fresh terminal with saved CWD + load scrollback
7. **Idle**: Daemon self-terminates after 5min with no sessions and no clients

## Persistence

Three persistence mechanisms in `src-tauri/src/persistence/`:
- **layout.rs** - Workspace/terminal metadata saved on exit (reads from session_metadata)
- **scrollback.rs** - Terminal buffer content per-session (5MB limit)
- **autosave.rs** - Background thread saves every 30s if dirty

Data stored via `tauri-plugin-store` in app data directory.

## Component Structure

```
App.ts           - Root: manages layout, keyboard shortcuts, reconnection logic
├── WorkspaceSidebar.ts  - Workspace list, new workspace dialog, drop target
├── TabBar.ts            - Terminal tabs with drag-drop reordering
└── TerminalPane.ts      - Canvas2D terminal pane (delegates to TerminalRenderer)
    └── TerminalRenderer.ts - Canvas2D rendering of godly-vt grid snapshots
```

## Terminal State Flow

User input → `terminalService.writeToTerminal()` → IPC → DaemonClient → named pipe → daemon → PTY
Shell output → daemon reader thread → ring buffer + godly-vt parser → named pipe → DaemonBridge → `terminal-output` event → `TerminalPane.fetchAndRenderSnapshot()` → Canvas2D paint
