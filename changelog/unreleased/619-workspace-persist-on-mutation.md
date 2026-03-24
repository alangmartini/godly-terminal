### Fixed
- **Workspace persistence crash safety** — session file writes are now atomic (write to temp + rename) with automatic backup recovery, preventing workspace loss from file corruption during crashes (#619)
- **Workspace mutations persisted immediately** — workspace create, delete, and rename now write session state to disk instantly instead of waiting for the 60-second autosave (#619)

### Tests
- Added 3 regression tests for workspace mutation persistence across crash recovery
- Added 3 tests for atomic write flow, backup recovery from corruption, and truncated JSON recovery
