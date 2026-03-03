### Fixed
- **MCP create_terminal duplicate command execution** — command is now written after waiting for the shell prompt to be ready, preventing the race condition where the command appeared to run twice (#534)
- **MCP worktree cleanup on terminal close** — closing a terminal that was created with a worktree now automatically removes the worktree in a background thread, preventing disk space accumulation (#534)
