### Fixed
- **Diagnostic log truncation** — Diagnostic logs now append instead of truncating on restart, preserving crash evidence. Logs rotate to `.prev.log` when exceeding 2MB.
