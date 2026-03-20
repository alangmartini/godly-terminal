### Changed
- **MCP server is now read-only** — removed 56 state-changing tools (close_terminal, delete_workspace, rename_terminal, etc.) to prevent interference between concurrent Claude Code agents. Only 33 read-only tools remain for inspecting workspace/terminal state, running commands, and reading output.
