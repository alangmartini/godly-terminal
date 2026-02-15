# Instance Configuration Reference

## Environment Variables

### GODLY_INSTANCE

Primary mechanism for daemon isolation. Set before launching Godly Terminal or its daemon.

- **Type:** string (alphanumeric, hyphens allowed)
- **Effect:** Appends `-<value>` to all instance-scoped resource names
- **Propagation:** The Tauri client forwards this as `--instance <value>` when spawning the daemon. The daemon's `main()` parses `--instance` and sets `GODLY_INSTANCE` internally.
- **Empty string:** Treated as unset (no suffix applied)

Examples:
```powershell
$env:GODLY_INSTANCE = "agent-1"    # suffix: -agent-1
$env:GODLY_INSTANCE = "test"       # suffix: -test
$env:GODLY_INSTANCE = ""           # no suffix (same as unset)
```

### GODLY_PIPE_NAME

Overrides the daemon pipe path entirely. Takes precedence over `GODLY_INSTANCE`.

- **Type:** full Windows named pipe path
- **Default:** `\\.\pipe\godly-terminal-daemon` (plus instance suffix if set)
- **Use case:** Tests that need a unique pipe name without setting GODLY_INSTANCE

Example:
```powershell
$env:GODLY_PIPE_NAME = "\\.\pipe\my-custom-daemon"
```

### GODLY_MCP_PIPE_NAME

Overrides the MCP pipe path entirely. Takes precedence over `GODLY_INSTANCE`.

- **Type:** full Windows named pipe path
- **Default:** `\\.\pipe\godly-terminal-mcp` (plus instance suffix if set)

Example:
```powershell
$env:GODLY_MCP_PIPE_NAME = "\\.\pipe\my-custom-mcp"
```

## Resource Isolation Matrix

| Resource | Derived From | Location |
|---|---|---|
| Daemon named pipe | `GODLY_PIPE_NAME` or `PIPE_NAME + instance_suffix()` | Kernel namespace |
| MCP named pipe | `GODLY_MCP_PIPE_NAME` or `MCP_PIPE_NAME + instance_suffix()` | Kernel namespace |
| PID file | `%APPDATA%/com.godly.terminal<suffix>/godly-daemon.pid` | Filesystem |
| Singleton mutex | `godly-daemon-lock-<pipe-basename>` | Kernel namespace |
| Debug log | `%APPDATA%/com.godly.terminal<suffix>/godly-daemon.log` | Filesystem |

## Daemon CLI Arguments

The daemon binary accepts:

```
godly-daemon [--instance <name>]
```

When `--instance` is provided, the daemon sets `GODLY_INSTANCE` in its own process environment before reading any protocol configuration. This ensures all pipe name and path derivations use the correct suffix.

This is necessary because WMI-launched processes (used when the Job Object denies `CREATE_BREAKAWAY_FROM_JOB`) do not inherit the parent's environment variables.

## Patterns for Parallel Agents

### Pattern 1: One Agent per Terminal

Create a terminal, set `GODLY_INSTANCE`, then launch Claude Code inside it:

```
create_terminal(workspace_id: "...")
write_to_terminal(terminal_id: "...", data: "$env:GODLY_INSTANCE = 'agent-1'\r\n")
write_to_terminal(terminal_id: "...", data: "claude\r\n")
```

### Pattern 2: Pre-isolated App Launch

Launch multiple Godly Terminal windows from separate shells:

```powershell
# Shell 1
$env:GODLY_INSTANCE = "primary"
godly-terminal.exe

# Shell 2
$env:GODLY_INSTANCE = "secondary"
godly-terminal.exe
```

Each window gets its own daemon, sessions, and named pipes.

### Pattern 3: Test Isolation (for daemon tests)

Use `GODLY_PIPE_NAME` for maximum isolation in tests:

```rust
let pipe_name = format!(r"\\.\pipe\test-{}", uuid::Uuid::new_v4());
cmd.env("GODLY_PIPE_NAME", &pipe_name)
   .env("GODLY_NO_DETACH", "1");
```

This avoids any collision with production or other test instances.
