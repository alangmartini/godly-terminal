### Fixed
- **Tab switching renders wrong content** — switching back to a previous tab that wasn't in the active layout now properly updates the layout and fetches the terminal grid, fixing the display mismatch (#644)
- **Closing tab causes "Session not found"** — closing a terminal that was the only leaf in the layout no longer leaves a stale reference; the layout is updated to show the next available terminal (#644)
