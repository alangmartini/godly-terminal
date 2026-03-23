### Fixed

- **Workspace notification badge appearing on switch** — fixed false notification badge when switching away from a workspace. The issue occurred when non-focused terminals in the active workspace accumulated unread output counts. Now all visible terminals in the active workspace are marked as read, and `record_output` correctly checks all terminals in the active workspace rather than just the focused one.
