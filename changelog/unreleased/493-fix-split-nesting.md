### Fixed

- **Split replaces existing layout instead of nesting** — Triggering a split on a pane already in a split view now correctly creates a nested split instead of replacing the existing layout. The fix restores suspended layout trees in `splitTerminalAt()` so the new split is nested into the existing tree. (PR #494)
