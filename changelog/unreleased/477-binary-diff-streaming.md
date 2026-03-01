### Added
- **Binary GridDiff streaming** — Stream binary-encoded grid diffs via `stream://` custom protocol, eliminating Tauri JSON serialization and IPC round-trips for grid snapshots. Binary payload is ~10x smaller than JSON. (#477)

### Changed
- **Adaptive diff generation rate** — Diff interval adapts to output mode: 3ms in interactive mode (typing) for minimal keypress-to-paint latency, 16ms in bulk mode (heavy output) to avoid flooding the bridge. (#477)
