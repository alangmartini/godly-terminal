# Output Flood Slowdown

## Issue

Terminal becomes extremely slow when pasting rapid-output commands like:
```powershell
while($true) { "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaas{}
```

## Root Cause

Three compounding issues in the output pipeline:

### 1. Useless data serialization in terminal-output events (PRIMARY)

Every PTY output chunk (up to 65KB binary) was being serialized as a JSON number array in the `terminal-output` Tauri event. A 65KB binary payload becomes ~260KB of JSON (each byte → number + comma). Under sustained output (hundreds of chunks/second), this created **megabytes/second of JSON serialization overhead**.

The frontend **never used this data** — it only used the event as a notification signal to fetch a grid snapshot via IPC.

### 2. No event coalescing in the emitter thread

The emitter thread called `app_handle.emit()` for every single output event with no deduplication. Under sustained output, this flooded the Tauri event system with thousands of events per second, all carrying the same message: "terminal X has new output".

### 3. No minimum interval between snapshot fetches

The frontend used `setTimeout(0)` to coalesce snapshot requests, but this only collapsed events within a single microtask. Under sustained output, the fetch→render→fetch cycle ran as fast as IPC allowed (~12fps at 85ms round-trip), with no frame budget enforcement.

## Fix

### Layer 1: Remove data from terminal-output events
- `EmitPayload::TerminalOutput` no longer carries a `data: Vec<u8>` field
- The JSON payload shrinks from ~260KB to ~50 bytes per event
- Frontend type and listener updated to match

### Layer 2: Event coalescing in the emitter thread
- After receiving an event, the emitter drains all immediately-available events
- Multiple `TerminalOutput` events for the same terminal are collapsed into one emit
- Non-output events (closed, process-changed) are preserved in order

### Layer 3: Frontend snapshot interval cap (60fps)
- `scheduleSnapshotFetch()` uses `setTimeout(16)` instead of `setTimeout(0)`
- Under sustained output, snapshot fetches are capped at ~60fps
- Combined with `snapshotPending` guard, actual rate is `min(60fps, 1/IPC_round_trip)`

## Files Changed

- `src-tauri/src/daemon_client/bridge.rs` — EmitPayload, emitter coalescing
- `src-tauri/src/daemon_client/client.rs` — Buffer replay emit
- `src/services/terminal-service.ts` — Remove data from listener type
- `src/components/TerminalPane.ts` — Snapshot interval cap
