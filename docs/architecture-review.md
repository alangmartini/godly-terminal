# Architecture Review: Bottleneck Analysis

**Date:** 2026-02-14
**Branch:** wt-arch-review

## Overview

A thorough review of the Godly Terminal architecture identifying performance bottlenecks across the daemon IPC layer, frontend rendering pipeline, and persistence/state management. Findings are ranked by severity and include specific file locations and suggested fixes.

---

## 1. CRITICAL: Binary Data Serialized as JSON Number Arrays

**Impact: Every single byte of terminal output is inflated ~4x across the entire pipeline.**

The `Vec<u8>` fields in `messages.rs` are serialized by serde_json as JSON arrays of integers:

```
"hello" → [104, 101, 108, 108, 111]   // 5 bytes → ~30 bytes of JSON
```

This affects three high-frequency message types:

| Location | Field | Hot path? |
|----------|-------|-----------|
| `protocol/src/messages.rs:29` | `Request::Write { data: Vec<u8> }` | Every keystroke |
| `protocol/src/messages.rs:63` | `Response::Buffer { data: Vec<u8> }` | Every reattach |
| `protocol/src/messages.rs:72` | `Event::Output { data: Vec<u8> }` | Every PTY output event |

A 4KB PTY read becomes ~16KB of JSON. A 1MB ring buffer replay becomes ~4MB. This is the single highest-impact bottleneck because it sits on the hottest code path (terminal output) and inflates data at both the daemon-to-client pipe and the Tauri-to-frontend IPC boundary.

The same inflation hits the frontend too. In `bridge.rs:79-85`, the Tauri emit serializes `data: Vec<u8>` into the event payload as a JSON number array. The frontend receives it as `number[]` (`terminal-service.ts:20`) and converts back via `new Uint8Array(data)` (`terminal-service.ts:62`).

**Fix:** Add `#[serde(with = "serde_bytes")]` to all `data: Vec<u8>` fields in `messages.rs`. This serializes as a byte string instead of an array of numbers, reducing wire size to ~1.3x (base64 in JSON) instead of ~4x.

---

## 2. HIGH: Scrollback Save Creates Million-Element JS Array

`TerminalPane.ts:231`:
```ts
await invoke('save_scrollback', {
  terminalId: this.terminalId,
  data: Array.from(data),  // data is Uint8Array up to ~5MB
});
```

`Array.from()` on a multi-MB `Uint8Array` creates a JS `number[]` with millions of elements, each requiring JSON serialization as individual integers. A 2MB scrollback becomes a ~10MB JSON payload just for the IPC call.

This runs every 5 minutes per terminal (`startScrollbackSaveInterval`) and on every terminal destroy.

**Fix:** Pass the `Uint8Array` directly through Tauri IPC (which supports binary), or base64-encode it.

---

## 3. HIGH: Process Monitor Takes Recursive System Snapshots

`process_monitor.rs:139-171`: `find_deepest_child()` calls `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)` at each recursion level. This enumerates **all processes on the system** for each level of the process tree.

With N terminals and average process tree depth D, this creates `N * D` full process snapshots every 2 seconds. On a system with hundreds of processes, each snapshot is expensive.

**Fix:** Take **one** snapshot, build a `HashMap<pid, Vec<child_pid>>` index, then walk the tree in memory. Reduces O(N*D) snapshots to O(1).

---

## 4. MEDIUM: Dual Ring Buffer Doubles Lock Contention on Reader Thread

`session.rs:178-181` -- every PTY read acquires the `output_history` lock:
```rust
{
    let mut history = reader_history.lock();
    append_to_ring(&mut history, &buf[..n]);
}
```

Then it acquires the `output_tx` lock to check if a client is attached (line 185). The `output_history` buffer exists solely for the MCP `ReadBuffer` command, which is used infrequently, yet it doubles the lock/copy cost on every single read from the PTY.

**Fix:** Consider making `output_history` a lock-free ring (e.g., `crossbeam`), or only populate it when an MCP client has explicitly requested it.

---

## 5. MEDIUM: read_output_history / search_output_history Triple Allocation

`session.rs:480-508`:
```rust
pub fn read_output_history(&self) -> Vec<u8> {
    self.output_history.lock().iter().copied().collect()  // alloc #1: copy 1MB
}

pub fn search_output_history(&self, text: &str, do_strip_ansi: bool) -> bool {
    let data = self.output_history.lock().iter().copied().collect::<Vec<u8>>();  // alloc #1
    let haystack = String::from_utf8_lossy(&data);  // alloc #2
    if do_strip_ansi {
        godly_protocol::ansi::strip_ansi(&haystack).contains(text)  // alloc #3
    }
}
```

Each `ReadBuffer` or `SearchBuffer` call copies up to 1MB from the `VecDeque`, converts to `String`, and optionally strips ANSI. Under MCP polling (e.g. `wait_for_text` loops), this generates significant allocation pressure.

**Fix:** Search directly on the `VecDeque` bytes without copying. For ANSI stripping, use a streaming scanner that operates on the `VecDeque` slices.

---

## 6. MEDIUM: detach_all_sessions Holds Lock During Sequential IPC

`terminal.rs:360-369`:
```rust
pub fn detach_all_sessions(...) -> Result<(), String> {
    let terminals = state.terminals.read();  // holds read lock
    for terminal_id in terminals.keys() {
        let _ = daemon.send_request(&request);  // up to 15s timeout each
        daemon.track_detach(terminal_id);
    }
    Ok(())
}
```

The `terminals` read lock is held while sending sequential IPC requests. With 10 terminals and a slow daemon, this could hold the lock for up to 150 seconds, blocking all other terminal state reads.

**Fix:** Clone the keys into a `Vec<String>`, drop the lock, then iterate.

---

## 7. LOW: attach_session Makes Two Sequential IPC Round-Trips

`terminal.rs:294-309`:
```rust
let response = daemon.send_request(&attach_request)?;  // round-trip 1: Attach
// ...
let sessions_response = daemon.send_request(&Request::ListSessions)?;  // round-trip 2
```

Every reattach pays for two sequential IPC round-trips. The second could be eliminated by including session metadata in the `Attach` response.

---

## 8. LOW: WorkspaceSidebar Full DOM Rebuild

`WorkspaceSidebar.ts:227`:

The sidebar render method destroys all children via setting container content to empty, then recreates all elements on every state change. Unlike `TabBar` which does incremental reconciliation, this drops event listeners, forces layout reflow, and creates GC pressure. Low impact because workspaces change rarely, but it is a pattern to avoid.

---

## 9. LOW: EventEmitter Silently Drops Terminal Output

`bridge.rs:57-69`: When the 4096-entry emitter channel fills (possible during extreme output bursts), events are dropped with only an atomic counter increment. This means terminal output can be silently lost without the user knowing.

The 4096 capacity provides good headroom (~16MB of buffered events), so this is unlikely in practice, but there is no recovery mechanism or user notification if it does happen.

---

## Summary

| # | Severity | Bottleneck | Where | Fix Complexity |
|---|----------|-----------|-------|----------------|
| 1 | **CRITICAL** | Binary data as JSON number arrays | `messages.rs` + `bridge.rs` | Low (`serde_bytes`) |
| 2 | **HIGH** | Scrollback `Array.from()` on multi-MB buffer | `TerminalPane.ts:231` | Low |
| 3 | **HIGH** | Recursive process snapshots | `process_monitor.rs:139` | Low-Medium |
| 4 | **MEDIUM** | Dual ring buffer lock contention | `session.rs:178` | Medium |
| 5 | **MEDIUM** | Triple allocation in search/read history | `session.rs:480-508` | Medium |
| 6 | **MEDIUM** | Lock held during sequential IPC | `terminal.rs:360` | Low |
| 7 | **LOW** | Double IPC round-trip on attach | `terminal.rs:294` | Medium (protocol change) |
| 8 | **LOW** | Sidebar full DOM rebuild | `WorkspaceSidebar.ts:227` | Low |
| 9 | **LOW** | Silent event drops under load | `bridge.rs:57` | Low |

Items 1-3 are the highest-ROI fixes. Item 1 alone would reduce wire traffic on the terminal output hot path by ~75%.
