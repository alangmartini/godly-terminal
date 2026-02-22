Current state: shitly, slow and buggy.

## Phone Remote Control

Control terminals and approve Claude Code prompts from your phone. No app needed — just a browser.

### Quick Start

```powershell
npm run phone
```

That's it. The script will:
1. Check ngrok is installed (with install instructions if not)
2. Build `godly-remote` if needed
3. Generate a secure API key
4. Start the remote server + ngrok tunnel
5. Display a **QR code** in your terminal

Scan the QR code with your phone camera — you're connected instantly. The API key is embedded in the URL, so there's no manual setup on the phone.

### Prerequisites

- [ngrok](https://ngrok.com/download) — install with `winget install ngrok.ngrok`, then run `ngrok config add-authtoken <your-token>` once
- Godly Terminal must be running (so the daemon is active)

### What You Can Do From Your Phone

- View all workspaces and terminal sessions
- See live terminal output (plain text, auto-refreshing)
- Send commands and input to any session
- Approve/deny Claude Code permission prompts with one tap
- Quick buttons: `y`, `n`, `Enter`, `Ctrl+C`
- Real-time SSE alerts when prompts need attention

### Manual Setup

If you prefer to configure things manually:

```powershell
# Build the binary
cd src-tauri && cargo build -p godly-remote --release

# Start with custom port and API key
$env:GODLY_REMOTE_API_KEY = "your-secret-key"
$env:GODLY_REMOTE_PORT = 3377
.\target\release\godly-remote.exe

# In another terminal, start ngrok
ngrok http 3377

# Open on phone: https://<ngrok-url>/phone
# Enter your API key in Settings
```

### Local Access (Same Network)

If your phone is on the same WiFi, you can skip ngrok entirely:

```
http://<your-pc-ip>:3377/phone
```

See [docs/phone-remote.md](docs/phone-remote.md) for full documentation.
