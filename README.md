Current state: shitly, slow and buggy.

## Phone Remote Control

Control terminals and approve Claude Code prompts from your phone. No app needed — just a browser.

```powershell
# Build and start with ngrok tunnel
cd src-tauri && cargo build -p godly-remote --release
.\src-tauri\remote\start-phone.ps1
```

Open `<ngrok-url>/phone` on your phone. See [docs/phone-remote.md](docs/phone-remote.md) for full setup.
