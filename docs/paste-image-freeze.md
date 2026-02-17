# Terminal Freeze on Paste/Drag During Active Output

**Status**: Fixed
**Severity**: High — complete terminal freeze
**Test file**: `src-tauri/daemon/tests/paste_image_freeze.rs`

## Bug Description

When the user pastes or drags data into a terminal that is actively producing output (e.g., running `dir /s`, `find`, or any command that generates continuous stdout), the entire terminal application freezes. All tabs become unresponsive, not just the one receiving the paste.

## Root Cause

A circular deadlock between the daemon's I/O thread and reader thread:

```
write_all() blocks (ConPTY input full)
→ I/O thread stuck → can't drain event channel (capacity 1024)
→ event channel fills → forwarding task blocks
→ output channel fills (capacity 64) → reader blocks in blocking_send()
→ reader stops reading PTY → PTY output pipe fills
→ shell stdout blocks → shell can't consume stdin
→ ConPTY can't drain input → write_all() stays blocked forever
```

### Key code paths

1. **I/O thread** (`server.rs:494-518`): handles `Write` requests directly by calling `session.write(data)` → `write_all()` synchronously. While `write_all()` blocks, the I/O thread cannot:
   - Read new requests from the named pipe
   - Write responses back to the client
   - Drain the event channel (output events from attached sessions)

2. **Reader thread** (`session.rs:206-267`): reads PTY output and sends it via `blocking_send()` on a bounded channel (capacity 64). When the output channel is full (because the forwarding task is blocked on the event channel, which is full because the I/O thread can't drain it), `blocking_send()` blocks the reader.

3. **ConPTY backpressure**: when the reader thread stops reading PTY output, the PTY output pipe fills. The shell blocks on stdout. When the shell blocks, it can't consume input from the console input buffer. ConPTY can't drain its input pipe. `write_all()` is permanently blocked.

### Trigger conditions

- Session must be **attached** (output events are being forwarded)
- Shell must be **producing active output** (fills the event/output channels)
- A **large write** occurs (>= ConPTY input buffer capacity, typically when paste data is written)

## Reproduction Tests

All 3 tests consistently fail (verified 4 runs, 0 passes):

| Test | Description |
|------|-------------|
| `test_write_during_heavy_output_deadlocks` | 1MB write during `dir /s` output → Ping times out |
| `test_deadlock_affects_all_sessions` | 1MB write to session A → ReadGrid for session B times out |
| `test_binary_paste_during_output_deadlocks` | 1MB binary data during heavy output → Ping times out |

Run command:
```bash
cd src-tauri && cargo test -p godly-daemon --test paste_image_freeze -- --test-threads=1
```

## Fix Applied

Two surgical changes in `server.rs`:

1. **Removed direct Write handling from I/O thread** (was lines 494-518). Write requests now fall through to the `_ =>` arm and go to the async handler via `req_tx`.

2. **Async handler uses `spawn_blocking` fire-and-forget** for Write. Instead of blocking the async handler on `session.write()`, it spawns a blocking task and immediately returns `Response::Ok`. Write ordering is preserved by the `session.writer` Mutex.

This breaks the deadlock at the first link: the I/O thread never blocks on `write_all()`, so it always drains events.

Fire-and-forget is safe because the frontend already treats Write as fire-and-forget (`daemon.send_fire_and_forget()` in `commands/terminal.rs`).

## Regression Risk

- Any change to the I/O thread's request handling loop
- Changes to the output channel capacity or blocking behavior
- Changes to `session.write()` implementation
