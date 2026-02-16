# Input Latency Investigation

## Problem

1. Typing in the terminal has ~500ms visible delay (pre-existing)
2. After WebGL2 renderer + dirty-region changes, opening a new terminal froze rendering entirely (regression, fixed)

## Regression: Frozen Terminal (fixed)

The dirty-region optimization added `get_grid_snapshot_diff`, a new IPC command. When the running daemon hasn't been rebuilt with this handler, the diff call fails silently. Because `cachedSnapshot` was set after the first successful full render, all subsequent renders took the diff path and silently failed — **no frames rendered**.

**Fix**: `TerminalPane.ts` now catches diff failures and permanently falls back to full snapshots via `useDiffSnapshots` flag. The first diff failure disables the diff path for the lifetime of that pane.

## The Real 500ms Bottleneck

### Architecture recap

Every keystroke echo requires **3 sequential IPC round-trips** through a **single bridge I/O thread**:

```
Keystroke
  → invoke('write_to_terminal')        [IPC #1: goes through bridge thread]
  → bridge writes request to named pipe
  → daemon writes to PTY
  → shell echoes back
  → daemon reads PTY → streams Event::Output through pipe
  → bridge reads event, queues to EventEmitter channel
  → EventEmitter thread → Tauri emit('terminal-output')  [IPC #2: event to JS]
  → TerminalPane.scheduleSnapshotFetch()
  → setTimeout(0) coalescing
  → invoke('get_grid_snapshot')        [IPC #3: goes through bridge thread AGAIN]
  → bridge writes request to pipe, reads response
  → response arrives in JS
  → CellDataEncoder.encode()
  → requestAnimationFrame → WebGL paint
```

### Why it's 500ms, not 1.6ms

The isolated benchmarks only measured JSON serialization cost (~0.8ms per serialize). The real bottleneck is the **bridge I/O thread contention**:

1. **Single-threaded bridge**: All pipe I/O (reads AND writes) happens on one thread. Requests queue in an mpsc channel and wait for the bridge thread to process them.

2. **Head-of-line blocking**: When the daemon is streaming PTY output events through the pipe, the bridge thread is busy reading those events. The `get_grid_snapshot` request sits in the channel waiting for the bridge to finish draining events, write the request, and read the response.

3. **Event flood during typing**: Typing produces shell output (prompt redraws, completions, command echo). Each output chunk creates an Event::Output message. The bridge reads these in a loop. The snapshot request can only be serviced BETWEEN event reads.

4. **Mutex contention in daemon**: The godly-vt parser is behind a `Mutex`. The PTY reader thread holds this lock while parsing output. The `ReadRichGrid` handler must also lock it. Under sustained output, the snapshot request blocks waiting for the lock.

5. **Tauri thread pool**: `invoke()` calls dispatch to a thread pool. Under load, all threads may be blocked waiting on `send_request` → bridge thread → pipe I/O.

### Benchmark results (for reference)

These measure isolated component costs — NOT the real end-to-end path:

| Operation | Time | Notes |
|-----------|------|-------|
| JSON serialize RichGridData 30x120 (Rust) | 800 us | Per snapshot |
| JSON deserialize (Rust) | ~800 us | Per snapshot |
| Binary encode (same data) | 66 us | 12x faster |
| CellDataEncoder.encode (JS, 30x80) | 300 us | Acceptable |
| Full JS pipeline (encode+copy, 30x80) | 300 us | Acceptable |

## How to Reproduce and Measure

### 1. Live measurement with PerfTracer (easiest)

```bash
npm run build:daemon:release && npm run tauri dev
```

In browser DevTools console:
```js
globalThis.__PERF_TRACE = true;
location.reload();
```

Type in the terminal for ~10 seconds, then check console for the summary table.

**What to look for:**
- `keydown_to_output` > 50ms → bridge I/O or daemon lock contention
- `get_grid_snapshot_ipc` > 10ms → pipe round-trip under contention
- `keydown_to_render` > 100ms → confirms user-visible delay
- `raf_wait` > 16ms → rendering throttled by vsync

### 2. Rust-side timing (automatic)

`grid.rs` logs via `eprintln!` when `daemon.send_request()` exceeds 5ms. Check stderr output or the Tauri dev console.

### 3. Bridge debug log

Check `%APPDATA%/com.godly.terminal*/godly-bridge-debug.log` for bridge phase timing and stall detection.

### 4. Automated full-path latency test (daemon integration)

`daemon/tests/input_latency_full_path.rs` simulates the complete bridge-level pipeline:

```
[Command thread] --channel--> [Bridge I/O thread] --pipe--> [Daemon]
```

The bridge I/O thread is a single thread that reads events AND services requests, exactly like the real app's `DaemonBridge`. Results:

| Scenario | avg | p95 | max | Notes |
|----------|-----|-----|-----|-------|
| Idle terminal | 76ms | 90ms | 90ms | Baseline: pipe round-trip + JSON serde |
| Heavy output (200k lines) | 85ms | 97ms | 97ms | Bridge contention adds ~10ms |

Run with:
```bash
cd src-tauri && cargo test -p godly-daemon --test input_latency_full_path -- --test-threads=1 --nocapture
```

**What the test proves**: The bridge-level round-trip is ~85ms (debug build). The remaining ~400ms gap between this and the user's perceived 500ms comes from the JS/Tauri layers (see "Where the other 400ms lives" below).

### 5. Daemon-only latency test

`daemon/tests/input_latency.rs` measures direct pipe round-trips without the bridge simulation:

```bash
cd src-tauri && cargo test -p godly-daemon --test input_latency -- --test-threads=1 --nocapture
```

## Where the Other 400ms Lives

The full-path test reproduces ~85ms at the bridge level. The user sees ~500ms. The remaining ~400ms comes from layers the test can't simulate:

### 1. Snapshot request cascade (BUG — FIXED)

`scheduleSnapshotFetch()` reset `snapshotPending = false` BEFORE the async `fetchAndRenderSnapshot()`. During the ~85ms IPC, new `terminal-output` events would schedule ANOTHER snapshot fetch, creating overlapping IPC requests that saturate the Tauri thread pool.

**Fix**: `snapshotPending` now resets AFTER `fetchAndRenderSnapshot()` completes. Events during a pending fetch are coalesced, not piled on.

### 2. Tauri invoke() dispatch overhead

Each `invoke()` call goes through: JS → WebView2 IPC → Rust thread pool → DaemonClient.send_request → bridge channel → response. The Rust thread pool dispatch + channel round-trip adds ~5-15ms per call.

### 3. JS event loop overhead

- `setTimeout(0)` on Windows: 4ms minimum (browser timer clamping)
- `requestAnimationFrame`: 0-16.7ms (vsync alignment)
- Combined: up to 20ms per frame

### 4. Debug vs Release build

The test runs debug builds where JSON serde is ~10x slower. Release builds should reduce the 85ms bridge round-trip to ~15-25ms.

## Optimizations Implemented

### Dirty-region tracking (differential snapshots)

Only changed rows are sent instead of the full grid.

**Stack changes:**
- `godly-vt/src/grid.rs`: Row-level `dirty_rows: Vec<bool>`
- `protocol/src/types.rs`: New `RichGridDiff` with `dirty_rows: Vec<(u16, RichGridRow)>`
- `daemon/src/session.rs`: `read_rich_grid_diff()` — sends only dirty rows
- `TerminalPane.ts`: Caches full snapshot, merges diffs, falls back on error

**Expected improvement**: Single character echo → 1 dirty row instead of 30 → ~97% less JSON to serialize. But this only helps the serialization cost (~0.8ms → ~0.03ms), not the bridge contention.

### Performance instrumentation

`PerfTracer` (zero-cost when disabled) instruments 10 pipeline stages. Enable with `globalThis.__PERF_TRACE = true`.

## Root Cause Fix Candidates

The dirty-region optimization helps serialization cost but doesn't fix the fundamental architecture issue. Real fixes would be:

1. **Eliminate IPC #3**: Instead of the frontend requesting a snapshot, have the daemon **push** grid diffs alongside output events. The bridge already reads output events — adding diff data to the event payload eliminates the round-trip.

2. **Separate request/event channels**: Use two pipe connections — one for streaming events (read-only), one for request/response (bidirectional). Snapshot requests wouldn't queue behind event reads.

3. **Binary framing for hot path** (`current_tasks/perf-binary-framing.md`): Replace JSON with binary for Event::Output and grid snapshots. Reduces serialization from ~0.8ms to ~0.07ms.

4. **Lock-free grid reads**: Use a double-buffer or snapshot-on-write pattern in godly-vt so reads don't contend with the PTY writer thread.

5. **Coalesce write + snapshot**: Combine `write_to_terminal` + `get_grid_snapshot` into a single pipe round-trip (`WriteAndReadGrid`).

## Test Coverage

- 20 dirty tracking tests in `godly-vt/tests/dirty_tracking.rs`
- 4 daemon-level latency tests in `daemon/tests/input_latency.rs`
- 2 full-path bridge simulation tests in `daemon/tests/input_latency_full_path.rs`
- Frontend benchmarks in `src/components/renderer/__tests__/performance.bench.ts`
- Rust benchmarks in `src-tauri/protocol/benches/snapshot_serialization.rs`
