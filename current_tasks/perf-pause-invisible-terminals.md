# Pause output rendering for invisible terminals

## Branch: `fix/pause-invisible-terminal-output`

## Problem

All terminal panes are mounted in the DOM regardless of visibility. Hidden tabs still receive output events, buffer chunks, and call `xterm.js write()` which runs the parser **synchronously on the main thread**. With N terminals generating output, only 1 is visible but all N run their xterm parsers, saturating the main thread.

This is the highest-impact scalability bottleneck.

## Scope

**Frontend only** — no Rust/daemon changes needed.

### Files likely modified

- `src/components/TerminalPane.ts` — add visibility-aware buffering
- `src/components/App.ts` — notify panes of visibility changes

### Approach

1. When a terminal tab becomes hidden (`setActive(false)`), stop calling `xterm.js write()`. Continue accumulating raw `Uint8Array` chunks in the output buffer.
2. When the tab becomes visible (`setActive(true)`), replay all buffered chunks in a single `write()` call, then resume normal flushing.
3. Cap the invisible buffer size (e.g., 5MB) to prevent memory growth. If exceeded, discard oldest chunks — the ring buffer in the daemon can replay on next attach if needed.
4. Consider whether `terminal.write()` on a hidden xterm instance triggers layout/reflow. If the container is `display:none`, xterm may skip rendering but still parse. Measure this — if parsing alone is cheap, the optimization may need to go deeper (skip parse entirely, replay raw bytes).

### Testing

- Write tests verifying that output chunks are buffered (not written) when terminal is inactive.
- Write tests verifying buffered chunks are flushed on activation.
- Write tests verifying buffer cap is enforced.
- Manual test: open 10 terminals, run `yes` or a build in all of them, verify UI stays responsive.

### Acceptance criteria

- Invisible terminals do NOT call `terminal.write()`.
- Switching to a previously-hidden terminal shows all missed output.
- No observable UI lag with 10+ terminals under heavy output.
