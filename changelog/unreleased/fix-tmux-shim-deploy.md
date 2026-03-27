### Fixed
- **tmux shim not deployed** — `tmux.exe` was missing from the build pipeline and installer, causing Claude Code's Agent Teams to fail with "Could not determine current tmux pane/window". Added the shim to `production-build.ps1`, WiX manifest, and `unlock-binaries.js`.
- **TMUX env vars set without shim** — The daemon unconditionally set `TMUX` and `TMUX_PANE` even when `tmux.exe` didn't exist, tricking Claude Code into trying tmux commands that would fail. Now only sets these when the shim binary is present.
- **Missing tmux command handlers** — Added no-op handlers for cosmetic tmux commands (`set-option`, `select-layout`, `resize-pane`, etc.) that Claude Code's TmuxBackend calls, preventing spurious error exit codes.
- **CLI parsing for `-l` and `-T` flags** — Added size (`-l`) and title (`-T`) to the tmux shim's value flags so `split-window -l 70%` and `select-pane -T name` parse correctly.
