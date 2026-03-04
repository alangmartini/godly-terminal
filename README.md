# Godly Terminal

A Windows terminal built for AI-assisted development workflows. Run 10-20 concurrent terminal sessions across multiple workspaces, with tmux-style session persistence, a background daemon that survives app restarts, and deep Claude Code integration via MCP.

## Why Godly Terminal?

Don't. Just don't. This is a shitty vide-coded terminal that... just kind of went too far?

Modern AI coding workflows involve multiple agents working in parallel — each in its own terminal, often across different git worktrees. Godly Terminal is designed for exactly this:

- **Sessions never die.** A background daemon owns all PTY sessions. Close the app, reopen it, and every terminal is exactly where you left it — scrollback, cursor position, running processes, all intact.
- **20+ terminals without breaking a sweat.** Invisible sessions are paused (no parsing, no rendering). A global scrollback budget prevents memory bloat. You only pay for what you see.
- **AI-native.** An MCP server exposes every terminal to Claude Code. Spawn terminals, read output, send keys, orchestrate multi-agent workflows — all from Claude's tool calls.
- **Phone remote.** Approve Claude Code permission prompts from your phone while AFK. No app install — just scan a QR code.

## Features

### Terminal Essentials
- **Workspaces** — group terminals by project, switch instantly between them
- **Split panes** — horizontal and vertical splits with keyboard shortcuts
- **Tab management** — drag-drop reordering, rename with F2, process name display
- **Canvas2D renderer** — high-performance rendering backed by a custom VT parser (godly-vt) with SIMD-accelerated parsing
- **Scrollback** — 10K lines per session, persisted to disk, survives restarts
- **Zoom** — Ctrl+=/- or Ctrl+scroll
- **Themes** — Tokyo Night (default) with a theme system for customization
- **Shell support** — PowerShell, CMD, WSL (with distro selection), custom shells

### Session Persistence
- Background daemon (`godly-daemon`) manages all PTY sessions
- App close sends `Detach` — sessions keep running
- App reopen sends `Attach` — ring buffer replays missed output
- Autosave every 30s (layout, scrollback, workspace metadata)
- Dead session detection with graceful fallback to saved CWD

### AI Integration
- **MCP server** (`godly-mcp`) — full Model Context Protocol server for Claude Code
  - Create/close terminals, read grid content, send input
  - `execute_command` — run a command and capture output in one call
  - `wait_for_text` / `wait_for_idle` — wait for specific output patterns
  - `send_keys` — Ctrl+C, arrow keys, Tab, etc.
  - Multi-agent orchestration across workspaces
- **Quick Claude** — Ctrl+Shift+Enter to spawn a Claude Code session with a prompt, auto-creates a git worktree
- **Worktree mode** — auto-creates isolated git worktrees per terminal for parallel agent work
- **Idle notifications** — sound alerts when terminals go idle (AI tool finished)

### Phone Remote Control
- Control terminals from your phone browser via ngrok tunnel
- View all workspaces and live terminal output
- Send commands and approve Claude Code permission prompts
- One-tap quick buttons: `y`, `n`, `Enter`, `Ctrl+C`
- Real-time SSE alerts when prompts need attention
- Setup: `npm run phone` (generates QR code)

### Plugin System
- Install community plugins from a GitHub-based registry
- Built-in plugins: Peon Ping (notification sounds), SmolLM2 (local LLM branch naming)
- Plugin API: audio playback, terminal events, toast notifications, settings persistence

## Architecture

```
┌─────────────────┐     Named Pipe IPC      ┌───────────────────┐
│  Tauri App       │◄──────────────────────►│  godly-daemon      │
│  (GUI + IPC)     │  attach/detach          │  (background)      │
│                  │  at will                │                    │
│  TypeScript UI   │                        │  PTY Sessions      │
│  Canvas2D render │                        │  Ring Buffers      │
└─────────────────┘                         │  godly-vt Parsers  │
     │                                      └───────────────────┘
     │ Tauri events                               │
     ▼                                            │ godly-pty-shim
  Browser window                                  ▼
  (vanilla DOM + Canvas)                     Shell processes
                                             (survive app close)
```

**No terminal parsing happens in the frontend.** The daemon's godly-vt parser is the single source of truth. The frontend is a pure display layer that fetches grid snapshots over IPC.

### Crate Structure

| Crate | Purpose |
|-------|---------|
| `godly-protocol` | Shared message types and wire format |
| `godly-daemon` | Background PTY manager with session lifecycle |
| `godly-vt` | SIMD-accelerated VT100 parser (forked from vt100-rust) |
| `godly-pty-shim` | Per-session PTY wrapper for crash isolation |
| `godly-mcp` | MCP server (stdio, SSE, HTTP transports) |
| `godly-notify` | Lightweight CLI for terminal notifications (~5ms) |
| `godly-remote` | Phone remote HTTP/WebSocket server |

### Frontend Stack

TypeScript with vanilla DOM — no React, no framework. Components are plain classes that manage their own DOM subtrees. State management via an observable store with `subscribe()`. Terminal rendering uses Canvas2D (with an optional WebGL path).

## Getting Started

### Prerequisites

- **Node.js** 20+
- **Rust** stable toolchain (via `rustup`)
- **cargo-nextest** (for running tests): `cargo install cargo-nextest`
- Windows 10/11

### Development

```bash
# Install frontend dependencies
npm install

# Build the background daemon (required before first run)
npm run build:daemon

# Start development mode (Vite + Tauri hot reload)
npm run tauri dev
```

### Production Build

```bash
npm run build:daemon:release
npm run tauri build
```

The installer is output to `src-tauri/target/release/bundle/`.

### Frontend Mode

Godly Terminal now defaults to the **native Iced + wgpu frontend**. The `GODLY_FRONTEND_MODE` environment variable controls which frontend runs:

| Value | Frontend | Description |
|-------|----------|-------------|
| `native` (default) | Iced + wgpu | Native GPU-accelerated shell |
| `web` | Tauri + TypeScript + Canvas2D | Legacy web-based frontend |
| `shadow` | Headless | No UI — for testing and automation |

To switch frontends, set the variable before launching:

```bash
# Use legacy web frontend
set GODLY_FRONTEND_MODE=web

# Development shortcuts
pnpm dev:native   # Default — Iced frontend
pnpm dev:web      # Legacy Tauri + Canvas2D frontend
```

See [docs/native-frontend-rollback.md](docs/native-frontend-rollback.md) for the full rollback guide, including persistent system-wide configuration.

### Running Tests

Six test tiers target different layers of the stack:

| Tier | Command | Environment | What it tests |
|------|---------|-------------|---------------|
| **Unit** | `npm test` | Node / jsdom | Store logic, services, keyboard routing, pure functions |
| **Browser** | `npm run test:browser` | Real Chromium (Playwright) | Canvas2D rendering, pixel correctness, CSS layout, pointer events |
| **Integration** | `npm run build:daemon && npm run test:integration` | Node + real daemon | Daemon protocol, session lifecycle, IPC, Quick Claude flow |
| **E2E** | `npm run test:e2e` | Full Tauri app (WebdriverIO) | Persistence across restarts, full user workflows, input latency |
| **Daemon** | `cd src-tauri && cargo nextest run -p godly-daemon` | Isolated daemon process | Concurrency, lock contention, memory leaks, pipe saturation |
| **Crate** | `cd src-tauri && cargo nextest run -p <crate>` | Rust unit | VT parser, serialization, data structures |

```bash
# Unit tests — store logic, services, keyboard handling
npm test

# Browser tests — Canvas2D rendering, real CSS flexbox layout
npm run test:browser
npm run test:browser:headed   # with visible Chromium window

# Integration tests — daemon protocol, session I/O, Quick Claude
npm run build:daemon && npm run test:integration

# E2E tests — full app lifecycle, persistence, keyboard shortcuts
npm run test:e2e

# Daemon tests — concurrency, performance, memory
cd src-tauri && cargo nextest run -p godly-daemon

# Crate tests — VT parser, protocol serialization
cd src-tauri && cargo nextest run -p godly-vt
cd src-tauri && cargo nextest run -p godly-protocol

# Smart runner — auto-detects affected crates from git diff
npm run test:smart

# Full Rust workspace (let CI handle this — use smart runner locally)
cd src-tauri && cargo nextest run --workspace
```

**File naming conventions:**

| Pattern | Tier | Location |
|---------|------|----------|
| `*.test.ts` | Unit | `src/` |
| `*.browser.test.ts` | Browser | `src/` |
| `*.integration.test.ts` | Integration | `integration/tests/` |
| `*.e2e.ts` | E2E | `e2e/specs/` |
| `*.rs` (in `tests/`) | Daemon / Crate | `src-tauri/<crate>/tests/` |

## Keyboard Shortcuts

| Action | Shortcut |
|--------|----------|
| New terminal | `Ctrl+T` |
| Close terminal | `Ctrl+W` |
| Next tab | `Ctrl+Tab` |
| Previous tab | `Ctrl+Shift+Tab` |
| Split right | `Ctrl+\` |
| Split down | `Ctrl+Alt+\` |
| Focus other pane | `Alt+\` |
| Unsplit | `Ctrl+Shift+\` |
| Copy | `Ctrl+Shift+C` |
| Paste | `Ctrl+Shift+V` |
| Zoom in/out | `Ctrl+=` / `Ctrl+-` |
| Quick Claude | Customizable in Settings |
| Rename tab | `F2` |

All shortcuts are customizable via Settings.

## MCP Integration

Godly Terminal ships with `godly-mcp`, a Model Context Protocol server that exposes terminal operations to AI tools like Claude Code.

### Setup for Claude Code

Add to your Claude Code MCP configuration:

```json
{
  "mcpServers": {
    "godly-terminal": {
      "command": "path/to/godly-mcp.exe"
    }
  }
}
```

The MCP binary is bundled with the app at `src-tauri/target/release/godly-mcp.exe` after a production build. It supports three transports:

- **stdio** (default) — for Claude Code integration
- **SSE** (`godly-mcp sse`) — persistent HTTP server for web clients
- **HTTP** (`godly-mcp --http`) — streamable HTTP

### Available MCP Tools

- `list_terminals` / `get_current_terminal` — discover terminal sessions
- `create_terminal` / `close_terminal` — manage terminal lifecycle
- `write_to_terminal` / `send_keys` — send input and key sequences
- `execute_command` — run a command and capture output (single round-trip)
- `read_terminal` / `read_grid` — read terminal buffer or visible screen
- `wait_for_idle` / `wait_for_text` — wait for output patterns
- `resize_terminal` — change terminal dimensions
- Workspace management tools

## Phone Remote

Control your terminals from any phone browser:

```bash
npm run phone
```

Scan the QR code — no app install needed. See [docs/phone-remote.md](docs/phone-remote.md) for full documentation.

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture Review](docs/architecture-review.md) | System design overview |
| [Plugin Development](docs/plugin-development.md) | Building plugins |
| [Phone Remote](docs/phone-remote.md) | Mobile terminal control |
| [MCP Testing](docs/mcp-testing.md) | MCP server test procedure |
| [Ultrafast I/O](docs/ultrafast-io-architecture.md) | I/O pipeline design |
| [Quick Claude](docs/quick-claude.md) | Quick Claude feature guide |

## License

[Business Source License 1.1](LICENSE)

- Non-production use is permitted (development, testing, personal use)
- Converts to Apache License 2.0 on **2031-02-07**
- Production use requires a commercial license until the change date
