# Plan: Automated Phone Setup Script

## Goal
One-command setup to expose godly-remote to the internet and display a scannable QR code. User scans with phone camera -> instantly connected, zero manual config.

## Changes

### 1. Modify `phone.html` — Auto-configure from URL params
**File**: `src-tauri/remote/static/phone.html`

Add URL parameter support in the `init()` function:
- If `?key=<api_key>` is present, auto-save it to localStorage
- Strip the param from URL (clean address bar) via `history.replaceState`
- This way the QR code URL embeds the API key — zero manual setup on phone

### 2. Create `scripts/setup-phone.ps1` — Orchestration script
**File**: `scripts/setup-phone.ps1`

Steps the script performs:
1. **Check ngrok** — verify installed, offer `winget install ngrok` if missing
2. **Check godly-remote binary** — look in release/debug target dirs, prompt to build if missing
3. **Generate API key** — 24-char random alphanumeric
4. **Start godly-remote** — background process with `GODLY_REMOTE_API_KEY` env var
5. **Start ngrok** — `ngrok http 3377` as background job
6. **Get public URL** — poll `http://localhost:4040/api/tunnels` until ready
7. **Build phone URL** — `<ngrok_url>/phone?key=<api_key>`
8. **Display QR code** — use `npx qrcode-terminal <url>` (Node.js already in project)
9. **Display URL as text** — for manual copy if QR doesn't scan
10. **Wait for Ctrl+C** — cleanup: stop ngrok + godly-remote processes

### 3. Add `qrcode-terminal` as devDependency
```
npm install --save-dev qrcode-terminal
```

Tiny package (no native deps) that renders QR codes as Unicode blocks in terminal.

### 4. Add npm script shortcut
**File**: `package.json`

```json
"phone": "pwsh scripts/setup-phone.ps1"
```

So users can just run `npm run phone`.

## Flow

```
User runs: npm run phone
  |-- Checks ngrok installed
  |-- Finds/builds godly-remote binary
  |-- Generates API key: "xK9mR2..."
  |-- Starts godly-remote (port 3377)
  |-- Starts ngrok tunnel
  |-- Gets URL: https://abc123.ngrok-free.app
  |-- Builds: https://abc123.ngrok-free.app/phone?key=xK9mR2...
  |-- Displays QR code in terminal
  |-- Prints URL for manual access
  |-- Waits... (Ctrl+C to stop)

User scans QR -> phone opens URL -> API key auto-saved -> dashboard loads
```

## Files modified
- `src-tauri/remote/static/phone.html` (add URL param auto-config, ~5 lines)
- `scripts/setup-phone.ps1` (new file)
- `package.json` (add devDep + npm script)
