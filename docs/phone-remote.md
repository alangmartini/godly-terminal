# Phone Remote Control

Control Godly Terminal from your phone — approve Claude Code prompts, monitor workspaces, and interact with terminals from anywhere.

## Overview

The phone remote is a mobile web UI served by `godly-remote` (the Axum HTTP server). No app install needed — open the URL in your phone's browser.

**Features:**
- Dashboard with active prompt alerts (approve/deny with one tap)
- Workspace and terminal list with live status
- Terminal text view with auto-refresh
- Quick input: `y`, `n`, Enter, Ctrl+C buttons
- Real-time SSE push notifications for prompt detection
- Works over ngrok tunnel for access from anywhere

## Architecture

```
Phone Browser ──HTTP/SSE──► godly-remote (port 3377)
                                │
                                │ Named Pipe IPC
                                ▼
                           godly-daemon (PTY sessions)
                                │
                                ▼
                           Shell processes
```

The phone UI is a single HTML file embedded in the `godly-remote` binary. No build step, no framework, no separate deployment.

## Quick Start

### Prerequisites

- `godly-daemon` running
- `godly-remote` built: `cd src-tauri && cargo build -p godly-remote --release`
- [ngrok](https://ngrok.com/download) installed (for remote access)

### One-command startup

```powershell
.\src-tauri\remote\start-phone.ps1
```

This will:
1. Start `godly-remote` on port 3377
2. Generate a random API key (printed to console)
3. Open an ngrok tunnel
4. Print the public URL

Open `<ngrok-url>/phone` on your phone, enter the API key in Settings.

### Manual startup

```powershell
# 1. Start the daemon (if not already running)
godly-daemon

# 2. Set an API key (recommended for remote access)
$env:GODLY_REMOTE_API_KEY = "your-secret-key"

# 3. Start the remote server
godly-remote

# 4. Start ngrok tunnel
ngrok http 3377
```

Open `http://localhost:3377/phone` (local) or `<ngrok-url>/phone` (remote).

## API Endpoints

All authenticated endpoints require either `X-API-Key` header or `?api_key=` query parameter.

### Workspaces

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/workspaces` | List workspaces with terminals and live status |

### Sessions

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/sessions/:id/text?lines=50` | Last N lines of terminal output as plain text |

### Prompts

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/prompts` | Scan all live sessions for active prompts |
| GET | `/api/sessions/:id/prompts` | Check single session for prompts |

### Events (SSE)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/events?api_key=...` | Server-Sent Events stream |

SSE event types:
- `prompt_detected` — new permission prompt found in a terminal
- `prompt_resolved` — prompt no longer visible (answered or scrolled away)
- `heartbeat` — keepalive every 15s

### Existing endpoints (unchanged)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/sessions` | List all daemon sessions |
| POST | `/api/sessions` | Create a new session |
| GET | `/api/sessions/:id/grid` | Full grid snapshot |
| POST | `/api/sessions/:id/write` | Write to terminal |
| GET | `/health` | Health check |

## Configuration

Add a `[phone]` section to `godly-remote.toml`:

```toml
[phone]
text_lines = 50   # Lines returned by /text endpoint (default: 50)

[monitor]
scan_rows = 10    # Bottom rows scanned for prompts (default: 10)
```

## Prompt Detection

The remote server detects Claude Code permission prompts by pattern-matching the bottom rows of terminal output. Detected patterns include:

| Pattern | Type |
|---------|------|
| `Do you want to proceed?` | yes_no_prompt |
| `Allow this action?` | tool_approval |
| `(Y)es ... (N)o` | yes_no_prompt |
| `Do you want to allow` | tool_approval |
| `Allow ... to run` | tool_approval |
| `Approve?` | tool_approval |
| `[Y/n]` | yes_no_prompt |
| `Press Enter to continue` | continue_prompt |

## Security

- Always use an API key when exposing over ngrok
- The `/phone` page itself has no auth (the API key is entered in-app and stored in localStorage)
- API key can be set via `GODLY_REMOTE_API_KEY` env var or `auth.api_key` in config
- The `?api_key=` query param is supported for SSE EventSource (which can't set custom headers)
