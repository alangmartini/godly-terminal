# Terminal Freeze Investigation Log

All fix attempts for the terminal freezing bug, in chronological order. The freeze manifests as the terminal becoming unresponsive to input and/or ceasing to display output, requiring the user to close and reopen the app.

**Status**: Partially resolved. Multiple root causes identified and fixed across 12 attempts. The full IPC pipeline has been hardened — pipe buffers, channel priority, emit blocking, DOM thrashing, daemon crashes, response starvation, mutex starvation, and unbounded event accumulation have all been addressed.

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

## Attempt 12: Backpressure + output coalescing + crash diagnostics

**PR**: [#63](https://github.com/alangmartini/godly-terminal/pull/63)
**Branch**: `feat/godly-notify-cli`

**Hypothesis**: Prior attempts fixed pipeline bottlenecks but never addressed the **source** — events accumulated unboundedly. Daemon logs showed the unbounded event channel grew to **170,388 events** in 4.7 seconds, pipe writes stalled 2.4s (bridge not reading fast enough), and the "Write batch limit hit" log fired hundreds of times/sec generating a 42MB log file. Additionally, the daemon sometimes dies silently — no panic, no error, no exception logged.

**Evidence from daemon log**:
- `event_queue=170388` — unbounded channel grew to 170K events in 4.7s
- `SLOW WRITE: Output took 2438.2ms` — pipe write blocked 2.4s
- 42MB log file from "Write batch limit hit" log spam (hundreds/sec)
- Daemon disappeared with zero log entries about the cause

**Fix**:
- **Larger reader buffer (4KB → 64KB)**: ConPTY `ReadFile` returns whatever's available. Under heavy load, bigger buffer = fewer, larger events. Average event was 126 bytes — 64KB buffer coalesces dozens of small ANSI sequences per read.
- **Bounded per-session channel (unbounded → 64)**: `try_send()` with three-way match: `Ok` → success, `Full` → fall back to ring buffer (backpressure), `Closed` → client disconnected. No data lost.
- **Bounded event channel (unbounded → 1024)**: Caps memory at ~1024 + (64 × N sessions) events max, vs 170K+ observed. Forwarding task uses `.send().await` which suspends when full → per-session channel fills → reader falls back to ring buffer. True end-to-end backpressure.
- **Write batch limit 8 → 128**: More events per iteration reduces loop overhead.
- **Deleted log spam**: Removed "Write batch limit hit" `daemon_log!` that generated 42MB log files.
- **Windows exception handler**: `SetUnhandledExceptionFilter` catches ACCESS_VIOLATION (0xC0000005), STACK_OVERFLOW (0xC00000FD), HEAP_CORRUPTION (0xC0000374), STACK_BUFFER_OVERRUN (0xC0000409) and writes exception code + address to log file using `try_lock()` to avoid deadlock.

**Backpressure flow**:
```
PTY → reader (64KB buf) → try_send per-session channel (bounded 64)
                               |                    |
                          (Full)                (OK)
                               ↓                    ↓
                         Ring buffer (1MB)    Forwarding task
                                                    ↓
                                          send().await event channel (bounded 1024)
                                              (blocks when full)
                                                    ↓
                                          I/O thread (128/iter) → pipe → bridge
```

**Result**: Event accumulation is now bounded. Under heavy output, the system gracefully degrades by storing excess data in ring buffers (which are already size-capped at 1MB) instead of accumulating unbounded events in memory. Silent daemon crashes will now leave diagnostic evidence in the log.

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
| Exception handler | `SetUnhandledExceptionFilter` | Catches ACCESS_VIOLATION, STACK_OVERFLOW, HEAP_CORRUPTION, STACK_BUFFER_OVERRUN |
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

Each layer had its own bottleneck. Fixing one revealed the next. Attempt 12 attacks the source — preventing unbounded event accumulation via backpressure, reducing event frequency via larger reads, and adding crash diagnostics for silent daemon deaths. All known layers have now been addressed through 12 fix attempts.

---

# Potential Remaining Causes

Deep-dive analysis from 5 independent perspectives (PTY/reader thread, IPC pipeline, frontend rendering, daemon lifecycle, concurrency). Findings below are NEW issues not addressed by Attempts 1–12, organized by category and severity.

---

## Category A: Data Flow & Output Loss

### A1. Ring buffer fallback data never replayed to attached client
**Severity**: HIGH
**File**: `daemon/src/session.rs:175-179`

When the per-session output channel is full (`TrySendError::Full`), data is written to the ring buffer as a fallback. However, this data is **never replayed to the still-attached client**. The ring buffer is only drained during `attach()` (line 269-278). If the channel fills transiently during heavy output and then drains, the data stored during the backpressure window is stranded — the client never receives it.

**Freeze mechanism**: Missing output in the terminal. In the worst case, a VT sequence like `\x1b[?25l` (hide cursor) gets sent via the channel but the matching `\x1b[?25h` (show cursor) goes to the ring buffer instead. The terminal appears frozen with no visible cursor. This was *introduced* by Attempt 12's backpressure mechanism — the ring buffer fallback was designed for detached sessions but is now also used during transient channel fullness while the client is attached.

### A2. No SessionClosed event when PTY process exits
**Severity**: HIGH
**Files**: `daemon/src/session.rs:146-148`, `daemon/src/server.rs:924-940`

When the reader thread detects EOF (`Ok(0)`) or a read error, it logs the event and exits. It does **not** set `running` to false, does not emit a `SessionClosed` event, and does not notify the attached client. The session stays in the `sessions` HashMap forever. The `SessionClosed` event is only sent on explicit `CloseSession` request — there is no mechanism for the daemon to proactively notify the client that a session's PTY has exited.

**Freeze mechanism**: If a shell process exits (user types `exit`, process crashes), the terminal tab remains open with no indication the process is dead. The user can type but nothing happens. This looks exactly like a freeze. Dead sessions also accumulate forwarding tasks (tokio) that never complete, since the `rx` channel stays open.

### A3. Race between attach() and reader thread — data gap
**Severity**: MEDIUM
**File**: `daemon/src/session.rs:265-293` vs `session.rs:155-202`

`attach()` drains the ring buffer (line 269), then sets `output_tx` (line 281-288), then sets `is_attached_flag` (line 291). Between draining the ring buffer and setting the channel, the reader thread can push data into the ring buffer (seeing `output_tx` is `None`). That data is never delivered. Under heavy output the window is large enough for multiple chunks to be lost.

**Freeze mechanism**: Data ordering gap after reattach — client gets old ring buffer, misses some data, then gets the live stream. Garbled terminal state.

### A4. Reader thread TOCTOU — attach() sender overwritten by stale cleanup
**Severity**: MEDIUM
**File**: `daemon/src/session.rs:189-191`

In the `TrySendError::Closed` branch, the reader thread: (1) drops `tx_guard` (releases lock), (2) sets `reader_attached = false`, (3) re-locks `output_tx` to set it to `None`. Between steps 1 and 3, `attach()` can run and install a new sender. The reader thread then overwrites it with `None`, losing the new attachment.

**Freeze mechanism**: Client thinks it's attached, but output stops flowing. Terminal appears frozen.

---

## Category B: IPC Pipeline & Serialization

### B1. JSON 4x serialization amplification on Vec<u8>
**Severity**: MEDIUM-HIGH
**File**: `protocol/src/messages.rs:59`, `protocol/src/frame.rs:8`

Output events carry `data: Vec<u8>`. The frame protocol uses `serde_json::to_vec()`. JSON encoding of `Vec<u8>` serializes as a **JSON array of numbers**: `[27, 91, 51, 50, 109, ...]`. A 64KB read yields ~262KB of JSON — a **4x amplification** on the hot path.

**Freeze mechanism**: The 256KB pipe buffer effectively holds only ~64KB of actual terminal data. Under heavy output, the pipe fills 4x faster than expected, causing daemon `write_message` to block, stalling the I/O thread and all request processing. This is likely a significant contributor to the "SLOW WRITE" stalls observed in daemon logs (Attempt 12 saw 2.4s stalls).

**Fix**: Add `#[serde(with = "serde_bytes")]` on `Vec<u8>` fields (serializes as base64 or byte string) or switch to bincode/MessagePack.

### B2. Bidirectional pipe buffer deadlock potential
**Severity**: MEDIUM
**File**: `daemon_client/bridge.rs:279-284`

Reader and writer are DuplicateHandle'd from the same file object. On Windows, synchronous named pipes serialize I/O per file object. If `write_message` blocks (pipe buffer full), `PeekNamedPipe` on the reader handle is serialized behind it. The bridge cannot read responses while a write is blocked, and the daemon may be trying to send a response but its own buffer is full too.

**Freeze mechanism**: Full bidirectional deadlock — neither side can make progress. The 256KB buffers make this rare, but the 4x JSON amplification (B1) reduces effective buffer to ~64KB, making it more likely under sustained heavy output.

### B3. DuplicateHandle File drop race
**Severity**: MEDIUM
**File**: `daemon_client/client.rs:142-172`

Both `File` objects (reader/writer) own independent handles to the same pipe via `DuplicateHandle`. During reconnection teardown, if one `File` is dropped while the other is still being read in the I/O thread, the read could hang on an invalid handle rather than returning an error.

**Freeze mechanism**: Bridge I/O thread hangs on invalid handle during reconnection.

### B4. Partial frame read blocks on broken pipe
**Severity**: MEDIUM
**File**: `protocol/src/frame.rs:22-28`

If the pipe breaks mid-frame (after 1-3 bytes of the 4-byte length prefix), `read_exact` blocks waiting for the remaining bytes until the OS detects the broken pipe — which on Windows can take seconds. Also, no maximum message size validation before allocation — a corrupted length prefix of 15MB causes a 15MB allocation + blocking read.

**Freeze mechanism**: Bridge I/O thread blocked for seconds on partial frame during daemon crash.

### B5. Unsafe raw pointer cast for handle extraction
**Severity**: MEDIUM (correctness)
**File**: `daemon_client/bridge.rs:505-513`

```rust
let reader_ptr = &**reader as *const dyn Read as *const std::fs::File;
unsafe { (*reader_ptr).as_raw_handle() as isize }
```

Casts a trait object pointer to `*const std::fs::File`. Only valid if the concrete type is `File`. If the reader type ever changes (e.g., wrapped in BufReader), this is undefined behavior — `PeekNamedPipe` gets a garbage handle.

---

## Category C: Frontend & xterm.js

### C1. xterm.js write() called per-event without batching
**Severity**: HIGH
**File**: `src/services/terminal-service.ts:56-63`, `src/components/TerminalPane.ts:146-151`

Each `terminal-output` Tauri event triggers a separate `this.terminal.write(data)` call. Under heavy output (hundreds of events/sec), every `write()` triggers xterm.js's internal parser. While xterm.js batches renders via RAF, the parser work for each individual `write()` accumulates as microtasks.

**Freeze mechanism**: Main thread saturated with queued microtasks from hundreds of individual `write()` calls per frame. Combined with C4 (hidden terminals), this multiplies by total terminal count.

**Fix**: Collect output data in a buffer and flush once per animation frame.

### C2. Hidden terminals still parse all output
**Severity**: HIGH
**File**: `src/components/TerminalPane.ts:146-151`, `src/components/App.ts:104-110`

When a TerminalPane is not visible (`setActive(false)` toggles a CSS class), output subscription is **never paused**. The xterm.js instance continues to parse all output. The renderer skips off-screen terminals, but the **parser** still runs for every `write()` call.

**Freeze mechanism**: With N terminals and M events/sec, the main thread does N×M parse operations. Only 1 terminal is visible, but all N are processing. For 10 terminals under heavy output, this is a 10x overhead.

**Fix**: Pause output subscription for inactive terminals and replay from ring buffer on reactivation.

### C3. Synchronous multi-MB scrollback serialization
**Severity**: MEDIUM
**File**: `src/components/TerminalPane.ts:167-181`

Every 5 minutes, `saveScrollback()` serializes the entire terminal buffer (up to 10,000 lines) via `serializeAddon.serialize()`. Then `new TextEncoder().encode()` creates a copy. Then `Array.from(data)` creates a THIRD copy (JS array of numbers for Tauri IPC).

**Freeze mechanism**: Synchronous multi-hundred-ms main thread block every 5 minutes. Multiple terminals can align their intervals, multiplying the impact.

### C4. WorkspaceSidebar still does full DOM rebuild
**Severity**: MEDIUM
**File**: `src/components/WorkspaceSidebar.ts:224-233`

Unlike TabBar (fixed in Attempt 7), `WorkspaceSidebar.render()` still uses `innerHTML = ''` and recreates ALL elements. Worse, `notificationStore.notify()` is **synchronous** (bypasses RAF batching), triggering a full sidebar rebuild outside the coalescing mechanism.

**Freeze mechanism**: Every notification event causes a synchronous full DOM rebuild, bypassing the batching added in Attempt 7.

### C5. process-changed triggers N render cycles every 2 seconds
**Severity**: MEDIUM
**File**: `src/services/terminal-service.ts:66-71`

ProcessMonitor fires `process-changed` every 2 seconds for every terminal. With N terminals, that's N state changes per cycle. If they arrive across different RAF frames, each triggers a separate render pass.

**Freeze mechanism**: Linear degradation with terminal count — 10+ terminals = 10+ render cycles every 2 seconds.

---

## Category D: Daemon Lifecycle & Resource Leaks

### D1. Child process handle leak via immortal thread
**Severity**: HIGH
**File**: `daemon/src/session.rs:237-240`

```rust
thread::spawn(move || {
    let _ = child;
});
```

Every session spawns a thread to hold the child handle alive. This thread never exits — `let _ = child` drops the handle immediately, but the thread itself persists. After many create/close cycles, threads accumulate (~1MB stack each on Windows). Additionally, the child process exit code is never reaped, leaking kernel process objects.

**Freeze mechanism**: After ~2000 sessions, 2GB of committed stack memory. Under memory pressure, the OS pages aggressively, stalling all daemon threads.

### D2. Reader thread not joined on session close
**Severity**: HIGH
**File**: `daemon/src/session.rs:124-235, 379-385`

`session.close()` sets `running = false`, but the reader thread blocks on `reader.read()` which is NOT interrupted by an AtomicBool. The thread only exits on PTY EOF or read error. If the shell process hangs (frozen WSL, network-waiting process), the reader thread and its resources (65KB buffer, Arc references) persist indefinitely.

**Freeze mechanism**: Zombie threads + resource accumulation from hung sessions.

### D3. Forwarding task leak on attach/detach cycles
**Severity**: HIGH
**File**: `daemon/src/server.rs:836-849`

Every `Attach` spawns a `tokio::spawn` forwarding task. On detach, the old channel sender is dropped, but if buffered items remain, the task drains them before exiting. If re-attach happens before the old task exits, TWO forwarding tasks exist for the same session. The old task holds a `msg_tx` clone, preventing event channel cleanup.

**Freeze mechanism**: Rapid reconnection scenarios accumulate dozens of tokio tasks holding channel senders.

### D4. accept_connection blocks indefinitely on shutdown
**Severity**: MEDIUM
**File**: `daemon/src/server.rs:186-232`

`ConnectNamedPipe` with `PIPE_WAIT` blocks indefinitely. When idle timeout fires and sets `running = false`, the daemon can't exit because `accept_connection().await` is stuck in `ConnectNamedPipe`. No cancellation mechanism exists.

**Freeze mechanism**: Daemon can't shut down. New daemon refuses to start (old holds mutex). App connects to zombie daemon that immediately exits. User sees a hang during connection attempts.

### D5. DaemonLock doesn't handle abandoned mutex
**Severity**: MEDIUM
**File**: `daemon/src/pid.rs:109-141`

If the previous daemon was killed without releasing the mutex, `CreateMutexW` returns `ERROR_ALREADY_EXISTS`. The new daemon assumes another instance is running and exits. In reality, the old daemon is dead.

**Freeze mechanism**: After a daemon crash (Task Manager kill, etc.), the new daemon refuses to start for a brief window until the kernel garbage-collects the abandoned mutex. User sees no daemon and frozen terminals.

### D6. Exception handler may crash on stack overflow
**Severity**: MEDIUM
**File**: `daemon/src/debug_log.rs:135-176`

The exception handler attempts string formatting and mutex acquisition during stack overflow, when only 4KB of stack remains. This may cause a double-fault that terminates the process without logging. Also, returning `EXCEPTION_EXECUTE_HANDLER` calls `ExitProcess()` which may not clean up named pipe handles properly.

**Freeze mechanism**: Daemon vanishes without evidence. Stale pipe handles confuse reconnection.

---

## Category E: Concurrency & Locking

### E1. All Tauri commands blocked during reconnect()
**Severity**: HIGH
**File**: `daemon_client/client.rs:377-432`

`reconnect()` holds `reconnect_lock` while performing multiple operations including `send_request` calls (re-attach) that can themselves block for up to 15s. ALL other Tauri command threads (write, resize, create, close) pile up behind `reconnect_lock` via the `send_request -> reconnect` path. The keepalive thread also blocks.

**Freeze mechanism**: Every terminal operation hangs during reconnection. If re-attach is slow (daemon under load), the entire app freezes for up to 15s × N sessions.

### E2. Blocking PTY write under sessions read lock
**Severity**: MEDIUM
**File**: `daemon/src/session.rs:321-357`

`write()` acquires `self.writer.lock()` and calls `write_all()` which can block under ConPTY backpressure. The `handle_request` function holds `sessions.read()` while calling `session.write()`. If the PTY write blocks, the read lock prevents `CloseSession` (needs write lock) from proceeding.

**Freeze mechanism**: ConPTY backpressure cascades to session management stall, blocking all session operations for this client.

### E3. Sync Tauri commands with 15s timeout exhaust thread pool
**Severity**: MEDIUM
**File**: `commands/terminal.rs` (all command handlers)

All terminal commands are synchronous `#[tauri::command]` handlers that call `send_request()` with a 15s timeout. `create_terminal` calls it twice (up to 30s). Under reconnection, all command threads block on `reconnect_lock`, potentially exhausting Tauri's thread pool.

**Freeze mechanism**: Thread pool saturation — no new commands can be dispatched while existing ones are blocked.

### E4. is_attached_flag desync from output_tx
**Severity**: MEDIUM (correctness)
**File**: `daemon/src/session.rs:190,291,298`

The `is_attached_flag` AtomicBool can desynchronize from the actual `output_tx` state due to non-atomic multi-step updates in `attach()`, `detach()`, and the reader thread's `TrySendError::Closed` handler. On x86 this is masked by TSO, but formally incorrect.

**Impact**: Stale attachment state reported in `info()`. Not a direct freeze cause but contributes to confusing diagnostics.

---

## Priority Matrix

### Critical (fix first — likely remaining freeze causes)

| ID | Issue | Category | Impact |
|----|-------|----------|--------|
| A1 | Ring buffer fallback data never replayed | Data Flow | Missing output / garbled VT state under load |
| A2 | No SessionClosed on PTY exit | Data Flow | Dead terminals appear permanently frozen |
| C1 | xterm.js write() per-event, no batching | Frontend | Main thread saturation under heavy output |
| C2 | Hidden terminals parse all output | Frontend | N× CPU overhead (N = total terminal count) |
| E1 | All commands blocked during reconnect | Concurrency | Full app freeze during reconnection |

### High (significant contributors)

| ID | Issue | Category | Impact |
|----|-------|----------|--------|
| B1 | JSON 4x serialization amplification | IPC | Pipe fills 4x faster, causes stalls |
| D1 | Child handle thread leak | Daemon | Memory exhaustion over time |
| D2 | Reader thread not joined | Daemon | Zombie threads from hung sessions |
| D3 | Forwarding task leak | Daemon | Task accumulation on reconnects |
| A4 | Reader TOCTOU overwrites new sender | Data Flow | Output stops after race |

### Medium (contributing factors)

| ID | Issue | Category | Impact |
|----|-------|----------|--------|
| B2 | Bidirectional pipe buffer deadlock | IPC | Full deadlock under extreme load |
| B3 | DuplicateHandle File drop race | IPC | Hang during reconnection |
| B4 | Partial frame read blocks | IPC | Seconds-long hang on daemon crash |
| C3 | Sync scrollback serialization | Frontend | Periodic multi-100ms freezes |
| C4 | WorkspaceSidebar sync DOM rebuild | Frontend | Bypasses RAF batching |
| C5 | process-changed N renders/2s | Frontend | Linear degradation with terminals |
| D4 | accept_connection blocks shutdown | Daemon | Daemon can't exit on idle |
| D5 | Abandoned mutex not handled | Daemon | New daemon won't start after crash |
| E2 | PTY write under sessions lock | Concurrency | ConPTY backpressure cascade |
| E3 | Thread pool exhaustion | Concurrency | All commands blocked on timeout |

### Low (correctness/robustness)

| ID | Issue | Category | Impact |
|----|-------|----------|--------|
| A3 | Attach data gap | Data Flow | Brief data loss on reattach |
| B5 | Unsafe raw pointer cast | IPC | UB if reader type changes |
| D6 | Exception handler stack overflow | Daemon | No crash evidence |
| E4 | is_attached_flag desync | Concurrency | Stale diagnostics |
