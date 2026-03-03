### Added
- **Font metrics auto-detection** — heuristic-based `FontMetrics` struct replaces hardcoded 9x18 cell sizes

### Changed
- **Terminal canvas refactor** — `TerminalCanvas` now carries grid data on the struct instead of in internal State, enabling per-terminal rendering
