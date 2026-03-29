mod app_state;
mod daemon_bridge;
mod event_bus;
mod mcp_bridge;
mod notification_state;
mod persistence;
mod search;
mod selection;
mod split_renderer;
mod terminal_renderer;
mod ui;

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
    selection: selection::SelectionState,
    scrollback_offset: usize,
    mouse_position: Option<(f64, f64)>,
    quad_pipeline: Option<ui::quad_renderer::QuadPipeline>,
    title_bar: ui::title_bar::TitleBar,
    tab_bar: ui::tab_bar::TabBar,
    status_bar: ui::status_bar::StatusBar,
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
            selection: selection::SelectionState::default(),
            scrollback_offset: 0,
            mouse_position: None,
            quad_pipeline: None,
            title_bar: ui::title_bar::TitleBar::new(),
            tab_bar: ui::tab_bar::TabBar::new(),
            status_bar: ui::status_bar::StatusBar::new(),
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
        self.quad_pipeline = Some(ui::quad_renderer::QuadPipeline::new(&device, format));

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

                // Initial grid fetch — the shell prompt may have been sent before the bridge was ready
                self.fetch_grid();
            }
            Err(e) => {
                log::error!("Failed to connect to daemon: {e}");
            }
        }
    }

    fn terminal_size(&self) -> (u16, u16) {
        if let (Some(gpu), Some(renderer)) = (&self.gpu, &self.renderer) {
            let metrics = renderer.font_metrics().scaled_for_render();
            let vw = gpu.config.width as f32;
            let vh = gpu.config.height as f32;
            let layout = ui::layout::ShellLayout::compute(vw, vh);
            let cols = (layout.terminal.width / metrics.cell_width).floor() as u16;
            let rows = (layout.terminal.height / metrics.cell_height).floor() as u16;
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

    fn pixel_to_grid(&self, x: f64, y: f64) -> godly_terminal_surface::GridPos {
        if let Some(renderer) = &self.renderer {
            let phys = renderer.font_metrics().scaled_for_render();
            let col = (x as f32 / phys.cell_width).floor() as usize;
            let row = (y as f32 / phys.cell_height).floor() as usize;
            godly_terminal_surface::GridPos { row, col }
        } else {
            godly_terminal_surface::GridPos { row: 0, col: 0 }
        }
    }

    fn scroll(&mut self, delta: isize) {
        let new_offset = (self.scrollback_offset as isize + delta).max(0) as usize;
        if new_offset == self.scrollback_offset { return; }
        self.scrollback_offset = new_offset;

        let Some(daemon) = &self.daemon else { return };
        let Some(session_id) = &self.active_session else { return };
        let Some(sender) = &self.sender else { return };

        let daemon = Arc::clone(daemon);
        let session_id = session_id.clone();
        let sender = sender.clone();
        let offset = self.scrollback_offset;

        std::thread::spawn(move || {
            match daemon.send_request(&Request::ScrollAndReadRichGrid {
                session_id: session_id.clone(),
                offset,
            }) {
                Ok(Response::RichGrid { grid }) => {
                    sender.send(AsyncEvent::GridFetched {
                        session_id,
                        grid: Box::new(grid),
                    });
                }
                _ => {}
            }
        });
    }

    fn copy_selection(&mut self) {
        if !self.selection.has_selection() { return; }
        let Some(grid) = &self.current_grid else { return };
        let text = self.selection.selected_text(grid);
        if text.is_empty() { return; }
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(&text);
        }
    }

    fn handle_ui_action(&self, action: ui::widget::UiAction) {
        match action {
            ui::widget::UiAction::CloseWindow => {
                // Handled by the event loop
            }
            ui::widget::UiAction::MinimizeWindow => {
                if let Some(w) = &self.window {
                    w.set_minimized(true);
                }
            }
            ui::widget::UiAction::MaximizeWindow => {
                if let Some(w) = &self.window {
                    w.set_maximized(!w.is_maximized());
                }
            }
            ui::widget::UiAction::DragWindow => {
                if let Some(w) = &self.window {
                    let _ = w.drag_window();
                }
            }
            ui::widget::UiAction::SwitchTab(id) => {
                log::info!("Switch to tab: {id}");
            }
            ui::widget::UiAction::NewTab | ui::widget::UiAction::CloseTab(_) => {
                // TODO: implement tab management
            }
        }
    }

    fn send_key_input(&self, bytes: Vec<u8>) {
        let Some(daemon) = &self.daemon else {
            log::warn!("send_key_input: no daemon");
            return;
        };
        let Some(session_id) = &self.active_session else {
            log::warn!("send_key_input: no active session");
            return;
        };

        log::debug!("Sending {} bytes to PTY: {:?}", bytes.len(), String::from_utf8_lossy(&bytes));
        let daemon = Arc::clone(daemon);
        let session_id = session_id.clone();
        std::thread::spawn(move || {
            match daemon.send_request(&Request::Write {
                session_id,
                data: bytes,
            }) {
                Ok(_) => {}
                Err(e) => log::error!("Write to PTY failed: {e}"),
            }
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

            let vw = gpu.config.width as f32;
            let vh = gpu.config.height as f32;
            let layout = ui::layout::ShellLayout::compute(vw, vh);

            // Build UI chrome quads
            let mut quads = Vec::new();
            quads.extend_from_slice(&self.title_bar.build_quads(layout.title_bar, vw, vh));
            quads.extend_from_slice(&self.tab_bar.build_quads(layout.tab_bar, vw, vh));
            quads.extend_from_slice(&self.status_bar.build_quads(layout.status_bar, vw, vh));

            // Draw chrome
            if let Some(quad_pipe) = &mut self.quad_pipeline {
                quad_pipe.draw(&gpu.device, &gpu.queue, &mut pass, &quads);
            }

            // Render terminal grid in the terminal area
            if let Some(grid) = &self.current_grid {
                if let Some(renderer) = &mut self.renderer {
                    renderer.render(
                        &gpu.device,
                        &gpu.queue,
                        &mut pass,
                        grid,
                        gpu.config.width,
                        gpu.config.height,
                        layout.terminal.x,
                        layout.terminal.y,
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
                // Re-fetch grid periodically to catch output we missed
                if self.active_session.is_some() {
                    self.fetch_grid();
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            AsyncEvent::TerminalOutput { .. } => {
                log::debug!("TerminalOutput event — fetching grid");
                self.fetch_grid();
            }
            AsyncEvent::GridFetched { grid, .. } => {
                // Log first row content for debugging
                if let Some(row) = grid.rows.first() {
                    let text: String = row.cells.iter().map(|c| c.content.as_str()).collect();
                    let trimmed = text.trim_end();
                    if !trimmed.is_empty() {
                        log::info!("GridFetched: {}x{}, first row: {:?}",
                            grid.dimensions.cols, grid.dimensions.rows, &trimmed[..trimmed.len().min(80)]);
                    } else {
                        log::info!("GridFetched: {}x{}, first row empty", grid.dimensions.cols, grid.dimensions.rows);
                    }
                }
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
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = Some((position.x, position.y));
                let (px, py) = (position.x as f32, position.y as f32);
                let gpu = self.gpu.as_ref();
                let (vw, vh) = gpu.map(|g| (g.config.width as f32, g.config.height as f32)).unwrap_or((1200.0, 800.0));
                let layout = ui::layout::ShellLayout::compute(vw, vh);

                // Route mouse to UI chrome
                let me = ui::widget::MouseEvent::Move { x: px, y: py };
                self.title_bar.on_mouse(me, layout.title_bar);
                self.tab_bar.on_mouse(me, layout.tab_bar);

                // Selection drag in terminal area
                if self.selection.active && layout.terminal.contains(px, py) {
                    let pos = self.pixel_to_grid(
                        (px - layout.terminal.x) as f64,
                        (py - layout.terminal.y) as f64,
                    );
                    self.selection.update(pos);
                }
                if let Some(w) = &self.window { w.request_redraw(); }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                use winit::event::MouseButton;
                if button == MouseButton::Left {
                    if let Some((x, y)) = self.mouse_position {
                        let (px, py) = (x as f32, y as f32);
                        let gpu = self.gpu.as_ref();
                        let (vw, vh) = gpu.map(|g| (g.config.width as f32, g.config.height as f32)).unwrap_or((1200.0, 800.0));
                        let layout = ui::layout::ShellLayout::compute(vw, vh);

                        if state == ElementState::Pressed {
                            // Check title bar first
                            let me = ui::widget::MouseEvent::Press { x: px, y: py };
                            if let Some(action) = self.title_bar.on_mouse(me, layout.title_bar) {
                                self.handle_ui_action(action);
                                return;
                            }
                            if let Some(action) = self.tab_bar.on_mouse(me, layout.tab_bar) {
                                self.handle_ui_action(action);
                                return;
                            }
                        }

                        // Terminal area selection
                        if layout.terminal.contains(px, py) {
                            let pos = self.pixel_to_grid(
                                (px - layout.terminal.x) as f64,
                                (py - layout.terminal.y) as f64,
                            );
                            match state {
                                ElementState::Pressed => {
                                    self.selection.start(pos);
                                    if let Some(w) = &self.window { w.request_redraw(); }
                                }
                                ElementState::Released => {
                                    self.selection.finish();
                                    self.copy_selection();
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => -y as isize * 3,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => -(pos.y / 20.0) as isize,
                };
                if lines != 0 {
                    self.scroll(lines);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    let mods = self.modifiers.state();
                    let adapter_mods = convert_modifiers(mods);
                    let adapter_key = convert_key(&event.logical_key);

                    // Check app shortcuts first
                    if let Some(action) = godly_app_adapter::shortcuts::check_app_shortcut(&adapter_key, adapter_mods) {
                        use godly_app_adapter::shortcuts::AppAction;
                        match action {
                            AppAction::NewTab => {
                                let (rows, cols) = self.terminal_size();
                                if let Some(daemon) = &self.daemon {
                                    let daemon = Arc::clone(daemon);
                                    let sender = self.sender.clone();
                                    let session_id = uuid::Uuid::new_v4().to_string();
                                    let id = session_id.clone();
                                    std::thread::spawn(move || {
                                        let _ = daemon.send_request(&Request::CreateSession {
                                            id,
                                            shell_type: ShellType::Windows,
                                            cwd: None,
                                            rows,
                                            cols,
                                            env: None,
                                        });
                                    });
                                    self.active_session = Some(session_id);
                                }
                            }
                            AppAction::CloseTab => {
                                if let Some(ref session_id) = self.active_session {
                                    let daemon = self.daemon.as_ref().map(Arc::clone);
                                    let id = session_id.clone();
                                    if let Some(daemon) = daemon {
                                        std::thread::spawn(move || {
                                            let _ = daemon.send_request(&Request::CloseSession { session_id: id });
                                        });
                                    }
                                }
                            }
                            AppAction::Copy => {
                                self.copy_selection();
                            }
                            AppAction::Paste => {
                                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                    if let Ok(text) = clipboard.get_text() {
                                        self.send_key_input(text.into_bytes());
                                    }
                                }
                            }
                            AppAction::ZoomIn | AppAction::ZoomOut | AppAction::ZoomReset => {
                                // TODO: zoom
                            }
                            AppAction::ScrollPageUp => {
                                self.scroll(24);
                            }
                            AppAction::ScrollPageDown => {
                                self.scroll(-24);
                            }
                            AppAction::ScrollToTop => {
                                self.scroll(10000);
                            }
                            AppAction::ScrollToBottom => {
                                self.scrollback_offset = 0;
                                self.fetch_grid();
                            }
                            AppAction::Find => {
                                // Toggle search - will be connected to search UI
                                log::info!("Find toggled");
                            }
                            _ => {}
                        }
                        return;
                    }

                    // Forward to PTY
                    if let Some(bytes) =
                        godly_app_adapter::keys::key_to_pty_bytes(&adapter_key, adapter_mods)
                    {
                        // Reset scroll to live when typing
                        if self.scrollback_offset > 0 {
                            self.scrollback_offset = 0;
                        }
                        self.send_key_input(bytes);
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window { w.request_redraw(); }
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
