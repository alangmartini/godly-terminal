# Binary Framing for Hot-Path IPC Messages

## Summary

Implemented binary wire format for the three message types that carry `Vec<u8>` data
on the daemon<->client IPC pipe:

- `Event::Output` (daemon->client, most frequent)
- `Request::Write` (client->daemon, per keystroke)
- `Response::Buffer` (daemon->client, ring buffer replay on attach)

## Problem

JSON serialization of `Vec<u8>` produces integer arrays (`[104,101,108,...]`),
causing ~4x size inflation for binary data. Protocol benchmarks showed binary
encoding is 12.3x faster than JSON for grid data.

## Solution

Binary frames use a different first-byte discriminator than JSON (which always
starts with `{` = 0x7B):

```
[4-byte BE length] [payload]

If payload[0] == 0x7B ('{'):  -> JSON (existing behavior)
If payload[0] is a type tag:  -> Binary frame

Binary frame layout:
  [1 byte type tag]       0x01=Output, 0x02=Write, 0x03=Buffer
  [1 byte session_id_len]
  [session_id_len bytes: session_id UTF-8]
  [remaining bytes: raw data]
```

All other message types stay JSON (human-readable, infrequent).

## Files Changed

| File | Change |
|------|--------|
| `protocol/src/frame.rs` | Added `write_daemon_message`, `read_daemon_message`, `write_request`, `read_request` + tests |
| `protocol/src/lib.rs` | Exported new functions |
| `daemon/src/server.rs` | 4 call sites -> typed functions |
| `src/daemon_client/bridge.rs` | 2 call sites -> typed functions |
| `mcp/src/daemon_direct.rs` | 2 call sites -> typed functions |
| `daemon/tests/*.rs` | ~20 mechanical renames across 10 test files |

## Backward Compatibility

- Generic `read_message`/`write_message` remain unchanged (used by MCP pipe)
- The reader auto-detects binary vs JSON by peeking the first byte
- A new client reading from an old daemon (or vice versa) works because JSON messages start with `{` (0x7B) which is never a valid binary tag

## Verification

- `cargo check -p godly-protocol -p godly-daemon -p godly-mcp` - all pass
- `cargo test -p godly-protocol` - 42 tests pass (including 10 new binary frame tests)
- `cargo test -p godly-daemon --bin godly-daemon` - 20 unit tests pass
- `cargo test -p godly-daemon --test test_isolation_guardrail` - passes
- All daemon integration tests compile (`cargo test -p godly-daemon --no-run`)
