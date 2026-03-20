### Fixed
- **Shift+Tab escape sequence** — Shift+Tab now correctly sends the reverse-tab escape sequence `ESC[Z` to the PTY instead of a bare tab character, fixing support in terminal applications like Claude Code that rely on backtab
