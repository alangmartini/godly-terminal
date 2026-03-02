### Fixed

- **Split view 2x2 resize now affects only adjacent panes** — In a 4-way split, dragging a horizontal divider previously resized all 4 panes due to the binary tree having a single shared H-divider. Added `GridNode` type with 4 independent ratios and dividers, auto-promoted from 2x2 patterns. Each divider now controls exactly 2 panes. (PR #524)

### Tests

- **Browser tests for 2x2 grid resize** — Verifies each of the 4 grid dividers independently affects only its 2 adjacent panes.
- **Unit tests for GridNode** — Covers `maybePromoteToGrid`, `updateGridRatioAtPath`, grid cases for all tree utility functions.
- **Rust tests for Grid variant** — 22 tests covering serde roundtrip, adjacency, removal/collapse, pruning, and backward compatibility.
