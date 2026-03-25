### Fixed
- **Terminal sessions lost after crash** — daemon no longer kills shim processes on exit, allowing the next daemon instance to recover surviving sessions via `reconnect_surviving_shims()` (#794)
