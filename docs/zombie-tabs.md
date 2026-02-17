# Zombie Terminal Tabs After PTY Exit (Issue A2)

**Status**: RESOLVED
**Branch**: `wt-fix-zombie-tabs`
**Regression Risk**: Medium - touches session lifecycle and attach logic

## Symptom

When a shell process exits (user types `exit`, process crashes), the terminal tab stays open forever. The frontend never receives notification, keeps polling for grid data, and gets "Session not found" errors.

## Root Cause

ConPTY on Windows does NOT produce EOF on the PTY pipe when the child process exits. The daemon's reader thread calls `read()` on the PTY pipe, which blocks forever even after the shell process has terminated. Because the reader thread never exits:

1. `running` flag stays `true`
2. The output channel is never dropped
3. The forwarding task never sends `SessionClosed`
4. The frontend is never notified

Additionally, if a client attaches to a session whose PTY has already exited (e.g., PTY died while no client was attached), the Attach handler spawns a forwarding task that blocks on `rx.recv()` forever because the reader thread is dead and never closes the channel.

## Fix

### Part 1: Child process monitor (`daemon/src/session.rs`)

Replaced the no-op "keep child handle alive" thread with a child process monitor thread:

```rust
thread::spawn(move || {
    let mut child = child;
    match child.wait() { /* log exit status */ }
    monitor_running.store(false, Ordering::Relaxed);
    monitor_attached.store(false, Ordering::Relaxed);
    *monitor_tx.lock() = None;  // drop output channel
});
```

This uses `child.wait()` to detect process exit independently of the PTY pipe. When the child exits, it performs the same cleanup as the reader thread EOF path: sets `running=false`, clears `is_attached`, and drops the output channel sender, which causes the forwarding task to detect the closed channel and send `SessionClosed`.

### Part 2: Attach handler dead session check (`daemon/src/server.rs`)

Added an `is_already_dead` check before spawning the forwarding task:

```rust
let is_already_dead = !session.is_running();
// ... in the spawned task:
if is_already_dead {
    let _ = tx.send(DaemonMessage::Event(Event::SessionClosed { session_id: sid })).await;
    return;
}
```

This handles the case where a client attaches to a session that already died. Instead of entering the forwarding loop (which would block forever on the dead channel), it immediately sends `SessionClosed`.

## Tests

### Daemon integration tests (`daemon/tests/zombie_tabs.rs`)
- `test_session_closed_on_pty_exit_while_attached` - PTY exit while client attached
- `test_session_closed_on_attach_to_dead_session` - Re-attach to already-dead session
- `test_cmd_exit_is_detected_by_daemon` - Diagnostic: child exit sets running=false
- `test_list_sessions_shows_dead_session` - ListSessions reports running=false

### Daemon unit test (`daemon/src/server.rs`)
- `test_attach_to_dead_session_sends_session_closed` - Forwarding task immediately sends SessionClosed when is_already_dead=true

### Frontend unit tests (`src/services/terminal-service.test.ts`)
- Terminal removal from store on `terminal-closed` event
- Output listener cleanup on terminal close
- Active terminal switching when active terminal is closed
- Graceful handling of unknown terminal IDs
- Output event routing to registered listeners

## Failed Approaches

None - the fix was straightforward once the root cause (ConPTY not producing EOF) was identified.

## Key Insight

The existing code assumed that `read()` on the PTY pipe would return EOF when the child process exits. This is true on Unix (where the slave end of the PTY closes), but NOT true on Windows with ConPTY. The PTY pipe stays open even after the child process terminates. The `portable_pty` crate's `Child::wait()` method is the reliable way to detect process exit on Windows.
