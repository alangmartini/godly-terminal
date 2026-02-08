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

/// Read a single JSON-RPC message from stdin using Content-Length framing.
pub fn read_message(reader: &mut impl BufRead) -> io::Result<Option<JsonRpcRequest>> {
    // Read headers until empty line
    let mut content_length: Option<usize> = None;

    mcp_log!("read_message: waiting for headers...");

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            mcp_log!("read_message: EOF while reading headers");
            return Ok(None); // EOF
        }

        let trimmed = line.trim();
        mcp_log!("read_message: header line ({} bytes): {:?}", bytes_read, trimmed);

        if trimmed.is_empty() {
            mcp_log!("read_message: end of headers");
            break; // End of headers
        }

        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            let parsed = value.trim().parse::<usize>();
            mcp_log!("read_message: Content-Length parsed: {:?}", parsed);
            content_length = Some(
                parsed.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            );
        }
    }

    let length = content_length.ok_or_else(|| {
        mcp_log!("read_message: ERROR — missing Content-Length header");
        io::Error::new(io::ErrorKind::InvalidData, "Missing Content-Length header")
    })?;

    mcp_log!("read_message: reading body ({} bytes)...", length);

    // Read body
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;

    let body_str = String::from_utf8_lossy(&body);
    mcp_log!("read_message: raw body: {}", body_str);

    let request: JsonRpcRequest =
        serde_json::from_slice(&body).map_err(|e| {
            mcp_log!("read_message: JSON parse error: {}", e);
            io::Error::new(io::ErrorKind::InvalidData, e)
        })?;

    mcp_log!("read_message: parsed OK — method={}, id={:?}", request.method, request.id);

    Ok(Some(request))
}

/// Write a JSON-RPC response to stdout using Content-Length framing.
pub fn write_message(writer: &mut impl Write, response: &JsonRpcResponse) -> io::Result<()> {
    let body = serde_json::to_string(response)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    mcp_log!("write_message: body length={}, content: {}", body.len(), body);

    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    writer.flush()?;

    mcp_log!("write_message: flushed OK");

    Ok(())
}
