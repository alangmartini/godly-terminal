//! Tauri commands for GPU-accelerated terminal rendering.
//!
//! These commands expose the `godly-renderer` GPU pipeline to the frontend.

use std::sync::Arc;
use tauri::State;

use crate::gpu_renderer::GpuRendererManager;

/// Check if GPU rendering is available on this system.
///
/// Returns `true` if a GPU adapter was found and the renderer initialized
/// successfully; `false` otherwise.
#[tauri::command]
pub fn gpu_renderer_available(
    gpu: State<'_, Arc<GpuRendererManager>>,
) -> bool {
    gpu.is_available()
}

/// Render a terminal to PNG bytes using the GPU renderer.
///
/// Fetches the current grid snapshot from the daemon, renders it via the GPU
/// pipeline, and returns the PNG image as a base64-encoded string.
#[tauri::command]
pub fn render_terminal_gpu(
    terminal_id: String,
    daemon: State<'_, Arc<crate::daemon_client::DaemonClient>>,
    gpu: State<'_, Arc<GpuRendererManager>>,
) -> Result<String, String> {
    use base64::Engine;
    use godly_protocol::{Request, Response};

    // Fetch grid snapshot from daemon
    let request = Request::ReadRichGrid {
        session_id: terminal_id,
    };
    let response = daemon.send_request(&request)?;

    let grid = match response {
        Response::RichGrid { grid } => grid,
        Response::Error { message } => return Err(message),
        _ => return Err("Unexpected response from daemon".to_string()),
    };

    // Render via GPU
    let png_bytes = gpu.render_terminal_png(&grid)?;

    // Return as base64
    Ok(base64::engine::general_purpose::STANDARD.encode(&png_bytes))
}

/// Get the cell size (width, height) in pixels from the GPU renderer.
/// Returns CSS pixels (not device pixels).
#[tauri::command]
pub fn get_gpu_cell_size(
    gpu: State<'_, Arc<GpuRendererManager>>,
) -> Result<(f64, f64), String> {
    gpu.cell_size()
}
