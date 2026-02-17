# Keyboard Input Freeze on Single Terminal Tab

## Status: RESOLVED

## Symptoms
- A single terminal tab becomes completely unresponsive to keyboard input
- Ctrl+C, Esc, and regular typing all fail
- Other tabs continue working normally
- Daemon, bridge, and session are all healthy (confirmed via logs)
- Zero Write requests reach the daemon for the frozen session

## Investigation

### Log Analysis
- **Daemon log**: Session reader is actively reading (reads and bytes increasing), zero send failures, attached=true
- **Bridge log**: Healthy pings (2-3ms), zero dropped events, zero slow writes
- **Key finding**: Zero `Write` requests in the entire daemon log — keyboard input never leaves the frontend

### Root Cause
The terminal canvas (`<canvas tabIndex=0>`) loses focus, and there is no recovery mechanism in single-pane mode.

The container's `mousedown` handler was gated behind `split-visible` class:
```typescript
this.container.addEventListener('mousedown', () => {
  if (this.container.classList.contains('split-visible')) {
    store.setActiveTerminal(this.terminalId);
  }
  // NO focus recovery for non-split mode
});
```

In single-pane mode, if focus is stolen by:
- Tab bar click (moves focus to body)
- Dialog open/close
- WebView2 native frame focus event

...the canvas never regains focus, so all `keydown` events stop reaching `handleKeyEvent`.

## Fix (PR #TBD)

### 1. Container mousedown always focuses canvas
```typescript
this.container.addEventListener('mousedown', () => {
  if (this.container.classList.contains('split-visible')) {
    store.setActiveTerminal(this.terminalId);
  }
  requestAnimationFrame(() => this.renderer.focus());
});
```

### 2. Double-tap focus in setActive()
A second `setTimeout(50ms)` focus attempt catches races where the first RAF focus is stolen by tab bar cleanup or WebView2.

### 3. Blur diagnostic logging
Canvas blur events on active panes log which element stole focus, making future focus issues easier to diagnose.

## Files Changed
- `src/components/TerminalPane.ts` — focus recovery logic
- `src/components/TerminalPane.focus-recovery.test.ts` — 8 regression tests

## Regression Risk
- LOW: The change only adds focus calls (never removes them)
- Split mode behavior unchanged (still calls `setActiveTerminal`)
- Guard on setTimeout backup prevents focusing an already-deactivated pane
