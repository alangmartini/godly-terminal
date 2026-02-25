//! Tauri commands for GPU-accelerated terminal rendering.
//!
//! These commands expose the `godly-renderer` GPU pipeline to the frontend.
//! When the `gpu-renderer` feature is disabled, stub commands return errors
//! indicating that GPU rendering is not available.

use std::sync::Arc;
use tauri::State;

use crate::gpu_renderer::GpuRendererManager;

/// Check if GPU rendering is available on this system.
///
/// Returns `true` if a GPU adapter was found and the renderer initialized
/// successfully; `false` otherwise (or if the feature is disabled).
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
#[cfg(feature = "gpu-renderer")]
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

/// Stub when the `gpu-renderer` feature is disabled.
#[tauri::command]
#[cfg(not(feature = "gpu-renderer"))]
pub fn render_terminal_gpu(
    _terminal_id: String,
) -> Result<String, String> {
    Err("GPU renderer not enabled. Build with --features gpu-renderer".to_string())
}
