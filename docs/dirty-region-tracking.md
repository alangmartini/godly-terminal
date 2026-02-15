# Dirty-Region Tracking

## Summary

Implemented row-level dirty tracking to avoid full-grid serialization on every keystroke. Previously, every `terminal-output` event triggered a full `ReadRichGrid` IPC call, serializing all 3,600+ cells (30x120 grid) as JSON. Now the frontend caches the last full snapshot and fetches only changed rows via a new `ReadRichGridDiff` endpoint.

## Changes

### godly-vt (terminal state engine)
- Added `dirty_rows: Vec<bool>` field to `Grid` struct
- `take_dirty_rows()` returns dirty flags and clears them (consumer pattern)
- `has_dirty_rows()` checks if any row needs update
- All grid mutation paths mark affected rows dirty:
  - Single-cell writes: `drawing_cell_mut()`, `current_row_mut()` mark only the affected row
  - Bulk operations (scroll, resize, clear, insert/delete lines): `mark_all_dirty()`
  - Erase forward/backward: marks the exact range of affected rows

### godly-protocol
- Added `RichGridDiff` type: carries only dirty rows + cursor + metadata
- Added `ReadRichGridDiff` request/response variants

### godly-daemon
- Added `read_rich_grid_diff()` to Session: takes dirty flags, builds diff
- Full repaint fallback when >=50% of rows are dirty (avoids overhead of many small row transfers)

### Tauri app
- Added `get_grid_snapshot_diff` IPC command

### Frontend (TerminalPane.ts)
- Caches last full `RichGridData` snapshot
- On subsequent output events, fetches `RichGridDiff` and merges dirty rows into cache
- Cache invalidated on scroll (viewport change) and resize (dimension change)
- Falls back to full snapshot when cache is null

## Test Coverage

20 integration tests in `godly-vt/tests/dirty_tracking.rs`:
- Creation state, take/clear cycle
- Single character writes dirty only one row
- Targeted row writes
- Scroll up/down marks all dirty
- Clear screen marks all dirty
- Erase forward/backward marks correct ranges
- Insert/delete lines marks all dirty
- Resize (grow and shrink) marks all dirty
- Alternate screen switch marks all dirty
- Multi-cycle dirty flag persistence
