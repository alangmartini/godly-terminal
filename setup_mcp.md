# Godly Terminal MCP Setup

This guide explains how to connect Claude Code to Godly Terminal via MCP (Model Context Protocol), giving Claude the ability to manage your terminals and workspaces.

## Prerequisites

- Godly Terminal installed (or built from source)
- Claude Code CLI installed

## How It Works

When you open a terminal tab in Godly Terminal, the shell session receives three environment variables:

| Variable | Purpose |
|---|---|
| `GODLY_SESSION_ID` | Identifies the current terminal session |
| `GODLY_WORKSPACE_ID` | Identifies the current workspace |
| `GODLY_MCP_BINARY` | Absolute path to `godly-mcp.exe` |

Claude Code uses `godly-mcp.exe` as an MCP stdio server. The binary connects to the running Godly Terminal app via a named pipe and forwards requests.

```
Claude Code  --stdio-->  godly-mcp.exe  --named pipe-->  Godly Terminal
```

## Setup Steps

### 1. Build the MCP binary (dev only)

If you're running from source, build the binary first:

```bash
npm run build:mcp
```

For release builds, this is handled automatically by `npm run tauri build`.

### 2. Find the binary path

Open a terminal tab inside Godly Terminal and run:

```powershell
echo $env:GODLY_MCP_BINARY
```

This prints the full path to `godly-mcp.exe`. Copy it — you'll need it in the next step.

If the variable is empty, the binary wasn't found next to the Godly Terminal executable. Make sure you built it (`npm run build:mcp`) or that you're using a release build.

### 3. Configure Claude Code

Add the MCP server to your Claude Code settings. You have two options:

**Option A: Global config** (applies to all projects)

Edit `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "godly-terminal": {
      "command": "C:\\path\\to\\godly-mcp.exe"
    }
  }
}
```

Replace the path with the value from step 2.

**Option B: Project config** (applies to one project)

Create a `.mcp.json` file in your project root:

```json
{
  "mcpServers": {
    "godly-terminal": {
      "command": "C:\\path\\to\\godly-mcp.exe"
    }
  }
}
```

### 4. Verify

1. Open Godly Terminal
2. Open a terminal tab
3. Run `claude` (Claude Code CLI) inside that tab
4. Ask Claude to list your terminals — it should use the `list_terminals` tool

## Available Tools

Once connected, Claude Code gets access to these tools:

### Terminal Management

| Tool | Description | Required Args |
|---|---|---|
| `get_current_terminal` | Info about the terminal Claude is running in | — |
| `list_terminals` | List all open terminals | — |
| `create_terminal` | Open a new terminal tab | `workspace_id`, optional `cwd` |
| `close_terminal` | Close a terminal | `terminal_id` |
| `rename_terminal` | Rename a terminal tab | `terminal_id`, `name` |
| `focus_terminal` | Switch to a specific tab | `terminal_id` |
| `write_to_terminal` | Send text to another terminal | `terminal_id`, `data` |

### Workspace Management

| Tool | Description | Required Args |
|---|---|---|
| `list_workspaces` | List all workspaces | — |
| `create_workspace` | Create a new workspace | `name`, `folder_path` |
| `switch_workspace` | Switch active workspace | `workspace_id` |
| `move_terminal_to_workspace` | Move a terminal to another workspace | `terminal_id`, `workspace_id` |

## Troubleshooting

### "GODLY_SESSION_ID not set"

Claude Code is not running inside a Godly Terminal tab. The env var is injected automatically when Godly Terminal creates a shell session. Make sure you launched Claude Code from within a Godly Terminal tab, not from a separate terminal.

### MCP binary not found

- **Dev builds**: Run `npm run build:mcp` to compile `godly-mcp.exe`
- **Release builds**: The binary should be in the same directory as `godly-terminal.exe`
- Check that `GODLY_MCP_BINARY` is set: `echo $env:GODLY_MCP_BINARY`

### Connection refused / pipe error

Godly Terminal must be running. The MCP binary connects to it via the named pipe `\\.\pipe\godly-terminal-mcp`. If the app isn't open, the pipe doesn't exist and connections fail.

### Custom pipe name

If you need a different pipe name (e.g., running multiple instances), set the `GODLY_MCP_PIPE_NAME` environment variable before launching both Godly Terminal and Claude Code.
