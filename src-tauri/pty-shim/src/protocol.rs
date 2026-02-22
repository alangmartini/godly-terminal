use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

// Binary frame tags
pub const TAG_WRITE: u8 = 0x10;
pub const TAG_BUFFER_DATA: u8 = 0x11;
pub const TAG_OUTPUT: u8 = 0x12;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ShimControlRequest {
    Resize { rows: u16, cols: u16 },
    Status,
    Shutdown,
    DrainBuffer,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ShimControlResponse {
    StatusInfo {
        shell_pid: u32,
        running: bool,
        rows: u16,
        cols: u16,
    },
    ShellExited {
        exit_code: Option<i64>,
    },
}

/// Write a length-prefixed frame (4-byte big-endian length + payload).
pub fn write_frame<W: Write>(writer: &mut W, payload: &[u8]) -> io::Result<()> {
    let len = payload.len() as u32;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(payload)?;
    Ok(())
}

/// Read a length-prefixed frame. Returns `None` on EOF.
pub fn read_frame<R: Read>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
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
            format!("Frame too large: {} bytes", len),
        ));
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(Some(buf))
}

/// Write a binary data frame (tag byte + raw data bytes, length-prefixed).
pub fn write_binary_frame<W: Write>(writer: &mut W, tag: u8, data: &[u8]) -> io::Result<()> {
    let mut payload = Vec::with_capacity(1 + data.len());
    payload.push(tag);
    payload.extend_from_slice(data);
    write_frame(writer, &payload)
}

/// Write a JSON control message as a length-prefixed frame.
pub fn write_json<W: Write, T: Serialize>(writer: &mut W, msg: &T) -> io::Result<()> {
    let json =
        serde_json::to_vec(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write_frame(writer, &json)
}

/// A parsed incoming frame: either a binary tagged message or a JSON control request.
#[derive(Debug)]
pub enum ShimFrame {
    Binary { tag: u8, data: Vec<u8> },
    Control(ShimControlRequest),
}

/// Parse a received frame buffer into either a binary frame or a JSON control request.
/// Binary frames have a non-`{` first byte; JSON control messages start with `{` (0x7B).
pub fn parse_incoming_frame(buf: &[u8]) -> io::Result<ShimFrame> {
    if buf.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Empty frame",
        ));
    }
    if buf[0] == 0x7B {
        // '{' — JSON control message
        let req: ShimControlRequest = serde_json::from_slice(buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(ShimFrame::Control(req))
    } else {
        Ok(ShimFrame::Binary {
            tag: buf[0],
            data: buf[1..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_write_read_frame_roundtrip() {
        let mut buf = Vec::new();
        let payload = b"hello world";
        write_frame(&mut buf, payload).unwrap();

        let mut cursor = Cursor::new(&buf);
        let result = read_frame(&mut cursor).unwrap().unwrap();
        assert_eq!(result, payload);
    }

    #[test]
    fn test_write_read_empty_frame() {
        let mut buf = Vec::new();
        write_frame(&mut buf, b"").unwrap();

        let mut cursor = Cursor::new(&buf);
        let result = read_frame(&mut cursor).unwrap().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_read_frame_eof_returns_none() {
        let buf: Vec<u8> = Vec::new();
        let mut cursor = Cursor::new(&buf);
        let result = read_frame(&mut cursor).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_read_frame_partial_length_returns_eof_error() {
        // Only 2 bytes of the 4-byte length header
        let buf = vec![0u8, 1];
        let mut cursor = Cursor::new(&buf);
        let result = read_frame(&mut cursor).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_read_frame_too_large_rejected() {
        // Encode a length of 17MB (over 16MB limit)
        let len: u32 = 17 * 1024 * 1024;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_be_bytes());
        // Don't need actual data — should fail on length check
        let mut cursor = Cursor::new(&buf);
        let err = read_frame(&mut cursor).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("Frame too large"));
    }

    #[test]
    fn test_multiple_frames_sequential() {
        let mut buf = Vec::new();
        write_frame(&mut buf, b"first").unwrap();
        write_frame(&mut buf, b"second").unwrap();
        write_frame(&mut buf, b"third").unwrap();

        let mut cursor = Cursor::new(&buf);
        assert_eq!(read_frame(&mut cursor).unwrap().unwrap(), b"first");
        assert_eq!(read_frame(&mut cursor).unwrap().unwrap(), b"second");
        assert_eq!(read_frame(&mut cursor).unwrap().unwrap(), b"third");
        assert!(read_frame(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn test_binary_frame_roundtrip() {
        let mut buf = Vec::new();
        let data = b"input bytes here";
        write_binary_frame(&mut buf, TAG_WRITE, data).unwrap();

        let mut cursor = Cursor::new(&buf);
        let frame_data = read_frame(&mut cursor).unwrap().unwrap();
        assert_eq!(frame_data[0], TAG_WRITE);
        assert_eq!(&frame_data[1..], data);
    }

    #[test]
    fn test_binary_frame_output_tag() {
        let mut buf = Vec::new();
        let data = b"\x1b[31mred text\x1b[0m";
        write_binary_frame(&mut buf, TAG_OUTPUT, data).unwrap();

        let mut cursor = Cursor::new(&buf);
        let frame_data = read_frame(&mut cursor).unwrap().unwrap();
        assert_eq!(frame_data[0], TAG_OUTPUT);
        assert_eq!(&frame_data[1..], data);
    }

    #[test]
    fn test_binary_frame_buffer_data_tag() {
        let mut buf = Vec::new();
        let data = b"ring buffer replay data";
        write_binary_frame(&mut buf, TAG_BUFFER_DATA, data).unwrap();

        let mut cursor = Cursor::new(&buf);
        let frame_data = read_frame(&mut cursor).unwrap().unwrap();
        assert_eq!(frame_data[0], TAG_BUFFER_DATA);
        assert_eq!(&frame_data[1..], data);
    }

    #[test]
    fn test_json_control_resize_roundtrip() {
        let req = ShimControlRequest::Resize { rows: 24, cols: 80 };
        let mut buf = Vec::new();
        write_json(&mut buf, &req).unwrap();

        let mut cursor = Cursor::new(&buf);
        let frame_data = read_frame(&mut cursor).unwrap().unwrap();
        let parsed = parse_incoming_frame(&frame_data).unwrap();
        match parsed {
            ShimFrame::Control(ShimControlRequest::Resize { rows, cols }) => {
                assert_eq!(rows, 24);
                assert_eq!(cols, 80);
            }
            other => panic!("Expected Resize, got {:?}", other),
        }
    }

    #[test]
    fn test_json_control_status_roundtrip() {
        let req = ShimControlRequest::Status;
        let mut buf = Vec::new();
        write_json(&mut buf, &req).unwrap();

        let mut cursor = Cursor::new(&buf);
        let frame_data = read_frame(&mut cursor).unwrap().unwrap();
        let parsed = parse_incoming_frame(&frame_data).unwrap();
        match parsed {
            ShimFrame::Control(ShimControlRequest::Status) => {}
            other => panic!("Expected Status, got {:?}", other),
        }
    }

    #[test]
    fn test_json_control_shutdown_roundtrip() {
        let req = ShimControlRequest::Shutdown;
        let mut buf = Vec::new();
        write_json(&mut buf, &req).unwrap();

        let mut cursor = Cursor::new(&buf);
        let frame_data = read_frame(&mut cursor).unwrap().unwrap();
        let parsed = parse_incoming_frame(&frame_data).unwrap();
        match parsed {
            ShimFrame::Control(ShimControlRequest::Shutdown) => {}
            other => panic!("Expected Shutdown, got {:?}", other),
        }
    }

    #[test]
    fn test_json_control_drain_buffer_roundtrip() {
        let req = ShimControlRequest::DrainBuffer;
        let mut buf = Vec::new();
        write_json(&mut buf, &req).unwrap();

        let mut cursor = Cursor::new(&buf);
        let frame_data = read_frame(&mut cursor).unwrap().unwrap();
        let parsed = parse_incoming_frame(&frame_data).unwrap();
        match parsed {
            ShimFrame::Control(ShimControlRequest::DrainBuffer) => {}
            other => panic!("Expected DrainBuffer, got {:?}", other),
        }
    }

    #[test]
    fn test_json_response_status_info() {
        let resp = ShimControlResponse::StatusInfo {
            shell_pid: 1234,
            running: true,
            rows: 30,
            cols: 120,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ShimControlResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, resp);
    }

    #[test]
    fn test_json_response_shell_exited_with_code() {
        let resp = ShimControlResponse::ShellExited {
            exit_code: Some(0),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ShimControlResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, resp);
    }

    #[test]
    fn test_json_response_shell_exited_no_code() {
        let resp = ShimControlResponse::ShellExited { exit_code: None };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ShimControlResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, resp);
    }

    #[test]
    fn test_parse_incoming_frame_empty_rejected() {
        let err = parse_incoming_frame(b"").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_parse_incoming_frame_binary_write() {
        let mut payload = vec![TAG_WRITE];
        payload.extend_from_slice(b"user input");
        match parse_incoming_frame(&payload).unwrap() {
            ShimFrame::Binary { tag, data } => {
                assert_eq!(tag, TAG_WRITE);
                assert_eq!(data, b"user input");
            }
            other => panic!("Expected Binary, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_incoming_frame_binary_single_byte() {
        let payload = vec![TAG_OUTPUT];
        match parse_incoming_frame(&payload).unwrap() {
            ShimFrame::Binary { tag, data } => {
                assert_eq!(tag, TAG_OUTPUT);
                assert!(data.is_empty());
            }
            other => panic!("Expected Binary, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_incoming_frame_invalid_json() {
        let payload = b"{invalid json}";
        let err = parse_incoming_frame(payload).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_frame_length_encoding() {
        // Verify the wire format: 4 bytes big-endian length then payload
        let mut buf = Vec::new();
        write_frame(&mut buf, b"AB").unwrap();
        assert_eq!(buf, vec![0, 0, 0, 2, b'A', b'B']);
    }

    #[test]
    fn test_binary_frame_wire_format() {
        let mut buf = Vec::new();
        write_binary_frame(&mut buf, TAG_WRITE, b"X").unwrap();
        // Length = 2 (1 tag + 1 data), then tag, then data
        assert_eq!(buf, vec![0, 0, 0, 2, TAG_WRITE, b'X']);
    }

    #[test]
    fn test_json_request_serialization_format() {
        let req = ShimControlRequest::Resize { rows: 24, cols: 80 };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"type\":\"resize\""));
        assert!(json.contains("\"rows\":24"));
        assert!(json.contains("\"cols\":80"));
    }

    #[test]
    fn test_json_response_serialization_format() {
        let resp = ShimControlResponse::StatusInfo {
            shell_pid: 42,
            running: true,
            rows: 24,
            cols: 80,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"type\":\"status_info\""));
        assert!(json.contains("\"shell_pid\":42"));
    }

    #[test]
    fn test_write_frame_large_payload() {
        let mut buf = Vec::new();
        let payload = vec![0xAA; 65536];
        write_frame(&mut buf, &payload).unwrap();

        let mut cursor = Cursor::new(&buf);
        let result = read_frame(&mut cursor).unwrap().unwrap();
        assert_eq!(result.len(), 65536);
        assert!(result.iter().all(|&b| b == 0xAA));
    }
}
