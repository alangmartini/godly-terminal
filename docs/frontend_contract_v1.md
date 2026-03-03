# Frontend Contract v1

This document freezes the daemon ↔ frontend protocol contract at v1.
Any frontend (Web/Tauri, Native/Iced, Shadow/headless) that implements this
contract can drive Godly Terminal sessions.

Version constant: `godly_protocol::FRONTEND_CONTRACT_VERSION = "1.0.0"`

---

## Wire Format

All messages are length-prefixed: `[4-byte big-endian length][payload]`.

Payload discrimination is by first byte:
- `0x7B` (`{`) → JSON (serde `#[serde(tag = ...)]` envelope)
- `0x01` → binary `Event::Output`
- `0x02` → binary `Request::Write`
- `0x03` → binary `Response::Buffer`
- `0x04` → binary `Event::GridDiff`

Binary frame layout: `[tag: u8][session_id_len: u8][session_id bytes][data bytes]`

See `godly-protocol/src/frame.rs` for encode/decode implementation.

---

## Request Variants

Sent from frontend → daemon. Defined in `messages.rs`.

| Variant | Fields | Semantics |
|---------|--------|-----------|
| `CreateSession` | `id`, `shell_type`, `cwd?`, `rows`, `cols`, `env?` | Create a new PTY session |
| `ListSessions` | — | List all active sessions |
| `Attach` | `session_id` | Attach to a session (starts receiving events) |
| `Detach` | `session_id` | Detach from a session (stop events) |
| `CloseSession` | `session_id` | Kill the session |
| `Write` | `session_id`, `data: Vec<u8>` | Send input bytes (binary frame, tag 0x02) |
| `Resize` | `session_id`, `rows`, `cols` | Resize the PTY |
| `ReadBuffer` | `session_id` | Read raw buffer data |
| `ReadGrid` | `session_id` | Read plain-text grid snapshot |
| `ReadRichGrid` | `session_id` | Read rich grid (per-cell attrs) |
| `ReadRichGridDiff` | `session_id` | Read differential grid (dirty rows only) |
| `ReadGridText` | `session_id`, `start_row`, `start_col`, `end_row`, `end_col`, `scrollback_offset` | Extract text between positions |
| `SetScrollback` | `session_id`, `offset` | Set scrollback viewport (0 = live) |
| `ScrollAndReadRichGrid` | `session_id`, `offset` | Set scrollback + return grid in one round-trip |
| `GetLastOutputTime` | `session_id` | Query last output timestamp + running status |
| `SearchBuffer` | `session_id`, `text`, `strip_ansi` | Search terminal buffer |
| `PauseSession` | `session_id` | Pause event streaming |
| `ResumeSession` | `session_id` | Resume event streaming |
| `Ping` | — | Health check |

---

## Response Variants

Sent from daemon → frontend (one per request).

| Variant | Fields | Semantics |
|---------|--------|-----------|
| `Ok` | — | Success (no data) |
| `Error` | `message` | Request failed |
| `SessionCreated` | `session: SessionInfo` | Session created successfully |
| `SessionList` | `sessions: Vec<SessionInfo>` | List of active sessions |
| `Buffer` | `session_id`, `data: Vec<u8>` | Buffer replay on attach (binary frame, tag 0x03) |
| `Grid` | `grid: GridData` | Plain-text grid snapshot |
| `RichGrid` | `grid: RichGridData` | Rich grid with per-cell attrs |
| `RichGridDiff` | `diff: RichGridDiff` | Differential grid snapshot |
| `GridText` | `text` | Extracted text from grid |
| `LastOutputTime` | `epoch_ms`, `running`, `exit_code?`, `input_expected?` | Session status |
| `SearchResult` | `found`, `running` | Buffer search result |
| `Pong` | — | Health check response |

---

## Event Variants

Pushed asynchronously from daemon → attached frontend.

| Variant | Fields | Semantics |
|---------|--------|-----------|
| `Output` | `session_id`, `data: Vec<u8>` | Raw PTY output (binary frame, tag 0x01) |
| `SessionClosed` | `session_id`, `exit_code?` | Session process exited |
| `ProcessChanged` | `session_id`, `process_name` | Foreground process changed |
| `GridDiff` | `session_id`, `diff: RichGridDiff` | Incremental grid update (binary frame, tag 0x04) |
| `Bell` | `session_id` | Terminal bell triggered |

---

## Key Types

### `RichGridData` (types.rs)
Full grid snapshot: `rows: Vec<RichGridRow>`, `cursor: CursorState`, `dimensions: GridDimensions`,
`alternate_screen`, `cursor_hidden`, `title`, `scrollback_offset`, `total_scrollback`.

### `RichGridDiff` (types.rs)
Differential snapshot: `dirty_rows: Vec<(u16, RichGridRow)>`, same metadata as RichGridData,
plus `full_repaint` flag.

### `RichGridCell` (types.rs)
Per-cell: `content`, `fg`, `bg` (hex or "default"), `bold`, `dim`, `italic`, `underline`,
`inverse`, `wide`, `wide_continuation`.

### `SessionInfo` (types.rs)
`id`, `shell_type`, `pid`, `rows`, `cols`, `cwd?`, `created_at`, `attached`, `running`,
`scrollback_rows`, `scrollback_memory_bytes`, `paused`, `title`.

### `ShellType` (types.rs)
Enum: `Windows`, `Pwsh`, `Cmd`, `Wsl { distribution? }`, `Custom { program, args? }`.

### `DaemonMessage` (messages.rs)
Top-level envelope: `Response(Response)` | `Event(Event)`, tagged with `kind`.

---

## Concurrent IPC

Requests can carry an optional `request_id: u32`. The daemon echoes it in the
response envelope so the client can match responses to requests without FIFO ordering.
Binary frames (Write, Output, GridDiff, Buffer) never carry request_id.

See `RequestEnvelope` and `DaemonMessageWriteEnvelope` in messages.rs.
