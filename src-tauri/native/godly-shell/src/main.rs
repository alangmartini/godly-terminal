mod daemon_bridge;
mod event_bus;
mod terminal_renderer;

use std::sync::Arc;

use daemon_bridge::ShellEventSink;
use event_bus::{AsyncEvent, EventSender};
use terminal_renderer::TerminalRenderer;

use godly_app_adapter::daemon_client::NativeDaemonClient;
use godly_protocol::messages::{Request, Response};
use godly_protocol::types::{RichGridData, ShellType};
use godly_terminal_surface::font_metrics::FontMetrics;

use wgpu::SurfaceTargetUnsafe;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::{Key, NamedKey},
    window::{Window, WindowAttributes, WindowId},
};

const DEFAULT_FONT_FAMILY: &str = "Cascadia Mono";

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    format: wgpu::TextureFormat,
}

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    proxy: EventLoopProxy<AsyncEvent>,
    heartbeat_started: bool,
    renderer: Option<TerminalRenderer>,
    daemon: Option<Arc<NativeDaemonClient>>,
    active_session: Option<String>,
    current_grid: Option<RichGridData>,
    modifiers: winit::event::Modifiers,
    sender: Option<EventSender>,
}

impl App {
    fn new(proxy: EventLoopProxy<AsyncEvent>) -> Self {
        Self {
            window: None,
            gpu: None,
            proxy,
            heartbeat_started: false,
            renderer: None,
            daemon: None,
            active_session: None,
            current_grid: None,
            modifiers: winit::event::Modifiers::default(),
            sender: None,
        }
    }

    fn init_gpu(&mut self, window: Arc<Window>) {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12 | wgpu::Backends::VULKAN,
            ..Default::default()
        });

        let surface = unsafe {
            instance
                .create_surface_unsafe(SurfaceTargetUnsafe::from_window(&*window).unwrap())
                .unwrap()
        };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("No suitable GPU adapter found");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("godly-shell"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::default(),
            },
        ))
        .expect("Failed to create device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        // Initialize terminal renderer
        let font_metrics = FontMetrics::from_font_size(14.0);
        let rasterizer = create_rasterizer();
        let renderer = TerminalRenderer::new(&device, &queue, format, font_metrics, rasterizer);
        self.renderer = Some(renderer);

        self.gpu = Some(GpuState {
            surface,
            device,
            queue,
            config,
            format,
        });
        self.window = Some(window);
    }

    fn connect_daemon(&mut self) {
        let sender = self.sender.as_ref().expect("sender must be set before connecting daemon");

        match NativeDaemonClient::connect_or_launch() {
            Ok(client) => {
                let client = Arc::new(client);
                let sink = Arc::new(ShellEventSink::new(sender.clone()));
                if let Err(e) = client.setup_bridge(sink) {
                    log::error!("Failed to setup daemon bridge: {e}");
                    return;
                }

                // Create a session
                let session_id = uuid::Uuid::new_v4().to_string();
                let (rows, cols) = self.terminal_size();
                match client.send_request(&Request::CreateSession {
                    id: session_id.clone(),
                    shell_type: ShellType::Windows,
                    cwd: None,
                    rows,
                    cols,
                    env: None,
                }) {
                    Ok(Response::SessionCreated { session }) => {
                        log::info!("Session created: {}", session.id);
                        self.active_session = Some(session.id);
                    }
                    Ok(other) => log::error!("Unexpected response: {other:?}"),
                    Err(e) => log::error!("Failed to create session: {e}"),
                }

                self.daemon = Some(client);
            }
            Err(e) => {
                log::error!("Failed to connect to daemon: {e}");
            }
        }
    }

    fn terminal_size(&self) -> (u16, u16) {
        if let (Some(gpu), Some(renderer)) = (&self.gpu, &self.renderer) {
            let metrics = renderer.font_metrics().scaled_for_render();
            let cols = (gpu.config.width as f32 / metrics.cell_width).floor() as u16;
            let rows = (gpu.config.height as f32 / metrics.cell_height).floor() as u16;
            (rows.max(1), cols.max(1))
        } else {
            (24, 80)
        }
    }

    fn fetch_grid(&self) {
        let Some(daemon) = &self.daemon else { return };
        let Some(session_id) = &self.active_session else { return };
        let Some(sender) = &self.sender else { return };

        let daemon = Arc::clone(daemon);
        let session_id = session_id.clone();
        let sender = sender.clone();

        std::thread::spawn(move || {
            match daemon.send_request(&Request::ReadRichGrid {
                session_id: session_id.clone(),
            }) {
                Ok(Response::RichGrid { grid }) => {
                    sender.send(AsyncEvent::GridFetched {
                        session_id,
                        grid: Box::new(grid),
                    });
                }
                Ok(_) => {}
                Err(e) => {
                    log::error!("Failed to read grid: {e}");
                }
            }
        });
    }

    fn send_key_input(&self, bytes: Vec<u8>) {
        let Some(daemon) = &self.daemon else { return };
        let Some(session_id) = &self.active_session else { return };

        let daemon = Arc::clone(daemon);
        let session_id = session_id.clone();
        std::thread::spawn(move || {
            let _ = daemon.send_request(&Request::Write {
                session_id,
                data: bytes,
            });
        });
    }

    fn render(&mut self) {
        let Some(gpu) = &self.gpu else { return };
        let frame = match gpu.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                gpu.surface.configure(&gpu.device, &gpu.config);
                return;
            }
            Err(e) => {
                log::error!("Surface error: {e:?}");
                return;
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            gpu.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render"),
                });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.071,
                            g: 0.071,
                            b: 0.082,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Render terminal grid if available
            if let Some(grid) = &self.current_grid {
                if let Some(renderer) = &mut self.renderer {
                    renderer.render(
                        &gpu.device,
                        &gpu.queue,
                        &mut pass,
                        grid,
                        gpu.config.width,
                        gpu.config.height,
                    );
                }
            }
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}

impl ApplicationHandler<AsyncEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = WindowAttributes::default()
            .with_title("Godly Terminal")
            .with_inner_size(winit::dpi::LogicalSize::new(1200.0, 800.0))
            .with_min_inner_size(winit::dpi::LogicalSize::new(400.0, 300.0))
            .with_decorations(false);

        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        self.init_gpu(window);

        // Set up event sender
        let sender = EventSender::new(self.proxy.clone());
        self.sender = Some(sender.clone());

        // Spawn heartbeat thread
        if !self.heartbeat_started {
            self.heartbeat_started = true;
            let hb_sender = sender.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                hb_sender.send(AsyncEvent::Heartbeat);
            });
        }

        // Connect to daemon
        self.connect_daemon();
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AsyncEvent) {
        match event {
            AsyncEvent::Heartbeat => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            AsyncEvent::TerminalOutput { .. } => {
                self.fetch_grid();
            }
            AsyncEvent::GridFetched { grid, .. } => {
                self.current_grid = Some(*grid);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            AsyncEvent::SessionClosed {
                session_id,
                exit_code,
            } => {
                log::info!("Session {session_id} closed (exit: {exit_code:?})");
            }
            _ => {
                log::debug!("Async event: {event:?}");
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width = size.width.max(1);
                    gpu.config.height = size.height.max(1);
                    gpu.surface.configure(&gpu.device, &gpu.config);
                }
                // Resize the terminal session
                if self.daemon.is_some() && self.active_session.is_some() {
                    let (rows, cols) = self.terminal_size();
                    let daemon = Arc::clone(self.daemon.as_ref().unwrap());
                    let session_id = self.active_session.clone().unwrap();
                    std::thread::spawn(move || {
                        let _ = daemon.send_request(&Request::Resize {
                            session_id,
                            rows,
                            cols,
                        });
                    });
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    let mods = self.modifiers.state();
                    let adapter_mods = convert_modifiers(mods);
                    let adapter_key = convert_key(&event.logical_key);

                    if let Some(bytes) =
                        godly_app_adapter::keys::key_to_pty_bytes(&adapter_key, adapter_mods)
                    {
                        self.send_key_input(bytes);
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Only redraw on demand (event-driven), not continuously
    }
}

fn convert_modifiers(state: winit::keyboard::ModifiersState) -> godly_app_adapter::keyboard::Modifiers {
    let mut m = godly_app_adapter::keyboard::Modifiers::empty();
    if state.shift_key() {
        m = m | godly_app_adapter::keyboard::Modifiers::SHIFT;
    }
    if state.control_key() {
        m = m | godly_app_adapter::keyboard::Modifiers::CTRL;
    }
    if state.alt_key() {
        m = m | godly_app_adapter::keyboard::Modifiers::ALT;
    }
    if state.super_key() {
        m = m | godly_app_adapter::keyboard::Modifiers::LOGO;
    }
    m
}

fn convert_key(key: &Key) -> godly_app_adapter::keyboard::Key {
    use godly_app_adapter::keyboard;
    match key {
        Key::Character(ch) => keyboard::Key::Character(ch.to_string().into()),
        Key::Named(named) => match named {
            NamedKey::Enter => keyboard::Key::Named(keyboard::Named::Enter),
            NamedKey::Backspace => keyboard::Key::Named(keyboard::Named::Backspace),
            NamedKey::Tab => keyboard::Key::Named(keyboard::Named::Tab),
            NamedKey::Escape => keyboard::Key::Named(keyboard::Named::Escape),
            NamedKey::Space => keyboard::Key::Named(keyboard::Named::Space),
            NamedKey::Delete => keyboard::Key::Named(keyboard::Named::Delete),
            NamedKey::Insert => keyboard::Key::Named(keyboard::Named::Insert),
            NamedKey::Home => keyboard::Key::Named(keyboard::Named::Home),
            NamedKey::End => keyboard::Key::Named(keyboard::Named::End),
            NamedKey::PageUp => keyboard::Key::Named(keyboard::Named::PageUp),
            NamedKey::PageDown => keyboard::Key::Named(keyboard::Named::PageDown),
            NamedKey::ArrowUp => keyboard::Key::Named(keyboard::Named::ArrowUp),
            NamedKey::ArrowDown => keyboard::Key::Named(keyboard::Named::ArrowDown),
            NamedKey::ArrowLeft => keyboard::Key::Named(keyboard::Named::ArrowLeft),
            NamedKey::ArrowRight => keyboard::Key::Named(keyboard::Named::ArrowRight),
            NamedKey::F1 => keyboard::Key::Named(keyboard::Named::F1),
            NamedKey::F2 => keyboard::Key::Named(keyboard::Named::F2),
            NamedKey::F3 => keyboard::Key::Named(keyboard::Named::F3),
            NamedKey::F4 => keyboard::Key::Named(keyboard::Named::F4),
            NamedKey::F5 => keyboard::Key::Named(keyboard::Named::F5),
            NamedKey::F6 => keyboard::Key::Named(keyboard::Named::F6),
            NamedKey::F7 => keyboard::Key::Named(keyboard::Named::F7),
            NamedKey::F8 => keyboard::Key::Named(keyboard::Named::F8),
            NamedKey::F9 => keyboard::Key::Named(keyboard::Named::F9),
            NamedKey::F10 => keyboard::Key::Named(keyboard::Named::F10),
            NamedKey::F11 => keyboard::Key::Named(keyboard::Named::F11),
            NamedKey::F12 => keyboard::Key::Named(keyboard::Named::F12),
            _ => keyboard::Key::Unidentified,
        },
        _ => godly_app_adapter::keyboard::Key::Unidentified,
    }
}

#[cfg(windows)]
fn create_rasterizer() -> Box<dyn godly_terminal_surface::glyph_rasterizer::GlyphRasterizer> {
    use godly_terminal_surface::directwrite_rasterizer::DirectWriteRasterizer;
    use godly_terminal_surface::glyph_rasterizer::GlyphRasterizer;
    use godly_terminal_surface::swash_rasterizer::SwashRasterizer;

    match DirectWriteRasterizer::new() {
        Ok(mut dw) => {
            if dw.load_system_font(DEFAULT_FONT_FAMILY).is_ok() {
                log::info!("[FONT] Using DirectWrite ClearType rasterizer with {DEFAULT_FONT_FAMILY}");
                Box::new(dw)
            } else {
                log::warn!("[FONT] DirectWrite: {DEFAULT_FONT_FAMILY} not found, falling back to swash");
                let mut r = SwashRasterizer::new();
                r.load_font(include_bytes!("../../iced-shell/fonts/GeistMono-Regular.ttf"), 0);
                Box::new(r)
            }
        }
        Err(e) => {
            log::warn!("[FONT] DirectWrite init failed ({e:?}), using swash rasterizer");
            let mut r = SwashRasterizer::new();
            r.load_font(include_bytes!("../../iced-shell/fonts/GeistMono-Regular.ttf"), 0);
            Box::new(r)
        }
    }
}

#[cfg(not(windows))]
fn create_rasterizer() -> Box<dyn godly_terminal_surface::glyph_rasterizer::GlyphRasterizer> {
    use godly_terminal_surface::glyph_rasterizer::GlyphRasterizer;
    let mut r = godly_terminal_surface::swash_rasterizer::SwashRasterizer::new();
    r.load_font(include_bytes!("../../iced-shell/fonts/GeistMono-Regular.ttf"), 0);
    Box::new(r)
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let event_loop = EventLoop::<AsyncEvent>::with_user_event().build().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let mut app = App::new(proxy);
    event_loop.run_app(&mut app).unwrap();
}
