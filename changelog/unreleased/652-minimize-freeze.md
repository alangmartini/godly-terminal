### Fixed
- **Terminal freeze after minimizing window** — When the window loses focus, all sessions are paused to prevent event backlog buildup. Sessions are resumed when the window regains focus, and grids are refreshed to show the latest state. Added early-return coalescing for redundant TerminalOutput events to prevent overwhelming the UI (#652)
