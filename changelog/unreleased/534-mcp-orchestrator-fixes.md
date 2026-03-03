### Fixed
- **MCP write_to_terminal timeout** — Use fire-and-forget for terminal writes in the MCP handler, matching the Tauri command handler behavior. Prevents 15-second timeout under load. (#534)
- **MCP worktree cleanup** — Automatically clean up git worktrees when terminals are closed via MCP. (#534)
- **MCP create_terminal duplicate command** — Fix command being executed twice when using the `command` parameter. (#534)
