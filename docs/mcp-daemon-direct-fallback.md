# godly-mcp Daemon Direct Fallback

## Status: Implemented

## Problem
When the Tauri app crashes, godly-mcp loses all terminal control even though the daemon (and all PTY sessions) survive. Claude Code can't manage terminals until the app restarts.

## Solution
Introduced a `Backend` trait abstraction in godly-mcp with two implementations:

- **AppBackend**: Routes requests through Tauri app via MCP pipe (full functionality)
- **DaemonDirectBackend**: Talks directly to daemon pipe (subset of tools)

### Connection State Machine
```
startup → try MCP pipe → AppBackend
              ↓ no
         try daemon pipe → DaemonDirectBackend
              ↓ no
            EXIT(1)

mid-session pipe error → try reconnect (MCP first, then daemon)
daemon-direct mode → each request probes MCP pipe → auto-upgrade to AppBackend
```

### Tool Classification in Fallback Mode

**Works (10 tools):** `list_terminals`, `get_current_terminal`, `create_terminal`, `close_terminal`, `write_to_terminal`, `read_terminal`, `resize_terminal`, `wait_for_idle`, `wait_for_text`, `ping`

**Returns clear error (13 tools):** `list_workspaces`, `create_workspace`, `delete_workspace`, `switch_workspace`, `get_active_workspace`, `get_active_terminal`, `focus_terminal`, `rename_terminal`, `move_terminal_to_workspace`, `notify`, `set_notification_enabled`, `get_notification_status`, `remove_worktree`

## Files Changed
- `protocol/src/ansi.rs` — Added shared `truncate_output()` function
- `src/mcp_server/handler.rs` — Delegates to `godly_protocol::ansi::truncate_output`
- `mcp/src/backend.rs` — NEW: `Backend` trait
- `mcp/src/app_backend.rs` — NEW: Wraps `McpPipeClient`
- `mcp/src/daemon_direct.rs` — NEW: Direct daemon communication + request translation
- `mcp/src/tools.rs` — `call_tool` takes `&mut dyn Backend` instead of `&mut McpPipeClient`
- `mcp/src/main.rs` — Connection strategy, failover, auto-reconnect
- `mcp/Cargo.toml` — Added `uuid` dependency

## Testing
- All Rust unit tests pass (protocol, daemon, app)
- All frontend tests pass (197 vitest)
- Production build succeeds
- Manual testing: Start app → MCP works normally → Kill app → fallback activates → Restart app → auto-reconnect
