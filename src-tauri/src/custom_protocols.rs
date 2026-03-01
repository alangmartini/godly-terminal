//! Custom URI scheme protocol handlers (`gpuframe://`).

use std::sync::Arc;

use crate::daemon_client::DaemonClient;
use crate::gpu_renderer::GpuRendererManager;
use godly_protocol::{Request, Response};

/// Handle a `gpuframe://` protocol request.
///
/// Expected URLs:
///   `gpuframe://render/{session_id}`             — returns PNG (backward compat)
///   `gpuframe://render/{session_id}?format=raw`  — returns raw RGBA with 8-byte header
///
/// Fetches the terminal grid from the daemon and renders it via the GPU pipeline.
pub(crate) fn handle_gpuframe_request(
    uri: &str,
    gpu: &Arc<GpuRendererManager>,
    daemon: &Arc<DaemonClient>,
) -> tauri::http::Response<Vec<u8>> {
    // Parse path and query from the URI
    let (path, query) = uri.split_once('?').unwrap_or((uri, ""));
    let use_raw = query.contains("format=raw");

    // Parse DPI scale factor from query (e.g. &dpr=1.5)
    let dpr = query
        .split('&')
        .find_map(|param| param.strip_prefix("dpr="))
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(1.0)
        .clamp(0.5, 4.0);

    let session_id = match path.strip_prefix("/render/") {
        Some(id) if !id.is_empty() => id,
        _ => {
            return tauri::http::Response::builder()
                .status(400)
                .header("Access-Control-Allow-Origin", "*")
                .body(b"Bad request. Use /render/{session_id}".to_vec())
                .unwrap();
        }
    };

    if !gpu.is_available_with_dpr(dpr) {
        return tauri::http::Response::builder()
            .status(503)
            .header("Access-Control-Allow-Origin", "*")
            .body(b"GPU renderer not available".to_vec())
            .unwrap();
    }

    // Fetch grid snapshot from daemon
    let request = Request::ReadRichGrid {
        session_id: session_id.to_string(),
    };
    let response = match daemon.send_request(&request) {
        Ok(r) => r,
        Err(e) => {
            let msg = format!("Daemon error: {e}");
            return tauri::http::Response::builder()
                .status(502)
                .header("Access-Control-Allow-Origin", "*")
                .body(msg.into_bytes())
                .unwrap();
        }
    };

    let grid = match response {
        Response::RichGrid { grid } => grid,
        Response::Error { message } => {
            return tauri::http::Response::builder()
                .status(404)
                .header("Access-Control-Allow-Origin", "*")
                .body(message.into_bytes())
                .unwrap();
        }
        _ => {
            return tauri::http::Response::builder()
                .status(500)
                .header("Access-Control-Allow-Origin", "*")
                .body(b"Unexpected daemon response".to_vec())
                .unwrap();
        }
    };

    if use_raw {
        // Raw RGBA format: [width: u32 LE][height: u32 LE][rgba_pixels...]
        match gpu.render_terminal_raw(&grid, dpr) {
            Ok(raw_bytes) => tauri::http::Response::builder()
                .status(200)
                .header("Content-Type", "application/octet-stream")
                .header("Access-Control-Allow-Origin", "*")
                .body(raw_bytes)
                .unwrap(),
            Err(e) => {
                let msg = format!("GPU render failed: {e}");
                tauri::http::Response::builder()
                    .status(500)
                    .header("Access-Control-Allow-Origin", "*")
                    .body(msg.into_bytes())
                    .unwrap()
            }
        }
    } else {
        // PNG format (backward compatible)
        match gpu.render_terminal_png(&grid, dpr) {
            Ok(png_bytes) => tauri::http::Response::builder()
                .status(200)
                .header("Content-Type", "image/png")
                .header("Access-Control-Allow-Origin", "*")
                .body(png_bytes)
                .unwrap(),
            Err(e) => {
                let msg = format!("GPU render failed: {e}");
                tauri::http::Response::builder()
                    .status(500)
                    .header("Access-Control-Allow-Origin", "*")
                    .body(msg.into_bytes())
                    .unwrap()
            }
        }
    }
}
