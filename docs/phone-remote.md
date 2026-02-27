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
- Works over Cloudflare Tunnel, ngrok, or local Wi-Fi

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

- Godly Terminal (or `godly-daemon`) running
- `godly-remote` built: `cd src-tauri && cargo build -p godly-remote --release` (the setup script builds it automatically if missing)
- For remote access: [cloudflared](https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/install-and-setup/) or [ngrok](https://ngrok.com/download)

### Automated setup

The setup script handles everything — starts `godly-remote`, generates API keys, optionally starts a tunnel, and displays a QR code.

**Local only** (phone on the same Wi-Fi):

```powershell
pwsh scripts/setup-phone.ps1
```

Binds `0.0.0.0`, detects your LAN IP, and shows a QR code your phone can scan.

**With Cloudflare Tunnel** (access from anywhere):

```powershell
pwsh scripts/setup-phone.ps1 -Tunnel cloudflare -TunnelName my-tunnel -Hostname phone.example.com
```

Requires a pre-configured named tunnel:
```powershell
cloudflared tunnel login
cloudflared tunnel create my-tunnel
cloudflared tunnel route dns my-tunnel phone.example.com
```

**With ngrok** (access from anywhere):

```powershell
# Auto-assigned URL:
pwsh scripts/setup-phone.ps1 -Tunnel ngrok

# Static domain:
pwsh scripts/setup-phone.ps1 -Tunnel ngrok -NgrokDomain my-app.ngrok-free.app
```

All modes generate and persist API key + password in `%APPDATA%/com.godly.terminal/remote-config.json`, so subsequent runs reuse the same credentials.

### Personal config

For frequent use, create `scripts/setup-phone.local.ps1` (gitignored) to wrap the generic script with your preferred tunnel settings:

```powershell
# scripts/setup-phone.local.ps1
& "$PSScriptRoot\setup-phone.ps1" `
    -Tunnel cloudflare `
    -TunnelName "my-tunnel" `
    -Hostname "phone.example.com" `
    @args
```

Then just run:
```powershell
pwsh scripts/setup-phone.local.ps1
```

### Manual startup

```powershell
# 1. Start the daemon (if not already running)
godly-daemon

# 2. Set an API key (recommended for remote access)
$env:GODLY_REMOTE_API_KEY = "your-secret-key"

# 3. Start the remote server
godly-remote

# 4. Optionally start a tunnel
ngrok http 3377
# or: cloudflared tunnel run my-tunnel
```

Open `http://localhost:3377/phone` (local) or your tunnel URL `/phone` (remote).

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

- Always use an API key when exposing over a tunnel (Cloudflare, ngrok, etc.)
- The `/phone` page itself has no auth (the API key is entered in-app and stored in localStorage)
- API key can be set via `GODLY_REMOTE_API_KEY` env var or `auth.api_key` in config
- The `?api_key=` query param is supported for SSE EventSource (which can't set custom headers)
