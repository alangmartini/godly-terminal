### Fixed
- **Scrollback dirty-flag row-index mismatch** — diff snapshots no longer send garbled data when scrolled into history; `set_scrollback()` now marks all rows dirty and diff extraction forces full repaint when scrollback offset is active (#445)
