# Dead Terminal Visibility + Daemon Session Memory Leak

## Problem

When a terminal process exits (crash, `exit` command, or Claude exits), two things went wrong:

1. **No visual feedback**: The tab was silently removed from the UI. Users had no indication that a process crashed vs. being intentionally closed.
2. **Daemon memory leak**: Dead sessions stayed in the daemon's `HashMap` forever (~60MB+ per session: vt_parser, ring_buffer, output_history, blocked reader thread).

## Root Cause

The `terminal-closed` event handler in `terminal-service.ts` called `store.removeTerminal()`, which immediately removed the tab from the UI. There was no `close_terminal` invoke to tell the daemon to clean up the session resources.

## Fix

### Frontend changes

- **`store.ts`**: Added `exited?: boolean` property to the `Terminal` interface.
- **`terminal-service.ts`**: Changed `terminal-closed` handler to call `store.updateTerminal(id, { exited: true })` instead of `store.removeTerminal(id)`. Also fires `invoke('close_terminal', ...)` to free daemon resources.
- **`TabBar.ts`**: Dead tabs get a `dead` CSS class (dimmed + line-through).
- **`TerminalPane.ts`**: Added `showExitedOverlay()` method that displays a "Process exited" overlay. Keyboard input is blocked for exited terminals.
- **`App.ts`**: Wires up overlay display in `handleStateChange()`.
- **`main.css`**: Added `.tab.dead` and `.terminal-exited-overlay` styles.

### Daemon resource cleanup

The fire-and-forget `close_terminal` invoke tells the daemon to drop the session from its HashMap, freeing:
- `vt_parser` (~20MB with scrollback)
- `ring_buffer` (~16MB)
- `output_history` (variable)
- Reader thread resources

## Regression Risk

- Tab close behavior: The close button (`handleCloseTab`) still calls `store.removeTerminal()` directly, so manually closing a tab works as before.
- Tests updated to verify `exited: true` instead of removal.
