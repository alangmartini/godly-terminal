# Laggy Autocomplete & Arrow-Up Input Latency — Tasklist

Tracks all approaches tried and remaining work for eliminating perceptible input lag when pressing arrow keys (history navigation) or interacting with shell autocomplete (tab completions, fzf, etc.).

**Symptom**: Arrow-up and autocomplete feel sluggish — noticeable delay between keypress and terminal response, especially after a brief pause in typing.

---

## Approaches Already Tried

### 1. Fire-and-forget terminal writes (DONE)

**Commit**: `f8db4f9` — "fix: eliminate ~2s input lag on arrow-up and rapid keystrokes"
**PR**: [#71](https://github.com/alangmartini/godly-terminal/pull/71)

**Root cause**: `write_to_terminal` was a synchronous Tauri command that blocked the thread pool waiting for the daemon's OK response on every keystroke. Under rapid input, all Tauri threads stalled waiting for round-trip IPC.

**Fix**:
- Added `send_fire_and_forget()` — sends Write requests without waiting for daemon acknowledgment
- Terminal writes no longer block the Tauri thread pool
- Reduced worst-case input lag from ~2s to <50ms

**Status**: Merged. Eliminated the blocking-IPC variant.

---

### 2. Faster output flushing (setTimeout vs RAF) (DONE)

**Commit**: `a7388a5` — "fix: reduce terminal input/output lag"
**PR**: [#71](https://github.com/alangmartini/godly-terminal/pull/71)

**Root cause**: Frontend used `requestAnimationFrame` (~16ms) to coalesce output writes to xterm.js. Echo latency was at least 16ms — noticeable for interactive tools like autocomplete.

**Fix**:
- Switched output flush timer from `requestAnimationFrame` to `setTimeout(0)` (~1ms)
- Still batches burst output, but fires 16x faster for interactive echo

**Status**: Merged. Echo latency reduced from ~16ms to ~1ms.

---

### 3. Direct I/O thread writes + adaptive polling (DONE)

**Commit**: `602e1bc` — "fix: reduce arrow key input latency with adaptive polling and direct I/O writes"
**PR**: [#76](https://github.com/alangmartini/godly-terminal/pull/76)

**Root cause**: Two issues:
1. Write requests bounced through tokio channels to an async handler, adding 2ms+ scheduler latency per keystroke
2. Bridge I/O thread used `thread::sleep(1ms)` which on Windows actually sleeps 15ms (default timer resolution)

**Fix**:
- Handle Write/Resize requests **directly** in the daemon I/O thread (skip tokio async handler entirely)
- Adaptive polling in bridge: spin 100 iterations with `yield_now()` before falling back to `sleep(1ms)`
- Fire-and-forget writes count orphan responses instead of pushing dead senders

**Status**: Merged. Eliminated tokio scheduler hop for latency-sensitive operations.

---

### 4. Windows timer resolution + WakeEvent (DONE)

**Commit**: `9223688` — "fix: eliminate Windows timer resolution penalty causing arrow key latency"
**PR**: [#77](https://github.com/alangmartini/godly-terminal/pull/77)

**Root cause**: Windows default timer resolution is 15.625ms. `thread::sleep(1ms)` actually sleeps ~15ms. Arrow keys pressed after a pause hit this penalty on every idle-to-active transition, adding ~30ms overhead (one sleep in bridge + one in daemon).

**Fix**:
- Call `timeBeginPeriod(1)` at startup in both Tauri app and daemon (1ms timer resolution)
- Added `WakeEvent` (Windows Event object) to bridge I/O thread
- `send_fire_and_forget()` and `try_send_request()` signal the WakeEvent, giving zero-latency wakeup
- Bridge waits on WakeEvent with 1ms timeout (for incoming daemon events) instead of blind sleep

**Status**: Merged. Zero-latency wakeup for user input while maintaining efficient polling.

---

### 5. Frontend DOM thrashing elimination (DONE)

**Commit**: `54c406a` — "fix: eliminate frontend DOM thrashing causing progressive terminal freeze"
**PR**: [#55](https://github.com/alangmartini/godly-terminal/pull/55)

**Root cause**: Frontend progressively froze due to per-event DOM operations. Each store notification triggered all subscribers synchronously. TabBar recreated all DOM elements on every update. ResizeObserver fired per-pixel without debouncing.

**Fix**:
- Batch store notifications with RAF — multiple `setState()` calls trigger one subscriber cycle
- Shallow change detection in `updateTerminal()` to skip no-op updates
- Incremental TabBar rendering (diff DOM in-place)
- Debounce ResizeObserver with RAF
- Move scrollback-save busy-wait to background thread

**Status**: Merged. Frontend handles high-frequency events without degrading.

---

### 6. Non-blocking emit channel (DONE)

**Commit**: `777b496` — "fix: non-blocking emit channel prevents terminal freeze from main-thread stalls"
**PR**: [#52](https://github.com/alangmartini/godly-terminal/pull/52)

**Root cause**: `app_handle.emit()` is synchronous. When the main thread stalls during DOM rendering, the bridge I/O thread blocks on emit, which also blocks the keepalive ping. All threads freeze simultaneously.

**Fix**:
- Route all hot-path emit calls through bounded `EventEmitter` channel with dedicated thread
- Bridge I/O thread uses `try_send()` (sub-microsecond) instead of blocking emit

**Status**: Merged. Bridge I/O thread decoupled from frontend rendering pressure.

---

### 7. Backpressure + larger read buffers (DONE)

**Commit**: `f84a99e` + `896b4ae` — "fix: use blocking_send for attached clients" + "fix: add backpressure to per-session output channel"
**PR**: [#63](https://github.com/alangmartini/godly-terminal/pull/63), [#64](https://github.com/alangmartini/godly-terminal/pull/64)

**Root cause**: Unbounded event channels grew to 170K+ events under heavy output. Reader buffer was 4KB, producing many small events. No backpressure — all events queued into memory.

**Fix**:
- Reader buffer 4KB → 64KB (fewer, larger events)
- Bounded per-session channel (64) with ring buffer fallback
- Bounded event channel (1024) with backpressure
- Write batch limit 8 → 128

**Status**: Merged. Memory-bounded under heavy output.

---

### 8. Mutex starvation fix (DONE)

**Commit**: `1e6a101` — "fix: resolve mutex starvation causing terminal freeze under heavy output"
**PR**: [#60](https://github.com/alangmartini/godly-terminal/pull/60)

**Root cause**: Under heavy output, the PTY reader thread held locks in a tight loop, starving handler threads. All terminals froze because the handler is sequential.

**Fix**:
- Lock-free `is_attached()` via `AtomicBool`
- Timeout-based locking in `attach()` (2s try_lock)
- Reader thread yields after channel send
- Dead I/O thread detection breaks handler loop

**Status**: Merged. Handler threads no longer block under heavy output.

---

## Remaining Work

### HIGH PRIORITY — Likely contributors to residual lag

#### T1. Pause output parsing for hidden terminals
**Files**: `TerminalPane.ts`, `App.ts`
**Task file**: `current_tasks/perf-pause-invisible-terminals.md`

Hidden terminals still call `terminal.write()` for every output event. xterm.js parser runs for all N terminals even though only 1 is visible. With 10 terminals under heavy output, that's 10x CPU overhead on the main thread — directly competing with the active terminal's input responsiveness.

**Fix**: Pause output subscription for inactive terminals. Buffer events or rely on daemon ring buffer for replay on reactivation.

---

#### T2. Batch xterm.js write() calls per frame
**Files**: `TerminalPane.ts`
**Related**: Already using `setTimeout(0)` for flush, but each event still produces a separate `write()` call within the same flush

Under heavy output, hundreds of small writes arrive per frame. Even with `setTimeout(0)` batching, if multiple events arrive between flushes, each is still written separately. Concatenating buffered chunks into a single `write()` call reduces parser invocations.

**Fix**: Collect all buffered output into a single string/Uint8Array before calling `terminal.write()` once per flush.

---

#### T3. Binary framing to eliminate JSON 4x amplification
**Files**: `protocol/src/messages.rs`, `protocol/src/frame.rs`
**Task file**: `current_tasks/perf-binary-framing.md`

`Vec<u8>` serialized as JSON array of numbers: `[27,91,51,50,109,...]` — 4x size amplification on every output event. The 256KB pipe buffer effectively holds only ~64KB of terminal data. Under heavy output, the pipe fills 4x faster and daemon writes stall.

**Fix**: Switch to binary-safe serialization (`serde_bytes`, bincode, or MessagePack) for output data fields.

---

### MEDIUM PRIORITY — Contributing factors

#### T4. Batch Tauri events
**Task file**: `current_tasks/perf-batch-tauri-events.md`

Each output chunk emits a separate Tauri event. The frontend receives hundreds of individual events per second. Batching multiple output chunks into a single Tauri event reduces IPC overhead and event listener invocations.

---

#### T5. Surface dropped events under backpressure
**Task file**: `current_tasks/perf-surface-dropped-events.md`

When backpressure kicks in (per-session channel full), output falls back to the ring buffer but is never replayed to the attached client. Missing output can leave the terminal in a garbled VT state — the user sees a "frozen" terminal that's actually just rendering incorrectly.

**Fix**: After backpressure subsides, replay ring buffer delta to the attached client.

---

#### T6. All Tauri commands blocked during reconnect
**Files**: `daemon_client/client.rs`

`reconnect()` holds `reconnect_lock` while re-attaching all sessions (up to 15s × N). Every other Tauri command blocks behind this lock. The entire app freezes during reconnection.

**Fix**: Make reconnection non-blocking. Queue commands during reconnection and replay after.

---

#### T7. Sync Tauri commands exhausting thread pool
**Files**: `commands/terminal.rs`

All terminal commands are synchronous with 15s timeout. Under reconnection or daemon slowness, all Tauri threads block, leaving no capacity for new commands.

**Fix**: Convert hot-path commands to `async` or use a dedicated thread pool for terminal IPC.

---

### LOW PRIORITY — Edge cases and robustness

#### T8. Bidirectional pipe buffer deadlock
**Files**: `daemon_client/bridge.rs`

Reader and writer share the same file object via `DuplicateHandle`. Synchronous named pipes serialize I/O per file object. If write blocks (pipe full), peek also blocks → neither side progresses.

**Fix**: Use separate pipe instances for read and write, or switch to overlapped I/O.

---

#### T9. Partial frame read blocks on broken pipe
**Files**: `protocol/src/frame.rs`

If pipe breaks mid-frame, `read_exact` blocks waiting for remaining bytes. No max message size validation — corrupted length prefix causes large allocation + blocking read.

**Fix**: Add read timeout and max message size cap.

---

#### T10. WorkspaceSidebar still does full DOM rebuild
**Files**: `WorkspaceSidebar.ts`

Unlike TabBar (fixed in attempt 5), WorkspaceSidebar clears `innerHTML` and recreates all elements on every update. `notificationStore.notify()` bypasses RAF batching.

**Fix**: Apply same incremental rendering pattern as TabBar.

---

## Timeline of fixes

```
Feb 11  [5] DOM thrashing elimination      (54c406a)
Feb 11  [6] Non-blocking emit channel       (777b496)
Feb 11  [8] Mutex starvation fix            (1e6a101)
Feb 12  [7] Backpressure + large buffers    (f84a99e, 896b4ae)
Feb 13  [2] setTimeout(0) output flush      (a7388a5)
Feb 13  [1] Fire-and-forget writes          (f8db4f9)
Feb 13  [3] Direct I/O + adaptive polling   (602e1bc)
Feb 13  [4] Timer resolution + WakeEvent    (9223688)
```

## References

- Full freeze investigation log: [docs/terminal-freeze-investigation.md](./terminal-freeze-investigation.md)
- Performance task files: `current_tasks/perf-*.md`
- Remaining issues taxonomy: see "Potential Remaining Causes" in freeze investigation doc
