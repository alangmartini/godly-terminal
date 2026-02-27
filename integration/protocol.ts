/**
 * TypeScript types mirroring the daemon wire protocol from
 * protocol/src/messages.rs and protocol/src/types.rs.
 *
 * Serde conventions:
 *   - Request/Response/Event: #[serde(tag = "type")]
 *   - DaemonMessage: #[serde(tag = "kind")] with flattened inner type
 *   - ShellType: #[serde(rename_all = "snake_case")] externally-tagged
 */

// ── ShellType ────────────────────────────────────────────────────────────

export type ShellType =
  | 'windows'
  | 'pwsh'
  | 'cmd'
  | { wsl: { distribution: string | null } }
  | { custom: { program: string; args?: string[] } };

// ── SessionInfo ──────────────────────────────────────────────────────────

export interface SessionInfo {
  id: string;
  shell_type: ShellType;
  pid: number;
  rows: number;
  cols: number;
  cwd: string | null;
  created_at: number;
  attached: boolean;
  running: boolean;
  scrollback_rows?: number;
  scrollback_memory_bytes?: number;
  paused?: boolean;
  title?: string;
}

// ── GridData ─────────────────────────────────────────────────────────────

export interface GridData {
  rows: string[];
  cursor_row: number;
  cursor_col: number;
  cols: number;
  num_rows: number;
  alternate_screen: boolean;
}

// ── Request ──────────────────────────────────────────────────────────────

export type Request =
  | { type: 'CreateSession'; id: string; shell_type: ShellType; rows: number; cols: number; cwd?: string; env?: Record<string, string> }
  | { type: 'ListSessions' }
  | { type: 'Attach'; session_id: string }
  | { type: 'Detach'; session_id: string }
  | { type: 'Write'; session_id: string; data: number[] }
  | { type: 'Resize'; session_id: string; rows: number; cols: number }
  | { type: 'CloseSession'; session_id: string }
  | { type: 'ReadBuffer'; session_id: string }
  | { type: 'GetLastOutputTime'; session_id: string }
  | { type: 'SearchBuffer'; session_id: string; text: string; strip_ansi: boolean }
  | { type: 'ReadGrid'; session_id: string }
  | { type: 'ReadRichGrid'; session_id: string }
  | { type: 'ReadGridText'; session_id: string; start_row: number; start_col: number; end_row: number; end_col: number; scrollback_offset: number }
  | { type: 'ReadRichGridDiff'; session_id: string }
  | { type: 'SetScrollback'; session_id: string; offset: number }
  | { type: 'ScrollAndReadRichGrid'; session_id: string; offset: number }
  | { type: 'PauseSession'; session_id: string }
  | { type: 'ResumeSession'; session_id: string }
  | { type: 'Ping' };

// ── Response ─────────────────────────────────────────────────────────────

export type Response =
  | { type: 'Ok' }
  | { type: 'Error'; message: string }
  | { type: 'SessionCreated'; session: SessionInfo }
  | { type: 'SessionList'; sessions: SessionInfo[] }
  | { type: 'Pong' }
  | { type: 'Buffer'; session_id: string; data: number[] }
  | { type: 'LastOutputTime'; epoch_ms: number; running: boolean; exit_code?: number }
  | { type: 'SearchResult'; found: boolean; running: boolean }
  | { type: 'Grid'; grid: GridData }
  | { type: 'GridText'; text: string };

// ── Event ────────────────────────────────────────────────────────────────

export type Event =
  | { type: 'Output'; session_id: string; data: number[] }
  | { type: 'SessionClosed'; session_id: string; exit_code?: number }
  | { type: 'ProcessChanged'; session_id: string; process_name: string }
  | { type: 'Bell'; session_id: string };

// ── DaemonMessage ────────────────────────────────────────────────────────
// #[serde(tag = "kind")] flattens with the inner Response/Event type field.

export type DaemonMessage =
  | ({ kind: 'Response' } & Response)
  | ({ kind: 'Event' } & Event);

// ── Binary frame tags ────────────────────────────────────────────────────

export const TAG_EVENT_OUTPUT = 0x01;
export const TAG_REQUEST_WRITE = 0x02;
export const TAG_RESPONSE_BUFFER = 0x03;
