# Input Latency Analysis: Arrow Up / Autocomplete Lag

**Status:** Investigation complete, fixes pending
**Date:** 2025-02-14
**Symptoms:** Noticeable lag when pressing arrow-up (command history) or tab (autocomplete)

## Root Cause Summary

The round-trip from keypress to screen output traverses ~20 hops across 3 processes (frontend → Tauri app → daemon → PTY → daemon → Tauri app → frontend). Four independent sleep points using Windows `thread::sleep(1ms)` — which actually sleeps ~15ms due to the default 15.625ms timer resolution — can add up to 60ms of pure sleep latency in the worst case.

## Full Round-Trip Path

```
Keypress → xterm onData → invoke('write_to_terminal') → Tauri IPC → Rust command
→ Bridge channel → Bridge I/O thread → Named pipe → Daemon I/O thread
→ session.write() → PTY → Shell processes key → PTY output
→ Daemon reader thread → output channel → forwarding task → event channel
→ Daemon I/O thread → Named pipe → Bridge I/O thread → EventEmitter channel
→ Emitter thread → app_handle.emit() → Tauri event → Frontend listener
→ outputBuffer.push() → setTimeout(0) → flushOutputBuffer() → xterm.write()
```

## Bottlenecks (Ranked by Impact)

### 1. CRITICAL: Windows Timer Resolution (~15ms per sleep point)

**Files:** `bridge.rs:487`, `server.rs:686`

Both the client bridge and daemon I/O thread use adaptive polling with `thread::sleep(1ms)` as idle fallback. On Windows, the default timer resolution is 15.625ms, so `sleep(1ms)` actually sleeps ~15ms.

The `SPIN_BEFORE_SLEEP = 100` mitigation is insufficient — 100 `yield_now()` calls complete in <1 microsecond, so the thread enters sleep almost immediately after the last activity.

There are 4 sleep points in the round-trip:
1. Bridge picks up the write request (0-15ms)
2. Daemon picks up the request from pipe (0-15ms)
3. Daemon I/O thread picks up the output event (0-15ms)
4. Bridge picks up the output event from pipe (0-15ms)

**Worst case: 60ms of pure sleep latency. Average case ~30ms.**

**Fix options:**
- Call `timeBeginPeriod(1)` to set Windows timer resolution to 1ms globally
- Use IOCP or `WaitForSingleObject` with timeout instead of spin+sleep polling
- Increase `SPIN_BEFORE_SLEEP` to 10,000–50,000 (spin for ~10-50µs instead of <1µs)

### 2. HIGH: JSON `number[]` Serialization for Terminal Output

**Files:** `bridge.rs:82`, `terminal-service.ts:59`

Terminal output `Vec<u8>` is serialized as a JSON array of numbers via `serde_json::json!`. Each byte becomes 1-3 JSON characters + comma overhead → ~4x size inflation. A 4KB terminal chunk becomes ~15KB of JSON. Frontend must parse the JSON array, then copy into `Uint8Array`.

**Fix:** Use base64 encoding (1.33x overhead vs 4x) or Tauri's binary event payload.

### 3. HIGH: Double Serialization of Event Payloads

**Files:** `bridge.rs:80-84`

Output data is serialized twice:
1. `serde_json::json!(...)` in the EventEmitter — creates a `serde_json::Value`
2. `app_handle.emit()` serializes that `Value` again for WebView2 IPC

Using a typed struct with `#[derive(Serialize)]` would avoid the intermediate `Value` allocation.

### 4. MEDIUM: Redundant Key Event Processing on Every Keypress

**File:** `TerminalPane.ts:98-147`

Every keypress triggers 3 independent chord lookups, each allocating objects and doing string concatenation:

```typescript
keybindingStore.matchAction(event);    // eventToChord → chordToString → Map.get
isAppShortcut(event);                  // eventToChord → chordToString → Map.get
isTerminalControlKey(event);           // eventToChord → chordToString → Map.get
```

Plus the keyup handler creates a fake keydown object and calls `isTerminalControlKey` again. That's 4 chord conversions per key event.

**Fix:** Single lookup at the top, reuse the result.

### 5. MEDIUM: Dual Mutex Locks Per PTY Read in Daemon

**File:** `session.rs:178-262`

Every PTY read chunk locks two mutexes sequentially:
- `output_history` (always, for MCP ReadBuffer)
- `output_tx` (always, for live output routing)

Under rapid output these locks contend with `attach()`, `detach()`, and `read_output_history()`.

### 6. MEDIUM: `invoke()` IPC Overhead for Every Keystroke

**File:** `terminal-service.ts:122`

Every keystroke goes through Tauri's full IPC: JS → WebView2 message → Rust deserialization → command dispatch. While the Rust side is fire-and-forget, rapid keystrokes create many concurrent Promises in the frontend.

**Fix:** Use Tauri's channel API or batch keystrokes with a microtask buffer.

### 7. LOW: Named Pipe Framing Uses JSON

**File:** `frame.rs:7-17`

Every message (including tiny 3-byte Write requests) goes through JSON serialization + deserialization. Binary formats like bincode or MessagePack would be faster.

### 8. LOW: `setTimeout(0)` Output Batching Adds ~1ms

**File:** `TerminalPane.ts:163`

Intentional batching, but adds ~1ms consistent latency to every output render cycle.

### 9. LOW: EventEmitter Thread Hop

**File:** `bridge.rs:38-53`

Output events go through an extra thread hop: Bridge I/O → SyncSender → Emitter thread → `app_handle.emit()`. Exists to prevent main-thread stalls from blocking bridge I/O.

## Expected Latency Breakdown

| Hop | Typical | Worst Case |
|-----|---------|------------|
| xterm → Tauri IPC | 1-2ms | 3ms |
| Bridge sleep (write) | 0-1ms | 15ms |
| Named pipe transit | <1ms | <1ms |
| Daemon sleep (read) | 0-1ms | 15ms |
| PTY write + shell process | 1-50ms | 200ms+ (autocomplete) |
| Daemon reader → event channel | <1ms | 5ms (lock contention) |
| Daemon I/O sleep (write event) | 0-1ms | 15ms |
| Named pipe transit back | <1ms | <1ms |
| Bridge sleep (read event) | 0-1ms | 15ms |
| JSON deserialization + Tauri emit | 1-2ms | 5ms |
| setTimeout(0) + xterm render | 1-2ms | 5ms |
| **Total** | **~10-60ms** | **~280ms** |

## Recommended Fix Priority

1. **Windows timer resolution** — `timeBeginPeriod(1)` or increase spin count (biggest single win)
2. **Binary output encoding** — base64 or raw bytes instead of JSON number arrays
3. **Typed emit structs** — eliminate double serialization
4. **Consolidate key handler lookups** — single chord match per keypress

## Attempts Log

- **2025-02-14:** Initial investigation. Full pipeline traced from keypress to render. No code changes yet — analysis only.
