use crate::GpuError;

/// Shared GPU device and queue for headless (offscreen) rendering.
///
/// The device can be shared across multiple `GpuRenderer` instances
/// if needed, but each renderer typically owns its own `GpuDevice`.
pub struct GpuDevice {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GpuDevice {
    /// Create a new GPU device for headless rendering.
    ///
    /// Uses `pollster::block_on` for synchronous initialization.
    /// Requests a low-power adapter since terminal rendering is background work.
    /// No surface is needed -- all rendering is to offscreen textures.
    pub fn new() -> Result<Self, GpuError> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok_or(GpuError::NoAdapter)?;

        log::info!(
            "GPU adapter: {:?} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("godly-renderer"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .map_err(|e| GpuError::DeviceError(e.to_string()))?;

        Ok(Self { device, queue })
    }
}
