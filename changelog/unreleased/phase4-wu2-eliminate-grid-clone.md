### Changed
- **Eliminate grid clone in render path** — Use borrowed references instead of cloning RichGridData per pane per frame, reducing memory pressure and CPU cost.
