# Godly Terminal

A Windows terminal built for AI-assisted development workflows. Run 10-20 concurrent terminal sessions across multiple workspaces, with tmux-style session persistence via a background daemon and deep Claude Code integration via MCP.

## Key Features

- **Session persistence** — a background daemon owns all PTY sessions. Close the app, reopen it, and every terminal is exactly where you left it.
- **Workspaces & splits** — group terminals by project, split panes horizontally/vertically, drag-drop tab reordering.
- **MCP server** — `godly-mcp` exposes every terminal to Claude Code: spawn terminals, read output, send keys, orchestrate multi-agent workflows.
- **Quick Claude** — spawn a Claude Code session with a prompt, auto-creates a git worktree for parallel agent work.
- **Phone remote** — approve Claude Code permission prompts from your phone. No app install — just scan a QR code (`pnpm phone`).
- **Plugin system** — install community plugins from a GitHub-based registry.
- **Canvas2D renderer** — backed by a custom SIMD-accelerated VT parser (godly-vt).

## Getting Started

Download the latest installer from [Releases](https://github.com/alangmartini/godly-terminal/releases). Windows 10/11 required.

## Building from Source

### Prerequisites

- **Node.js** 20+ and **pnpm** (frontend toolchain only)
- **Rust** stable toolchain (via `rustup`)
- **cargo-nextest**: `cargo install cargo-nextest`

### Development

```bash
pnpm install
pnpm build:daemon        # required before first run
pnpm tauri dev
```

### Production Build

```bash
pnpm build:daemon:release
pnpm tauri build
```

The installer is output to `src-tauri/target/release/bundle/`.

## Testing

```bash
pnpm test                 # unit tests (Vitest + jsdom)
pnpm test:browser         # browser tests (real Chromium via Playwright)
pnpm test:integration     # integration tests (real daemon, requires pnpm build:daemon)
pnpm test:e2e             # E2E tests (full Tauri app via WebdriverIO)
pnpm test:smart           # auto-detects affected crates from git diff

cd src-tauri && cargo nextest run -p godly-daemon   # daemon tests
cd src-tauri && cargo nextest run -p godly-vt       # VT parser tests
```

## MCP Setup

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

The MCP binary is bundled at `src-tauri/target/release/godly-mcp.exe` after a production build. Supports stdio (default), SSE, and HTTP transports.

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

No terminal parsing happens in the frontend. The daemon's godly-vt parser is the single source of truth. The frontend is a pure display layer that fetches grid snapshots over IPC.

### Crate Structure

| Crate | Purpose |
|-------|---------|
| `godly-protocol` | Shared message types and wire format |
| `godly-daemon` | Background PTY manager with session lifecycle |
| `godly-vt` | SIMD-accelerated VT100 parser (forked from vt100-rust) |
| `godly-pty-shim` | Per-session PTY wrapper for crash isolation |
| `godly-mcp` | MCP server (stdio, SSE, HTTP transports) |
| `godly-notify` | Lightweight CLI for terminal notifications |
| `godly-remote` | Phone remote HTTP/WebSocket server |

## License

[Business Source License 1.1](LICENSE) — non-production use permitted. Converts to Apache License 2.0 on **2031-02-07**.
