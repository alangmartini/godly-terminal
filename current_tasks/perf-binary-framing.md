# Binary framing instead of JSON for IPC protocol

## Branch: `perf/binary-framing`

## Problem

Every message between daemon and client — including raw PTY output — is serialized as JSON via serde. A 4KB chunk of terminal output becomes `{"kind":"Event","type":"Output","session_id":"...","data":[104,101,108,...]}` with the binary data expanded as a JSON integer array. This adds ~30-50% overhead on throughput and significant CPU cost for serialization/deserialization.

The single bridge I/O thread (`bridge.rs`) is already the throughput bottleneck (~10-50 MB/s aggregate). Reducing serialization overhead directly increases the ceiling.

## Scope

**Full stack** — protocol, daemon, and client changes.

### Files likely modified

- `src-tauri/protocol/src/lib.rs` — replace JSON serde with binary framing
- `src-tauri/protocol/src/messages.rs` — message types (may stay as serde for non-hot-path messages)
- `src-tauri/protocol/src/frame.rs` — frame reading/writing (already exists, may need extension)
- `src-tauri/daemon/src/server.rs` — server-side message handling
- `src-tauri/src/daemon_client/bridge.rs` — client-side message handling
- `src-tauri/src/daemon_client/client.rs` — request/response serialization

### Approach

1. **Keep JSON for control messages** (CreateSession, Attach, ListSessions, etc.) — they're infrequent and human-readable JSON is useful for debugging.
2. **Use binary framing for hot-path messages only**:
   - `Event::Output { session_id, data }` — the dominant message type by volume
   - `Request::Write { session_id, data }` — user input
3. Binary frame format: `[type: u8][session_id_len: u8][session_id: bytes][payload_len: u32][payload: bytes]`
4. Detect message type from the existing length-prefixed framing — add a version/type byte to distinguish JSON vs binary frames.
5. Update `read_message` / `write_message` in `protocol/src/frame.rs` to handle both formats.

### Testing

- Unit tests for binary frame round-trip serialization.
- Integration tests: daemon sends binary output events, client deserializes correctly.
- Benchmark: measure throughput before/after with a `yes`-style output generator.
- Backward compatibility: ensure control messages still work as JSON.

### Acceptance criteria

- Output events use binary framing (no JSON serialization for data payload).
- Write requests use binary framing.
- Control messages remain JSON.
- Measurable throughput improvement (target: 2x for sustained output).
- All existing tests pass.
