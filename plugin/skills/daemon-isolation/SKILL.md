---
name: daemon-isolation
description: This skill should be used when the user asks to "isolate a daemon", "create an isolated terminal session", "run a separate daemon", "use GODLY_INSTANCE", "spin up a separate godly terminal", or mentions running multiple Godly Terminal instances that should not share a daemon.
version: 0.1.0
---

# Daemon Isolation

Godly Terminal supports running multiple fully independent instances, each with its own background daemon, named pipes, PID file, and mutex lock. Isolation is controlled via the `GODLY_INSTANCE` environment variable.

## When to Use Daemon Isolation

- Launching parallel Claude Code agents that each need their own terminal session manager
- Running a test/dev instance alongside production
- Spawning isolated terminal environments that survive independently

## How It Works

Setting `GODLY_INSTANCE` before launching Godly Terminal (or its daemon) causes all instance-scoped resources to use a suffixed name:

| Resource | Default | With `GODLY_INSTANCE=work` |
|---|---|---|
| Daemon pipe | `\\.\pipe\godly-terminal-daemon` | `\\.\pipe\godly-terminal-daemon-work` |
| MCP pipe | `\\.\pipe\godly-terminal-mcp` | `\\.\pipe\godly-terminal-mcp-work` |
| PID directory | `%APPDATA%\com.godly.terminal\` | `%APPDATA%\com.godly.terminal-work\` |
| Mutex lock | `godly-daemon-lock-godly-terminal-daemon` | `godly-daemon-lock-godly-terminal-daemon-work` |

The daemon receives the instance name via `--instance <name>` CLI arg (required because WMI-launched processes do not inherit env vars). The Tauri client forwards `GODLY_INSTANCE` as `--instance` automatically.

## Launching an Isolated Session via MCP

To spin up a terminal that runs under an isolated daemon, use the `create_terminal` MCP tool after setting the instance env var in the shell:

```powershell
# Step 1: Create a terminal in the current workspace
create_terminal(workspace_id: "<id>")

# Step 2: Write the GODLY_INSTANCE export into that terminal
write_to_terminal(terminal_id: "<new_id>", data: "$env:GODLY_INSTANCE = 'agent-1'\r\n")
```

Any Godly Terminal or daemon launched from that shell inherits the isolated instance name.

## Launching an Isolated Instance Directly

To start an entirely separate Godly Terminal app with its own daemon:

```powershell
$env:GODLY_INSTANCE = "work"
npm run tauri dev   # or launch the built binary
```

## Environment Variable Precedence

Three env vars control pipe names, in order of precedence:

1. **`GODLY_PIPE_NAME`** / **`GODLY_MCP_PIPE_NAME`** — explicit full pipe path override (highest priority, used mainly in tests)
2. **`GODLY_INSTANCE`** — appends `-<name>` suffix to default pipe paths
3. Default constants — `\\.\pipe\godly-terminal-daemon` and `\\.\pipe\godly-terminal-mcp`

## Important Constraints

- **Persistence is not namespaced.** Layout, scrollback, and store data are shared across instances. Two instances writing to the same store keys can conflict.
- **The daemon self-terminates** after 5 minutes with no sessions and no clients. Each isolated daemon has its own idle timer.
- **Singleton enforcement** uses a Windows named mutex derived from the pipe name. Each `GODLY_INSTANCE` value gets its own mutex, so multiple daemons can coexist safely.

## Additional Resources

### Reference Files

For detailed configuration options and low-level override examples:
- **`references/instance-config.md`** — Full env var reference and advanced override patterns
