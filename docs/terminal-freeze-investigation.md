# Terminal Freeze Investigation Log

All fix attempts for the terminal freezing bug, in chronological order. The freeze manifests as the terminal becoming unresponsive to input and/or ceasing to display output, requiring the user to close and reopen the app.

**Status**: Partially resolved. Multiple root causes identified and fixed across 11 attempts. The full IPC pipeline has been hardened — pipe buffers, channel priority, emit blocking, DOM thrashing, daemon crashes, response starvation, and mutex starvation have all been addressed.

---

## Attempt 1: Re-attach sessions after daemon reconnection

**PR**: [#27](https://github.com/alangmartini/godly-terminal/pull/27) — merged 2026-02-08
**Commit**: `d7ef5b0`
**Branch**: `wt-853e62`

**Hypothesis**: When the named pipe connection breaks (e.g. after system sleep/wake), the daemon auto-detaches all sessions. The reconnection logic established a new pipe but never re-attached sessions, so output events stopped flowing and the terminal appeared frozen.

**Fix**:
- Track attached session IDs in `DaemonClient`
- Re-attach all tracked sessions after reconnection with ring buffer replay
- Add 30s keepalive ping to detect broken connections proactively

**Result**: Fixed the sleep/wake freeze scenario. Did not fix freezes under heavy output.

---

## Attempt 2: Pipe deadlock and priority inversion

**PR**: [#49](https://github.com/alangmartini/godly-terminal/pull/49) — merged 2026-02-10
**Commit**: `4ed76f6`
**Branch**: `fix/terminal-freeze-pipe-deadlock`

**Hypothesis**: Three interacting issues caused the freeze:
1. **Pipe buffer too small (4KB)**: A single JSON-serialized output event exceeds 4KB, causing `write_all()` to block until the client reads. While blocked, the daemon I/O thread cannot read incoming requests, freezing ALL terminals.
2. **Daemon I/O write starvation**: The old loop wrote all queued messages before checking for incoming data. Under heavy output, user input (Write, Resize) was never read.
3. **Bridge priority inversion**: The bridge's `continue` after reading an event looped back to read more events before checking for outgoing requests. High-throughput output from one terminal starved input writes for all terminals.

**Fix**:
- Increase pipe buffer from 4KB to 256KB (262144 bytes)
- Limit daemon writes to 8 per iteration, then check for incoming data
- Limit bridge event reads to 8 per iteration, then check for requests
- Add file-based debug logging to daemon and bridge (`godly-daemon-debug.log`, `godly-bridge-debug.log` in `%APPDATA%/com.godly.terminal/`)

**Result**: Significantly reduced freeze frequency. Still occurred under sustained heavy output (e.g. Claude CLI generating large responses).

---

## Attempt 3: Missing timeouts on daemon IPC

**PR**: [#50](https://github.com/alangmartini/godly-terminal/pull/50) — merged 2026-02-10
**Commit**: `88dbdd0`
**Branch**: `fix/terminal-freeze-missing-timeouts`

**Hypothesis**: The bridge used infinite-blocking `recv()` in `try_send_request`. If the bridge I/O thread died or the daemon stopped responding, every caller hung forever on `response_rx.recv()`. Log evidence: bridge was healthy at 172s then stopped logging entirely.

**Fix**:
- Replace `recv()` with `recv_timeout(5s)` in `try_send_request`
- Drain `pending_responses` on bridge exit to unblock waiting callers with explicit error
- Increase ProcessMonitor poll interval from 1s to 2s to reduce pipe pressure
- Reduce keepalive interval from 30s to 10s for faster broken-connection detection

**Result**: Eliminated the infinite-hang variant. Callers now time out and can recover. But the root cause of the I/O thread dying was still unaddressed.

---

## Attempt 4: Bridge phase tracking + keepalive watchdog (diagnostic)

**PR**: [#51](https://github.com/alangmartini/godly-terminal/pull/51) — merged 2026-02-10
**Commit**: `8bdf534`
**Branch**: `fix/terminal-freeze-missing-timeouts` (same branch, second commit)

**Purpose**: Purely diagnostic — adds visibility into *where* the bridge I/O thread gets stuck.

**Changes**:
- `BridgeHealth` struct with atomic phase tracking (`AtomicU8` for current phase, `AtomicU64` for last activity timestamp)
- Bridge I/O thread updates phase before each potentially-blocking operation: `peek_pipe`, `read_message`, `emit_event`, `recv_request`, `write_message`
- Keepalive thread checks bridge health — if stalled >15s, logs the stuck phase and duration

**Result**: Provided the data that led to discovering the `emit()` blocking issue in Attempt 5.

---

## Attempt 5: Non-blocking emit channel

**PR**: [#52](https://github.com/alangmartini/godly-terminal/pull/52) — merged 2026-02-10
**Commit**: `777b496`
**Branch**: `fix/nonblocking-emit-keepalive-diagnostics`

**Hypothesis**: Tauri's `app_handle.emit()` is synchronous. When the main thread stalls (e.g. during heavy DOM rendering), any thread calling `emit()` blocks — including the bridge I/O thread AND the ProcessMonitor. The keepalive's `ping()` also blocks because it routes through the same bridge. All threads freeze simultaneously.

**Fix**:
- Route all hot-path `emit()` calls through a bounded non-blocking channel (`EventEmitter`) with a dedicated "tauri-emitter" thread
- Bridge I/O thread enqueues via `try_send()` (sub-microsecond) instead of blocking on `emit()`
- ProcessMonitor routes `process-changed` through emitter
- Rewrite keepalive: watchdog check runs BEFORE ping so it fires even when ping blocks

**Result**: Fixed the main-thread-stall variant. The bridge I/O thread no longer blocks on emit. But new freeze patterns emerged under even heavier output.

---

## Attempt 6: Keepalive diagnostics to log file

**PR**: [#53](https://github.com/alangmartini/godly-terminal/pull/53) — merged 2026-02-11
**Commit**: `7831b01`
**Branch**: `fix/keepalive-log-to-file`

**Purpose**: The installed app has no terminal for stderr. Route all keepalive diagnostics to `godly-bridge-debug.log` via `bridge_log()` so they're visible at `%APPDATA%/com.godly.terminal/godly-bridge-debug.log`.

**Result**: Operational improvement — made diagnostics available in production.

---

## Attempt 7: Eliminate frontend DOM thrashing

**PR**: [#55](https://github.com/alangmartini/godly-terminal/pull/55) — merged 2026-02-11
**Commit**: `54c406a`
**Branch**: `wt-c1a388`

**Hypothesis**: The frontend was causing progressive freeze through excessive DOM operations:
- Every `setState()` triggered all subscribers immediately, causing redundant renders under rapid-fire events
- `TabBar.render()` cleared `innerHTML` and recreated ALL tabs on every notification
- `ResizeObserver` fired per pixel of resize without debouncing, causing IPC call spam
- Window close handler blocked the main thread with a busy-wait for scrollback save

**Fix**:
- Batch store notifications with `requestAnimationFrame` — multiple rapid `setState()` calls trigger only one subscriber cycle per frame
- Add shallow change detection to `updateTerminal()` to skip no-op updates
- Make `TabBar.render()` incremental: diff existing DOM elements in-place
- Debounce `ResizeObserver` with `requestAnimationFrame`
- Move scrollback-save busy-wait to a background thread on window close

**Result**: Fixed the progressive UI freeze pattern. Frontend could now handle high-frequency events without degrading. But backend freeze under extreme output volume remained.

---

## Attempt 8: Session recovery after daemon crash/restart

**PR**: [#56](https://github.com/alangmartini/godly-terminal/pull/56) — merged 2026-02-11
**Commits**: `9939a51`, `69893f1`, `68e7029`
**Branch**: `fix/session-recovery-and-diagnostics`

**Hypothesis**: The daemon was dying unexpectedly and terminals appeared frozen because the app didn't know sessions were lost. Three sub-issues:

1. **Premature daemon exit**: `has_clients` was a single `AtomicBool`. When any client disconnected, it set `false` even if other clients were connected. The idle-timeout checker (no sessions + no clients + 5min → exit) killed the daemon during reconnections.
2. **Bridge log corruption**: `bridge_log_init()` re-opened the log with `truncate(true)` on reconnect but the `OnceLock` kept the old handle, creating a ~75KB null-byte gap.
3. **Silent session loss**: When the daemon restarted and old sessions were gone, nobody notified the frontend.

**Fix**:
- Replace `AtomicBool` with `AtomicUsize` client count using `fetch_add`/`fetch_sub`
- Skip bridge log re-open when `OnceLock` is already initialized
- Emit `sessions-lost` event and log when sessions vanish after reconnect

**Result**: Fixed the "daemon dies during reconnection" scenario. Terminals now show clear error state instead of silent freeze.

---

## Attempt 9: Split daemon I/O into response and event channels

**PR**: [#57](https://github.com/alangmartini/godly-terminal/pull/57) — merged 2026-02-11
**Commits**: `ed88b0a`, `7d981db`, `c030506`
**Branch**: `fix/terminal-freeze-under-heavy-output`

**Hypothesis**: The daemon used a single unbounded channel for both responses and output events. Under heavy terminal output (e.g. Claude CLI), hundreds of output events queued ahead of responses, causing the client to time out after 5s waiting for Write/Resize/Ping acknowledgment.

Additionally, the singleton check (`is_daemon_running`) had a TOCTOU race — between process start and pipe creation, multiple daemons could pass the check and run in parallel with isolated session stores.

**Fix**:
- **Split channels**: High-priority response channel (always drained first) + normal-priority event channel (batch-limited). I/O thread writes all pending responses before any output events.
- **Named mutex**: Replace pipe-based singleton check with `CreateMutexW` for atomic, race-free enforcement. The mutex auto-releases on crash.
- **Safety margins**: EventEmitter capacity 256 → 4096 (~16MB buffer), response timeout 5s → 15s.

**Result**: Response starvation fixed — user input no longer stalls under heavy output. Multiple daemon instances eliminated. But mutex starvation in the session layer was discovered as a remaining freeze vector.

---

## Attempt 10: Preserve daemon logs across restarts (diagnostic)

**PR**: [#59](https://github.com/alangmartini/godly-terminal/pull/59) — merged 2026-02-11
**Commit**: `2adbfbc`
**Branch**: `fix/terminal-freeze-under-heavy-output`

**Hypothesis**: The daemon crashed and we had no way to diagnose it — the new daemon truncated the log on startup, destroying all evidence.

**Fix**:
- Switch daemon and bridge logs from truncate to append mode
- Add log rotation at 2MB (current → `.prev.log`)
- Install panic hook (daemon has no console; panics were silently lost)
- Add periodic HEALTH logs: session count, client count, memory usage
- Log channel queue depths (resp_queue, event_queue) in io_thread stats
- Log ring buffer size and attachment state in session reader stats

**Result**: Operational improvement — crash evidence now survives restarts. Enabled root cause analysis for future occurrences.

---

## Attempt 11: Mutex starvation under heavy output

**PR**: [#60](https://github.com/alangmartini/godly-terminal/pull/60) — merged 2026-02-12
**Commit**: `1e6a101`
**Branch**: `fix/mutex-starvation-heavy-output`

**Hypothesis**: Under heavy output, the PTY reader thread holds `output_tx` and `ring_buffer` locks in a tight loop. Handler threads that need these locks for `attach()`, `detach()`, or `is_attached()` block indefinitely, causing the handler loop to stall. Since the handler is sequential, ALL terminals freeze — no Write, Resize, Ping, or Attach can be processed. The bridge detects the stall and reconnects, but the new handler also blocks on `session.attach()` → `output_tx.lock()`, causing unbounded client accumulation (2, 3, 4, 5...).

**Evidence from daemon log**:
- `[34.464s] Received request: ListSessions` — no "Sending response" ever follows
- `io_thread stats: reads=100, writes=150 (resp=96)` — 4 requests never responded
- Client count grows: `clients=1` → `clients=2` → ... → `clients=5` (handlers never exit)

**Fix**:
- **Lock-free `is_attached()`**: Added `is_attached_flag: Arc<AtomicBool>` so `is_attached()` and `info()` read an atomic instead of locking `output_tx`. Prevents handler stalls on `ListSessions`/info queries.
- **Timeout-based locking in `attach()`**: `ring_buffer.try_lock_for(2s)` and `output_tx.try_lock_for(2s)` instead of unconditional `.lock()`. Returns empty buffer/skips sender setup if timeout expires, preventing indefinite handler blocking.
- **Reader thread yield**: After successful channel send, the reader thread calls `thread::yield_now()` to give handler threads a chance to acquire the locks.
- **Dead I/O thread detection**: Handler loop checks `io_running` flag each iteration and breaks if the I/O thread has died, preventing stuck handler accumulation across reconnects.

**Result**: Handler threads no longer block on session locks under heavy output. `ListSessions` and `info()` are entirely lock-free. `attach()` degrades gracefully (empty buffer replay) instead of blocking forever. Stale handlers exit promptly when the pipe dies.

---

## Related fixes (not directly freeze, but contributing factors)

### Ctrl+C interrupt failure in ConPTY

**PR**: [#58](https://github.com/alangmartini/godly-terminal/pull/58) — merged 2026-02-11
**Commit**: `f652bee`

ConPTY does not translate raw `\x03` written to its input pipe into `CTRL_C_EVENT` for child processes. Fix: detect `\x03` in `DaemonSession::write()` and terminate child processes via process tree enumeration (`CreateToolhelp32Snapshot`).

This contributed to perceived "freeze" when users couldn't interrupt long-running processes.

### Double paste and broken Ctrl+C in frontend

**PR**: [#34](https://github.com/alangmartini/godly-terminal/pull/34) (within `wt-a67d6e`)
**Commit**: `2fddd68`

Missing `event.preventDefault()` calls caused double paste and Ctrl+C interception by WebView2. Fixed by adding prevention on clipboard handlers and keyup events.

### Job Object kills daemon on dev exit

**PRs**: [#8](https://github.com/alangmartini/godly-terminal/pull/8), [#9](https://github.com/alangmartini/godly-terminal/pull/9) — merged 2026-02-07

`cargo tauri dev` creates a Windows Job Object that kills the daemon when cargo exits. Fixed by launching daemon via WMI to escape the Job Object.

### Pipe busy during reconnection

**PR**: [#3](https://github.com/alangmartini/godly-terminal/pull/3) — merged 2026-02-07
**Commit**: `fix/terminal-persistence-bugs`

`is_daemon_running()` and `try_connect()` treated `INVALID_HANDLE_VALUE` as "daemon not running" without checking for `ERROR_PIPE_BUSY`. This caused duplicate daemons to spawn, overwriting the PID file and orphaning sessions.

---

## Diagnostic infrastructure added

| Tool | Location | Purpose |
|------|----------|---------|
| Daemon debug log | `%APPDATA%/com.godly.terminal/godly-daemon-debug.log` | Timestamped daemon events, slow operation detection |
| Bridge debug log | `%APPDATA%/com.godly.terminal/godly-bridge-debug.log` | Bridge I/O operations, keepalive diagnostics |
| Bridge phase tracking | `BridgeHealth` struct | Atomic tracking of which operation the bridge is stuck on |
| Keepalive watchdog | `start_keepalive()` thread | Detects bridge stalls >15s, logs stuck phase |
| HEALTH logs | Periodic daemon logging | Session count, client count, memory (working set) |
| Channel depth logs | I/O thread stats | Detect unbounded channel growth under backpressure |
| Panic hook | Daemon `main()` | Captures panics to log file (daemon has no console) |
| Log rotation | 2MB limit | Current → `.prev.log` to avoid unbounded growth |

---

## Root cause summary

The terminal freeze is not a single bug but a cascade of interacting issues across the IPC pipeline:

```
PTY output (heavy) → reader thread (holds locks) → ring buffer / output_tx
                                                          ↓
daemon I/O thread ← response/event channels ← session handlers (wait for locks)
       ↓
  named pipe (was 4KB, now 256KB)
       ↓
bridge I/O thread → EventEmitter → Tauri emit (was synchronous, now async)
       ↓
  frontend store → DOM updates (was thrashing, now RAF-batched)
```

Each layer had its own bottleneck. Fixing one revealed the next. All known layers have now been addressed through 11 fix attempts.
