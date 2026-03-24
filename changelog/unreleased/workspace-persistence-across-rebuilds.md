### Fixed
- **Workspace persistence across rebuilds** — workspaces with dead PTY sessions are no longer silently dropped when daemon survives rebuild; fresh sessions are created instead to preserve workspace metadata
