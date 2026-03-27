### Fixed
- **tmux shim not reachable from shell** — `portable_pty` reads PATH from the Windows registry, discarding the daemon's runtime PATH modification that includes the tmux shim directory. The pty-shim now explicitly prepends its binary directory to the shell's PATH on the CommandBuilder (#847)
- **Stale tmux state across app restarts** — `tmux-state.json` persisted pane mappings from previous sessions, causing `ensure_initialized` to skip re-initialization. The shim now checks that `TMUX_PANE` maps to the current `GODLY_SESSION_ID` and clears stale state (#847)
