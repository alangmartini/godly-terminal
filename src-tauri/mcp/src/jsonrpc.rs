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

/// Read a single JSON-RPC message from stdin (newline-delimited JSON).
pub fn read_message(reader: &mut impl BufRead) -> io::Result<Option<JsonRpcRequest>> {
    mcp_log!("read_message: waiting for line...");

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            mcp_log!("read_message: EOF");
            return Ok(None);
        }

        let trimmed = line.trim();

        // Skip empty lines and Content-Length headers (some clients send them)
        if trimmed.is_empty() || trimmed.starts_with("Content-Length:") {
            mcp_log!("read_message: skipping non-JSON line: {:?}", trimmed);
            continue;
        }

        mcp_log!("read_message: got line ({} bytes): {}", bytes_read, trimmed);

        let request: JsonRpcRequest =
            serde_json::from_str(trimmed).map_err(|e| {
                mcp_log!("read_message: JSON parse error: {}", e);
                io::Error::new(io::ErrorKind::InvalidData, e)
            })?;

        mcp_log!("read_message: parsed OK — method={}, id={:?}", request.method, request.id);

        return Ok(Some(request));
    }
}

/// Write a JSON-RPC response to stdout (newline-delimited JSON).
pub fn write_message(writer: &mut impl Write, response: &JsonRpcResponse) -> io::Result<()> {
    let body = serde_json::to_string(response)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    mcp_log!("write_message: content: {}", body);

    writeln!(writer, "{}", body)?;
    writer.flush()?;

    mcp_log!("write_message: flushed OK");

    Ok(())
}
