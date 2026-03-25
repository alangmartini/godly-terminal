### Added
- **tmux compatibility shim** — New `tmux.exe` binary that intercepts tmux CLI commands and translates them to Godly Terminal operations. Enables Claude Code's Agent Teams split-pane feature to work natively in Godly Terminal without requiring a real tmux installation. Includes session lifecycle, pane management, and key sending commands.
- **TMUX environment injection** — Terminal sessions now set `$TMUX` and `$TMUX_PANE` environment variables, allowing Claude Code to auto-detect a tmux-compatible environment.
