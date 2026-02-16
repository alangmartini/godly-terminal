# Bug: Terminal content erased when switching tabs

## Problem

When switching from one terminal to another in the same workspace, the content of the previously-active terminal is completely erased. Switching back shows a blank terminal.

## Root Cause

Two-part failure:

### 1. Frontend sends degenerate resize (trigger)

When switching tabs, the hidden pane's CSS changes to `display: none` (0x0 dimensions). The `ResizeObserver` fires for the hidden pane, calling `fit()`. Inside `fit()`:

1. `getGridSize()` reads `getBoundingClientRect()` on the hidden container -> `{width: 0, height: 0}`
2. Grid size is computed as `{rows: 1, cols: 1}` due to `Math.max(1, Math.floor(0 / cellSize))`
3. `resizeTerminal(terminalId, 1, 1)` is sent to the daemon

### 2. Grid truncation destroys content (data loss)

When the daemon receives `Resize(1, 1)`, `grid.set_size()` physically truncates:

- `row.resize(1, ...)` — each row truncated to 1 column
- `rows.resize(1, ...)` — all rows except the first are dropped

After the round-trip `24x80 -> 1x1 -> 24x80`, only the letter "L" (first character of the first line) survives.

## Fix

Added a visibility guard at the top of `TerminalPane.fit()`:

```typescript
if (!this.container.offsetWidth || !this.container.offsetHeight) {
  return;
}
```

`offsetWidth` and `offsetHeight` are 0 for elements with `display: none`, which catches exactly the hidden-pane scenario without affecting normal resize behavior.

## Files Changed

- `src/components/TerminalPane.ts` — added visibility guard in `fit()`

## Tests

- `src/components/TerminalPane.tab-switch.test.ts` — 5 frontend tests verifying the guard
- `src-tauri/godly-vt/tests/tab_switch_resize.rs` — 5 Rust tests documenting grid truncation behavior
