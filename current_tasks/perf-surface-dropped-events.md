# Surface dropped events and enable recovery

## Branch: `fix/dropped-event-recovery`

## Problem

The emitter channel in `bridge.rs` has a 4096-slot bounded buffer. When full (under sustained heavy output from many terminals), events are silently dropped — only a counter is incremented and logged every 30 seconds. The user sees missing terminal output with no indication that data was lost. The lost data is not recoverable because it was already forwarded past the daemon's ring buffer.

## Scope

**Rust bridge + frontend UI.**

### Files likely modified

- `src-tauri/src/daemon_client/bridge.rs` — emit drop notifications, track per-terminal drop counts
- `src/services/terminal-service.ts` — listen for drop events
- `src/components/TerminalPane.ts` — show indicator when output was lost
- Possibly `src/components/TerminalPane.css` or equivalent styles

### Approach

1. **Track drops per terminal**: When `try_send()` fails in the bridge, record which `session_id` was affected and how many bytes were dropped.
2. **Emit a drop notification event**: After N drops (or every 100ms during a drop burst), emit a `terminal-output-dropped` event with `{ terminal_id, bytes_dropped }`.
3. **Frontend indicator**: When a terminal receives a drop notification, show a subtle inline indicator (e.g., `[... output truncated ...]` in the terminal, or a banner above the terminal pane).
4. **Recovery via ring buffer replay**: Optionally, when drops are detected, the frontend could request a ring buffer replay from the daemon (`Attach` with replay) to recover the lost output. This is complex — consider as a follow-up.

### Simpler alternative

Instead of per-terminal tracking, just emit a global `output-dropped` event when the channel is full. The frontend shows a toast notification: "Heavy output — some terminal data was skipped." This requires minimal code changes.

### Testing

- Unit test: verify drop event is emitted when emitter channel is full.
- Integration test: simulate heavy output, verify drop notification arrives at frontend.
- Manual test: run `yes` in 10 terminals simultaneously, verify drop indicator appears.

### Acceptance criteria

- User is notified when terminal output is lost.
- Notification identifies which terminal(s) were affected.
- No false positives under normal usage.
