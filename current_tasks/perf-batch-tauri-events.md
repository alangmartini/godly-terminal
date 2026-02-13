# Batch Tauri events for terminal output

## Branch: `perf/batch-tauri-events`

## Problem

The bridge emits one Tauri event per output chunk. Each `app_handle.emit()` call crosses the Rust→JS boundary, triggers JSON serialization of the payload, and invokes the JS event listener. Under heavy output from multiple terminals, this creates thousands of JS event callbacks per second, each allocating a new `Uint8Array`.

While per-terminal buffering in `TerminalPane` reduces `xterm.js write()` calls, the upstream event overhead (Tauri emit → JS callback → Map lookup → buffer push) is still O(events).

## Scope

**Rust bridge + frontend service** — daemon unchanged.

### Files likely modified

- `src-tauri/src/daemon_client/bridge.rs` — batch events in emitter thread before calling `app_handle.emit()`
- `src/services/terminal-service.ts` — handle batched event payloads

### Approach

1. **In the emitter thread** (`bridge.rs`): instead of emitting each `EmitPayload::TerminalOutput` immediately, drain the channel and group output events by `terminal_id` over a 1-2ms window.
2. **Emit a single batched event** per terminal per batch window: `{ terminal_id, chunks: [Uint8Array, ...] }` or concatenate into a single `data` blob.
3. **Frontend**: update the `terminal-output` listener to handle both single and batched payloads (or switch entirely to batched format).
4. **Non-output events** (`ProcessChanged`, `TerminalClosed`) should be emitted immediately — no batching for control events.

### Alternative: concatenate in bridge

Instead of sending arrays of chunks, concatenate all output for the same terminal into a single `Vec<u8>` before emitting. This reduces the JS side to exactly what it already expects — a single `data` array per event — and eliminates the need for frontend changes.

### Testing

- Verify output still renders correctly under normal use.
- Verify high-throughput output (e.g., `seq 1000000`) renders without gaps.
- Measure: count Tauri events/sec before and after under sustained output.
- Manual test: 5 terminals running `yes`, UI should stay responsive.

### Acceptance criteria

- Under heavy output, Tauri events/sec drops by 5-10x.
- No visible change in output rendering or latency.
- Control events (process-changed, terminal-closed) are not delayed.
