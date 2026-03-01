use std::io;

use serde::{de::DeserializeOwned, Serialize};

use crate::binary_diff::{decode_grid_diff, encode_grid_diff_into};
use crate::messages::{DaemonMessage, Event, Request, Response};

// ── Binary frame type tags ──────────────────────────────────────────────
// JSON payloads always start with '{' (0x7B) due to #[serde(tag = ...)].
// Binary frames use a different first byte as discriminator.

const TAG_EVENT_OUTPUT: u8 = 0x01;
const TAG_REQUEST_WRITE: u8 = 0x02;
const TAG_RESPONSE_BUFFER: u8 = 0x03;
const TAG_EVENT_GRID_DIFF: u8 = 0x04;

// ── Shim binary frame tags ──────────────────────────────────────────────
// Used for daemon <-> pty-shim communication. Different tag range (0x1x)
// from daemon <-> client tags (0x0x) to avoid confusion.

pub const TAG_SHIM_WRITE: u8 = 0x10;       // daemon -> shim: input bytes
pub const TAG_SHIM_BUFFER_DATA: u8 = 0x11; // shim -> daemon: ring buffer replay
pub const TAG_SHIM_OUTPUT: u8 = 0x12;      // shim -> daemon: live PTY output

/// Encode a binary frame: [tag][session_id_len][session_id bytes][data bytes]
fn encode_binary_frame(tag: u8, session_id: &str, data: &[u8]) -> Vec<u8> {
    let sid_bytes = session_id.as_bytes();
    let sid_len = sid_bytes.len() as u8;
    let mut buf = Vec::with_capacity(2 + sid_bytes.len() + data.len());
    buf.push(tag);
    buf.push(sid_len);
    buf.extend_from_slice(sid_bytes);
    buf.extend_from_slice(data);
    buf
}

/// Decode a binary frame, returning (tag, session_id, data).
fn decode_binary_frame(buf: &[u8]) -> io::Result<(u8, &str, &[u8])> {
    if buf.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Binary frame too short",
        ));
    }
    let tag = buf[0];
    let sid_len = buf[1] as usize;
    if buf.len() < 2 + sid_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Binary frame session_id truncated",
        ));
    }
    let session_id = std::str::from_utf8(&buf[2..2 + sid_len])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let data = &buf[2 + sid_len..];
    Ok((tag, session_id, data))
}

/// Write a length-prefixed payload (shared by all write functions).
fn write_length_prefixed<W: io::Write>(writer: &mut W, payload: &[u8]) -> io::Result<()> {
    let len = payload.len() as u32;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(payload)?;
    // NOTE: Do NOT call flush() here. On Windows named pipes, FlushFileBuffers()
    // blocks until the other end reads all data, which can cause deadlocks when
    // the reader is in a spawn_blocking task. Named pipes in byte mode deliver
    // data immediately via write_all() without needing explicit flush.
    Ok(())
}

/// Read a length-prefixed payload from a reader.
/// Returns None on EOF (zero-length read on the length prefix).
fn read_length_prefixed<R: io::Read>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > 16 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Message too large: {} bytes", len),
        ));
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(Some(buf))
}

// ── Generic JSON functions (unchanged, used by MCP pipe) ────────────────

/// Write a length-prefixed JSON message to a writer.
/// Format: 4-byte big-endian length + JSON bytes
pub fn write_message<W: io::Write, T: Serialize>(writer: &mut W, msg: &T) -> io::Result<()> {
    let json = serde_json::to_vec(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write_length_prefixed(writer, &json)
}

/// Read a length-prefixed JSON message from a reader.
/// Returns None on EOF (zero-length read on the length prefix).
pub fn read_message<R: io::Read, T: DeserializeOwned>(reader: &mut R) -> io::Result<Option<T>> {
    let buf = match read_length_prefixed(reader)? {
        Some(buf) => buf,
        None => return Ok(None),
    };
    let msg =
        serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some(msg))
}

// ── Typed binary-aware functions (daemon <-> client hot path) ───────────

/// Write a DaemonMessage, using binary framing for hot-path types.
///
/// - `Event::Output` → binary (tag 0x01)
/// - `Response::Buffer` → binary (tag 0x03)
/// - `Event::GridDiff` → binary (tag 0x04) — compact binary diff, ~10x smaller than JSON
/// - Everything else → JSON
pub fn write_daemon_message<W: io::Write>(writer: &mut W, msg: &DaemonMessage) -> io::Result<()> {
    match msg {
        DaemonMessage::Event(Event::Output { session_id, data }) => {
            let frame = encode_binary_frame(TAG_EVENT_OUTPUT, session_id, data);
            write_length_prefixed(writer, &frame)
        }
        DaemonMessage::Event(Event::GridDiff { session_id, diff }) => {
            // Binary-encode the diff, then wrap in our binary frame format.
            let mut diff_bytes = Vec::new();
            encode_grid_diff_into(diff, &mut diff_bytes);
            let frame = encode_binary_frame(TAG_EVENT_GRID_DIFF, session_id, &diff_bytes);
            write_length_prefixed(writer, &frame)
        }
        DaemonMessage::Response(Response::Buffer { session_id, data }) => {
            let frame = encode_binary_frame(TAG_RESPONSE_BUFFER, session_id, data);
            write_length_prefixed(writer, &frame)
        }
        _ => write_message(writer, msg),
    }
}

/// Result of reading a daemon message with zero-copy support for binary diffs.
///
/// The bridge uses this to forward binary-encoded GridDiff frames directly
/// to the DiffStreamRegistry without deserializing + re-encoding.
#[derive(Debug)]
pub enum ReadResult {
    /// A fully parsed DaemonMessage.
    Message(DaemonMessage),
    /// A binary-encoded GridDiff frame. Contains the session_id and raw binary
    /// diff bytes (already in the compact `encode_grid_diff` format) that can
    /// be pushed directly to the diff stream registry.
    RawGridDiff {
        session_id: String,
        binary_diff: Vec<u8>,
    },
    /// End of stream.
    Eof,
}

/// Read a DaemonMessage with zero-copy support for binary GridDiff frames.
///
/// Returns `ReadResult::RawGridDiff` for GridDiff events so the bridge can
/// push the pre-encoded bytes directly to the DiffStreamRegistry without
/// deserializing and re-encoding.
///
/// For all other message types, returns `ReadResult::Message`.
pub fn read_daemon_message_ext<R: io::Read>(reader: &mut R) -> io::Result<ReadResult> {
    let buf = match read_length_prefixed(reader)? {
        Some(buf) => buf,
        None => return Ok(ReadResult::Eof),
    };

    if buf.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Empty message",
        ));
    }

    if buf[0] == 0x7B {
        // JSON message
        let msg = serde_json::from_slice(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(ReadResult::Message(msg))
    } else {
        // Binary frame
        let (tag, session_id, data) = decode_binary_frame(&buf)?;
        match tag {
            TAG_EVENT_OUTPUT => Ok(ReadResult::Message(DaemonMessage::Event(Event::Output {
                session_id: session_id.to_string(),
                data: data.to_vec(),
            }))),
            TAG_EVENT_GRID_DIFF => {
                // Return raw binary diff bytes for zero-copy forwarding
                Ok(ReadResult::RawGridDiff {
                    session_id: session_id.to_string(),
                    binary_diff: data.to_vec(),
                })
            }
            TAG_RESPONSE_BUFFER => Ok(ReadResult::Message(DaemonMessage::Response(
                Response::Buffer {
                    session_id: session_id.to_string(),
                    data: data.to_vec(),
                },
            ))),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown daemon binary frame tag: 0x{:02X}", tag),
            )),
        }
    }
}

/// Read a DaemonMessage, auto-detecting binary vs JSON by first byte.
///
/// - First byte == 0x7B ('{') → JSON
/// - First byte is a type tag → binary frame
///
/// For GridDiff events (tag 0x04), this fully decodes the binary diff.
/// Use `read_daemon_message_ext` instead if you need zero-copy forwarding.
pub fn read_daemon_message<R: io::Read>(reader: &mut R) -> io::Result<Option<DaemonMessage>> {
    match read_daemon_message_ext(reader)? {
        ReadResult::Message(msg) => Ok(Some(msg)),
        ReadResult::RawGridDiff {
            session_id,
            binary_diff,
        } => {
            // Decode the binary diff into a full RichGridDiff
            let (diff, _) = decode_grid_diff(&binary_diff).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to decode binary GridDiff: {}", e),
                )
            })?;
            Ok(Some(DaemonMessage::Event(Event::GridDiff {
                session_id,
                diff,
            })))
        }
        ReadResult::Eof => Ok(None),
    }
}

/// Write a Request, using binary framing for hot-path types.
///
/// - `Request::Write` → binary (tag 0x02)
/// - Everything else → JSON
pub fn write_request<W: io::Write>(writer: &mut W, msg: &Request) -> io::Result<()> {
    match msg {
        Request::Write { session_id, data } => {
            let frame = encode_binary_frame(TAG_REQUEST_WRITE, session_id, data);
            write_length_prefixed(writer, &frame)
        }
        _ => write_message(writer, msg),
    }
}

/// Read a Request, auto-detecting binary vs JSON by first byte.
///
/// - First byte == 0x7B ('{') → JSON
/// - First byte is a type tag → binary frame
pub fn read_request<R: io::Read>(reader: &mut R) -> io::Result<Option<Request>> {
    let buf = match read_length_prefixed(reader)? {
        Some(buf) => buf,
        None => return Ok(None),
    };

    if buf.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Empty message",
        ));
    }

    if buf[0] == 0x7B {
        // JSON message
        let msg = serde_json::from_slice(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(Some(msg))
    } else {
        // Binary frame
        let (tag, session_id, data) = decode_binary_frame(&buf)?;
        match tag {
            TAG_REQUEST_WRITE => Ok(Some(Request::Write {
                session_id: session_id.to_string(),
                data: data.to_vec(),
            })),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown request binary frame tag: 0x{:02X}", tag),
            )),
        }
    }
}

// ── Shim binary frame types and functions ────────────────────────────────

/// A frame received from a shim pipe.
#[derive(Debug)]
pub enum ShimFrame {
    /// Binary data frame with tag (0x10, 0x11, 0x12)
    Binary { tag: u8, data: Vec<u8> },
    /// JSON control message (raw bytes, needs deserialization by caller)
    Json(Vec<u8>),
}

/// Write a binary data frame for shim communication: [tag][data]
/// Used for Write (0x10), BufferData (0x11), Output (0x12) messages.
pub fn write_shim_binary<W: io::Write>(writer: &mut W, tag: u8, data: &[u8]) -> io::Result<()> {
    let mut payload = Vec::with_capacity(1 + data.len());
    payload.push(tag);
    payload.extend_from_slice(data);
    write_length_prefixed(writer, &payload)
}

/// Write a shim JSON control message (ShimRequest or ShimResponse).
pub fn write_shim_json<W: io::Write, T: Serialize>(writer: &mut W, msg: &T) -> io::Result<()> {
    write_message(writer, msg)
}

/// Read a frame from a shim pipe and classify it.
/// Returns None on EOF.
pub fn read_shim_frame<R: io::Read>(reader: &mut R) -> io::Result<Option<ShimFrame>> {
    let buf = match read_length_prefixed(reader)? {
        Some(buf) => buf,
        None => return Ok(None),
    };
    if buf.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Empty shim frame"));
    }
    if buf[0] == 0x7B {
        // JSON control message — could be ShimRequest or ShimResponse
        // Caller must try deserializing as the expected type
        Ok(Some(ShimFrame::Json(buf)))
    } else {
        let tag = buf[0];
        let data = buf[1..].to_vec();
        Ok(Some(ShimFrame::Binary { tag, data }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // ── Generic JSON roundtrip tests (unchanged) ─────────────────────

    #[test]
    fn test_roundtrip() {
        let msg = "hello world".to_string();
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();

        let mut cursor = Cursor::new(buf);
        let result: Option<String> = read_message(&mut cursor).unwrap();
        assert_eq!(result, Some("hello world".to_string()));
    }

    #[test]
    fn test_eof_returns_none() {
        let mut cursor = Cursor::new(Vec::<u8>::new());
        let result: Option<String> = read_message(&mut cursor).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_multiple_messages() {
        let mut buf = Vec::new();
        write_message(&mut buf, &1u32).unwrap();
        write_message(&mut buf, &2u32).unwrap();
        write_message(&mut buf, &3u32).unwrap();

        let mut cursor = Cursor::new(buf);
        assert_eq!(read_message::<_, u32>(&mut cursor).unwrap(), Some(1));
        assert_eq!(read_message::<_, u32>(&mut cursor).unwrap(), Some(2));
        assert_eq!(read_message::<_, u32>(&mut cursor).unwrap(), Some(3));
        assert_eq!(read_message::<_, u32>(&mut cursor).unwrap(), None);
    }

    // ── Binary frame roundtrip tests ─────────────────────────────────

    #[test]
    fn binary_event_output_roundtrip() {
        let msg = DaemonMessage::Event(Event::Output {
            session_id: "sess-123".into(),
            data: vec![104, 101, 108, 108, 111],
        });

        let mut buf = Vec::new();
        write_daemon_message(&mut buf, &msg).unwrap();

        let mut cursor = Cursor::new(buf);
        let result = read_daemon_message(&mut cursor).unwrap().unwrap();

        match result {
            DaemonMessage::Event(Event::Output { session_id, data }) => {
                assert_eq!(session_id, "sess-123");
                assert_eq!(data, vec![104, 101, 108, 108, 111]);
            }
            other => panic!("Expected Event::Output, got {:?}", other),
        }
    }

    #[test]
    fn binary_grid_diff_roundtrip() {
        use crate::types::{CursorState, GridDimensions, RichGridCell, RichGridDiff, RichGridRow};

        let diff = RichGridDiff {
            dirty_rows: vec![(
                3,
                RichGridRow {
                    cells: vec![
                        RichGridCell {
                            content: "A".into(),
                            fg: "#cd3131".into(),
                            bg: "default".into(),
                            bold: true,
                            dim: false,
                            italic: false,
                            underline: false,
                            inverse: false,
                            wide: false,
                            wide_continuation: false,
                        },
                        RichGridCell {
                            content: " ".into(),
                            fg: "default".into(),
                            bg: "#1e1e1e".into(),
                            bold: false,
                            dim: false,
                            italic: true,
                            underline: false,
                            inverse: false,
                            wide: false,
                            wide_continuation: false,
                        },
                    ],
                    wrapped: false,
                },
            )],
            cursor: CursorState { row: 3, col: 1 },
            dimensions: GridDimensions { rows: 24, cols: 80 },
            alternate_screen: false,
            cursor_hidden: false,
            title: "test".into(),
            scrollback_offset: 10,
            total_scrollback: 500,
            full_repaint: false,
        };
        let msg = DaemonMessage::Event(Event::GridDiff {
            session_id: "sess-diff".into(),
            diff: diff.clone(),
        });

        let mut buf = Vec::new();
        write_daemon_message(&mut buf, &msg).unwrap();

        // Verify it's using binary framing (first byte after length prefix is NOT '{')
        assert_ne!(buf[4], 0x7B, "GridDiff should use binary framing, not JSON");

        // Test read_daemon_message (full decode)
        let mut cursor = Cursor::new(buf.clone());
        let result = read_daemon_message(&mut cursor).unwrap().unwrap();
        match result {
            DaemonMessage::Event(Event::GridDiff { session_id, diff: decoded }) => {
                assert_eq!(session_id, "sess-diff");
                assert_eq!(decoded.dirty_rows.len(), 1);
                assert_eq!(decoded.dirty_rows[0].0, 3);
                assert_eq!(decoded.dirty_rows[0].1.cells.len(), 2);
                assert_eq!(decoded.dirty_rows[0].1.cells[0].content, "A");
                assert!(decoded.dirty_rows[0].1.cells[0].bold);
                assert_eq!(decoded.cursor.row, 3);
                assert_eq!(decoded.cursor.col, 1);
                assert_eq!(decoded.title, "test");
                assert_eq!(decoded.scrollback_offset, 10);
                assert_eq!(decoded.total_scrollback, 500);
            }
            other => panic!("Expected Event::GridDiff, got {:?}", other),
        }

        // Test read_daemon_message_ext (raw bytes for zero-copy)
        let mut cursor = Cursor::new(buf.clone());
        match read_daemon_message_ext(&mut cursor).unwrap() {
            ReadResult::RawGridDiff { session_id, binary_diff } => {
                assert_eq!(session_id, "sess-diff");
                assert!(!binary_diff.is_empty());
                // Verify the binary diff can be decoded
                let (decoded, _) = decode_grid_diff(&binary_diff).unwrap();
                assert_eq!(decoded.dirty_rows.len(), 1);
            }
            other => panic!("Expected RawGridDiff, got {:?}", other),
        }

        // Verify binary is much smaller than JSON would be
        let mut json_buf = Vec::new();
        write_message(&mut json_buf, &DaemonMessage::Event(Event::GridDiff {
            session_id: "sess-diff".into(),
            diff,
        })).unwrap();
        assert!(
            buf.len() < json_buf.len(),
            "Binary ({} bytes) should be smaller than JSON ({} bytes)",
            buf.len(),
            json_buf.len()
        );
    }

    #[test]
    fn binary_grid_diff_in_mixed_sequence() {
        use crate::types::{CursorState, GridDimensions, RichGridDiff};

        let diff = RichGridDiff {
            dirty_rows: vec![],
            cursor: CursorState { row: 0, col: 0 },
            dimensions: GridDimensions { rows: 24, cols: 80 },
            alternate_screen: false,
            cursor_hidden: false,
            title: String::new(),
            scrollback_offset: 0,
            total_scrollback: 0,
            full_repaint: false,
        };

        let mut buf = Vec::new();
        // Output (binary tag 0x01)
        write_daemon_message(&mut buf, &DaemonMessage::Event(Event::Output {
            session_id: "s1".into(),
            data: vec![65],
        })).unwrap();
        // GridDiff (binary tag 0x04)
        write_daemon_message(&mut buf, &DaemonMessage::Event(Event::GridDiff {
            session_id: "s1".into(),
            diff: diff.clone(),
        })).unwrap();
        // JSON (Pong)
        write_daemon_message(&mut buf, &DaemonMessage::Response(Response::Pong)).unwrap();

        let mut cursor = Cursor::new(buf);

        // Read Output
        let m1 = read_daemon_message(&mut cursor).unwrap().unwrap();
        assert!(matches!(m1, DaemonMessage::Event(Event::Output { .. })));

        // Read GridDiff
        let m2 = read_daemon_message(&mut cursor).unwrap().unwrap();
        assert!(matches!(m2, DaemonMessage::Event(Event::GridDiff { .. })));

        // Read Pong
        let m3 = read_daemon_message(&mut cursor).unwrap().unwrap();
        assert!(matches!(m3, DaemonMessage::Response(Response::Pong)));

        // EOF
        assert!(read_daemon_message(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn binary_request_write_roundtrip() {
        let msg = Request::Write {
            session_id: "sess-456".into(),
            data: vec![3], // Ctrl+C
        };

        let mut buf = Vec::new();
        write_request(&mut buf, &msg).unwrap();

        let mut cursor = Cursor::new(buf);
        let result = read_request(&mut cursor).unwrap().unwrap();

        match result {
            Request::Write { session_id, data } => {
                assert_eq!(session_id, "sess-456");
                assert_eq!(data, vec![3]);
            }
            other => panic!("Expected Request::Write, got {:?}", other),
        }
    }

    #[test]
    fn binary_response_buffer_roundtrip() {
        let msg = DaemonMessage::Response(Response::Buffer {
            session_id: "sess-789".into(),
            data: vec![27, 91, 72], // ESC[H
        });

        let mut buf = Vec::new();
        write_daemon_message(&mut buf, &msg).unwrap();

        let mut cursor = Cursor::new(buf);
        let result = read_daemon_message(&mut cursor).unwrap().unwrap();

        match result {
            DaemonMessage::Response(Response::Buffer { session_id, data }) => {
                assert_eq!(session_id, "sess-789");
                assert_eq!(data, vec![27, 91, 72]);
            }
            other => panic!("Expected Response::Buffer, got {:?}", other),
        }
    }

    #[test]
    fn json_messages_still_work_through_typed_functions() {
        // Non-binary DaemonMessage variants go through JSON
        let msg = DaemonMessage::Response(Response::Pong);
        let mut buf = Vec::new();
        write_daemon_message(&mut buf, &msg).unwrap();

        let mut cursor = Cursor::new(buf);
        let result = read_daemon_message(&mut cursor).unwrap().unwrap();
        assert!(matches!(result, DaemonMessage::Response(Response::Pong)));

        // Non-binary Request variants go through JSON
        let msg = Request::Ping;
        let mut buf = Vec::new();
        write_request(&mut buf, &msg).unwrap();

        let mut cursor = Cursor::new(buf);
        let result = read_request(&mut cursor).unwrap().unwrap();
        assert!(matches!(result, Request::Ping));
    }

    #[test]
    fn mixed_binary_and_json_sequence() {
        let mut buf = Vec::new();

        // Write a mix of binary and JSON messages
        write_daemon_message(
            &mut buf,
            &DaemonMessage::Event(Event::Output {
                session_id: "s1".into(),
                data: vec![65, 66, 67],
            }),
        )
        .unwrap();

        write_daemon_message(
            &mut buf,
            &DaemonMessage::Response(Response::Pong),
        )
        .unwrap();

        write_daemon_message(
            &mut buf,
            &DaemonMessage::Response(Response::Buffer {
                session_id: "s2".into(),
                data: vec![1, 2, 3],
            }),
        )
        .unwrap();

        write_daemon_message(
            &mut buf,
            &DaemonMessage::Event(Event::SessionClosed {
                session_id: "s3".into(),
                exit_code: Some(1),
            }),
        )
        .unwrap();

        // Read them all back
        let mut cursor = Cursor::new(buf);

        let m1 = read_daemon_message(&mut cursor).unwrap().unwrap();
        assert!(matches!(
            m1,
            DaemonMessage::Event(Event::Output { ref session_id, .. }) if session_id == "s1"
        ));

        let m2 = read_daemon_message(&mut cursor).unwrap().unwrap();
        assert!(matches!(m2, DaemonMessage::Response(Response::Pong)));

        let m3 = read_daemon_message(&mut cursor).unwrap().unwrap();
        assert!(matches!(
            m3,
            DaemonMessage::Response(Response::Buffer { ref session_id, .. }) if session_id == "s2"
        ));

        let m4 = read_daemon_message(&mut cursor).unwrap().unwrap();
        assert!(matches!(
            m4,
            DaemonMessage::Event(Event::SessionClosed { ref session_id, exit_code: Some(1) }) if session_id == "s3"
        ));

        assert!(read_daemon_message(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn binary_empty_data() {
        let msg = DaemonMessage::Event(Event::Output {
            session_id: "s".into(),
            data: vec![],
        });

        let mut buf = Vec::new();
        write_daemon_message(&mut buf, &msg).unwrap();

        let mut cursor = Cursor::new(buf);
        let result = read_daemon_message(&mut cursor).unwrap().unwrap();
        match result {
            DaemonMessage::Event(Event::Output { session_id, data }) => {
                assert_eq!(session_id, "s");
                assert!(data.is_empty());
            }
            other => panic!("Expected Event::Output, got {:?}", other),
        }
    }

    #[test]
    fn binary_large_data() {
        let large_data = vec![0xAB; 1_000_000]; // 1MB
        let msg = DaemonMessage::Event(Event::Output {
            session_id: "large".into(),
            data: large_data.clone(),
        });

        let mut buf = Vec::new();
        write_daemon_message(&mut buf, &msg).unwrap();

        let mut cursor = Cursor::new(buf);
        let result = read_daemon_message(&mut cursor).unwrap().unwrap();
        match result {
            DaemonMessage::Event(Event::Output { session_id, data }) => {
                assert_eq!(session_id, "large");
                assert_eq!(data.len(), 1_000_000);
                assert_eq!(data, large_data);
            }
            other => panic!("Expected Event::Output, got {:?}", other),
        }
    }

    #[test]
    fn binary_long_session_id() {
        // session_id_len is a u8, so max 255 bytes
        let long_id = "a".repeat(255);
        let msg = Request::Write {
            session_id: long_id.clone(),
            data: vec![42],
        };

        let mut buf = Vec::new();
        write_request(&mut buf, &msg).unwrap();

        let mut cursor = Cursor::new(buf);
        let result = read_request(&mut cursor).unwrap().unwrap();
        match result {
            Request::Write { session_id, data } => {
                assert_eq!(session_id, long_id);
                assert_eq!(data, vec![42]);
            }
            other => panic!("Expected Request::Write, got {:?}", other),
        }
    }

    #[test]
    fn binary_eof_returns_none() {
        let mut cursor = Cursor::new(Vec::<u8>::new());
        assert!(read_daemon_message(&mut cursor).unwrap().is_none());
        assert!(read_request(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn binary_frame_is_smaller_than_json() {
        let data = vec![0u8; 1000];
        let msg = DaemonMessage::Event(Event::Output {
            session_id: "test-session".into(),
            data: data.clone(),
        });

        let mut binary_buf = Vec::new();
        write_daemon_message(&mut binary_buf, &msg).unwrap();

        let mut json_buf = Vec::new();
        write_message(&mut json_buf, &msg).unwrap();

        // Binary: 4 (len) + 1 (tag) + 1 (sid_len) + 12 (sid) + 1000 (data) = 1018
        // JSON: 4 (len) + ~5000+ (each 0 byte = "0," in JSON array)
        assert!(
            binary_buf.len() < json_buf.len(),
            "Binary ({} bytes) should be smaller than JSON ({} bytes)",
            binary_buf.len(),
            json_buf.len()
        );
    }

    // ── Shim binary frame roundtrip tests ────────────────────────────

    #[test]
    fn shim_binary_write_roundtrip() {
        let input_data = b"hello from daemon";
        let mut buf = Vec::new();
        write_shim_binary(&mut buf, TAG_SHIM_WRITE, input_data).unwrap();

        let mut cursor = Cursor::new(buf);
        let frame = read_shim_frame(&mut cursor).unwrap().unwrap();
        match frame {
            ShimFrame::Binary { tag, data } => {
                assert_eq!(tag, TAG_SHIM_WRITE);
                assert_eq!(data, input_data);
            }
            other => panic!("Expected Binary frame, got {:?}", other),
        }
    }

    #[test]
    fn shim_binary_buffer_data_roundtrip() {
        let replay_data = vec![27, 91, 72, 27, 91, 50, 74]; // ESC[H ESC[2J
        let mut buf = Vec::new();
        write_shim_binary(&mut buf, TAG_SHIM_BUFFER_DATA, &replay_data).unwrap();

        let mut cursor = Cursor::new(buf);
        let frame = read_shim_frame(&mut cursor).unwrap().unwrap();
        match frame {
            ShimFrame::Binary { tag, data } => {
                assert_eq!(tag, TAG_SHIM_BUFFER_DATA);
                assert_eq!(data, replay_data);
            }
            other => panic!("Expected Binary frame, got {:?}", other),
        }
    }

    #[test]
    fn shim_binary_output_roundtrip() {
        let output_data = b"shell output bytes";
        let mut buf = Vec::new();
        write_shim_binary(&mut buf, TAG_SHIM_OUTPUT, output_data).unwrap();

        let mut cursor = Cursor::new(buf);
        let frame = read_shim_frame(&mut cursor).unwrap().unwrap();
        match frame {
            ShimFrame::Binary { tag, data } => {
                assert_eq!(tag, TAG_SHIM_OUTPUT);
                assert_eq!(data, output_data.to_vec());
            }
            other => panic!("Expected Binary frame, got {:?}", other),
        }
    }

    #[test]
    fn shim_json_control_message_roundtrip() {
        use crate::messages::ShimRequest;

        let req = ShimRequest::Resize { rows: 24, cols: 80 };
        let mut buf = Vec::new();
        write_shim_json(&mut buf, &req).unwrap();

        let mut cursor = Cursor::new(buf);
        let frame = read_shim_frame(&mut cursor).unwrap().unwrap();
        match frame {
            ShimFrame::Json(json_bytes) => {
                let deserialized: ShimRequest = serde_json::from_slice(&json_bytes).unwrap();
                match deserialized {
                    ShimRequest::Resize { rows, cols } => {
                        assert_eq!(rows, 24);
                        assert_eq!(cols, 80);
                    }
                    other => panic!("Expected Resize, got {:?}", other),
                }
            }
            other => panic!("Expected Json frame, got {:?}", other),
        }
    }

    #[test]
    fn shim_json_response_roundtrip() {
        use crate::messages::ShimResponse;

        let resp = ShimResponse::StatusInfo {
            shell_pid: 999,
            running: true,
            rows: 30,
            cols: 120,
        };
        let mut buf = Vec::new();
        write_shim_json(&mut buf, &resp).unwrap();

        let mut cursor = Cursor::new(buf);
        let frame = read_shim_frame(&mut cursor).unwrap().unwrap();
        match frame {
            ShimFrame::Json(json_bytes) => {
                let deserialized: ShimResponse = serde_json::from_slice(&json_bytes).unwrap();
                match deserialized {
                    ShimResponse::StatusInfo { shell_pid, running, rows, cols } => {
                        assert_eq!(shell_pid, 999);
                        assert!(running);
                        assert_eq!(rows, 30);
                        assert_eq!(cols, 120);
                    }
                    other => panic!("Expected StatusInfo, got {:?}", other),
                }
            }
            other => panic!("Expected Json frame, got {:?}", other),
        }
    }

    #[test]
    fn shim_binary_empty_data() {
        let mut buf = Vec::new();
        write_shim_binary(&mut buf, TAG_SHIM_OUTPUT, &[]).unwrap();

        let mut cursor = Cursor::new(buf);
        let frame = read_shim_frame(&mut cursor).unwrap().unwrap();
        match frame {
            ShimFrame::Binary { tag, data } => {
                assert_eq!(tag, TAG_SHIM_OUTPUT);
                assert!(data.is_empty());
            }
            other => panic!("Expected Binary frame, got {:?}", other),
        }
    }

    #[test]
    fn shim_binary_large_data() {
        let large_data = vec![0xCD; 500_000]; // 500KB
        let mut buf = Vec::new();
        write_shim_binary(&mut buf, TAG_SHIM_BUFFER_DATA, &large_data).unwrap();

        let mut cursor = Cursor::new(buf);
        let frame = read_shim_frame(&mut cursor).unwrap().unwrap();
        match frame {
            ShimFrame::Binary { tag, data } => {
                assert_eq!(tag, TAG_SHIM_BUFFER_DATA);
                assert_eq!(data.len(), 500_000);
                assert_eq!(data, large_data);
            }
            other => panic!("Expected Binary frame, got {:?}", other),
        }
    }

    #[test]
    fn shim_eof_returns_none() {
        let mut cursor = Cursor::new(Vec::<u8>::new());
        assert!(read_shim_frame(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn shim_mixed_binary_and_json_sequence() {
        use crate::messages::{ShimRequest, ShimResponse};

        let mut buf = Vec::new();

        // Binary: daemon writes input to shim
        write_shim_binary(&mut buf, TAG_SHIM_WRITE, b"ls\r\n").unwrap();
        // JSON: daemon sends resize
        write_shim_json(&mut buf, &ShimRequest::Resize { rows: 50, cols: 132 }).unwrap();
        // Binary: shim sends output
        write_shim_binary(&mut buf, TAG_SHIM_OUTPUT, b"file1.txt\nfile2.txt\n").unwrap();
        // JSON: shim sends status
        write_shim_json(&mut buf, &ShimResponse::StatusInfo {
            shell_pid: 42,
            running: true,
            rows: 50,
            cols: 132,
        }).unwrap();

        let mut cursor = Cursor::new(buf);

        // Frame 1: binary write
        let f1 = read_shim_frame(&mut cursor).unwrap().unwrap();
        match f1 {
            ShimFrame::Binary { tag, data } => {
                assert_eq!(tag, TAG_SHIM_WRITE);
                assert_eq!(data, b"ls\r\n");
            }
            other => panic!("Expected Binary frame, got {:?}", other),
        }

        // Frame 2: JSON resize
        let f2 = read_shim_frame(&mut cursor).unwrap().unwrap();
        match f2 {
            ShimFrame::Json(json_bytes) => {
                let req: ShimRequest = serde_json::from_slice(&json_bytes).unwrap();
                assert!(matches!(req, ShimRequest::Resize { rows: 50, cols: 132 }));
            }
            other => panic!("Expected Json frame, got {:?}", other),
        }

        // Frame 3: binary output
        let f3 = read_shim_frame(&mut cursor).unwrap().unwrap();
        match f3 {
            ShimFrame::Binary { tag, data } => {
                assert_eq!(tag, TAG_SHIM_OUTPUT);
                assert_eq!(data, b"file1.txt\nfile2.txt\n");
            }
            other => panic!("Expected Binary frame, got {:?}", other),
        }

        // Frame 4: JSON status
        let f4 = read_shim_frame(&mut cursor).unwrap().unwrap();
        match f4 {
            ShimFrame::Json(json_bytes) => {
                let resp: ShimResponse = serde_json::from_slice(&json_bytes).unwrap();
                match resp {
                    ShimResponse::StatusInfo { shell_pid, running, .. } => {
                        assert_eq!(shell_pid, 42);
                        assert!(running);
                    }
                    other => panic!("Expected StatusInfo, got {:?}", other),
                }
            }
            other => panic!("Expected Json frame, got {:?}", other),
        }

        // EOF
        assert!(read_shim_frame(&mut cursor).unwrap().is_none());
    }
}
