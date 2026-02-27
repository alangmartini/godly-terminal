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

# Rust check only (changed crate — NOT --workspace)
cd src-tauri && cargo check -p <crate-you-modified>

# Run Rust tests (all workspace members, requires cargo-nextest)
cd src-tauri && cargo nextest run --workspace

# Run Rust tests (fast profile — skip stress/perf tests)
cd src-tauri && cargo nextest run --workspace --profile fast

# Smart test runner (only affected crates based on git diff)
npm run test:smart

# Run TypeScript unit tests
npm test

# Run browser tests (real Chromium via Playwright)
npm run test:browser

# Run browser tests with visible browser window
npm run test:browser:headed
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

Track all bugs and investigations as **GitHub Issues**, not local docs.

### When starting a bug investigation:
1. Search existing issues: `gh issue list --search "<keywords>" --state all --limit 10`
2. If a matching closed issue exists, read it (`gh issue view N`) — the bug may have regressed
3. Create a new issue or reopen the existing one with appropriate labels (`bug`, `performance`, `daemon`, `frontend`, `mcp`, `ux`)
4. Comment on the issue with each approach tried, including what failed and why

### During investigation:
- Add a comment for each significant attempt (what you tried, result, why it failed/succeeded)
- Include relevant code snippets, test commands, and root cause analysis in comments
- Use the issue body for the canonical summary (symptom, root cause, fix)

### When resolved:
- Reference the issue in the PR description with `fixes #N` (GitHub auto-closes on merge)
- Add a final comment with regression risk assessment and relevant test commands

### Reference docs
Architecture docs, design specs, and testing guides stay in `docs/` — only investigation/bug tracking uses GitHub Issues.

## Frontend Test Tiers

Three tiers of frontend tests, each for different use cases:

| Tier | Naming | Command | Environment | Use for |
|------|--------|---------|-------------|---------|
| Unit | `*.test.ts` | `npm test` | Node/jsdom | Logic, state, services, pure functions |
| Browser | `*.browser.test.ts` | `npm run test:browser` | Real Chromium | Canvas2D rendering, real layout, pointer events |
| E2E | `e2e/*.ts` | `npm run test:e2e` | Tauri + WebdriverIO | Full app integration with daemon |

- Browser tests run in headless Chromium via Vitest Browser Mode + Playwright
- Use `npm run test:browser:headed` to see the browser window during tests
- Browser test setup auto-mocks Tauri APIs (`src/test-utils/browser-setup.ts`)
- Prefer browser tests over jsdom when testing: Canvas2D, `getBoundingClientRect`, CSS flexbox, real DOM events

## Project-Specific Workflow Notes

These extend the global CLAUDE.md workflows (bug fix, feature development):

- **Bug fixes**: Write a full test **suite** (not a single test) to reproduce the bug.
- **Features**: Write **E2E tests** (`npm run test:e2e`), not just unit tests. For Canvas2D/layout features, also write **browser tests** (`*.browser.test.ts`).
- **Performance issues**: Always write automated reproducible tests that demonstrate the problem under realistic conditions. Isolated component benchmarks are useful but insufficient — the test must exercise the real bottleneck (e.g., concurrent I/O, lock contention, IPC round-trips). See `daemon/tests/input_latency.rs` and `daemon/tests/handler_starvation.rs` for patterns.

## User-Like Testing (Post-Implementation)

After completing any feature or bug fix that has a visual/UX component, **ask the user** if they'd like you to run user-like testing via `/manual-testing <feature>`. Prefer testing on **Godly Staging** (`npm run staging:dev`) to avoid disrupting the production app.

The testing framework combines:
- **godly-terminal MCP** — `execute_js` (DOM/store inspection), `capture_screenshot` (canvas PNG), split view control
- **pyautogui-mcp** — Real OS-level mouse/keyboard/screenshot for drag-and-drop, divider resize, keyboard shortcuts

See `.claude/skills/manual-testing.md` for the full testing procedure.

## Parallel Agent Workflow

- **Task claiming**: Create `current_tasks/<branch-name>.md` with scope and files. Check for overlap first. Remove when PR merges.
- **Branch naming**: `wt-<scope>` (e.g., `wt-fix-scrollback`, `wt-feat-search`).
- **Stay in sync**: Rebase from master before opening a PR. If another agent merges first, rebase on top.
- **Scope boundaries**: Each agent owns one clearly scoped task. Avoid modifying the same files as another active agent.

## Task Board Protocol

A global `kanban-board` MCP server tracks work across all projects. Godly Terminal has its own dedicated **board/tab** to keep its tasks organized separately from other projects.

### Board access:
- **All:** Default view showing all tasks across all projects
- **godly-terminal:** Tasks specific to Godly Terminal development (this is your project board)
- Other projects have their own boards (Typesense, personal-assistant, etc.)

### Task naming convention:
- **Format:** `Action - <key-identifier>`
- **No project prefix** (board is already project-scoped for godly-terminal tasks)
- **Action verb** matches work type: `Fix`, `Add`, `Refactor`, `Investigate`, etc.
- **Key identifier:** The specific issue (component, feature name, issue #, etc.)
- **Examples:**
  - `Fix Terminal scrolling - missing text`
  - `Add binary framing - IPC messages`
  - `Investigate memory leak - Arc clones`
  - `Refactor GPU renderer - Phase 3+4`

### When starting work on a feature or bug fix:
1. Use `mcp__kanban-board__create_task` with `board_id="godly-terminal"`
2. Follow the naming convention above (status: `todo`)
3. Immediately use `mcp__kanban-board__move_task` to move it to `in_progress`

### During implementation:
- If you discover sub-tasks, create them with the same `board_id="godly-terminal"`
- If you hit a blocker, use `mcp__kanban-board__update_task` to add blocker details to the description

### When implementation is done:
- Use `mcp__kanban-board__move_task` to move the task to `validation` (awaiting review)
- Use `mcp__kanban-board__move_task` to move to `done` after validation passes

## Output Hygiene

- **Run targeted tests first**: `cargo nextest run -p godly-daemon` before the full suite.
- **Summarize failures**: State the root cause concisely, don't paste full stack traces.
- **Avoid verbose flags**: No `--verbose` or `--nocapture` unless actively debugging a specific test.
- **Incremental verification**: `cargo check` before `cargo nextest run`. One crate before all crates.

## Verification Requirements

**IMPORTANT**: CI runs full builds and tests on every PR. Locally, run only lightweight checks:

### Local checks (required before considering work complete):

1. **Cargo check** (changed crate only — NOT `--workspace`):
   ```bash
   cd src-tauri && cargo check -p <crate-you-modified>
   ```

2. **Run tests for changed crates only** (requires `cargo-nextest`):
   ```bash
   cd src-tauri && cargo nextest run -p <crate-you-modified>
   ```

   Or use the smart test runner to auto-detect affected crates:
   ```bash
   npm run test:smart
   ```

3. **Frontend unit tests** (only if you touched TS/JS):
   ```bash
   npm test
   ```

4. **Frontend browser tests** (if you touched components with Canvas2D, layout, or pointer events):
   ```bash
   npm run test:browser
   ```

5. If any step fails, fix and repeat.

### What CI handles (so you don't have to):
- `cargo check --workspace` (cross-crate type checking)
- `cargo nextest run --workspace` (full test suite, 3 daemon partitions)
- `tsc --noEmit` (TypeScript strict check)
- `npm run build` (production Vite build)
- Full release build of daemon/mcp/notify binaries

Do NOT run `cargo check --workspace`, `npm run build`, or `cargo nextest run --workspace` locally unless debugging a CI failure. Let CI catch cross-crate breakage — local checks are for fast feedback only.

### Staging verification (ask before running)

After completing a feature or bug fix, **ask the user** if they want you to build and install Godly Staging to test the change in an isolated environment:

```bash
npm run staging:build && npm run staging:install
```

This builds a fully isolated "Godly Terminal (Staging)" installation with separate pipes, app data, and daemon. Use it to verify the fix/feature works end-to-end in a real terminal before opening a PR. Do NOT run this automatically — always ask first, as it takes several minutes.

## Product Vision

Godly Terminal is built for **AI-assisted development workflows**. The primary use case is running multiple workspaces, each containing 2+ Claude Code instances (or other AI tools) working in parallel. A typical session has 10-20 concurrent terminal sessions, with only 1-2 visible at any time.

This means the critical performance axis is **not** single-terminal rendering speed — it's **multi-session efficiency**: low memory per session, fast workspace switching, intelligent resource allocation between visible and background terminals, and robust session persistence for long-running AI processes.

### Design Priorities (in order)
1. **Session persistence** — AI tool sessions are long-running and valuable; never lose them
2. **Multi-session scalability** — 20+ concurrent sessions without degradation
3. **Workspace switching speed** — instant context switch between groups of terminals
4. **Background efficiency** — minimize resources for terminals the user isn't looking at
5. **Visible terminal responsiveness** — low latency for the 1-2 terminals currently on screen

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
2. **Use `GODLY_INSTANCE`** — every test that sets `GODLY_PIPE_NAME` must also set `GODLY_INSTANCE` to isolate the shim metadata directory. Without it, the test daemon reads the production metadata dir and kills live shim processes. Use: `.env("GODLY_INSTANCE", pipe_name.trim_start_matches(r"\\.\pipe\"))`.
3. **Kill by PID, not by name** — NEVER use `taskkill /F /IM godly-daemon.exe` (kills ALL daemon processes). Use `child.kill()` for child-process daemons or `taskkill /F /PID <pid>` for detached daemons.
4. **Use `GODLY_NO_DETACH=1`** — keeps the test daemon as a child process so `child.kill()` works for cleanup.
5. **Pattern to follow** — see `handler_starvation.rs` or `memory_stress.rs` for the `DaemonFixture` pattern with proper isolation.

### Guardrail test

`daemon/tests/test_isolation_guardrail.rs` automatically scans all daemon test files for violations of these rules. It runs as part of the normal test suite and will fail if any test file:
- Uses `taskkill /IM` (process-name kill)
- Imports the production `PIPE_NAME` constant
- Spawns a daemon without `GODLY_PIPE_NAME` or `--instance` isolation
- Spawns a daemon without `GODLY_INSTANCE` (metadata directory isolation)

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

## Log File Locations

| Component | File | Location |
|-----------|------|----------|
| Daemon | `godly-daemon-debug.log` | `%APPDATA%/com.godly.terminal/` |
| Bridge | `godly-bridge-debug.log` | `%APPDATA%/com.godly.terminal/` |
| MCP | `godly-mcp.log` | Next to `godly-mcp.exe` binary |
| Frontend | `frontend.log` | `%APPDATA%/com.godly.terminal/logs/` |

All rotate to `.prev.log` at 2MB. Append-mode, survive restarts.

## MCP Testing

See [docs/mcp-testing.md](docs/mcp-testing.md) for the full MCP test procedure and known gaps.
