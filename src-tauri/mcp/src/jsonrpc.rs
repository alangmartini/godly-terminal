use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::log::mcp_log;

/// JSON-RPC request (MCP uses JSON-RPC 2.0 over stdio with Content-Length framing)
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// JSON-RPC response
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i64, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

/// Read a single JSON-RPC message from stdin.
///
/// Supports both Content-Length framing (MCP spec) and raw JSON lines
/// (what Claude Code actually sends). Auto-detects the format per message.
pub fn read_message(reader: &mut impl BufRead) -> io::Result<Option<JsonRpcRequest>> {
    mcp_log!("read_message: waiting for input...");

    // Read lines, auto-detecting whether we get Content-Length headers or raw JSON.
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            mcp_log!("read_message: EOF");
            return Ok(None);
        }

        let trimmed = line.trim();

        // Skip empty lines (header/body separator, or blank lines between JSON messages)
        if trimmed.is_empty() {
            if content_length.is_some() {
                break; // End of headers — read body next
            }
            continue; // Blank line between raw JSON messages
        }

        // Content-Length header → switch to framed mode
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().ok();
            mcp_log!("read_message: Content-Length = {:?}", content_length);
            continue;
        }

        // If we haven't seen Content-Length, this line is raw JSON
        if content_length.is_none() {
            mcp_log!("read_message: raw JSON line ({} bytes)", trimmed.len());
            return parse_request(trimmed);
        }

        // Unexpected non-header line while reading headers — skip it
        mcp_log!("read_message: ignoring unknown header: {}", trimmed);
    }

    // Content-Length framing: read exactly that many bytes
    let length = content_length.unwrap(); // safe: we only break from loop when Some
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;

    let body_str = String::from_utf8(body)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    mcp_log!("read_message: framed body ({} bytes): {}", length, body_str);
    parse_request(&body_str)
}

fn parse_request(json_str: &str) -> io::Result<Option<JsonRpcRequest>> {
    let request: JsonRpcRequest = serde_json::from_str(json_str).map_err(|e| {
        mcp_log!("read_message: JSON parse error: {}", e);
        io::Error::new(io::ErrorKind::InvalidData, e)
    })?;

    mcp_log!(
        "read_message: parsed OK — method={}, id={:?}",
        request.method,
        request.id
    );

    Ok(Some(request))
}

/// Write a JSON-RPC response to stdout with Content-Length framing (MCP stdio transport).
pub fn write_message(writer: &mut impl Write, response: &JsonRpcResponse) -> io::Result<()> {
    let body = serde_json::to_string(response)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    mcp_log!("write_message: body ({} bytes): {}", body.len(), body);

    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    writer.flush()?;

    mcp_log!("write_message: flushed OK");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::BufReader;

    fn make_framed_message(body: &str) -> Vec<u8> {
        format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
    }

    // --- Content-Length framing tests ---

    #[test]
    fn read_message_parses_content_length_framing() {
        let body = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}"#;
        let input = make_framed_message(body);
        let mut reader = BufReader::new(&input[..]);

        let req = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, Some(json!(0)));
    }

    #[test]
    fn read_message_handles_multiple_framed_messages() {
        let body1 = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}"#;
        let body2 = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
        let mut input = make_framed_message(body1);
        input.extend_from_slice(&make_framed_message(body2));
        let mut reader = BufReader::new(&input[..]);

        let req1 = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(req1.method, "initialize");

        let req2 = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(req2.method, "tools/list");
    }

    #[test]
    fn read_message_errors_on_invalid_json_in_framed_body() {
        let input = make_framed_message("not valid json");
        let mut reader = BufReader::new(&input[..]);

        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    // --- Raw JSON line tests (what Claude Code actually sends) ---

    #[test]
    fn read_message_parses_raw_json_line() {
        // Claude Code sends raw JSON lines without Content-Length framing
        let input = b"{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"initialize\",\"params\":{}}\n";
        let mut reader = BufReader::new(&input[..]);

        let req = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, Some(json!(0)));
    }

    #[test]
    fn read_message_parses_multiple_raw_json_lines() {
        let input = b"{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"initialize\"}\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n";
        let mut reader = BufReader::new(&input[..]);

        let req1 = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(req1.method, "initialize");

        let req2 = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(req2.method, "tools/list");
    }

    #[test]
    fn read_message_parses_raw_notification_without_id() {
        let input = b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n";
        let mut reader = BufReader::new(&input[..]);

        let req = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(req.method, "notifications/initialized");
        assert!(req.id.is_none());
    }

    #[test]
    fn read_message_skips_blank_lines_in_raw_mode() {
        let input = b"\n\n{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"initialize\"}\n";
        let mut reader = BufReader::new(&input[..]);

        let req = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(req.method, "initialize");
    }

    // --- Common tests ---

    #[test]
    fn read_message_returns_none_on_eof() {
        let mut reader = BufReader::new(&b""[..]);
        assert!(read_message(&mut reader).unwrap().is_none());
    }

    #[test]
    fn write_message_content_length_matches_body() {
        let response = JsonRpcResponse::success(Some(json!(0)), json!({"ok": true}));
        let mut output = Vec::new();

        write_message(&mut output, &response).unwrap();

        let output_str = String::from_utf8(output).unwrap();
        let header_end = output_str.find("\r\n\r\n").unwrap();
        let header = &output_str[..header_end];
        let body = &output_str[header_end + 4..];

        let claimed_len: usize = header
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(claimed_len, body.len());

        let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 0);
        assert_eq!(parsed["result"]["ok"], true);
    }
}
