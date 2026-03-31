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
use std::time::Instant;

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
    tab_bar: ui::tab_bar::TabBar,
    sidebar: ui::sidebar::Sidebar,
    status_bar: ui::status_bar::StatusBar,
    scale_factor: f32,
    window_focused: bool,
    scrollbar_hover_anim: ui::anim::Anim,
    focus_dim_anim: ui::anim::Anim,
    cursor_blink_anim: ui::anim::Anim,
    cursor_blink_phase: bool,
    cursor_blink_timer: f32,
    last_frame_time: Instant,
    is_maximized: bool,
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
            tab_bar: {
                let mut tb = ui::tab_bar::TabBar::new();
                // Pre-populate demo tabs to match the reference UI
                tb.tabs = vec![
                    ui::tab_bar::TabInfo { id: "demo-1".into(), title: "plane".into(), active: true, unread_count: 0 },
                    ui::tab_bar::TabInfo { id: "demo-2".into(), title: "opensessions".into(), active: false, unread_count: 3 },
                    ui::tab_bar::TabInfo { id: "demo-3".into(), title: "quiver".into(), active: false, unread_count: 0 },
                    ui::tab_bar::TabInfo { id: "demo-4".into(), title: "godly-terminal".into(), active: false, unread_count: 12 },
                    ui::tab_bar::TabInfo { id: "demo-5".into(), title: "notes".into(), active: false, unread_count: 0 },
                ];
                tb
            },
            scale_factor: 1.0,
            window_focused: true,
            scrollbar_hover_anim: ui::anim::Anim::default(),
            focus_dim_anim: ui::anim::Anim::default(),
            cursor_blink_anim: {
                let mut a = ui::anim::Anim::default();
                a.snap(1.0);
                a
            },
            cursor_blink_phase: true,
            cursor_blink_timer: 0.0,
            last_frame_time: Instant::now(),
            is_maximized: false,
            sidebar: ui::sidebar::Sidebar::new(),
            status_bar: {
                let mut sb = ui::status_bar::StatusBar::new();
                sb.cwd = std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                sb.process_name = "pwsh".into();
                // Detect git branch
                if let Ok(output) = std::process::Command::new("git")
                    .args(["rev-parse", "--abbrev-ref", "HEAD"])
                    .output()
                {
                    if output.status.success() {
                        sb.git_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    }
                }
                // Detect git diff summary
                if let Ok(output) = std::process::Command::new("git")
                    .args(["diff", "--shortstat"])
                    .output()
                {
                    if output.status.success() {
                        let stat = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if !stat.is_empty() {
                            sb.git_diff_summary = stat;
                        }
                    }
                }
                sb
            },
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

        // Initialize terminal renderer with DPI-aware font metrics
        let scale_factor = window.scale_factor() as f32;
        self.scale_factor = scale_factor;
        log::info!("DPI scale factor: {scale_factor}");
        let font_data: &[u8] = include_bytes!("../../iced-shell/fonts/GeistMono-Regular.ttf");
        let rasterizer = create_rasterizer();
        let font_size = 14.0_f32;
        let font_metrics = FontMetrics::from_font_bytes(font_size, font_data)
            .with_scale_factor(scale_factor);
        log::info!("Font metrics: cell={}x{}, font_size={}, baseline={}, scale={}",
            font_metrics.cell_width, font_metrics.cell_height,
            font_metrics.font_size, font_metrics.baseline_offset, font_metrics.scale_factor);
        let mut renderer = TerminalRenderer::new(&device, &queue, format, font_metrics, rasterizer);

        // Load proportional sans-serif font for UI chrome labels
        if let Some(ui_rast) = create_ui_rasterizer() {
            renderer.set_ui_rasterizer(ui_rast);
            log::info!("[FONT] UI font loaded, avg advance = {:.1}px", renderer.ui_avg_advance());
        }

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
                        // Update first demo tab to point to real session
                        if let Some(first) = self.tab_bar.tabs.first_mut() {
                            first.id = session.id.clone();
                        }
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
            let layout = ui::layout::ShellLayout::compute(vw, vh, true, self.scale_factor);
            let cols = (layout.terminal_content.width / metrics.cell_width).floor() as u16;
            let rows = (layout.terminal_content.height / metrics.cell_height).floor() as u16;
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

    fn send_key_input(&mut self, bytes: Vec<u8>) {
        // Reset cursor blink on keystroke — cursor should be visible right after typing
        self.cursor_blink_anim.snap(1.0);
        self.cursor_blink_phase = true;
        self.cursor_blink_timer = 0.0;

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

    /// The accent color of the currently active tab (cycles through palette).
    fn active_accent(&self) -> [f32; 4] {
        const ACCENTS: &[[f32; 4]] = &[
            ui::builder::colors::ACCENT_BLUE,
            ui::builder::colors::ACCENT_GREEN,
            ui::builder::colors::ACCENT_PEACH,
            ui::builder::colors::ACCENT_MAUVE,
            ui::builder::colors::ACCENT_RED,
        ];
        self.tab_bar.tabs.iter().enumerate()
            .find(|(_, t)| t.active)
            .map(|(i, _)| ACCENTS[i % ACCENTS.len()])
            .unwrap_or(ui::builder::colors::ACCENT_BLUE)
    }

    fn render(&mut self) {
        // Frame-rate independent delta time (clamped to avoid spiral-of-death)
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32().min(0.1);
        self.last_frame_time = now;

        // Tick all hover animations and request another frame if any are active
        let mut animating = false;
        animating |= self.tab_bar.tick_animations(dt);
        animating |= self.sidebar.tick_animations(dt);
        animating |= self.status_bar.tick_animations(dt);
        animating |= self.focus_dim_anim.tick(ui::anim::timing::SLOW, dt);

        // Cursor blink: smooth fade between visible/invisible every ~500ms.
        // Only blinks for Blink* cursor styles; Steady* styles stay fully visible.
        {
            let is_blink_style = self.current_grid.as_ref().map_or(false, |g| {
                use godly_protocol::types::CursorShape;
                matches!(g.cursor.cursor_style,
                    CursorShape::BlinkBlock | CursorShape::BlinkUnderline | CursorShape::BlinkBar)
            });
            if is_blink_style && self.window_focused {
                self.cursor_blink_timer += dt;
                if self.cursor_blink_timer >= 0.5 {
                    self.cursor_blink_timer = 0.0;
                    self.cursor_blink_phase = !self.cursor_blink_phase;
                    self.cursor_blink_anim.set(if self.cursor_blink_phase { 1.0 } else { 0.0 });
                }
                animating |= self.cursor_blink_anim.tick(ui::anim::timing::BLINK, dt);
            } else {
                // Steady cursor or window unfocused: always visible
                self.cursor_blink_anim.snap(1.0);
                self.cursor_blink_phase = true;
                self.cursor_blink_timer = 0.0;
            }
        }

        if animating {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }

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

        // Prepare terminal data BEFORE starting the render pass
        let vw = gpu.config.width as f32;
        let vh = gpu.config.height as f32;
        let layout = ui::layout::ShellLayout::compute(vw, vh, true, self.scale_factor);

        // Update status bar with current terminal dimensions
        self.status_bar.terminal_size = self.terminal_size();

        // Build UI chrome (quads + text commands)
        let phys_metrics = self.renderer.as_ref().map(|r| r.font_metrics().scaled_for_render());
        let ui_avg_advance = self.renderer.as_ref().map_or(0.0, |r| r.ui_avg_advance());
        let ui_text_handle = if let Some(m) = phys_metrics {
            let mut tr = ui::builder::UiTextRenderer::new(m.cell_width, m.cell_height, self.scale_factor);
            tr.ui_avg_advance = ui_avg_advance;
            tr
        } else {
            ui::builder::UiTextRenderer::new(8.0, 16.0, self.scale_factor)
        };
        let mut ui_builder = ui::builder::UiBuilder::new(vw, vh);

        // Terminal area background (BG_BASE) — must come before chrome overlays
        ui_builder.fill(layout.terminal, ui::builder::colors::BG_BASE);

        // Directional panel cast shadows — each chrome panel casts a shadow
        // onto the terminal content area proportional to its visual weight.
        // Top-left lighting model: tab bar (top) casts the strongest shadow,
        // sidebar (left) casts a medium shadow, status bar (bottom) and right
        // edge cast lighter shadows.  This replaces a single omnidirectional
        // inner shadow with more realistic directional depth.
        {
            // Base inner shadow (reduced — directional shadows add the rest)
            ui_builder.fill_inner_shadow(
                layout.terminal,
                [0.0, 0.0, 0.0, 0.08],
                0.0,
                ui_text_handle.s(4.0),
            );

            // Tab bar cast shadow (strongest — topmost elevated panel)
            {
                let shadow_h = ui_text_handle.s(12.0);
                ui_builder.fill_gradient(
                    ui::widget::Rect {
                        x: layout.terminal.x,
                        y: layout.terminal.y,
                        width: layout.terminal.width,
                        height: shadow_h,
                    },
                    [0.0, 0.0, 0.0, 0.10],
                    [0.0, 0.0, 0.0, 0.0],
                );
                // Subtle accent-tinted glow spill from the active tab into the
                // content area.  This creates a warm colored light that connects
                // the tab bar accent to the terminal, enhancing visual continuity.
                let accent = self.active_accent();
                let breath = 0.85 + 0.15 * self.tab_bar.glow_phase().sin();
                let glow_h = ui_text_handle.s(18.0);
                let glow_color = [accent[0], accent[1], accent[2], 0.03 * breath];
                let glow_zero = [accent[0], accent[1], accent[2], 0.0];
                ui_builder.fill_gradient(
                    ui::widget::Rect {
                        x: layout.terminal.x,
                        y: layout.terminal.y,
                        width: layout.terminal.width,
                        height: glow_h,
                    },
                    glow_color,
                    glow_zero,
                );
            }

            // Sidebar cast shadow (medium — side panel casting rightward)
            if layout.sidebar.width > 0.0 {
                let shadow_w = ui_text_handle.s(10.0);
                ui_builder.fill_gradient_h(
                    ui::widget::Rect {
                        x: layout.terminal.x,
                        y: layout.terminal.y,
                        width: shadow_w,
                        height: layout.terminal.height,
                    },
                    [0.0, 0.0, 0.0, 0.07],
                    [0.0, 0.0, 0.0, 0.0],
                );
            }

            // Status bar cast shadow (subtle — bottom panel casting upward)
            {
                let shadow_h = ui_text_handle.s(8.0);
                ui_builder.fill_gradient(
                    ui::widget::Rect {
                        x: layout.terminal.x,
                        y: layout.terminal.y + layout.terminal.height - shadow_h,
                        width: layout.terminal.width,
                        height: shadow_h,
                    },
                    [0.0, 0.0, 0.0, 0.0],
                    [0.0, 0.0, 0.0, 0.05],
                );
            }

            // Right edge shadow (lightest — no heavy panel, just window edge)
            {
                let shadow_w = ui_text_handle.s(5.0);
                ui_builder.fill_gradient_h(
                    ui::widget::Rect {
                        x: layout.terminal.x + layout.terminal.width - shadow_w,
                        y: layout.terminal.y,
                        width: shadow_w,
                        height: layout.terminal.height,
                    },
                    [0.0, 0.0, 0.0, 0.0],
                    [0.0, 0.0, 0.0, 0.03],
                );
            }

            // Concave corner fills soften 90° corners where chrome panels meet the
            // terminal content area, creating a smooth inset feel.
            {
                let corner_r = ui_text_handle.s(6.0);

                // Left-side corners (sidebar-terminal junction)
                if layout.sidebar.width > 0.0 {
                    // Top-left corner
                    let corner_rect = ui::widget::Rect {
                        x: layout.terminal.x,
                        y: layout.terminal.y,
                        width: corner_r,
                        height: corner_r,
                    };
                    ui_builder.fill_rounded_custom(corner_rect, ui::builder::colors::BG_DARK, [corner_r, 0.0, 0.0, 0.0]);

                    // Bottom-left corner — uses sidebar's gradient bottom color
                    // (BG_DARK * 0.9) instead of flat BG_DARK, so the corner blends
                    // seamlessly with the sidebar's vertical gradient at this y.
                    let sidebar_bottom_color = [
                        ui::builder::colors::BG_DARK[0] * 0.9,
                        ui::builder::colors::BG_DARK[1] * 0.9,
                        ui::builder::colors::BG_DARK[2] * 0.9,
                        1.0,
                    ];
                    let bottom_corner = ui::widget::Rect {
                        x: layout.terminal.x,
                        y: layout.terminal.y + layout.terminal.height - corner_r,
                        width: corner_r,
                        height: corner_r,
                    };
                    ui_builder.fill_rounded_custom(bottom_corner, sidebar_bottom_color, [0.0, 0.0, 0.0, corner_r]);
                }

                // Right-side corners (tab-bar/status-bar to window edge)
                let right_corner_r = ui_text_handle.s(4.0);
                // Top-right corner — uses tab bar's gradient bottom color
                // (BG_DARK * 0.92) for seamless junction at the tab bar edge.
                let tab_bar_bottom_color = [
                    ui::builder::colors::BG_DARK[0] * 0.92,
                    ui::builder::colors::BG_DARK[1] * 0.92,
                    ui::builder::colors::BG_DARK[2] * 0.92,
                    1.0,
                ];
                let tr_corner = ui::widget::Rect {
                    x: layout.terminal.x + layout.terminal.width - right_corner_r,
                    y: layout.terminal.y,
                    width: right_corner_r,
                    height: right_corner_r,
                };
                ui_builder.fill_rounded_custom(tr_corner, tab_bar_bottom_color, [0.0, right_corner_r, 0.0, 0.0]);

                // Bottom-right corner — uses status bar's top gradient color
                // (BG_SURFACE) so the corner blends with the status bar at the
                // terminal-to-status-bar junction instead of mismatching.
                let br_corner = ui::widget::Rect {
                    x: layout.terminal.x + layout.terminal.width - right_corner_r,
                    y: layout.terminal.y + layout.terminal.height - right_corner_r,
                    width: right_corner_r,
                    height: right_corner_r,
                };
                ui_builder.fill_rounded_custom(br_corner, ui::builder::colors::BG_SURFACE, [0.0, 0.0, right_corner_r, 0.0]);
            }

            // Corner vignette: radial darkening at corners with intensity
            // weighted by panel junction importance.  Top-left is strongest
            // (tab bar + sidebar converge), other corners are lighter.
            {
                let vig_r = ui_text_handle.s(24.0);
                let vig_blur = ui_text_handle.s(16.0);
                // Top-left: strongest (two heavy panels converge)
                let tl_alpha = if layout.sidebar.width > 0.0 { 0.10 } else { 0.06 };
                ui_builder.fill_shadow(
                    ui::widget::Rect { x: layout.terminal.x, y: layout.terminal.y, width: vig_r, height: vig_r },
                    [0.0, 0.0, 0.0, tl_alpha], vig_r * 0.3, vig_blur,
                );
                // Top-right: medium (tab bar edge)
                ui_builder.fill_shadow(
                    ui::widget::Rect { x: layout.terminal.x + layout.terminal.width - vig_r, y: layout.terminal.y, width: vig_r, height: vig_r },
                    [0.0, 0.0, 0.0, 0.06], vig_r * 0.3, vig_blur,
                );
                // Bottom-left: medium (sidebar + status bar converge)
                let bl_alpha = if layout.sidebar.width > 0.0 { 0.08 } else { 0.04 };
                ui_builder.fill_shadow(
                    ui::widget::Rect { x: layout.terminal.x, y: layout.terminal.y + layout.terminal.height - vig_r, width: vig_r, height: vig_r },
                    [0.0, 0.0, 0.0, bl_alpha], vig_r * 0.3, vig_blur,
                );
                // Bottom-right: lightest
                ui_builder.fill_shadow(
                    ui::widget::Rect { x: layout.terminal.x + layout.terminal.width - vig_r, y: layout.terminal.y + layout.terminal.height - vig_r, width: vig_r, height: vig_r },
                    [0.0, 0.0, 0.0, 0.04], vig_r * 0.3, vig_blur,
                );
            }

            // Edge vignettes — soft gradient darkening along content edges.
            // Creates cinematic framing that draws attention to the center.
            // Top edge (below tab bar): subtle drop shadow
            {
                let edge_h = ui_text_handle.s(12.0);
                let tc = &layout.terminal;
                // Top edge shadow (cast by tab bar)
                ui_builder.fill_gradient(
                    ui::widget::Rect { x: tc.x, y: tc.y, width: tc.width, height: edge_h },
                    [0.0, 0.0, 0.0, 0.06],
                    [0.0, 0.0, 0.0, 0.0],
                );
                // Left edge shadow (cast by sidebar)
                if layout.sidebar.width > 0.0 {
                    let side_w = ui_text_handle.s(8.0);
                    ui_builder.fill_gradient_h(
                        ui::widget::Rect { x: tc.x, y: tc.y, width: side_w, height: tc.height },
                        [0.0, 0.0, 0.0, 0.05],
                        [0.0, 0.0, 0.0, 0.0],
                    );
                }
                // Bottom edge (above status bar): very subtle upward shadow
                let bot_h = ui_text_handle.s(6.0);
                ui_builder.fill_gradient(
                    ui::widget::Rect { x: tc.x, y: tc.y + tc.height - bot_h, width: tc.width, height: bot_h },
                    [0.0, 0.0, 0.0, 0.0],
                    [0.0, 0.0, 0.0, 0.04],
                );
            }
        }

        // Empty terminal welcome state — styled welcome screen with branded
        // header, status indicator, and keyboard shortcut cards.
        if self.current_grid.is_none() {
            let s = |v: f32| ui_text_handle.s(v);
            let _cw = ui_text_handle.cell_width;
            let ch = ui_text_handle.cell_height;
            let tc = &layout.terminal_content;
            let bg = ui::builder::colors::BG_BASE;
            let active_accent = self.active_accent();

            let status = if self.daemon.is_none() {
                "Connecting to daemon..."
            } else if self.active_session.is_none() {
                "Starting session..."
            } else {
                "Waiting for output..."
            };

            // Center block vertically at ~33% from top (visual balance)
            let center_x = tc.x + tc.width / 2.0;
            let block_y = tc.y + tc.height * 0.33;

            // --- Radial spotlight behind welcome content ---
            // Creates a soft, centered glow that fills the dark content area
            // and draws the eye toward the welcome screen elements.
            {
                let spot_w = tc.width * 0.65;
                let spot_h = tc.height * 0.55;
                let spot_rect = ui::widget::Rect {
                    x: center_x - spot_w / 2.0,
                    y: block_y - ch * 6.0,
                    width: spot_w,
                    height: spot_h,
                };
                let breath = 0.85 + 0.15 * self.tab_bar.glow_phase().sin();
                ui_builder.fill_shadow(spot_rect,
                    [active_accent[0], active_accent[1], active_accent[2], 0.018 * breath],
                    spot_w * 0.3, spot_w * 0.4);
            }

            // --- Hero terminal icon ---
            // Large branding icon above the title — a stylized monitor with
            // prompt caret, rendered at ~4× the tab-bar icon size for hero
            // presence.  Accent-tinted with a soft halo glow behind it.
            let hero_icon_size = ch * 4.0;
            let hero_x = center_x - hero_icon_size / 2.0;
            let hero_y = block_y - hero_icon_size - s(12.0);
            let hero_rect = ui::widget::Rect {
                x: hero_x, y: hero_y,
                width: hero_icon_size, height: hero_icon_size,
            };
            // Halo glow behind icon (breathing, accent-tinted)
            let breath = 0.85 + 0.15 * self.tab_bar.glow_phase().sin();
            let halo_expand = s(10.0);
            let halo_rect = ui::widget::Rect {
                x: hero_x - halo_expand, y: hero_y - halo_expand,
                width: hero_icon_size + halo_expand * 2.0,
                height: hero_icon_size + halo_expand * 2.0,
            };
            ui_builder.fill_shadow(halo_rect,
                [active_accent[0], active_accent[1], active_accent[2], 0.06 * breath],
                hero_icon_size * 0.3, s(18.0));
            // Icon stroke with accent tint (brighter than tab-bar version)
            let hero_icon_fg = [
                ui::builder::colors::FG_MUTED[0] * 0.55 + active_accent[0] * 0.45,
                ui::builder::colors::FG_MUTED[1] * 0.55 + active_accent[1] * 0.45,
                ui::builder::colors::FG_MUTED[2] * 0.55 + active_accent[2] * 0.45,
                0.55,
            ];
            let hero_t = (1.8 * ui_text_handle.scale).max(1.0);
            ui_builder.icon_terminal(hero_rect, hero_t, hero_icon_fg);

            // --- Branded header ---
            let title = "Godly Terminal";
            let title_w = ui_text_handle.text_width_ui(title);
            let title_x = center_x - title_w / 2.0;
            // Title text with subtle accent tint
            let title_fg = [
                ui::builder::colors::FG_SECONDARY[0] * 0.80 + active_accent[0] * 0.20,
                ui::builder::colors::FG_SECONDARY[1] * 0.80 + active_accent[1] * 0.20,
                ui::builder::colors::FG_SECONDARY[2] * 0.80 + active_accent[2] * 0.20,
                0.7,
            ];
            ui_builder.text_ui_bold(&ui_text_handle, title, title_x, block_y, title_fg, bg);

            // Subtitle line — "GPU-accelerated terminal" in very muted text
            let subtitle = "GPU-accelerated terminal";
            let subtitle_w = ui_text_handle.text_width_ui(subtitle);
            let subtitle_y = block_y + ch + s(2.0);
            let subtitle_fg = [
                ui::builder::colors::FG_MUTED[0],
                ui::builder::colors::FG_MUTED[1],
                ui::builder::colors::FG_MUTED[2],
                0.45,
            ];
            ui_builder.text_ui(&ui_text_handle, subtitle,
                center_x - subtitle_w / 2.0, subtitle_y,
                subtitle_fg, bg);

            // Accent underline below subtitle (breathing, matches active tab)
            let breath = 0.85 + 0.15 * self.tab_bar.glow_phase().sin();
            let underline_w = title_w * 0.6;
            let underline_y = subtitle_y + ch + s(4.0);
            let underline_h = s(1.5);
            let underline_color = [active_accent[0], active_accent[1], active_accent[2], 0.25 * breath];
            let underline_zero = [active_accent[0], active_accent[1], active_accent[2], 0.0];
            ui_builder.fill_gradient_h(
                ui::widget::Rect {
                    x: center_x - underline_w / 2.0,
                    y: underline_y,
                    width: underline_w * 0.25,
                    height: underline_h,
                },
                underline_zero, underline_color,
            );
            ui_builder.fill(
                ui::widget::Rect {
                    x: center_x - underline_w * 0.25,
                    y: underline_y,
                    width: underline_w * 0.5,
                    height: underline_h,
                },
                underline_color,
            );
            ui_builder.fill_gradient_h(
                ui::widget::Rect {
                    x: center_x + underline_w * 0.25,
                    y: underline_y,
                    width: underline_w * 0.25,
                    height: underline_h,
                },
                underline_color, underline_zero,
            );

            // --- Status message with animated loading indicator ---
            let status_y = underline_y + s(16.0);
            let status_w = ui_text_handle.text_width_ui(status);
            ui_builder.text_ui(&ui_text_handle, status,
                center_x - status_w / 2.0, status_y,
                ui::builder::colors::FG_MUTED, bg);

            // Spinning arc indicator — small ring with a moving bright segment
            // that suggests "loading" without being distracting.
            {
                let spin_phase = self.tab_bar.glow_phase() * 1.5; // slightly faster spin
                let arc_r = ch * 0.4;
                let arc_cx = center_x - status_w / 2.0 - s(14.0);
                let arc_cy = status_y + ch / 2.0;
                // Background ring (very faint)
                let ring_rect = ui::widget::Rect {
                    x: arc_cx - arc_r, y: arc_cy - arc_r,
                    width: arc_r * 2.0, height: arc_r * 2.0,
                };
                ui_builder.stroke_rounded(ring_rect, arc_r, 0.8,
                    [active_accent[0], active_accent[1], active_accent[2], 0.08]);
                // Bright arc segment — 3 dots positioned along the ring at
                // the leading edge of a rotating sweep
                let dot_sz = s(2.0);
                for k in 0..3u32 {
                    let angle = spin_phase + k as f32 * 0.3;
                    let fade = 1.0 - k as f32 * 0.3;
                    let dx = arc_cx + arc_r * angle.cos() - dot_sz / 2.0;
                    let dy = arc_cy + arc_r * angle.sin() - dot_sz / 2.0;
                    ui_builder.fill_rounded(
                        ui::widget::Rect { x: dx, y: dy, width: dot_sz, height: dot_sz },
                        [active_accent[0], active_accent[1], active_accent[2], 0.5 * fade],
                        dot_sz / 2.0,
                    );
                }
            }

            // --- Keyboard shortcut cards ---
            // Each hint is rendered as a styled card: [key badge] description
            let hints = [
                ("Ctrl+T", "New tab"),
                ("Ctrl+W", "Close tab"),
                ("Ctrl+Tab", "Next tab"),
                ("Ctrl+,", "Settings"),
            ];

            let card_pad_h = s(8.0);
            let card_pad_v = s(4.0);
            let card_gap = s(6.0);
            let key_badge_pad_h = s(5.0);
            let key_badge_pad_v = s(2.0);
            let key_badge_radius = s(3.0);
            let card_radius = s(5.0);
            let key_desc_gap = s(10.0);

            // Calculate card width (all cards same width for alignment)
            let max_key_w = hints.iter()
                .map(|(k, _)| ui_text_handle.text_width(k))
                .fold(0.0f32, f32::max);
            let max_desc_w = hints.iter()
                .map(|(_, d)| ui_text_handle.text_width_ui(d))
                .fold(0.0f32, f32::max);
            let card_inner_w = (max_key_w + key_badge_pad_h * 2.0) + key_desc_gap + max_desc_w;
            let card_w = card_inner_w + card_pad_h * 2.0;
            let card_h = ch + card_pad_v * 2.0;

            let cards_start_y = status_y + ch + s(20.0);
            let card_x = center_x - card_w / 2.0;

            // Card container — subtle rounded backdrop behind all cards
            let container_pad = s(10.0);
            let container_rect = ui::widget::Rect {
                x: card_x - container_pad,
                y: cards_start_y - container_pad,
                width: card_w + container_pad * 2.0,
                height: hints.len() as f32 * (card_h + card_gap) - card_gap + container_pad * 2.0,
            };
            let container_bg = [
                ui::builder::colors::BG_DARK[0],
                ui::builder::colors::BG_DARK[1],
                ui::builder::colors::BG_DARK[2],
                0.3,
            ];
            // Subtle drop shadow below the card container for floating depth
            let shadow_rect = ui::widget::Rect {
                x: container_rect.x + s(2.0),
                y: container_rect.y + s(3.0),
                width: container_rect.width - s(4.0),
                height: container_rect.height,
            };
            ui_builder.fill_shadow(shadow_rect, [0.0, 0.0, 0.0, 0.12], s(8.0), s(10.0));
            ui_builder.fill_rounded(container_rect, container_bg, s(8.0));
            // Inner shadow for recessed depth on card container
            ui_builder.fill_inner_shadow_custom(container_rect,
                [0.0, 0.0, 0.0, 0.08], [s(8.0); 4], s(4.0));
            ui_builder.stroke_rounded(container_rect, s(8.0), 0.5,
                [ui::builder::colors::BORDER[0], ui::builder::colors::BORDER[1],
                 ui::builder::colors::BORDER[2], 0.25]);

            for (i, (key, desc)) in hints.iter().enumerate() {
                let y = cards_start_y + i as f32 * (card_h + card_gap);

                // Card background (subtle gradient)
                let card_rect = ui::widget::Rect {
                    x: card_x, y, width: card_w, height: card_h,
                };
                let card_top = [
                    ui::builder::colors::BG_SURFACE[0] * 0.6,
                    ui::builder::colors::BG_SURFACE[1] * 0.6,
                    ui::builder::colors::BG_SURFACE[2] * 0.6,
                    0.4,
                ];
                let card_bot = [
                    ui::builder::colors::BG_SURFACE[0] * 0.5,
                    ui::builder::colors::BG_SURFACE[1] * 0.5,
                    ui::builder::colors::BG_SURFACE[2] * 0.5,
                    0.35,
                ];
                ui_builder.fill_rounded_gradient(card_rect, card_top, card_bot, card_radius);
                ui_builder.stroke_rounded(card_rect, card_radius, 0.5,
                    [ui::builder::colors::BORDER[0], ui::builder::colors::BORDER[1],
                     ui::builder::colors::BORDER[2], 0.15]);

                // Key badge (darker inset pill)
                let key_w = ui_text_handle.text_width(key);
                let badge_w = key_w + key_badge_pad_h * 2.0;
                let badge_h = ch + key_badge_pad_v * 2.0;
                let badge_x = card_x + card_pad_h;
                let badge_y = y + (card_h - badge_h) / 2.0;
                let badge_rect = ui::widget::Rect {
                    x: badge_x, y: badge_y, width: badge_w, height: badge_h,
                };
                // Key badge: darker background with subtle border (like a physical keycap)
                let badge_bg_top = [
                    ui::builder::colors::BG_DARK[0] * 1.1,
                    ui::builder::colors::BG_DARK[1] * 1.1,
                    ui::builder::colors::BG_DARK[2] * 1.1,
                    0.9,
                ];
                let badge_bg_bot = [
                    ui::builder::colors::BG_DARK[0] * 0.9,
                    ui::builder::colors::BG_DARK[1] * 0.9,
                    ui::builder::colors::BG_DARK[2] * 0.9,
                    0.9,
                ];
                // Drop shadow below keycap for physical "raised key" depth
                let keycap_shadow_rect = ui::widget::Rect {
                    x: badge_x + s(1.0),
                    y: badge_y + s(1.5),
                    width: badge_w - s(2.0),
                    height: badge_h,
                };
                ui_builder.fill_shadow(keycap_shadow_rect,
                    [0.0, 0.0, 0.0, 0.2], key_badge_radius, s(3.0));
                ui_builder.fill_rounded_gradient(badge_rect, badge_bg_top, badge_bg_bot, key_badge_radius);
                // Top highlight (keycap bevel)
                ui_builder.hline_fade(
                    badge_x + key_badge_radius, badge_y + 1.0,
                    badge_w - key_badge_radius * 2.0, 1.0,
                    [1.0, 1.0, 1.0, 0.10], s(4.0),
                );
                // Bottom shadow (keycap depth)
                ui_builder.hline_fade(
                    badge_x + key_badge_radius, badge_y + badge_h - 1.0,
                    badge_w - key_badge_radius * 2.0, 1.0,
                    [0.0, 0.0, 0.0, 0.20], s(4.0),
                );
                ui_builder.stroke_rounded(badge_rect, key_badge_radius, 0.5,
                    [ui::builder::colors::BORDER[0], ui::builder::colors::BORDER[1],
                     ui::builder::colors::BORDER[2], 0.5]);

                // Key text (centered in badge)
                let key_text_x = badge_x + key_badge_pad_h;
                let key_text_y = y + (card_h - ch) / 2.0;
                ui_builder.text(&ui_text_handle, key, key_text_x, key_text_y,
                    ui::builder::colors::FG_PRIMARY, ui::builder::colors::BG_DARK);

                // Description text (after badge) — proportional for natural reading
                let desc_x = badge_x + badge_w + key_desc_gap;
                ui_builder.text_ui(&ui_text_handle, desc, desc_x, key_text_y,
                    ui::builder::colors::FG_MUTED, bg);
            }

            // Version indicator — very muted, below the card container
            let version_str = concat!("v", env!("CARGO_PKG_VERSION"));
            let version_w = ui_text_handle.text_width_ui(version_str);
            let version_y = container_rect.y + container_rect.height + s(12.0);
            let version_fg = [
                ui::builder::colors::FG_MUTED[0],
                ui::builder::colors::FG_MUTED[1],
                ui::builder::colors::FG_MUTED[2],
                0.3,
            ];
            ui_builder.text_ui(&ui_text_handle, version_str,
                center_x - version_w / 2.0, version_y,
                version_fg, bg);
        }

        // Scrollbar (rendered before chrome so it layers under borders)
        // Hover proximity: scrollbar widens and brightens when mouse is near.
        if let Some(grid) = &self.current_grid {
            let visible_rows = self.terminal_size().0 as usize;
            let total = grid.total_scrollback + visible_rows;
            if total > visible_rows && visible_rows > 0 {
                let s = |v: f32| ui_text_handle.s(v);
                let track_margin = s(2.0);

                // Mouse proximity calculation — how close is cursor to scrollbar edge?
                // The Anim smoothly interpolates so width/opacity transitions are buttery.
                let scrollbar_edge_x = layout.terminal.x + layout.terminal.width - track_margin;
                let proximity_zone = s(40.0); // pixels within which scrollbar reacts
                let raw_proximity = if let Some((mx, _my)) = self.mouse_position {
                    let mx = mx as f32;
                    let dist = (scrollbar_edge_x - mx).abs();
                    if dist < proximity_zone {
                        let t = 1.0 - (dist / proximity_zone);
                        t * t // quadratic ease-in for snappier feel near the edge
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };
                self.scrollbar_hover_anim.set(raw_proximity);
                let sb_animating = self.scrollbar_hover_anim.tick(ui::anim::timing::MEDIUM, dt);
                if sb_animating {
                    if let Some(w) = &self.window { w.request_redraw(); }
                }
                let hover_t = self.scrollbar_hover_anim.value();

                // Interpolate bar width: thin (6px) when far, wide (10px) when close
                let bar_w_min = s(6.0);
                let bar_w_max = s(10.0);
                let bar_w = bar_w_min + (bar_w_max - bar_w_min) * hover_t;

                let track_rect = ui::widget::Rect {
                    x: layout.terminal.x + layout.terminal.width - bar_w - track_margin,
                    y: layout.terminal.y + track_margin,
                    width: bar_w,
                    height: layout.terminal.height - track_margin * 2.0,
                };

                // Thumb size and position
                let ratio = visible_rows as f32 / total as f32;
                let thumb_h = (track_rect.height * ratio).max(s(20.0));
                // scrollback_offset=0 means at bottom (live), higher = scrolled up
                let scroll_frac = if grid.total_scrollback > 0 {
                    1.0 - (self.scrollback_offset as f32 / grid.total_scrollback as f32)
                } else {
                    1.0
                };
                let thumb_y = track_rect.y + (track_rect.height - thumb_h) * scroll_frac;

                let thumb_rect = ui::widget::Rect {
                    x: track_rect.x,
                    y: thumb_y,
                    width: bar_w,
                    height: thumb_h,
                };

                // Track background: more visible on hover
                let track_alpha = 0.03 + 0.04 * hover_t;
                ui_builder.fill_rounded(track_rect, [1.0, 1.0, 1.0, track_alpha], bar_w / 2.0);

                // Thumb shadow for depth (subtle, only visible on hover proximity)
                if hover_t > 0.1 {
                    let shadow_alpha = 0.12 * hover_t;
                    ui_builder.fill_shadow(thumb_rect, [0.0, 0.0, 0.0, shadow_alpha], bar_w / 2.0, s(3.0));
                }

                // Thumb: SDF gradient for 3D cylinder feel (brighter top → darker bottom).
                // When actively scrolled, the thumb picks up a subtle accent tint
                // matching the active tab — visual coherence with cursor and selection.
                let is_scrolled = self.scrollback_offset > 0;
                let accent = self.active_accent();
                let base_alpha = if is_scrolled { 0.28 } else { 0.15 };
                let scroll_alpha = base_alpha + 0.20 * hover_t;
                // Blend toward accent color when scrolled + hovering
                let accent_blend = if is_scrolled { hover_t * 0.3 } else { 0.0 };
                let thumb_r = 1.0 * (1.0 - accent_blend) + accent[0] * accent_blend;
                let thumb_g = 1.0 * (1.0 - accent_blend) + accent[1] * accent_blend;
                let thumb_b = 1.0 * (1.0 - accent_blend) + accent[2] * accent_blend;
                let thumb_top = [thumb_r, thumb_g, thumb_b, scroll_alpha * (1.0 + 0.15 * hover_t)];
                let thumb_bottom = [thumb_r, thumb_g, thumb_b, scroll_alpha * (1.0 - 0.1 * hover_t)];
                let base_border = if is_scrolled { 0.12 } else { 0.08 };
                let border_alpha = base_border + 0.10 * hover_t;
                let thumb_border = [thumb_r, thumb_g, thumb_b, border_alpha];
                ui_builder.fill_rounded_gradient(thumb_rect, thumb_top, thumb_bottom, bar_w / 2.0);
                ui_builder.stroke_rounded(thumb_rect, bar_w / 2.0, 0.5, thumb_border);

                // Grip marks: three small etched horizontal lines in the center
                // of the thumb.  These provide visual affordance (signals "you can
                // drag me") and appear only when the mouse is close enough.  Each
                // line is a dark-light pair for a subtle inset/emboss effect.
                if hover_t > 0.15 && thumb_h > s(28.0) {
                    let grip_alpha = (hover_t - 0.15) / 0.85; // 0→1 over the hover range
                    let grip_w = (bar_w * 0.45).round().max(2.0);
                    let grip_x = thumb_rect.x + (bar_w - grip_w) / 2.0;
                    let grip_cy = thumb_y + thumb_h / 2.0;
                    let grip_spacing = s(3.0);
                    let grip_dark = [0.0, 0.0, 0.0, 0.20 * grip_alpha];
                    let grip_light = [1.0, 1.0, 1.0, 0.08 * grip_alpha];
                    for gi in -1i32..=1 {
                        let gy = grip_cy + gi as f32 * grip_spacing - 0.5;
                        ui_builder.hline_aa(grip_x, gy, grip_w, 1.0, grip_dark);
                        ui_builder.hline_aa(grip_x, gy + 1.0, grip_w, 1.0, grip_light);
                    }
                }

                // Scroll-away fog: subtle gradients at edges when scrolled,
                // hinting that there's content beyond the visible viewport.
                if self.scrollback_offset > 0 {
                    // Bottom fog — newer content below
                    let fog_h = s(8.0);
                    let fog_rect = ui::widget::Rect {
                        x: layout.terminal.x,
                        y: layout.terminal.y + layout.terminal.height - fog_h,
                        width: layout.terminal.width,
                        height: fog_h,
                    };
                    ui_builder.fill_gradient(fog_rect,
                        [0.0, 0.0, 0.0, 0.0],
                        [0.0, 0.0, 0.0, 0.15],
                    );
                }
                // Top fog — older scrollback content above (always present when
                // there's scrollback history, regardless of current scroll position)
                if grid.total_scrollback > 0 && self.scrollback_offset < grid.total_scrollback {
                    let fog_h = s(6.0);
                    let fog_rect = ui::widget::Rect {
                        x: layout.terminal.x,
                        y: layout.terminal.y,
                        width: layout.terminal.width,
                        height: fog_h,
                    };
                    ui_builder.fill_gradient(fog_rect,
                        [0.0, 0.0, 0.0, 0.10],
                        [0.0, 0.0, 0.0, 0.0],
                    );
                }
            }
        }

        // Tab bar now serves as title bar (full width at top, includes window buttons)
        self.tab_bar.sidebar_width = layout.sidebar.width;
        self.tab_bar.build(&mut ui_builder, layout.tab_bar, &ui_text_handle);
        self.sidebar.build(&mut ui_builder, layout.sidebar, &ui_text_handle);
        self.status_bar.sidebar_width = layout.sidebar.width;
        self.status_bar.build(&mut ui_builder, layout.status_bar, &ui_text_handle, self.tab_bar.glow_phase());

        // Breadcrumb/path bar — thin bar between tab bar and content showing
        // the current working directory as segmented path with chevron separators.
        {
            let bc = &layout.breadcrumb;
            let s = |v: f32| ui_text_handle.s(v);
            let ch = ui_text_handle.cell_height;

            // Background: slightly lighter than content for subtle visual separation
            let bc_bg = [
                ui::builder::colors::BG_BASE[0] * 0.96,
                ui::builder::colors::BG_BASE[1] * 0.96,
                ui::builder::colors::BG_BASE[2] * 0.96,
                1.0,
            ];
            ui_builder.fill(*bc, bc_bg);

            // Bottom separator — very subtle groove
            ui_builder.hline_aa(bc.x, bc.bottom() - 1.0, bc.width, 1.0,
                [ui::builder::colors::BORDER[0], ui::builder::colors::BORDER[1],
                 ui::builder::colors::BORDER[2], 0.3]);
            // Left inner shadow for sidebar-cast depth
            ui_builder.fill_gradient_h(
                ui::widget::Rect { x: bc.x, y: bc.y, width: s(6.0), height: bc.height },
                [0.0, 0.0, 0.0, 0.06],
                [0.0, 0.0, 0.0, 0.0],
            );

            // Path segments: show CWD as breadcrumb with "›" separators
            let cwd = &self.status_bar.cwd;
            if !cwd.is_empty() {
                let y_center = bc.y + (bc.height - ch) / 2.0;
                let mut x = bc.x + s(12.0);
                let icon_sz = ch * 0.75;
                let icon_t = (0.8 * ui_text_handle.scale).max(1.0);

                // Small folder icon at start
                ui_builder.icon_folder(
                    ui::widget::Rect { x, y: bc.y + (bc.height - icon_sz) / 2.0, width: icon_sz, height: icon_sz },
                    icon_t,
                    [ui::builder::colors::FG_MUTED[0], ui::builder::colors::FG_MUTED[1],
                     ui::builder::colors::FG_MUTED[2], 0.6],
                );
                x += icon_sz + s(6.0);

                // Split path into segments — show at most 4 segments,
                // with ellipsis for earlier segments that are elided.
                let sep = if cwd.contains('\\') { '\\' } else { '/' };
                let all_segments: Vec<&str> = cwd.split(sep).filter(|s| !s.is_empty()).collect();
                let max_segments = 4;
                let (show_ellipsis, segments) = if all_segments.len() > max_segments {
                    (true, &all_segments[all_segments.len() - max_segments..])
                } else {
                    (false, all_segments.as_slice())
                };
                let chevron_fg = [ui::builder::colors::FG_MUTED[0], ui::builder::colors::FG_MUTED[1],
                                  ui::builder::colors::FG_MUTED[2], 0.4];
                let segment_fg = [ui::builder::colors::FG_MUTED[0], ui::builder::colors::FG_MUTED[1],
                                  ui::builder::colors::FG_MUTED[2], 0.65];
                let last_fg = ui::builder::colors::FG_SECONDARY;

                if show_ellipsis {
                    ui_builder.text_ui(&ui_text_handle, "\u{2026}", x, y_center, chevron_fg, bc_bg);
                    x += ui_text_handle.text_width_ui("\u{2026}") + s(2.0);
                    ui_builder.text_ui(&ui_text_handle, "\u{203A}", x, y_center, chevron_fg, bc_bg);
                    x += ui_text_handle.text_width_ui("\u{203A}") + s(4.0);
                }
                for (i, seg) in segments.iter().enumerate() {
                    if i > 0 {
                        // Chevron separator
                        ui_builder.text_ui(&ui_text_handle, "\u{203A}", x, y_center, chevron_fg, bc_bg);
                        x += ui_text_handle.text_width_ui("\u{203A}") + s(4.0);
                    }
                    let is_last = i == segments.len() - 1;
                    let fg = if is_last { last_fg } else { segment_fg };
                    // Last segment gets a subtle pill background to highlight
                    // the current directory — matches VS Code/Zed breadcrumb style
                    if is_last {
                        let seg_w = ui_text_handle.text_width_ui(seg);
                        let pill_pad = s(4.0);
                        let pill_h = ch * 0.9;
                        let pill_y = bc.y + (bc.height - pill_h) / 2.0;
                        let pill_rect = ui::widget::Rect {
                            x: x - pill_pad, y: pill_y,
                            width: seg_w + pill_pad * 2.0, height: pill_h,
                        };
                        let pill_r = s(3.0);
                        ui_builder.fill_rounded(pill_rect,
                            [ui::builder::colors::BG_SURFACE[0],
                             ui::builder::colors::BG_SURFACE[1],
                             ui::builder::colors::BG_SURFACE[2], 0.35],
                            pill_r);
                        ui_builder.stroke_rounded(pill_rect, pill_r, 0.5,
                            [ui::builder::colors::BORDER[0],
                             ui::builder::colors::BORDER[1],
                             ui::builder::colors::BORDER[2], 0.15]);
                    }
                    ui_builder.text_ui(&ui_text_handle, seg, x, y_center, fg, bc_bg);
                    x += ui_text_handle.text_width_ui(seg) + s(4.0);
                }
            }
        }
        // Window outer border — multi-layer shadow + border for professional depth.
        // When maximized, shadows are invisible (window fills screen) so skip them
        // to save GPU work.  Borders and accent top edge still render for polish.
        {
            let r = if self.is_maximized { 0.0 } else { ui_text_handle.s(3.0) };
            let full = ui::widget::Rect { x: 0.0, y: 0.0, width: vw, height: vh };

            if !self.is_maximized {
                // Two-layer shadow: far shadow (wide, faint) + near shadow (tight, darker).
                let far_shadow = ui::widget::Rect { x: -2.0, y: 0.0, width: vw + 4.0, height: vh + 4.0 };
                ui_builder.fill_shadow(far_shadow, [0.0, 0.0, 0.0, 0.12], r + 2.0, ui_text_handle.s(10.0));
                ui_builder.fill_shadow(full, [0.0, 0.0, 0.0, 0.30], r, ui_text_handle.s(3.0));
                // Outer border: darker edge against desktop
                ui_builder.stroke_rounded(full, r, 1.0, [0.05, 0.05, 0.08, 0.9]);
                // Inner highlight: subtle bright edge just inside for depth
                let inner = ui::widget::Rect { x: 1.0, y: 1.0, width: vw - 2.0, height: vh - 2.0 };
                ui_builder.stroke_rounded(inner, r.max(1.0) - 1.0, 1.0, [1.0, 1.0, 1.0, 0.04]);
            }

            // Accent-tinted top edge: picks up the active tab's accent color.
            // 2px height for visibility; stronger alpha when focused for a
            // prominent colored "brand bar" at the top of the window (like VS Code).
            let active_accent = self.active_accent();
            let breath = 0.85 + 0.15 * self.tab_bar.glow_phase().sin();
            let accent_alpha = if self.window_focused { 0.30 * breath } else { 0.08 };
            let accent_fade = ui_text_handle.s(40.0);
            let accent_h = if self.is_maximized { 2.0 } else { 2.0 };
            let accent_full = [active_accent[0], active_accent[1], active_accent[2], accent_alpha];
            let accent_zero = [active_accent[0], active_accent[1], active_accent[2], 0.0];
            let top_w = vw - r * 2.0;
            ui_builder.fill_gradient_h(
                ui::widget::Rect { x: r, y: 0.0, width: accent_fade, height: accent_h },
                accent_zero, accent_full,
            );
            ui_builder.fill(
                ui::widget::Rect { x: r + accent_fade, y: 0.0, width: top_w - accent_fade * 2.0, height: accent_h },
                accent_full,
            );
            ui_builder.fill_gradient_h(
                ui::widget::Rect { x: vw - r - accent_fade, y: 0.0, width: accent_fade, height: accent_h },
                accent_full, accent_zero,
            );
            // Glow spill below the accent bar for soft light emission
            let glow_below = [active_accent[0], active_accent[1], active_accent[2], accent_alpha * 0.3];
            let glow_below_zero = [active_accent[0], active_accent[1], active_accent[2], 0.0];
            ui_builder.fill_gradient(
                ui::widget::Rect { x: r + accent_fade, y: accent_h, width: top_w - accent_fade * 2.0, height: ui_text_handle.s(4.0) },
                glow_below, glow_below_zero,
            );
        }

        let (chrome_quads, text_commands) = ui_builder.finish();

        // Prepare atlas pipeline with terminal grid + UI text
        if let Some(renderer) = &mut self.renderer {
            renderer.prepare(
                &gpu.device,
                &gpu.queue,
                self.current_grid.as_ref(),
                gpu.config.width,
                gpu.config.height,
                layout.terminal_content.x,
                layout.terminal_content.y,
                layout.terminal_content.width,
                layout.terminal_content.height,
                &text_commands,
            );
        }

        // Pre-compute accent color before render pass (avoids borrow conflicts)
        let active_accent = self.active_accent();

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // One Dark BG_DARK (#1b1e24) in linear space
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0094,
                            g: 0.0118,
                            b: 0.0170,
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

            // Draw chrome quads (backgrounds, borders)
            if let Some(quad_pipe) = &mut self.quad_pipeline {
                quad_pipe.draw(&gpu.device, &gpu.queue, &mut pass, &chrome_quads);
            }

            // Draw terminal content + UI text (both through atlas pipeline)
            if let Some(renderer) = &self.renderer {
                renderer.draw(&mut pass);
            }

            // Draw selection highlight overlay (between text and cursor)
            if let Some(quad_pipe) = &mut self.quad_pipeline {
                if self.selection.has_selection() {
                    if let (Some(grid), Some(r)) = (&self.current_grid, &self.renderer) {
                        let m = r.font_metrics().scaled_for_render();
                        let cw = m.cell_width;
                        let ch = m.cell_height;
                        let (sel_start, sel_end) = self.selection.normalized();
                        let cols = grid.dimensions.cols as usize;
                        let radius = 3.0 * self.scale_factor;
                        // Selection accent matches active tab for visual coherence
                        let accent = active_accent;
                        let sel_color = [accent[0], accent[1], accent[2], 0.22];
                        let sel_border = [accent[0], accent[1], accent[2], 0.10];

                        let mut sel_verts = Vec::new();

                        // Selection bounding-box glow: soft Gaussian emission around the
                        // entire selection area.  Creates a subtle "spotlight" effect that
                        // draws the eye to selected content — matching the accent glow
                        // language used on active tabs and sidebar indicators.
                        {
                            let first_row_px = (sel_start.row as f32 * ch).round() + layout.terminal_content.y;
                            let last_row_py = ((sel_end.row + 1) as f32 * ch).round() + layout.terminal_content.y;
                            let bbox_x = layout.terminal_content.x;
                            let bbox_w = layout.terminal_content.width;
                            let bbox_h = last_row_py - first_row_px;
                            if bbox_h > 0.0 {
                                sel_verts.extend_from_slice(
                                    &ui::quad_renderer::quad_vertices_sdf(
                                        bbox_x, first_row_px, bbox_w, bbox_h,
                                        vw, vh,
                                        [accent[0], accent[1], accent[2], 0.04],
                                        [radius; 4], 0.0, [0.0; 4],
                                        6.0 * self.scale_factor,
                                    ),
                                );
                            }
                        }

                        for row in sel_start.row..=sel_end.row {
                            if row >= grid.rows.len() { break; }

                            // Determine column range for this row
                            let col_start = if row == sel_start.row { sel_start.col } else { 0 };
                            let col_end = if row == sel_end.row {
                                sel_end.col
                            } else {
                                cols.saturating_sub(1)
                            };
                            if col_start > col_end { continue; }

                            let px = (col_start as f32 * cw).round() + layout.terminal_content.x;
                            let py = (row as f32 * ch).round() + layout.terminal_content.y;
                            let pw = ((col_end - col_start + 1) as f32 * cw).round();
                            let ph = ch;

                            // Clip to terminal area
                            if px >= layout.terminal_content.x + layout.terminal_content.width
                                || py >= layout.terminal_content.y + layout.terminal_content.height
                            {
                                continue;
                            }

                            // Corner radii: round outer edges of the selection shape
                            let is_single = sel_start.row == sel_end.row;
                            let is_first = row == sel_start.row;
                            let is_last = row == sel_end.row;
                            let radii = if is_single {
                                [radius; 4]
                            } else if is_first {
                                [radius, radius, 0.0, 0.0]
                            } else if is_last {
                                [0.0, 0.0, radius, radius]
                            } else {
                                [0.0; 4]
                            };

                            sel_verts.extend_from_slice(
                                &ui::quad_renderer::quad_vertices_sdf(
                                    px, py, pw, ph,
                                    vw, vh, sel_color,
                                    radii, 0.5, sel_border, 0.0,
                                ),
                            );
                        }

                        if !sel_verts.is_empty() {
                            quad_pipe.draw(&gpu.device, &gpu.queue, &mut pass, &sel_verts);
                        }
                    }
                }
            }

            // Draw cursor overlay via SDF quad pipeline (on top of terminal text)
            // for rounded corners and subtle glow — professional look.
            if let Some(quad_pipe) = &mut self.quad_pipeline {
                if let Some(grid) = &self.current_grid {
                    if !grid.cursor_hidden {
                        if let Some(r) = &self.renderer {
                            let m = r.font_metrics().scaled_for_render();
                            let cw = m.cell_width;
                            let ch = m.cell_height;
                            let cpx = (grid.cursor.col as f32 * cw).round() + layout.terminal_content.x;
                            let cpy = (grid.cursor.row as f32 * ch).round() + layout.terminal_content.y;

                            // Clip cursor to terminal area
                            let clip_r = layout.terminal_content.x + layout.terminal_content.width;
                            let clip_b = layout.terminal_content.y + layout.terminal_content.height;
                            if cpx < clip_r && cpy < clip_b {
                                use godly_protocol::types::CursorShape;
                                let blink_t = self.cursor_blink_anim.value();
                                let accent = active_accent;
                                let radius = 2.0 * self.scale_factor;
                                let focused = self.window_focused;

                                let (cx, cy, cwidth, cheight) = match grid.cursor.cursor_style {
                                    CursorShape::BlinkBlock | CursorShape::SteadyBlock => {
                                        (cpx, cpy, cw, ch)
                                    }
                                    CursorShape::BlinkUnderline | CursorShape::SteadyUnderline => {
                                        let uh = (2.0 * self.scale_factor).max(2.0);
                                        (cpx, cpy + ch - uh, cw, uh)
                                    }
                                    CursorShape::BlinkBar | CursorShape::SteadyBar => {
                                        let bw = (2.0 * self.scale_factor).max(2.0);
                                        (cpx, cpy, bw, ch)
                                    }
                                };

                                let is_block = matches!(grid.cursor.cursor_style,
                                    CursorShape::BlinkBlock | CursorShape::SteadyBlock);

                                let mut cursor_verts = Vec::new();

                                if focused {
                                    // Focused cursor: accent-tinted body with gradient for 3D depth.
                                    // Blends white toward the active tab accent for visual coherence
                                    // with selection highlights, glow, and tab chrome.
                                    let accent_blend = 0.15;
                                    let base_r = 1.0 * (1.0 - accent_blend) + accent[0] * accent_blend;
                                    let base_g = 1.0 * (1.0 - accent_blend) + accent[1] * accent_blend;
                                    let base_b = 1.0 * (1.0 - accent_blend) + accent[2] * accent_blend;
                                    let base_a = 0.85 * blink_t;

                                    // Glow behind cursor (accent-colored Gaussian emission)
                                    let glow_color = [accent[0], accent[1], accent[2], 0.14 * blink_t];
                                    cursor_verts.extend_from_slice(
                                        &ui::quad_renderer::quad_vertices_sdf(
                                            cx, cy, cwidth, cheight,
                                            vw, vh, glow_color,
                                            [radius; 4], 0.0, [0.0; 4],
                                            4.0 * self.scale_factor,
                                        ),
                                    );

                                    // Cursor body: SDF gradient (brighter top → slightly darker bottom)
                                    // for consistent 3D depth with the rest of the UI chrome.
                                    let cursor_top = [base_r, base_g, base_b, base_a];
                                    let cursor_bot = [base_r * 0.92, base_g * 0.92, base_b * 0.92, base_a];
                                    cursor_verts.extend_from_slice(
                                        &ui::quad_renderer::quad_vertices_sdf_gradient(
                                            cx, cy, cwidth, cheight,
                                            vw, vh, cursor_top, cursor_bot,
                                            [radius; 4], 0.0, [0.0; 4],
                                        ),
                                    );
                                } else if is_block {
                                    // Unfocused block cursor: hollow outline (standard terminal behavior).
                                    // Professional terminals (iTerm2, VS Code, Windows Terminal, kitty)
                                    // switch to an outline when the window loses focus — signals
                                    // "this pane isn't receiving input" without hiding the cursor.
                                    let outline_w = (1.0 * self.scale_factor).max(1.0);
                                    let outline_color = [
                                        0.7 * (1.0 - 0.15) + accent[0] * 0.15,
                                        0.7 * (1.0 - 0.15) + accent[1] * 0.15,
                                        0.7 * (1.0 - 0.15) + accent[2] * 0.15,
                                        0.5 * blink_t,
                                    ];
                                    // Faint glow (dimmer than focused)
                                    let glow_color = [accent[0], accent[1], accent[2], 0.06 * blink_t];
                                    cursor_verts.extend_from_slice(
                                        &ui::quad_renderer::quad_vertices_sdf(
                                            cx, cy, cwidth, cheight,
                                            vw, vh, glow_color,
                                            [radius; 4], 0.0, [0.0; 4],
                                            3.0 * self.scale_factor,
                                        ),
                                    );
                                    // Hollow outline (transparent fill + border)
                                    cursor_verts.extend_from_slice(
                                        &ui::quad_renderer::quad_vertices_sdf(
                                            cx, cy, cwidth, cheight,
                                            vw, vh, [0.0, 0.0, 0.0, 0.0],
                                            [radius; 4], outline_w, outline_color, 0.0,
                                        ),
                                    );
                                } else {
                                    // Unfocused bar/underline cursor: dimmed solid (thin enough
                                    // that outline wouldn't be visible, so just reduce opacity).
                                    let dim_color = [
                                        0.7 * (1.0 - 0.15) + accent[0] * 0.15,
                                        0.7 * (1.0 - 0.15) + accent[1] * 0.15,
                                        0.7 * (1.0 - 0.15) + accent[2] * 0.15,
                                        0.45 * blink_t,
                                    ];
                                    cursor_verts.extend_from_slice(
                                        &ui::quad_renderer::quad_vertices_sdf(
                                            cx, cy, cwidth, cheight,
                                            vw, vh, dim_color,
                                            [radius; 4], 0.0, [0.0; 4], 0.0,
                                        ),
                                    );
                                }

                                quad_pipe.draw(&gpu.device, &gpu.queue, &mut pass, &cursor_verts);
                            }
                        }
                    }
                }
            }

            // Unfocused window dimming — drawn last, on top of everything.
            // Professional apps (VS Code, Zed) subtly desaturate/dim when inactive.
            // Uses a vignette approach: edges dim more than center, creating a
            // natural depth-of-field "faded" look rather than a flat overlay.
            // Cold blue-black tint shifts color temperature cooler.
            let dim_t = self.focus_dim_anim.value();
            if dim_t > 0.005 {
                if let Some(quad_pipe) = &mut self.quad_pipeline {
                    let dim_color = [0.02, 0.02, 0.06];
                    let mut dim_verts = Vec::new();
                    // Base uniform dim (lighter than before — vignette adds the rest)
                    dim_verts.extend_from_slice(&ui::quad_renderer::quad_vertices(
                        0.0, 0.0, vw, vh, vw, vh,
                        [dim_color[0], dim_color[1], dim_color[2], 0.10 * dim_t],
                    ));
                    // Edge vignette: darker bands at all four edges that add to the
                    // base dim.  Creates a natural tunnel-vision effect.
                    let vig_w = vw * 0.18;
                    let vig_h = vh * 0.15;
                    let vig_alpha = 0.08 * dim_t;
                    let vig_full = [dim_color[0], dim_color[1], dim_color[2], vig_alpha];
                    let vig_zero = [dim_color[0], dim_color[1], dim_color[2], 0.0];
                    // Left edge
                    dim_verts.extend_from_slice(&ui::quad_renderer::quad_vertices_gradient_h(
                        0.0, 0.0, vig_w, vh, vw, vh, vig_full, vig_zero,
                    ));
                    // Right edge
                    dim_verts.extend_from_slice(&ui::quad_renderer::quad_vertices_gradient_h(
                        vw - vig_w, 0.0, vig_w, vh, vw, vh, vig_zero, vig_full,
                    ));
                    // Top edge
                    dim_verts.extend_from_slice(&ui::quad_renderer::quad_vertices_gradient(
                        0.0, 0.0, vw, vig_h, vw, vh, vig_full, vig_zero,
                    ));
                    // Bottom edge
                    dim_verts.extend_from_slice(&ui::quad_renderer::quad_vertices_gradient(
                        0.0, vh - vig_h, vw, vig_h, vw, vh, vig_zero, vig_full,
                    ));
                    quad_pipe.draw(&gpu.device, &gpu.queue, &mut pass, &dim_verts);
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
            .with_inner_size(winit::dpi::LogicalSize::new(1100.0, 680.0))
            .with_min_inner_size(winit::dpi::LogicalSize::new(400.0, 300.0))
            .with_decorations(false)
            .with_maximized(true);

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
                std::thread::sleep(std::time::Duration::from_millis(50));
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
                // Track maximized state to skip outer shadow rendering
                if let Some(w) = &self.window {
                    self.is_maximized = w.is_maximized();
                }
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
            WindowEvent::Focused(focused) => {
                self.window_focused = focused;
                self.focus_dim_anim.set(if focused { 0.0 } else { 1.0 });
                if let Some(w) = &self.window { w.request_redraw(); }
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
                let layout = ui::layout::ShellLayout::compute(vw, vh, true, self.scale_factor);

                // Route mouse to UI chrome
                let me = ui::widget::MouseEvent::Move { x: px, y: py };
                self.tab_bar.on_mouse(me, layout.tab_bar, self.scale_factor);
                self.sidebar.on_mouse(me, layout.sidebar, self.scale_factor);
                // Status bar needs UiTextRenderer for pill hit-testing
                if let Some(renderer) = &self.renderer {
                    let m = renderer.font_metrics().scaled_for_render();
                    let mut ui_text = ui::builder::UiTextRenderer::new(m.cell_width, m.cell_height, self.scale_factor);
                    ui_text.ui_avg_advance = renderer.ui_avg_advance();
                    self.status_bar.sidebar_width = layout.sidebar.width;
                    self.status_bar.on_mouse(me, layout.status_bar, &ui_text);
                }

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
                        let layout = ui::layout::ShellLayout::compute(vw, vh, true, self.scale_factor);

                        if state == ElementState::Pressed {
                            // Check tab bar (which includes window buttons + drag)
                            let me = ui::widget::MouseEvent::Press { x: px, y: py };
                            if let Some(action) = self.tab_bar.on_mouse(me, layout.tab_bar, self.scale_factor) {
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
                    log::info!("KEY: {:?} mods={:?}", event.logical_key, self.modifiers.state());
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

/// Create a proportional sans-serif rasterizer for UI chrome labels.
/// Falls back gracefully: returns None if no suitable font is available.
#[cfg(windows)]
fn create_ui_rasterizer() -> Option<Box<dyn godly_terminal_surface::glyph_rasterizer::GlyphRasterizer>> {
    use godly_terminal_surface::directwrite_rasterizer::DirectWriteRasterizer;

    let mut dw = DirectWriteRasterizer::new().ok()?;
    // Try Segoe UI Variable (Windows 11), fall back to Segoe UI (Windows 10)
    if dw.load_system_font("Segoe UI Variable").is_ok() {
        log::info!("[FONT] UI font: Segoe UI Variable (proportional sans-serif)");
        Some(Box::new(dw))
    } else if dw.load_system_font("Segoe UI").is_ok() {
        log::info!("[FONT] UI font: Segoe UI (proportional sans-serif)");
        Some(Box::new(dw))
    } else {
        log::warn!("[FONT] No proportional UI font available, using monospace for all text");
        None
    }
}

#[cfg(not(windows))]
fn create_ui_rasterizer() -> Option<Box<dyn godly_terminal_surface::glyph_rasterizer::GlyphRasterizer>> {
    None // Proportional font not yet supported on non-Windows
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let event_loop = EventLoop::<AsyncEvent>::with_user_event().build().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let mut app = App::new(proxy);
    event_loop.run_app(&mut app).unwrap();
}
