### Fixed

- **Godly Remote shows stale terminal sessions** — fixed bug where dead terminals (PTY sessions that died but were still in persisted layout file) were returned as `alive: false` in the list_workspaces API response. Now dead terminals are excluded entirely from the response instead of being shown as dead entries.
