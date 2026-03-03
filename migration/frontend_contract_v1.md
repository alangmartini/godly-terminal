# Frontend Contract v1

This document defines the concrete IPC/MCP contracts between the frontend (TypeScript), the Tauri backend (Rust), and the daemon — as they exist today. It serves as the **stability contract** for Lane A: any change to a surface listed here is a **breaking change** that requires coordinated migration across layers.

## 1. Architecture Layers

```
┌──────────────────────────────────────────────────────────────────┐
│                        Frontend (TypeScript)                     │
│  TerminalService · WorkspaceService · Store · TerminalRenderer   │
└────────┬───────────────────────────────────┬─────────────────────┘
         │  Tauri IPC (invoke / listen)      │  stream:// protocol
         │  JSON-serialized                  │  binary frames
         ▼                                   ▼
┌──────────────────────────────────────────────────────────────────┐
│                     Tauri Backend (Rust)                          │
│  commands/terminal.rs · commands/grid.rs · commands/workspace.rs  │
│  DaemonClient · DaemonBridge · AppState                          │
└────────┬─────────────────────────────────────────────────────────┘
         │  Named Pipe IPC (binary-framed JSON + binary frames)
         ▼
┌──────────────────────────────────────────────────────────────────┐
│                     Daemon (godly-daemon)                         │
│  server.rs · session.rs · godly-vt parser · ring buffer          │
└──────────────────────────────────────────────────────────────────┘
```

A separate MCP pipe connects `godly-mcp` to the Tauri backend (Section 6).

---

## 2. Daemon Wire Protocol

Transport: Windows Named Pipe (`\\.\pipe\godly-terminal-daemon`).

Framing: length-prefixed. Each message is `[u32le payload_length][payload_bytes]`. Payloads are **either** JSON (first byte `{` / `0x7B`) or binary (first byte is a tag `0x01`–`0x04`).

### 2.1 Request → Daemon (client sends)

Envelope: `RequestEnvelope { request_id?: u32, ...Request }`. The `request_id` enables concurrent in-flight requests; when omitted, responses are matched sequentially.

| Request variant | Key fields | Response |
|---|---|---|
| `CreateSession` | `id, shell_type, cwd?, rows, cols, env?` | `SessionCreated { session }` |
| `ListSessions` | — | `SessionList { sessions }` |
| `Attach` | `session_id` | `Ok` or `Buffer { session_id, data }` |
| `Detach` | `session_id` | `Ok` |
| `CloseSession` | `session_id` | `Ok` |
| `Write` | `session_id, data: Vec<u8>` | `Ok` (or fire-and-forget) |
| `Resize` | `session_id, rows, cols` | `Ok` (or fire-and-forget) |
| `ReadBuffer` | `session_id` | `Buffer { session_id, data }` |
| `ReadGrid` | `session_id` | `Grid { grid: GridData }` |
| `ReadRichGrid` | `session_id` | `RichGrid { grid: RichGridData }` |
| `ReadRichGridDiff` | `session_id` | `RichGridDiff { diff: RichGridDiff }` |
| `ReadGridText` | `session_id, start_row(i32), start_col, end_row(i32), end_col, scrollback_offset` | `GridText { text }` |
| `SetScrollback` | `session_id, offset` | `Ok` |
| `ScrollAndReadRichGrid` | `session_id, offset` | `RichGrid { grid }` |
| `GetLastOutputTime` | `session_id` | `LastOutputTime { epoch_ms, running, exit_code?, input_expected? }` |
| `SearchBuffer` | `session_id, text, strip_ansi` | `SearchResult { found, running }` |
| `PauseSession` | `session_id` | `Ok` |
| `ResumeSession` | `session_id` | `Ok` |
| `Ping` | — | `Pong` |

### 2.2 Daemon → Client (async events)

Wrapped in `DaemonMessage::Event(event)`. Events are pushed without a prior request. They carry no `request_id`.

| Event variant | Fields | Trigger |
|---|---|---|
| `Output` | `session_id, data: Vec<u8>` | PTY output available |
| `SessionClosed` | `session_id, exit_code?` | Shell process exited |
| `ProcessChanged` | `session_id, process_name` | Foreground process changed |
| `GridDiff` | `session_id, diff: RichGridDiff` | Grid rows changed (pushed to attached clients) |
| `Bell` | `session_id` | BEL character received |

### 2.3 Binary Frame Tags

Binary frames share the same length-prefixed transport but use a tag byte instead of JSON `{`:

| Tag | Direction | Format | Purpose |
|---|---|---|---|
| `0x01` | daemon → client | `[tag][sid_len][sid_bytes][output_bytes]` | `Event::Output` (fast path) |
| `0x02` | client → daemon | `[tag][sid_len][sid_bytes][input_bytes]` | `Request::Write` (fast path) |
| `0x03` | daemon → client | `[tag][sid_len][sid_bytes][buffer_bytes]` | `Response::Buffer` (attach replay) |
| `0x04` | daemon → client | `[tag][sid_len][sid_bytes][binary_diff]` | `Event::GridDiff` (binary-encoded) |

### 2.4 Binary Diff Wire Format (tag `0x04`)

Compact encoding of `RichGridDiff` (~5KB for 80×24 full repaint vs ~50KB JSON):

```
Header (variable length):
  magic:              2B  "GD"
  version:            1B  0x01
  cursor_row:         u16LE
  cursor_col:         u16LE
  grid_rows:          u16LE
  grid_cols:          u16LE
  flags:              u8  (bit0=alternate_screen, bit1=cursor_hidden, bit2=full_repaint)
  dirty_row_count:    u16LE
  scrollback_offset:  u32LE
  total_scrollback:   u32LE
  title_len:          u16LE
  title:              [title_len bytes, UTF-8]

Per dirty row:
  row_index:          u16LE
  row_flags:          u8    (bit0=wrapped)
  cell_count:         u16LE

Per cell:
  content_len:        u8
  content:            [content_len bytes, UTF-8]
  fg:                 color (0x00=default | 0x01+3B=RGB)
  bg:                 color (same encoding)
  attrs:              u8    (bit0=bold, bit1=dim, bit2=italic, bit3=underline,
                             bit4=inverse, bit5=wide, bit6=wide_continuation)
```

Multiple diffs can be concatenated in a single binary frame.

---

## 3. Tauri IPC Commands (Frontend → Backend)

Frontend calls `invoke(command, params)` and receives a JSON-serialized response. Parameter names use `camelCase` on the wire (Tauri's automatic conversion from Rust `snake_case`).

### 3.1 Terminal Lifecycle

| Command | Parameters | Returns | Fire-and-forget? |
|---|---|---|---|
| `create_terminal` | `workspaceId, cwdOverride?, shellTypeOverride?, idOverride?, worktreeName?, nameOverride?` | `CreateTerminalResult { id, worktree_branch? }` | No |
| `close_terminal` | `terminalId` | `()` | No |
| `attach_session` | `sessionId, workspaceId, name` | `()` | No |
| `reconnect_sessions` | — | `SessionInfo[]` | No |
| `detach_all_sessions` | — | `()` | No |

### 3.2 Terminal I/O

| Command | Parameters | Returns | Fire-and-forget? |
|---|---|---|---|
| `write_to_terminal` | `terminalId, data: string` | `()` | Yes (non-blocking) |
| `resize_terminal` | `terminalId, rows, cols` | `()` | Yes (non-blocking) |

**Invariant**: `write_to_terminal` converts `\n` → `\r` and `\r\n` → `\r` before sending to the daemon. Callers must not pre-convert.

### 3.3 Grid Queries

| Command | Parameters | Returns |
|---|---|---|
| `get_grid_snapshot` | `terminalId` | `RichGridData` |
| `get_grid_snapshot_diff` | `terminalId` | `RichGridDiff` |
| `get_grid_dimensions` | `terminalId` | `[rows: u16, cols: u16]` |
| `get_grid_text` | `terminalId, startRow(i32), startCol, endRow(i32), endCol, scrollbackOffset` | `string` |
| `set_scrollback` | `terminalId, offset` | `()` |
| `scroll_and_get_snapshot` | `terminalId, offset` | `RichGridData` |

**Invariant**: `scrollback_offset=0` means live view (bottom of buffer). `offset>0` scrolls into history. Row coordinates in `get_grid_text` are viewport-relative and can be negative for selections extending above the viewport.

### 3.4 Session Control

| Command | Parameters | Returns |
|---|---|---|
| `rename_terminal` | `terminalId, name` | `()` |
| `sync_active_terminal` | `terminalId?` | `()` |
| `pause_session` | `sessionId` | `()` |
| `resume_session` | `sessionId` | `()` |

### 3.5 Quick Claude

| Command | Parameters | Returns |
|---|---|---|
| `quick_claude` | `workspaceId, prompt, branchName?, skipFetch?, noWorktree?, aiTool?` | `QuickClaudeResult { terminal_id, worktree_branch? }` |

Returns immediately; prompt delivery happens on a background thread.

---

## 4. Tauri Events (Backend → Frontend)

Events emitted via `app_handle.emit()` and received via `listen()`.

| Event name | Payload type | Source |
|---|---|---|
| `terminal-output` | `{ terminal_id: string }` | DaemonBridge (PTY output available) |
| `terminal-grid-diff` | `{ terminal_id: string, diff: RichGridDiff }` | DaemonBridge (grid rows changed) |
| `process-changed` | `{ terminal_id: string, process_name: string }` | DaemonBridge |
| `terminal-closed` | `{ terminal_id: string, exit_code: number \| null }` | DaemonBridge |
| `quick-claude-ready` | `{ terminal_id: string, display_name: string }` | Quick Claude background thread |

**Invariant**: `terminal-output` is a **notification only** — it carries no data. The frontend must fetch grid state via `get_grid_snapshot` or the stream:// protocol after receiving this event.

---

## 5. Stream Protocol (Frontend ↔ Backend)

Custom Tauri protocol handler registered at `stream.localhost`. Frontend uses `fetch()` + `ReadableStream`.

### 5.1 Output Stream

```
GET http://stream.localhost/terminal-output/{session_id}
```

Returns a `ReadableStream` of raw bytes. Each non-empty chunk is a signal that output is available (equivalent to `terminal-output` event). Frontend calls `onData()` callback to trigger a grid snapshot fetch.

**Reconnection**: Exponential backoff with jitter (1s base, 10s max). Circuit breaker opens after 5 consecutive failures; probe interval 10s. `triggerProbe()` enables instant recovery on tab switch.

### 5.2 Diff Stream

```
GET http://stream.localhost/terminal-diff/{session_id}
```

Returns a `ReadableStream` of binary-encoded `RichGridDiff` frames (Section 2.4 format). Multiple diffs may be concatenated in a single chunk. Frontend decodes via `decodeAllDiffs()`.

**Reconnection**: Same exponential backoff as output stream (no circuit breaker).

---

## 6. MCP Protocol (godly-mcp ↔ Tauri App)

Transport: Named Pipe (`\\.\pipe\godly-terminal-mcp`). Same length-prefixed framing as daemon protocol. Messages are JSON `McpRequest` → `McpResponse`.

### 6.1 Key MCP Commands (subset — terminal/grid relevant)

| McpRequest | McpResponse | Notes |
|---|---|---|
| `ReadTerminal { terminal_id, mode?, lines?, strip_ansi? }` | `TerminalOutput { content }` | Raw buffer read |
| `ReadGrid { terminal_id }` | `GridSnapshot { rows, cursor_row, cursor_col, cols, num_rows, alternate_screen }` | Parsed grid |
| `WriteToTerminal { terminal_id, data }` | `Ok` | |
| `ExecuteCommand { terminal_id, command, idle_ms, timeout_ms }` | `CommandOutput { output, completed, last_output_ago_ms, running, input_expected? }` | |
| `WaitForIdle { terminal_id, idle_ms, timeout_ms }` | `WaitResult { completed, last_output_ago_ms }` | |
| `WaitForText { terminal_id, text, timeout_ms }` | `WaitResult { completed, last_output_ago_ms }` | |
| `SendKeys { terminal_id, keys }` | `Ok` | |

---

## 7. Core Data Types

### 7.1 RichGridData (full snapshot)

```typescript
interface RichGridData {
  rows: RichGridRow[];
  cursor: { row: u16, col: u16 };
  dimensions: { rows: u16, cols: u16 };
  alternate_screen: boolean;
  cursor_hidden: boolean;
  title: string;
  scrollback_offset: number;   // 0 = live view
  total_scrollback: number;
}
```

### 7.2 RichGridDiff (differential)

```typescript
interface RichGridDiff {
  dirty_rows: [row_index: u16, RichGridRow][];
  cursor: { row: u16, col: u16 };
  dimensions: { rows: u16, cols: u16 };
  alternate_screen: boolean;
  cursor_hidden: boolean;
  title: string;
  scrollback_offset: number;
  total_scrollback: number;
  full_repaint: boolean;       // if true, dirty_rows contains ALL rows
}
```

### 7.3 RichGridRow / RichGridCell

```typescript
interface RichGridRow {
  cells: RichGridCell[];
  wrapped: boolean;
}

interface RichGridCell {
  content: string;             // UTF-8 character(s), may include combining chars
  fg: string;                  // "#rrggbb" or "default"
  bg: string;                  // "#rrggbb" or "default"
  bold: boolean;
  dim: boolean;
  italic: boolean;
  underline: boolean;
  inverse: boolean;
  wide: boolean;
  wide_continuation: boolean;
}
```

### 7.4 SessionInfo

```typescript
interface SessionInfo {
  id: string;
  shell_type: ShellType;
  pid: number;
  rows: number;
  cols: number;
  cwd: string | null;
  created_at: number;          // epoch seconds
  attached: boolean;
  running: boolean;
  scrollback_rows: number;
  scrollback_memory_bytes: number;
  paused: boolean;
  title: string;               // OSC window title
}
```

### 7.5 ShellType

```typescript
type ShellType =
  | "windows"
  | "pwsh"
  | "cmd"
  | { wsl: { distribution: string | null } }
  | { custom: { program: string, args: string[] | null } };
```

### 7.6 LayoutNode

```typescript
type LayoutNode =
  | { type: "leaf", terminal_id: string }
  | { type: "split", direction: "horizontal" | "vertical", ratio: number,
      first: LayoutNode, second: LayoutNode }
  | { type: "grid", col_ratios: [number, number], row_ratios: [number, number],
      children: [LayoutNode, LayoutNode, LayoutNode, LayoutNode] };
```

---

## 8. Key Invariants

These are properties that must hold across all layers. Violating any of these is a regression.

### 8.1 Data Flow

1. **Single source of truth**: The daemon's `godly-vt` parser is the sole authority for terminal grid state. The frontend never parses VT sequences — it only renders snapshots.
2. **No data in output events**: `terminal-output` events carry no terminal data. They are signals to fetch the latest grid state.
3. **Diff ⊆ Full**: Every field in `RichGridDiff` exists in `RichGridData`. When `full_repaint=true`, a diff is semantically equivalent to a full snapshot.

### 8.2 Session Lifecycle

4. **Create-then-attach**: `CreateSession` + `Attach` must be called in sequence. The daemon does not auto-attach on creation.
5. **Detach-not-close on window close**: App close sends `Detach` (sessions survive). `CloseSession` kills the PTY process.
6. **Reconnect on restart**: App restart calls `ListSessions` → `Attach` for each surviving session. Ring buffer replays missed output into the godly-vt parser.

### 8.3 I/O

7. **Fire-and-forget writes**: `write_to_terminal` and `resize_terminal` do not block on daemon response. This prevents thread pool saturation under rapid input.
8. **Newline conversion**: The Tauri command layer converts `\n` → `\r` before forwarding to the daemon. Frontend callers send `\r` for Enter (keyboard) or `\n` for programmatic writes.

### 8.4 Scrollback

9. **Offset semantics**: `scrollback_offset=0` is live view (bottom). Values > 0 scroll into history. The daemon clamps to `[0, total_scrollback]`.
10. **Viewport-relative coordinates**: `ReadGridText` row coordinates are viewport-relative. Negative rows refer to content above the visible viewport. `scrollback_offset` is needed to translate to absolute buffer positions.

### 8.5 Binary Protocol

11. **Tag discrimination**: First byte determines format. `0x7B` (`{`) = JSON. `0x01`–`0x04` = binary frame with `[tag][sid_len][sid][data]` structure.
12. **Backward compatibility**: `RequestEnvelope` without `request_id` falls back to sequential matching. `DaemonMessage` without `request_id` is treated as a broadcast/event.
13. **Binary diff versioning**: Magic bytes `"GD"` + version byte `0x01`. Unknown versions must be rejected. Unknown color tags decode as `"default"` for forward compatibility.

### 8.6 Isolation

14. **Instance isolation**: `GODLY_INSTANCE` env var suffixes all pipe names and metadata directories. Tests MUST set unique values to avoid interfering with production.
15. **Pipe name override**: `GODLY_PIPE_NAME` takes precedence over computed names (including instance suffix).

---

## 9. Version History

| Version | Date | Changes |
|---|---|---|
| v1 | 2026-03-02 | Initial contract documentation — captures existing state |
