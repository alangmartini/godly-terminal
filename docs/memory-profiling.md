# Memory Profiling Guide

This document describes how to profile memory usage across all three memory spaces in Godly Terminal.

## 1. Daemon (Rust) — DHAT Heap Profiling

DHAT records every heap allocation (site, size, lifetime, peak usage) and outputs a JSON file.

### Setup

```bash
# Build and run daemon with DHAT profiling enabled
cargo run -p godly-daemon --features leak-check
```

### Usage

1. Launch the daemon with `--features leak-check`
2. Use the app normally (create terminals, type commands, close terminals)
3. Shut down the daemon (close all sessions and wait for idle timeout, or kill the process)
4. Open the generated `dhat-heap.json` in the [DHAT Viewer](https://nnethercote.github.io/dh_view/dh_view.html)

### What to look for

- **Total bytes still-live at exit**: Should be near zero (only static/global state)
- **Per-site breakdown**: Sort by "bytes still-live" to find allocations that were never freed
- **Allocation lifetimes**: Long-lived allocations in `session.rs` are expected while sessions exist; they should be freed on `CloseSession`
- **Ring buffer**: `VecDeque` in `session.rs` should cap at 1MB per session

## 2. Tauri App Backend (Rust) — DHAT Heap Profiling

Same approach as the daemon, but for the Tauri app process (DaemonClient, Bridge, state management).

### Setup

```bash
# Run the app with DHAT profiling (from src-tauri/)
cargo tauri dev --features leak-check
```

### Usage

1. Use the app: create/close terminals, switch workspaces, resize windows
2. Close the app window
3. Inspect `dhat-heap.json` output

### What to look for

- **DaemonBridge channels**: Should be freed on disconnect
- **AppState entries**: Terminal/workspace entries should be cleaned up on close
- **ProcessMonitor**: Threads and channels should be freed on stop
- **Event listeners**: Tauri event subscriptions should not accumulate

## 3. Daemon (Rust) — RSS Stress Tests

Automated tests that spawn a real daemon, exercise it under stress, and assert memory stays within bounds.

### Running

```bash
# Run all memory stress tests (from src-tauri/)
cargo test -p godly-daemon --test memory_stress -- --nocapture
```

### Test cases

| Test | What it measures | Threshold |
|------|-----------------|-----------|
| `test_session_create_destroy_no_leak` | 50 create+attach+write+detach+close cycles | < 5 MB growth |
| `test_attach_detach_no_leak` | 3 sessions x 100 attach/detach cycles | < 5 MB growth |
| `test_heavy_output_no_leak` | 10 MB written through 1 session | < 5 MB growth |

### How it works

- Each test spawns an isolated daemon on a unique named pipe (via `GODLY_PIPE_NAME` env var)
- Uses Windows `GetProcessMemoryInfo` to measure Working Set Size (RSS equivalent)
- Includes warmup phase to let the allocator settle before measuring
- Reports memory at regular intervals during the stress run

## 4. Frontend (TypeScript) — DevTools Heap Snapshots

Tauri 2.0 enables Chrome DevTools in debug builds by default.

### Manual Profiling Procedure

1. **Start the app in dev mode**:
   ```bash
   npm run tauri dev
   ```

2. **Open DevTools**: Press `F12` or right-click and select "Inspect"

3. **Take baseline snapshot**:
   - Go to the **Memory** tab
   - Select "Heap snapshot"
   - Click "Take snapshot"
   - Label it "Baseline"

4. **Exercise the app**:
   - Create 20 terminals
   - Type some commands in each
   - Close all 20 terminals
   - Repeat 2-3 times

5. **Take comparison snapshot**:
   - Take another heap snapshot
   - Label it "After stress"

6. **Compare snapshots**:
   - Select the second snapshot
   - Change view to "Comparison" (dropdown at top)
   - Select the baseline snapshot to compare against

7. **Look for leaks**:
   - Filter by "Detached" to find DOM nodes not garbage-collected
   - Search for `Terminal` — xterm.js instances should be fully collected
   - Search for `ResizeObserver` — should not accumulate
   - Check `(closure)` entries — event listener closures should not grow

### Known areas to watch

| Component | What could leak | How to check |
|-----------|----------------|--------------|
| `store.subscribe()` | Listener arrays grow | Search for `Array` in store module |
| `TerminalPane` | xterm.js `Terminal` instances not disposed | Search for `Terminal` objects |
| `App.ts` keydown listener | Global listener never removed | Check `(closure)` count |
| `TerminalService` | Tauri event listeners persist | Search for listener closures |

### Allocation Timeline (advanced)

For real-time tracking:

1. In DevTools Memory tab, select "Allocation instrumentation on timeline"
2. Click Start
3. Create and destroy terminals
4. Click Stop
5. Blue bars = allocations still alive. Look for bars that stay blue after terminal close.

## 5. Running All Checks

Use the automated script to run daemon-side checks:

```powershell
.\scripts\check-memory-leaks.ps1
```

This runs:
1. RSS stress tests (automated pass/fail)
2. DHAT profiling with analysis (daemon)

Frontend profiling must be done manually via DevTools (see section 4 above).
