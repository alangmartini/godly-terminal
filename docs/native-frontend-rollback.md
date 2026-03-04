# Native Frontend Rollback Guide

## Overview

As of Phase 5, Godly Terminal defaults to the native Iced frontend. The legacy Tauri + TypeScript
frontend is preserved as a fallback for at least 2 release cycles.

## Rollback Procedure

### Quick rollback (per-session)

Set the environment variable before launching:

```
set GODLY_FRONTEND_MODE=web
godly-terminal.exe
```

Or in PowerShell:

```powershell
$env:GODLY_FRONTEND_MODE = "web"
& "godly-terminal.exe"
```

### Persistent rollback (system-wide)

Add `GODLY_FRONTEND_MODE=web` as a system or user environment variable:

1. Open **System Properties** > **Environment Variables**
2. Add a new User variable: `GODLY_FRONTEND_MODE` = `web`
3. Restart Godly Terminal

### Development rollback

Use the web dev server:

```bash
pnpm dev:web    # Launches Tauri + TypeScript frontend
```

## When to rollback

- If the native frontend crashes on startup
- If rendering is broken on your GPU/driver combination
- If a specific feature is missing from the native frontend
- If you experience input issues (IME, dead keys)

## Reporting issues

If you need to roll back, please file an issue at the project repository describing:
- What went wrong (crash, rendering issue, etc.)
- Your GPU model and driver version (if rendering-related)
- Steps to reproduce

This helps us reach 100% parity and remove the web fallback in a future release.
