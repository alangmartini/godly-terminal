# Ultrafast I/O Architecture

Goal: instant-feeling terminal responses for interactive use (arrow keys, autocomplete, tab completion) and high throughput for bulk output (cat, build logs).

## Current Latency Profile

| Path | Current Latency | Target |
|------|----------------|--------|
| Input (keypress → PTY) | 4-12ms | Keep as-is |
| Output (PTY → screen) | 100-400ms | <30ms interactive, <80ms bulk |

Input is already well-optimized with fire-and-forget writes. Output is where all the pain is.

## Pipeline Overview

### Input Path (4-12ms — already good)

```
Keystroke (xterm.js)
  → TypeScript callback                         ~0.5ms
  → Tauri invoke                                ~1-5ms
  → Tauri command handler (fire-and-forget)      ~0.5ms
  → DaemonClient channel enqueue                 ~0.1ms
  → Bridge I/O thread (JSON serialize + pipe)    ~1-2ms
  → Daemon I/O thread (peek + read + direct)     ~1ms
  → Session.write() → PTY master                 ~0.5-1ms
```

Key optimizations already in place:
- **Fire-and-forget writes**: input doesn't wait for daemon response
- **Direct handling in I/O thread**: Write/Resize bypass the async handler, eliminating 2 tokio channel hops

### Output Path (100-400ms — needs work)

```
PTY output (kernel)
  → Reader thread 64KB read                      ~0.5-5ms
  → Per-session bounded channel                   ~0.1-50ms
  → Forwarding task (tokio)                       ~0.1ms
  → Event queue → Daemon I/O thread               ~1-5ms
  → JSON serialization (64KB → 256KB!)            ~10-50ms   ← BOTTLENECK
  → Named pipe write                              ~5-20ms
  → Bridge I/O thread (JSON deserialization)       ~50-200ms  ← BOTTLENECK
  → EventEmitter channel                           ~0.1ms
  → EventEmitter thread → app_handle.emit()        ~1-5ms
  → Tauri main thread (re-serializes to JSON!)     ~5-20ms   ← BOTTLENECK
  → Frontend event listener                        ~0.1ms
  → Batch to outputBuffer (setTimeout 0)           ~0-10ms
  → xterm.js.write() VT parsing                    ~5-50ms
  → DOM update (requestAnimationFrame)             ~0-16ms
```

## The 3 Bottlenecks

### 1. JSON serialization of binary data (biggest offender)

Every 64KB of PTY output becomes ~256KB of JSON because `Vec<u8>` serializes as a JSON number array: `[27,91,65,27,91,66,...]`. This **4x amplification** happens on every output event.

**Where it happens:**
- `protocol/src/frame.rs` — `serde_json::to_vec()` on `Event::Output { data: Vec<u8> }`
- Daemon I/O thread spends 10-50ms encoding
- Bridge I/O thread spends 50-200ms decoding

### 2. Triple serialization

Data gets serialized/deserialized 3 times before xterm.js sees it:

```
Daemon: serde_json::to_vec(Event::Output)     → JSON bytes on pipe
Bridge: serde_json::from_slice(DaemonMessage)  → Rust struct
Tauri:  app_handle.emit(serde_json::json!())   → JSON string to frontend
Frontend: JSON.parse(event.payload)            → JavaScript object
```

### 3. Hidden terminals parse all output

Every terminal — visible or not — receives output events and feeds them to xterm.js. VT parsing costs 5-50ms per write. With N terminals open, that's N× CPU for output that nobody sees.

## Proposed Changes

### P0: Binary Protocol for Output Events

**What:** Replace JSON framing with binary framing for `Event::Output` messages.

**Binary frame format:**
```
┌─────────────┬──────────┬─────────────────┬──────────┐
│ Length (4B)  │ Type (1B)│ Session ID (var) │ Payload  │
│ big-endian   │ 0x01=Out │ len-prefixed     │ raw bytes│
└─────────────┴──────────┴─────────────────┴──────────┘
```

- Output data stays as raw bytes — **zero serialization** on the hot path
- Keep JSON (`Type=0x00`) for low-frequency control messages (create, close, list, attach)
- The frame reader peeks at the type byte to decide JSON vs binary decoding

**Files to change:**
- `protocol/src/frame.rs` — add `write_binary_event()` / `read_message()` dispatching
- `protocol/src/messages.rs` — add binary serialization for `Event::Output`
- `daemon/src/server.rs` — use binary path for output events in I/O thread
- `src-tauri/src/daemon_client/bridge.rs` — binary deserialization path

**Expected improvement:** 3-5x output throughput. Eliminates 4x amplification entirely.

### P0: Direct Byte Passthrough (Skip Tauri Event JSON)

**What:** Bypass `app_handle.emit()` for terminal output. Instead, pass raw bytes directly to the frontend.

**Approach — Tauri custom protocol handler:**
```
tauri://localhost/terminal-stream/{session_id}
```
- Register a custom protocol in Tauri that serves terminal output as a streaming response
- Frontend connects via `fetch()` or `ReadableStream`
- Bridge writes raw bytes to the stream — no JSON encoding
- Frontend reads `Uint8Array` chunks directly into xterm.js

**Alternative — SharedArrayBuffer ring:**
- Allocate a `SharedArrayBuffer` per terminal
- Bridge thread writes PTY output directly into the shared buffer
- Frontend reads via `Atomics.wait()` / polling
- Zero-copy from Rust to JavaScript

**Files to change:**
- `src-tauri/src/daemon_client/bridge.rs` — write to stream/buffer instead of EventEmitter
- `src-tauri/src/lib.rs` — register custom protocol or shared buffer
- `src/services/terminal-service.ts` — consume stream instead of Tauri event
- `src/components/TerminalPane.ts` — wire new output source

**Expected improvement:** Eliminates 2 of 3 serialization passes. ~20-50ms saved per event.

### P1: Pause Hidden Terminals

**What:** Unsubscribe from output events for non-visible terminals. Use the daemon's ring buffer (already exists, 1MB per session) to catch up on tab switch.

**How it works:**
1. When user switches tabs, send `Detach` for old terminal, `Attach` for new one
2. Daemon ring buffer captures output while detached
3. On re-attach, daemon replays ring buffer (already implemented for reconnection)
4. xterm.js parses only the visible terminal's output

**Lighter alternative — frontend-only:**
1. Keep all terminals attached (ring buffer replay is complex for tab switches)
2. But skip `terminal.write()` for hidden terminals
3. Buffer raw bytes per hidden terminal
4. On tab switch, write buffered bytes to xterm.js

**Files to change:**
- `src/components/TerminalPane.ts` — conditional `terminal.write()` based on visibility
- `src/components/TabBar.ts` or `App.ts` — notify terminals of visibility changes

**Expected improvement:** CPU proportional to 1 terminal instead of N. Most impactful with many tabs open.

### P1: Adaptive Output Batching

**What:** Different strategies for interactive vs bulk output.

**Interactive mode** (small, infrequent output — shell prompts, arrow key responses):
- Forward immediately with no coalescing delay
- Target: <5ms from daemon read to xterm.js write
- Trigger: output chunk <1KB and >50ms since last output

**Bulk mode** (large, rapid output — cat, build logs, grep):
- Coalesce into larger chunks, throttle to ~60fps (16ms intervals)
- Target: maximize throughput, smooth scrolling
- Trigger: output chunk >4KB or <10ms since last output

**Where to implement:**
- Daemon reader thread already has a 64KB buffer — add mode detection here
- Frontend `flushOutputBuffer()` — use `requestAnimationFrame` instead of `setTimeout(0)` in bulk mode

**Files to change:**
- `daemon/src/session.rs` — mode detection + forwarding strategy
- `src/components/TerminalPane.ts` — adaptive flush timing

**Expected improvement:** Better interactive feel. Arrow up/autocomplete responses feel instant because they skip coalescing.

### P2: Multiplexed Streaming (Future)

**What:** Replace per-message request/response on named pipe with multiplexed streams.

**Why it helps:**
- Current: each output event has its own frame header + JSON envelope overhead
- Proposed: session output is a continuous byte stream, multiplexed by session ID
- Reduces per-byte overhead for sustained output

**Rough design:**
```
┌──────────┬────────────┬──────────┬─────────┐
│ Stream ID │ Chunk len  │ Flags    │ Data    │
│ (2 bytes) │ (2 bytes)  │ (1 byte) │ (var)   │
└──────────┴────────────┴──────────┴─────────┘
```

**Why P2:** Diminishing returns after binary protocol (P0). The per-frame overhead is small once we stop JSON-encoding the payload. Worth doing if we need >100MB/s throughput.

## Expected Results

| Scenario | Current | After P0 | After P0+P1 |
|----------|---------|----------|-------------|
| Arrow up (small response) | 15-30ms | 8-15ms | 5-10ms |
| Tab completion | 20-50ms | 10-20ms | 8-15ms |
| `cat large-file` throughput | ~2MB/s | ~8MB/s | ~10MB/s |
| 10 terminals open (CPU) | 10x baseline | 10x baseline | 1x baseline |
| `ls -la` output | 50-150ms | 20-50ms | 15-30ms |

## Implementation Order

```
Phase 1 — Binary protocol (P0)
  ├── Add binary frame type to protocol crate
  ├── Binary write path in daemon I/O thread
  ├── Binary read path in bridge I/O thread
  └── Tests: round-trip binary frames, mixed JSON+binary

Phase 2 — Direct byte passthrough (P0)
  ├── Tauri custom protocol or shared buffer setup
  ├── Bridge writes to new channel instead of EventEmitter
  ├── Frontend stream consumer
  └── Tests: output arrives correctly, no data loss

Phase 3 — Pause hidden terminals (P1)
  ├── Visibility tracking in TerminalPane
  ├── Conditional terminal.write()
  ├── Buffer management for hidden terminals
  └── Tests: output not lost on tab switch

Phase 4 — Adaptive batching (P1)
  ├── Mode detection in daemon reader thread
  ├── Adaptive flush timing in frontend
  └── Tests: interactive latency, bulk throughput
```

## Reference: Current Buffer Sizes

| Buffer | Size | Location |
|--------|------|----------|
| Named pipe (in/out) | 256KB | `daemon/src/server.rs`, `daemon_client/client.rs` |
| Daemon reader buffer | 64KB | `daemon/src/session.rs` |
| Per-session output channel | 64 events | `daemon/src/session.rs` |
| Daemon event queue | 1024 events | `daemon/src/server.rs` |
| EventEmitter channel | 4096 events | `daemon_client/bridge.rs` |
| Ring buffer (per session) | 1MB | `daemon/src/session.rs` |
| Daemon I/O batch limit | 128 events/iter | `daemon/src/server.rs` |
| Bridge I/O event limit | 8 events/iter | `daemon_client/bridge.rs` |

## Reference: Key Files

| Component | File |
|-----------|------|
| Protocol framing | `src-tauri/protocol/src/frame.rs` |
| Protocol messages | `src-tauri/protocol/src/messages.rs` |
| Daemon I/O thread | `src-tauri/daemon/src/server.rs` |
| Daemon session/reader | `src-tauri/daemon/src/session.rs` |
| Bridge I/O thread | `src-tauri/src/daemon_client/bridge.rs` |
| Bridge EventEmitter | `src-tauri/src/daemon_client/bridge.rs` |
| Daemon client | `src-tauri/src/daemon_client/client.rs` |
| Tauri commands | `src-tauri/src/commands/terminal.rs` |
| Frontend terminal service | `src/services/terminal-service.ts` |
| Frontend terminal pane | `src/components/TerminalPane.ts` |
