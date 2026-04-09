# Godly Terminal

A Windows terminal built for AI-assisted development workflows. Run 10-20 concurrent terminal sessions across multiple workspaces, with tmux-style session persistence via a background daemon and deep Claude Code integration via MCP.

## Key Features

- **Session persistence** — a background daemon owns all PTY sessions. Close the app, reopen it, and every terminal is exactly where you left it.
- **Workspaces & splits** — group terminals by project, split panes horizontally/vertically, drag-drop tab reordering.
- **MCP server** — `godly-mcp` exposes every terminal to Claude Code: spawn terminals, read output, send keys, orchestrate multi-agent workflows.
- **Quick Claude** — spawn a Claude Code session with a prompt, auto-creates a git worktree for parallel agent work.
- **Phone remote** — approve Claude Code permission prompts from your phone. No app install — just scan a QR code.
- **Plugin system** — install community plugins from a GitHub-based registry.
- **GPU-accelerated renderer** — Iced + wgpu frontend backed by a custom SIMD-accelerated VT parser (godly-vt).

## Getting Started

Download the latest installer from [Releases](https://github.com/alangmartini/godly-terminal/releases). Windows 10/11 required.

## Building from Source

### Prerequisites

- **Rust** stable toolchain (via `rustup`)
- **cargo-nextest**: `cargo install cargo-nextest`

### Development

```bash
cd src-tauri && cargo build -p godly-daemon && cargo run -p godly-iced-shell
```

### Production Build

```bash
cd src-tauri && cargo build --release -p godly-daemon -p godly-iced-shell -p godly-mcp -p godly-notify -p godly-pty-shim -p godly-remote
```

## Testing

```bash
cd src-tauri && cargo nextest run -p godly-daemon      # daemon tests
cd src-tauri && cargo nextest run -p godly-vt           # VT parser tests
cd src-tauri && cargo nextest run -p godly-parity-harness  # contract tests
cd src-tauri && cargo nextest run --workspace           # everything
```

For native-vs-web visual parity work, see [`docs/omx-visual-parity-workflow.md`](docs/omx-visual-parity-workflow.md) and use [`omx-visual-parity-loop.ps1`](omx-visual-parity-loop.ps1) or [`scripts/start-omx-parity-team.ps1`](scripts/start-omx-parity-team.ps1).

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

The MCP binary is at `src-tauri/target/release/godly-mcp.exe` after a release build. Supports stdio (default), SSE, and HTTP transports.

## Architecture

```
┌─────────────────┐     Named Pipe IPC      ┌───────────────────┐
│  Iced + wgpu     │◄──────────────────────►│  godly-daemon      │
│  (native GUI)    │  attach/detach          │  (background)      │
│                  │  at will                │                    │
│  godly-iced-shell│                        │  PTY Sessions      │
│  GPU rendering   │                        │  Ring Buffers      │
└─────────────────┘                         │  godly-vt Parsers  │
                                            └───────────────────┘
                                                  │
                                                  │ godly-pty-shim
                                                  ▼
                                             Shell processes
                                             (survive app close)
```

No terminal parsing happens in the frontend. The daemon's godly-vt parser is the single source of truth. The frontend fetches grid snapshots over IPC.

### Crate Structure

| Crate | Purpose |
|-------|---------|
| `godly-iced-shell` | Native Iced + wgpu GUI application |
| `godly-app-adapter` | Daemon client, MCP pipe server, clipboard, sound, shortcuts |
| `godly-terminal-surface` | wgpu terminal renderer with font metrics |
| `godly-features-shell` | Workspace/tab/layout state reducers |
| `godly-protocol` | Shared message types and wire format |
| `godly-daemon` | Background PTY manager with session lifecycle |
| `godly-vt` | SIMD-accelerated VT100 parser (forked from vt100-rust) |
| `godly-pty-shim` | Per-session PTY wrapper for crash isolation |
| `godly-mcp` | MCP server (stdio, SSE, HTTP transports) |
| `godly-notify` | Lightweight CLI for terminal notifications |
| `godly-remote` | Phone remote HTTP/WebSocket server |

## License

[Business Source License 1.1](LICENSE) — non-production use permitted. Converts to Apache License 2.0 on **2031-02-07**.
