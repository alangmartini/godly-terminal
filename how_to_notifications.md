# Godly Terminal Notifications

Godly Terminal can play a chime and show a badge on the terminal tab to alert you when something finishes. There are two ways to trigger notifications from Claude Code: **CLAUDE.md instructions** (MCP tool calls) and **hooks** (CLI mode). You can use either or both.

## Option 1: Hooks (recommended)

Hooks run shell commands automatically on Claude Code events. Because the `godly-mcp` binary supports CLI mode, you can trigger notifications without any CLAUDE.md instructions — the hook fires on its own and the `GODLY_SESSION_ID` env var (set automatically in every Godly Terminal shell) tells it which tab to notify.

### Setup

Add this to your Claude Code settings file (`.claude/settings.json` in your project, or the global `~/.claude/settings.json`):

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": ".*",
        "hooks": [
          {
            "type": "command",
            "command": "godly-mcp notify -m \"Tool completed\""
          }
        ]
      }
    ]
  }
}
```

You can customize when notifications fire by changing the event and matcher:

| Event | When it fires |
|---|---|
| `PostToolUse` | After any tool call (Bash, Read, Write, etc.) |
| `PostToolUse` with `"matcher": "Bash"` | Only after Bash tool calls |
| `Stop` | When Claude finishes its response |

### CLI reference

```
godly-mcp notify                          # Notify with no message
godly-mcp notify -m "Build done"          # Notify with a message
godly-mcp notify --message "Tests passed" # Long form
godly-mcp notify --terminal-id <ID>       # Target a specific terminal tab
godly-mcp --help                          # General help
godly-mcp notify --help                   # Notify subcommand help
```

The terminal ID is resolved in this order:
1. `--terminal-id` flag (if provided)
2. `GODLY_SESSION_ID` environment variable (set automatically)

### Why hooks over CLAUDE.md?

- **Zero context window cost** — hooks run as shell commands, not as LLM tool calls
- **No CLAUDE.md needed** — nothing to add to project instructions
- **Reliable** — fires every time, not dependent on Claude remembering to call a tool

## Option 2: CLAUDE.md instructions (MCP tool calls)

This approach tells Claude to call the `notify` MCP tool directly. It works, but uses a tool call each time (which costs context window tokens).

### Setup

Add this to your project's `CLAUDE.md` (or `~/.claude/CLAUDE.md` for all projects):

```markdown
## Notifications

When you finish a long-running task (build, test suite, complex refactor),
call the `notify` MCP tool to alert the user:

- Call `mcp__godly-terminal__notify` with an optional `message` parameter
- Example: `notify` with `{"message": "Build complete"}`
```

Claude will then call the `notify` tool via MCP when it decides the task warrants an alert. You can make the instructions more or less aggressive depending on how often you want notifications.

### Related MCP tools

| Tool | Description |
|---|---|
| `notify` | Send a sound notification (uses `GODLY_SESSION_ID` automatically) |
| `set_notification_enabled` | Enable/disable notifications for a terminal or workspace |
| `get_notification_status` | Check if notifications are enabled |

## Combining both

You can use both approaches at the same time. Hooks give you guaranteed, automatic notifications on every event, while CLAUDE.md instructions let Claude send targeted notifications with descriptive messages at logical completion points.

A practical combo:

1. **Hook on `Stop`** — always notify when Claude finishes responding
2. **CLAUDE.md** — ask Claude to notify with a message on milestone completions ("Tests passing", "PR created")

```json
{
  "hooks": {
    "Stop": [
      {
        "matcher": ".*",
        "hooks": [
          {
            "type": "command",
            "command": "godly-mcp notify"
          }
        ]
      }
    ]
  }
}
```
