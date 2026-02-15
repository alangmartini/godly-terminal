# Integration Map: xterm.js → godly-vt

## Current Data Flow

```
┌──────────────┐     Named Pipe      ┌──────────────────┐
│  Daemon       │◄──────────────────►│  Tauri App        │
│  session.rs   │  Event::Output     │  DaemonBridge     │
│  ring buffer  │  { data: Vec<u8> } │  EventEmitter     │
└──────────────┘                     └────────┬──────────┘
                                              │ Tauri event: "terminal-output"
                                              │ Payload: { terminal_id, data: number[] }
                                              ▼
                                     ┌──────────────────┐
                                     │  TerminalService  │
                                     │  terminal-service │
                                     │  .ts              │
                                     └────────┬──────────┘
                                              │ Uint8Array callback
                                              ▼
                                     ┌──────────────────┐
                                     │  TerminalPane.ts  │
                                     │  outputBuffer[]   │
                                     │  flushOutputBuffer│
                                     └────────┬──────────┘
                                              │ terminal.write(merged)
                                              ▼
                                     ┌──────────────────┐
                                     │  xterm.js         │
                                     │  ANSI parser      │
                                     │  internal grid    │
                                     │  WebGL renderer   │
                                     └──────────────────┘
```

## xterm.js Touchpoints in TerminalPane.ts

| Operation | Current xterm.js API | godly-vt Replacement |
|-----------|---------------------|---------------------|
| Create terminal | `new Terminal({...})` | `godly-vt` grid created in Rust, thin JS wrapper |
| Write output | `terminal.write(data)` | Feed bytes to godly-vt parser (Rust-side) |
| Read input | `terminal.onData(cb)` | **No change** — input doesn't go through parser |
| Title change | `terminal.onTitleChange(cb)` | godly-vt emits title from OSC 0/2 parsing |
| Custom keys | `attachCustomKeyEventHandler()` | **No change** — keyboard handling stays in JS |
| Get selection | `terminal.getSelection()` | Read cell range from godly-vt grid |
| Focus | `terminal.focus()` | **No change** — DOM focus, not parser concern |
| Fit/resize | `fitAddon.fit()`, `terminal.rows/cols` | godly-vt grid resize, expose rows/cols |
| Scrollback save | `serializeAddon.serialize()` | godly-vt grid serialization (Rust-native) |
| Scrollback load | `terminal.write(ansiText)` | Feed to godly-vt parser OR binary deserialize |
| GPU rendering | WebglAddon / CanvasAddon | **Phase 4** — wgpu or keep WebGL reading grid |

## What Changes Per Layer

### Backend (Rust) — Primary changes

**New crate: `godly-vt/`**
- Forked from vt100 with vte vendored
- Added to workspace in `src-tauri/Cargo.toml`
- Contains: parser, grid, cell, screen types

**`daemon/src/session.rs`** — Enhanced
- After reading PTY output, also feed bytes to godly-vt parser
- Grid state lives alongside ring buffer
- Exposes grid for MCP queries

**`src-tauri/src/daemon_client/bridge.rs`** — Minor change
- Continue emitting `terminal-output` events (unchanged)
- Optionally also maintain a local godly-vt grid for the app process

**`src-tauri/src/commands/`** — New commands
- `read_grid_cells(terminal_id, row, col_start, col_end)` — for frontend rendering
- `get_grid_dimensions(terminal_id)` — rows, cols, scrollback length
- `get_grid_selection(terminal_id, start, end)` — text extraction for copy

### Frontend (TypeScript) — Phased changes

**Phase 1-3: xterm.js stays as renderer**
- TerminalPane still uses xterm.js Terminal for rendering
- `terminal.write()` still receives raw bytes
- No visible change to user

**Phase 4: xterm.js replaced**
- TerminalPane renders from godly-vt grid snapshots
- Either: wgpu native surface, or Canvas2D reading grid via IPC
- Remove xterm.js dependency

### MCP — Improved

**Current:** `read_terminal` → daemon output_history → raw bytes → ANSI strip
**After:** `read_terminal` → daemon godly-vt grid → structured cell data → clean text

No ANSI stripping needed — read directly from parsed grid.

## Files Requiring Changes

### Must change
- `src/components/TerminalPane.ts` — Terminal init, write, scrollback, selection
- `src/main.ts` — CSS import (xterm.css removal in Phase 4)
- `src-tauri/Cargo.toml` — Add godly-vt to workspace
- `src-tauri/daemon/src/session.rs` — Grid state alongside ring buffer

### New files
- `src-tauri/godly-vt/` — Entire new crate (forked)
- `src-tauri/src/commands/grid.rs` — Grid query commands (Phase 4)

### No change needed
- `src/services/terminal-service.ts` — Works with bytes, format unchanged
- `src-tauri/src/daemon_client/bridge.rs` — Event routing unchanged
- `src-tauri/src/persistence/scrollback.rs` — Works with bytes
- `src-tauri/src/state/` — App state management unchanged

## npm Dependencies Impact

### Keep (Phase 1-3)
All xterm packages stay during hybrid phase.

### Remove (Phase 4-5)
```
@xterm/xterm
@xterm/addon-fit          → sizing logic moves to Rust or custom JS
@xterm/addon-serialize    → godly-vt has native serialization
@xterm/addon-web-links    → implement in godly-vt or custom renderer
@xterm/addon-webgl        → replaced by wgpu or custom Canvas
@xterm/addon-canvas       → replaced by wgpu or custom Canvas
```

## Key Architectural Decision

**Scrollback format change:**
- Current: ANSI-encoded string (re-parsed on load)
- Proposed: Binary grid format (direct deserialize, preserves all cell metadata)
- Backward compat: Can still accept ANSI input for loading old scrollback files
- This enables AI annotations, semantic zones, and images to survive persistence
