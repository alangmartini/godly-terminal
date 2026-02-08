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

/// Read a single JSON-RPC message from stdin using Content-Length framing (MCP stdio transport).
pub fn read_message(reader: &mut impl BufRead) -> io::Result<Option<JsonRpcRequest>> {
    mcp_log!("read_message: waiting for headers...");

    // Read headers until empty line
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            mcp_log!("read_message: EOF while reading headers");
            return Ok(None);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            break; // Empty line = end of headers
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().ok();
            mcp_log!("read_message: Content-Length = {:?}", content_length);
        } else {
            mcp_log!("read_message: ignoring header: {}", trimmed);
        }
    }

    let content_length = match content_length {
        Some(len) => len,
        None => {
            mcp_log!("read_message: missing Content-Length header");
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Missing Content-Length header",
            ));
        }
    };

    // Read exactly content_length bytes for the body
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;

    let body_str = String::from_utf8(body)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    mcp_log!("read_message: body ({} bytes): {}", content_length, body_str);

    let request: JsonRpcRequest = serde_json::from_str(&body_str).map_err(|e| {
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
    fn read_message_returns_none_on_eof() {
        let input: &[u8] = b"";
        let mut reader = BufReader::new(input);

        let result = read_message(&mut reader).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn read_message_handles_notification_without_id() {
        let body = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let input = make_framed_message(body);
        let mut reader = BufReader::new(&input[..]);

        let req = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(req.method, "notifications/initialized");
        assert!(req.id.is_none());
    }

    #[test]
    fn read_message_handles_multiple_sequential_messages() {
        let body1 = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}"#;
        let body2 = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
        let mut input = make_framed_message(body1);
        input.extend_from_slice(&make_framed_message(body2));
        let mut reader = BufReader::new(&input[..]);

        let req1 = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(req1.method, "initialize");
        assert_eq!(req1.id, Some(json!(0)));

        let req2 = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(req2.method, "tools/list");
        assert_eq!(req2.id, Some(json!(1)));
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

        // Verify Content-Length value matches actual body length
        let claimed_len: usize = header
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(claimed_len, body.len());

        // Verify body is valid JSON with expected fields
        let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 0);
        assert_eq!(parsed["result"]["ok"], true);
    }

    #[test]
    fn read_message_errors_on_missing_content_length() {
        // Headers with no Content-Length, then empty line
        let input = b"X-Custom: foo\r\n\r\n{}";
        let mut reader = BufReader::new(&input[..]);

        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn read_message_errors_on_invalid_json() {
        let bad_body = "not valid json";
        let input = make_framed_message(bad_body);
        let mut reader = BufReader::new(&input[..]);

        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
