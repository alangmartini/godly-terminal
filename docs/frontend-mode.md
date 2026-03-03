# Frontend Mode

Godly Terminal supports multiple frontend implementations that share the same
daemon and protocol layer.

## Modes

| Mode | Env Value | Description |
|------|-----------|-------------|
| **Web** | `web` (default) | Tauri + TypeScript + Canvas2D. The current production frontend. |
| **Native** | `native` | Iced + wgpu. The migration target (Phase 1+). |
| **Shadow** | `shadow` | Headless mode for contract testing. No rendering. |

## Configuration

Set the `GODLY_FRONTEND_MODE` environment variable before launching:

```bash
# Default (Web/Tauri)
GODLY_FRONTEND_MODE=web

# Native (Iced + wgpu)
GODLY_FRONTEND_MODE=native

# Shadow (headless testing)
GODLY_FRONTEND_MODE=shadow
```

In Rust code, use `godly_protocol::frontend_mode()` to read the current mode.

## Cargo Feature

The `native-frontend` feature flag in `src-tauri/Cargo.toml` can be used for
conditional compilation when both frontends coexist during the migration.

## Architecture

Both frontends communicate with the daemon over the same named pipe IPC protocol.
The protocol contract is frozen at v1 — see `docs/frontend_contract_v1.md`.

```
                  ┌──────────────┐
                  │  godly-daemon │
                  └──────┬───────┘
                         │ Named Pipe IPC
              ┌──────────┴──────────┐
              │                     │
    ┌─────────┴──────────┐  ┌──────┴─────────┐
    │  Tauri App (Web)   │  │  Iced (Native)  │
    │  Canvas2D renderer │  │  wgpu renderer  │
    └────────────────────┘  └────────────────┘
```
