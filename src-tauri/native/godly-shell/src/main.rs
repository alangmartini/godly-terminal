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

use std::cell::RefCell;
use std::rc::Rc;
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

const TERMINAL_FONT_CANDIDATES: &[&str] = &[
    "JetBrains Mono",
    "Cascadia Code",
    "Fira Code",
    "Consolas",
    "Cascadia Mono",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SceneMode {
    LiveShell,
    WebReferenceCrop,
}

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    format: wgpu::TextureFormat,
}

struct App {
    scene_mode: SceneMode,
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
    layout_engine: RefCell<ui::layout::ShellLayoutEngine>,
    ui_text_layout: Option<Rc<ui::text_layout::UiTextLayoutEngine>>,
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
    /// Progress bar fill percentage (0.0–1.0), animated when streaming.
    progress_pct: f32,
    progress_timer: f32,
    /// Right panel state
    right_panel: ui::right_panel::RightPanel,
    reference_pane: ui::reference_pane::ReferencePane,
    sidebar_width: f32,
    right_panel_width: f32,
    /// Resize handle state
    left_resize_dragging: bool,
    right_resize_dragging: bool,
    left_resize_hover: bool,
    right_resize_hover: bool,
    left_resize_anim: ui::anim::Anim,
    right_resize_anim: ui::anim::Anim,
}

impl App {
    fn new(proxy: EventLoopProxy<AsyncEvent>, scene_mode: SceneMode) -> Self {
        let mut tab_bar = ui::tab_bar::TabBar::new();
        // Pre-populate demo tabs to match the web reference exactly:
        // Web: opensessions(#6366f1), opensessions(#10b981, badge 3, ACTIVE),
        //      work(#f97316), opensessions(#8b5cf6, badge 12), opensessions(#6366f1)
        use ui::builder::colors;
        tab_bar.tabs = vec![
            ui::tab_bar::TabInfo {
                id: "demo-1".into(),
                title: "opensessions".into(),
                active: false,
                unread_count: 0,
                accent: Some(colors::ACCENT_BLUE),
            },
            ui::tab_bar::TabInfo {
                id: "demo-2".into(),
                title: "opensessions".into(),
                active: true,
                unread_count: 3,
                accent: Some(colors::ACCENT_EMERALD),
            },
            ui::tab_bar::TabInfo {
                id: "demo-3".into(),
                title: "work".into(),
                active: false,
                unread_count: 0,
                accent: Some(colors::ACCENT_ORANGE),
            },
            ui::tab_bar::TabInfo {
                id: "demo-4".into(),
                title: "opensessions".into(),
                active: false,
                unread_count: 12,
                accent: Some(colors::ACCENT_MAUVE),
            },
            ui::tab_bar::TabInfo {
                id: "demo-5".into(),
                title: "opensessions".into(),
                active: false,
                unread_count: 0,
                accent: Some(colors::ACCENT_BLUE),
            },
        ];

        let mut sidebar = ui::sidebar::Sidebar::new();
        let mut right_panel = ui::right_panel::RightPanel::new();
        let mut status_bar = ui::status_bar::StatusBar::new();

        // Demo data matching web reference — populated in LiveShell mode
        // so the status bar and agent panel have visible content.
        status_bar.cwd = "~/Documents/work/opensessions".into();
        status_bar.git_branch = "main".into();
        status_bar.git_diff_summary = "1 file changed +21 ~4 -70".into();
        status_bar.streaming = true;

        if scene_mode == SceneMode::WebReferenceCrop {
            tab_bar.crop_mode = true;
            tab_bar.show_brand = false;
            tab_bar.show_indicators = false;
            tab_bar.show_window_controls = false;
            tab_bar.show_new_tab_button = false;
            tab_bar.show_tab_close_buttons = false;
            sidebar.show_footer_sections = false;
            right_panel.visible = false;
        }

        Self {
            scene_mode,
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
            layout_engine: RefCell::new(ui::layout::ShellLayoutEngine::new()),
            ui_text_layout: None,
            tab_bar,
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
            progress_pct: 0.05,
            progress_timer: 0.0,
            right_panel,
            reference_pane: ui::reference_pane::ReferencePane::new(),
            sidebar_width: ui::layout::SIDEBAR_WIDTH,
            right_panel_width: ui::layout::RIGHT_PANEL_WIDTH,
            left_resize_dragging: false,
            right_resize_dragging: false,
            left_resize_hover: false,
            right_resize_hover: false,
            left_resize_anim: ui::anim::Anim::default(),
            right_resize_anim: ui::anim::Anim::default(),
            sidebar,
            status_bar: {
                status_bar.cwd = std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                status_bar.process_name = "pwsh".into();
                status_bar.streaming = true; // match web reference demo state
                                             // Detect git branch
                if let Ok(output) = std::process::Command::new("git")
                    .args(["rev-parse", "--abbrev-ref", "HEAD"])
                    .output()
                {
                    if output.status.success() {
                        status_bar.git_branch =
                            String::from_utf8_lossy(&output.stdout).trim().to_string();
                    }
                }
                // Detect git diff summary — parse --shortstat into
                // "N file(s) changed +M -K" format matching web reference.
                if let Ok(output) = std::process::Command::new("git")
                    .args(["diff", "--shortstat"])
                    .output()
                {
                    if output.status.success() {
                        let stat = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if !stat.is_empty() {
                            let mut file_count = String::new();
                            let mut parts = Vec::new();
                            for segment in stat.split(',') {
                                let seg = segment.trim();
                                if seg.contains("changed") {
                                    file_count = seg.to_string();
                                } else if seg.contains("insertion") {
                                    if let Some(n) = seg
                                        .split_whitespace()
                                        .next()
                                        .and_then(|s| s.parse::<u32>().ok())
                                    {
                                        parts.push(format!("+{}", n));
                                    }
                                } else if seg.contains("deletion") {
                                    if let Some(n) = seg
                                        .split_whitespace()
                                        .next()
                                        .and_then(|s| s.parse::<u32>().ok())
                                    {
                                        parts.push(format!("-{}", n));
                                    }
                                }
                            }
                            if !file_count.is_empty() || !parts.is_empty() {
                                let mut summary = Vec::new();
                                if !file_count.is_empty() {
                                    summary.push(file_count);
                                }
                                summary.extend(parts);
                                status_bar.git_diff_summary = summary.join(" ");
                            }
                        }
                    }
                }
                status_bar
            },
        }
    }

    fn is_web_reference_crop(&self) -> bool {
        self.scene_mode == SceneMode::WebReferenceCrop
    }

    fn ui_scale(&self) -> f32 {
        if self.is_web_reference_crop() {
            1.0
        } else {
            self.scale_factor
        }
    }

    fn ui_raster_scale(&self) -> f32 {
        let surface_scale = self.scale_factor.max(1.0);
        self.ui_scale() / surface_scale
    }

    fn init_gpu(&mut self, window: Arc<Window>) {
        let winit_size = window.inner_size();
        let phys_size = window_surface_size(&window);
        let scale = window.scale_factor();
        log::info!(
            "[DPI] init surface: winit={}x{}, hwnd-client={}x{}, scale={}",
            winit_size.width,
            winit_size.height,
            phys_size.width,
            phys_size.height,
            scale,
        );
        // DX12 preferred — cleaner shader compilation and better Windows integration.
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

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("godly-shell"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
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
            width: winit_size.width.max(1),
            height: winit_size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let scale_factor = scale as f32;
        self.scale_factor = scale_factor;
        log::info!("[DPI] rendering at physical resolution, scale_factor={scale_factor}");
        let font_size = 14.0_f32;
        let terminal_font = create_terminal_font_setup(font_size, scale_factor);
        let font_metrics = terminal_font.font_metrics;
        log::info!(
            "Font metrics: cell={}x{}, font_size={}, baseline={}, scale={}",
            font_metrics.cell_width,
            font_metrics.cell_height,
            font_metrics.font_size,
            font_metrics.baseline_offset,
            font_metrics.scale_factor
        );
        log::info!("[FONT] Terminal mono font: {}", terminal_font.family);
        let mut renderer = TerminalRenderer::new(
            &device,
            &queue,
            format,
            font_metrics,
            terminal_font.rasterizer,
        );
        let mut ui_families = ui::text_layout::UiFontFamilies {
            sans: "Segoe UI".into(),
            serif: "Georgia".into(),
            mono: terminal_font.family.clone(),
        };

        if let Some(ui_font) = create_ui_sans_font() {
            ui_families.sans = ui_font.family.clone();
            renderer.set_ui_rasterizer(ui_font.rasterizer);
            log::info!(
                "[FONT] UI sans font loaded, avg advance = {:.1}px",
                renderer.ui_avg_advance()
            );
        }
        if let Some(ui_font) = create_ui_serif_font() {
            ui_families.serif = ui_font.family.clone();
            renderer.set_ui_serif_rasterizer(ui_font.rasterizer);
            log::info!("[FONT] UI serif font loaded: {}", ui_families.serif);
        }
        if self.is_web_reference_crop() {
            if let Some(ui_font) = create_ui_mono_font(&terminal_font.family) {
                renderer.set_ui_mono_rasterizer(ui_font.rasterizer);
                log::info!(
                    "[FONT] UI mono screenshot font loaded (grayscale): {}",
                    ui_font.family
                );
            }
        }
        self.ui_text_layout = create_ui_text_layout_engine(ui_families).map(Rc::new);

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
        let sender = self
            .sender
            .as_ref()
            .expect("sender must be set before connecting daemon");

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
            let layout = self.shell_layout(vw, vh);
            let cols = (layout.terminal_content.width / metrics.cell_width).floor() as u16;
            let rows = (layout.terminal_content.height / metrics.cell_height).floor() as u16;
            (rows.max(1), cols.max(1))
        } else {
            (24, 80)
        }
    }

    fn shell_layout(&self, viewport_w: f32, viewport_h: f32) -> ui::layout::ShellLayout {
        self.layout_engine.borrow_mut().compute(
            viewport_w,
            viewport_h,
            true,
            self.right_panel.visible && !self.is_web_reference_crop(),
            !self.is_web_reference_crop(),
            self.sidebar_width,
            self.right_panel_width,
            self.ui_scale(),
        )
    }

    fn fetch_grid(&self) {
        let Some(daemon) = &self.daemon else { return };
        let Some(session_id) = &self.active_session else {
            return;
        };
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
        if self.is_web_reference_crop() {
            return;
        }
        let new_offset = (self.scrollback_offset as isize + delta).max(0) as usize;
        if new_offset == self.scrollback_offset {
            return;
        }
        self.scrollback_offset = new_offset;

        let Some(daemon) = &self.daemon else { return };
        let Some(session_id) = &self.active_session else {
            return;
        };
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
        if !self.selection.has_selection() {
            return;
        }
        let Some(grid) = &self.current_grid else {
            return;
        };
        let text = self.selection.selected_text(grid);
        if text.is_empty() {
            return;
        }
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

        log::debug!(
            "Sending {} bytes to PTY: {:?}",
            bytes.len(),
            String::from_utf8_lossy(&bytes)
        );
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

    /// The accent color of the currently active tab.
    /// Uses per-tab color if set, otherwise falls back to index-based rotation.
    fn active_accent(&self) -> [f32; 4] {
        self.tab_bar
            .tabs
            .iter()
            .enumerate()
            .find(|(_, t)| t.active)
            .map(|(i, t)| t.accent.unwrap_or_else(|| self.tab_bar.accent_for(i)))
            .unwrap_or(ui::builder::colors::ACCENT_BLUE)
    }

    fn render(&mut self) {
        // Frame-rate independent delta time (clamped to avoid spiral-of-death)
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_frame_time)
            .as_secs_f32()
            .min(0.1);
        self.last_frame_time = now;

        // Tick all hover animations and request another frame if any are active
        let mut animating = false;
        animating |= self.tab_bar.tick_animations(dt);
        animating |= self.sidebar.tick_animations(dt);
        animating |= self.status_bar.tick_animations(dt);
        animating |= self.right_panel.tick_animations(dt);
        self.left_resize_anim
            .set(if self.left_resize_hover || self.left_resize_dragging {
                1.0
            } else {
                0.0
            });
        self.right_resize_anim
            .set(if self.right_resize_hover || self.right_resize_dragging {
                1.0
            } else {
                0.0
            });
        animating |= self.left_resize_anim.tick(ui::anim::timing::HOVER, dt);
        animating |= self.right_resize_anim.tick(ui::anim::timing::HOVER, dt);
        animating |= self.focus_dim_anim.tick(ui::anim::timing::SLOW, dt);

        // Progress bar animation (when streaming)
        if self.status_bar.streaming {
            self.progress_timer += dt;
            if self.progress_timer >= 0.3 {
                self.progress_timer -= 0.3;
                // Advance by random-ish 1-2% (use frame time as pseudo-random)
                let inc = 0.01 + (dt * 1000.0).fract() * 0.01;
                self.progress_pct = (self.progress_pct + inc).min(0.90);
                if self.progress_pct >= 0.89 {
                    self.progress_pct = 0.05; // reset cycle
                }
            }
            animating = true;
        } else {
            self.progress_pct = 0.05;
            self.progress_timer = 0.0;
        }

        // Cursor blink: smooth fade between visible/invisible every ~500ms.
        // Only blinks for Blink* cursor styles; Steady* styles stay fully visible.
        {
            let is_blink_style = self.current_grid.as_ref().map_or(false, |g| {
                use godly_protocol::types::CursorShape;
                matches!(
                    g.cursor.cursor_style,
                    CursorShape::BlinkBlock | CursorShape::BlinkUnderline | CursorShape::BlinkBar
                )
            });
            if is_blink_style && self.window_focused {
                self.cursor_blink_timer += dt;
                if self.cursor_blink_timer >= 0.5 {
                    self.cursor_blink_timer = 0.0;
                    self.cursor_blink_phase = !self.cursor_blink_phase;
                    self.cursor_blink_anim
                        .set(if self.cursor_blink_phase { 1.0 } else { 0.0 });
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
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render"),
            });

        // Prepare terminal data BEFORE starting the render pass
        let vw = gpu.config.width as f32;
        let vh = gpu.config.height as f32;
        let layout = self.shell_layout(vw, vh);
        let reference_crop = self.is_web_reference_crop();
        let ui_scale = self.ui_scale();

        // Update status bar with current terminal dimensions
        self.status_bar.terminal_size = self.terminal_size();

        // Build UI chrome (quads + text commands)
        let ui_metrics = self.renderer.as_ref().map(|r| {
            if reference_crop {
                *r.font_metrics()
            } else {
                r.font_metrics().scaled_for_render()
            }
        });
        let ui_avg_advance = self.renderer.as_ref().map_or(0.0, |r| r.ui_avg_advance());
        let ui_text_handle = if let Some(m) = ui_metrics {
            let mut tr = ui::builder::UiTextRenderer::new(
                m.cell_width,
                m.cell_height,
                m.font_size,
                ui_scale,
            );
            tr.ui_avg_advance = ui_avg_advance;
            tr.layout_engine = self.ui_text_layout.clone();
            tr.raster_scale = self.ui_raster_scale();
            tr
        } else {
            let mut tr = ui::builder::UiTextRenderer::new(8.0, 16.0, 14.0, ui_scale);
            tr.raster_scale = self.ui_raster_scale();
            tr
        };
        let mut ui_builder = ui::builder::UiBuilder::new(vw, vh);

        // Terminal area background (BG_BASE) — must come before chrome overlays
        ui_builder.fill(layout.terminal, ui::builder::colors::BG_BASE);

        // Subtle directional shadows — kept minimal to match web reference's flat style.
        // Only retain lightweight edge shadows for panel depth, no vignettes or glows.
        if !reference_crop {
            // Tab bar cast shadow (subtle top edge darkening)
            let shadow_h = ui_text_handle.s(6.0);
            ui_builder.fill_gradient(
                ui::widget::Rect {
                    x: layout.terminal.x,
                    y: layout.terminal.y,
                    width: layout.terminal.width,
                    height: shadow_h,
                },
                [0.0, 0.0, 0.0, 0.06],
                [0.0, 0.0, 0.0, 0.0],
            );

            // Sidebar cast shadow (subtle left edge darkening)
            if layout.sidebar.width > 0.0 {
                let shadow_w = ui_text_handle.s(4.0);
                ui_builder.fill_gradient_h(
                    ui::widget::Rect {
                        x: layout.terminal.x,
                        y: layout.terminal.y,
                        width: shadow_w,
                        height: layout.terminal.height,
                    },
                    [0.0, 0.0, 0.0, 0.04],
                    [0.0, 0.0, 0.0, 0.0],
                );
            }
        }

        // Empty terminal welcome state — styled welcome screen with branded
        // header, status indicator, and keyboard shortcut cards.
        if reference_crop {
            self.reference_pane.build(
                &mut ui_builder,
                layout.terminal_content,
                &ui_text_handle,
                self.tab_bar.glow_phase(),
            );
        } else if self.current_grid.is_none() {
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
                let breath = 0.92 + 0.08 * self.tab_bar.glow_phase().sin();
                ui_builder.fill_shadow(
                    spot_rect,
                    [
                        active_accent[0],
                        active_accent[1],
                        active_accent[2],
                        0.020 * breath,
                    ],
                    spot_w * 0.3,
                    spot_w * 0.4,
                );
            }

            // --- Hero terminal icon ---
            // Large branding icon above the title inside a subtle rounded pill
            // background. Replaces the raw halo with a more refined treatment
            // that matches professional welcome page icon styling.
            let hero_icon_size = ch * 3.5;
            let hero_pill_pad = s(10.0);
            let hero_pill_size = hero_icon_size + hero_pill_pad * 2.0;
            let hero_pill_x = center_x - hero_pill_size / 2.0;
            let hero_pill_y = block_y - hero_pill_size - s(10.0);
            let hero_pill_rect = ui::widget::Rect {
                x: hero_pill_x,
                y: hero_pill_y,
                width: hero_pill_size,
                height: hero_pill_size,
            };
            let hero_pill_r = s(14.0);
            // Soft drop shadow for floating depth
            let breath = 0.92 + 0.08 * self.tab_bar.glow_phase().sin();
            ui_builder.fill_shadow(
                ui::widget::Rect {
                    x: hero_pill_x + s(2.0),
                    y: hero_pill_y + s(3.0),
                    width: hero_pill_size - s(4.0),
                    height: hero_pill_size,
                },
                [0.0, 0.0, 0.0, 0.15],
                hero_pill_r,
                s(10.0),
            );
            // Pill background: subtle accent-tinted fill
            let pill_bg = [
                active_accent[0] * 0.10 + ui::builder::colors::BG_SURFACE[0] * 0.90,
                active_accent[1] * 0.10 + ui::builder::colors::BG_SURFACE[1] * 0.90,
                active_accent[2] * 0.10 + ui::builder::colors::BG_SURFACE[2] * 0.90,
                0.6,
            ];
            let pill_bg_top = [
                pill_bg[0] * 1.08,
                pill_bg[1] * 1.08,
                pill_bg[2] * 1.08,
                pill_bg[3],
            ];
            ui_builder.fill_rounded_gradient(hero_pill_rect, pill_bg_top, pill_bg, hero_pill_r);
            // Accent-tinted border
            let pill_border = [
                active_accent[0] * 0.25 + ui::builder::colors::BORDER[0] * 0.75,
                active_accent[1] * 0.25 + ui::builder::colors::BORDER[1] * 0.75,
                active_accent[2] * 0.25 + ui::builder::colors::BORDER[2] * 0.75,
                0.35 * breath,
            ];
            ui_builder.stroke_rounded(hero_pill_rect, hero_pill_r, 0.5, pill_border);
            // Icon centered inside the pill
            let hero_x = hero_pill_x + hero_pill_pad;
            let hero_y = hero_pill_y + hero_pill_pad;
            let hero_rect = ui::widget::Rect {
                x: hero_x,
                y: hero_y,
                width: hero_icon_size,
                height: hero_icon_size,
            };
            // Icon stroke with accent tint
            let hero_icon_fg = [
                ui::builder::colors::FG_MUTED[0] * 0.35 + active_accent[0] * 0.65,
                ui::builder::colors::FG_MUTED[1] * 0.35 + active_accent[1] * 0.65,
                ui::builder::colors::FG_MUTED[2] * 0.35 + active_accent[2] * 0.65,
                0.70,
            ];
            let hero_t = (1.6 * ui_text_handle.scale).max(1.0);
            ui_builder.icon_terminal(hero_rect, hero_t, hero_icon_fg);

            // --- Branded header ---
            let title = "Godly Terminal";
            let title_w = ui_text_handle.text_width_ui(title);
            let title_x = center_x - title_w / 2.0;
            // Title text with accent tint — prominent as hero heading
            let title_fg = [
                ui::builder::colors::FG_PRIMARY[0] * 0.80 + active_accent[0] * 0.20,
                ui::builder::colors::FG_PRIMARY[1] * 0.80 + active_accent[1] * 0.20,
                ui::builder::colors::FG_PRIMARY[2] * 0.80 + active_accent[2] * 0.20,
                0.88,
            ];
            ui_builder.text_ui_bold_scaled(&ui_text_handle, title, title_x, block_y, title_fg, bg, 1.0);

            // Subtitle line — "GPU-accelerated terminal" in very muted text
            let subtitle = "GPU-accelerated terminal";
            let subtitle_w = ui_text_handle.text_width_ui(subtitle);
            let subtitle_y = block_y + ch + s(2.0);
            let subtitle_fg = [
                ui::builder::colors::FG_MUTED[0] * 0.7 + ui::builder::colors::FG_SECONDARY[0] * 0.3,
                ui::builder::colors::FG_MUTED[1] * 0.7 + ui::builder::colors::FG_SECONDARY[1] * 0.3,
                ui::builder::colors::FG_MUTED[2] * 0.7 + ui::builder::colors::FG_SECONDARY[2] * 0.3,
                0.55,
            ];
            ui_builder.text_ui(
                &ui_text_handle,
                subtitle,
                center_x - subtitle_w / 2.0,
                subtitle_y,
                subtitle_fg,
                bg,
            );

            // Accent underline below subtitle (breathing, matches active tab)
            let breath = 0.92 + 0.08 * self.tab_bar.glow_phase().sin();
            let underline_w = title_w * 0.6;
            let underline_y = subtitle_y + ch + s(4.0);
            let underline_h = s(1.5);
            let underline_color = [
                active_accent[0],
                active_accent[1],
                active_accent[2],
                0.25 * breath,
            ];
            let underline_zero = [active_accent[0], active_accent[1], active_accent[2], 0.0];
            ui_builder.fill_gradient_h(
                ui::widget::Rect {
                    x: center_x - underline_w / 2.0,
                    y: underline_y,
                    width: underline_w * 0.25,
                    height: underline_h,
                },
                underline_zero,
                underline_color,
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
                underline_color,
                underline_zero,
            );

            // --- Status message with animated loading indicator ---
            let status_y = underline_y + s(16.0);
            let status_w = ui_text_handle.text_width_ui(status);
            ui_builder.text_ui(
                &ui_text_handle,
                status,
                center_x - status_w / 2.0,
                status_y,
                ui::builder::colors::FG_MUTED,
                bg,
            );

            // Spinning arc indicator — small ring with a moving bright segment
            // that suggests "loading" without being distracting.
            {
                let spin_phase = self.tab_bar.glow_phase() * 1.5; // slightly faster spin
                let arc_r = ch * 0.4;
                let arc_cx = center_x - status_w / 2.0 - s(14.0);
                let arc_cy = status_y + ch / 2.0;
                // Background ring (very faint)
                let ring_rect = ui::widget::Rect {
                    x: arc_cx - arc_r,
                    y: arc_cy - arc_r,
                    width: arc_r * 2.0,
                    height: arc_r * 2.0,
                };
                ui_builder.stroke_rounded(
                    ring_rect,
                    arc_r,
                    0.8,
                    [active_accent[0], active_accent[1], active_accent[2], 0.08],
                );
                // Bright arc segment — 3 dots positioned along the ring at
                // the leading edge of a rotating sweep
                let dot_sz = s(2.0);
                for k in 0..3u32 {
                    let angle = spin_phase + k as f32 * 0.3;
                    let fade = 1.0 - k as f32 * 0.3;
                    let dx = arc_cx + arc_r * angle.cos() - dot_sz / 2.0;
                    let dy = arc_cy + arc_r * angle.sin() - dot_sz / 2.0;
                    ui_builder.fill_rounded(
                        ui::widget::Rect {
                            x: dx,
                            y: dy,
                            width: dot_sz,
                            height: dot_sz,
                        },
                        [
                            active_accent[0],
                            active_accent[1],
                            active_accent[2],
                            0.5 * fade,
                        ],
                        dot_sz / 2.0,
                    );
                }
            }

            // --- Keyboard shortcut cards (2×2 grid) ---
            // Compact two-column layout matching VS Code/Zed welcome pages.
            let hints = [
                ("Ctrl+T", "New tab"),
                ("Ctrl+W", "Close tab"),
                ("Ctrl+Tab", "Next tab"),
                ("Ctrl+,", "Settings"),
            ];

            let card_pad_h = s(8.0);
            let card_pad_v = s(4.0);
            let card_gap_h = s(8.0); // horizontal gap between columns
            let card_gap_v = s(6.0); // vertical gap between rows
            let key_badge_pad_h = s(5.0);
            let key_badge_pad_v = s(2.0);
            let key_badge_radius = s(3.0);
            let card_radius = s(5.0);
            let key_desc_gap = s(8.0);

            // Calculate per-cell width (measure longest key+desc in each column)
            let max_key_w = hints
                .iter()
                .map(|(k, _)| ui_text_handle.text_width(k))
                .fold(0.0f32, f32::max);
            let max_desc_w = hints
                .iter()
                .map(|(_, d)| ui_text_handle.text_width_ui(d))
                .fold(0.0f32, f32::max);
            let cell_inner_w = (max_key_w + key_badge_pad_h * 2.0) + key_desc_gap + max_desc_w;
            let cell_w = cell_inner_w + card_pad_h * 2.0;
            let card_h = ch + card_pad_v * 2.0;

            // Grid dimensions: 2 columns, 2 rows
            let grid_w = cell_w * 2.0 + card_gap_h;
            let grid_h = card_h * 2.0 + card_gap_v;

            let cards_start_y = status_y + ch + s(20.0);
            let grid_x = center_x - grid_w / 2.0;

            // Card container — subtle rounded backdrop behind the grid
            let container_pad = s(10.0);
            let container_rect = ui::widget::Rect {
                x: grid_x - container_pad,
                y: cards_start_y - container_pad,
                width: grid_w + container_pad * 2.0,
                height: grid_h + container_pad * 2.0,
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
            ui_builder.fill_inner_shadow_custom(
                container_rect,
                [0.0, 0.0, 0.0, 0.08],
                [s(8.0); 4],
                s(4.0),
            );
            ui_builder.stroke_rounded(
                container_rect,
                s(8.0),
                0.5,
                [
                    ui::builder::colors::BORDER[0],
                    ui::builder::colors::BORDER[1],
                    ui::builder::colors::BORDER[2],
                    0.25,
                ],
            );

            for (i, (key, desc)) in hints.iter().enumerate() {
                let col = i % 2;
                let row = i / 2;
                let cell_x = grid_x + col as f32 * (cell_w + card_gap_h);
                let y = cards_start_y + row as f32 * (card_h + card_gap_v);

                // Card background (subtle gradient)
                let card_rect = ui::widget::Rect {
                    x: cell_x,
                    y,
                    width: cell_w,
                    height: card_h,
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
                ui_builder.stroke_rounded(
                    card_rect,
                    card_radius,
                    0.5,
                    [
                        ui::builder::colors::BORDER[0],
                        ui::builder::colors::BORDER[1],
                        ui::builder::colors::BORDER[2],
                        0.15,
                    ],
                );

                // Key badge (darker inset pill)
                let key_w = ui_text_handle.text_width(key);
                let badge_w = key_w + key_badge_pad_h * 2.0;
                let badge_h = ch + key_badge_pad_v * 2.0;
                let badge_x = cell_x + card_pad_h;
                let badge_y = y + (card_h - badge_h) / 2.0;
                let badge_rect = ui::widget::Rect {
                    x: badge_x,
                    y: badge_y,
                    width: badge_w,
                    height: badge_h,
                };
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
                // Drop shadow below keycap
                let keycap_shadow_rect = ui::widget::Rect {
                    x: badge_x + s(1.0),
                    y: badge_y + s(1.5),
                    width: badge_w - s(2.0),
                    height: badge_h,
                };
                ui_builder.fill_shadow(
                    keycap_shadow_rect,
                    [0.0, 0.0, 0.0, 0.2],
                    key_badge_radius,
                    s(3.0),
                );
                ui_builder.fill_rounded_gradient(
                    badge_rect,
                    badge_bg_top,
                    badge_bg_bot,
                    key_badge_radius,
                );
                // Top highlight (keycap bevel)
                ui_builder.hline_fade(
                    badge_x + key_badge_radius,
                    badge_y + 1.0,
                    badge_w - key_badge_radius * 2.0,
                    1.0,
                    [1.0, 1.0, 1.0, 0.10],
                    s(4.0),
                );
                // Bottom shadow (keycap depth)
                ui_builder.hline_fade(
                    badge_x + key_badge_radius,
                    badge_y + badge_h - 1.0,
                    badge_w - key_badge_radius * 2.0,
                    1.0,
                    [0.0, 0.0, 0.0, 0.20],
                    s(4.0),
                );
                ui_builder.stroke_rounded(
                    badge_rect,
                    key_badge_radius,
                    0.5,
                    [
                        ui::builder::colors::BORDER[0],
                        ui::builder::colors::BORDER[1],
                        ui::builder::colors::BORDER[2],
                        0.5,
                    ],
                );

                // Key text (centered in badge)
                let key_text_x = badge_x + key_badge_pad_h;
                let key_text_y = y + (card_h - ch) / 2.0;
                ui_builder.text_mixed(
                    &ui_text_handle,
                    key,
                    key_text_x,
                    key_text_y,
                    ui::builder::colors::FG_PRIMARY,
                    ui::builder::colors::BG_DARK,
                );

                // Description text (after badge) — proportional for natural reading
                let desc_x = badge_x + badge_w + key_desc_gap;
                let desc_fg = [
                    ui::builder::colors::FG_MUTED[0] * 0.5
                        + ui::builder::colors::FG_SECONDARY[0] * 0.5,
                    ui::builder::colors::FG_MUTED[1] * 0.5
                        + ui::builder::colors::FG_SECONDARY[1] * 0.5,
                    ui::builder::colors::FG_MUTED[2] * 0.5
                        + ui::builder::colors::FG_SECONDARY[2] * 0.5,
                    0.75,
                ];
                ui_builder.text_ui(&ui_text_handle, desc, desc_x, key_text_y, desc_fg, bg);
            }

            // Thin separator between shortcut grid and CTA section
            let sep_y = container_rect.y + container_rect.height + s(10.0);
            let sep_w = grid_w * 0.6;
            let sep_color = [
                ui::builder::colors::BORDER[0],
                ui::builder::colors::BORDER[1],
                ui::builder::colors::BORDER[2],
                0.15,
            ];
            ui_builder.hline_fade(
                center_x - sep_w / 2.0,
                sep_y,
                sep_w,
                1.0,
                sep_color,
                s(12.0),
            );

            // Version indicator — very muted, below separator
            let version_str = concat!("v", env!("CARGO_PKG_VERSION"));
            let version_w = ui_text_handle.text_width_ui(version_str);
            let version_y = sep_y + s(10.0);
            let version_fg = [
                ui::builder::colors::FG_MUTED[0],
                ui::builder::colors::FG_MUTED[1],
                ui::builder::colors::FG_MUTED[2],
                0.38,
            ];
            ui_builder.text_ui(
                &ui_text_handle,
                version_str,
                center_x - version_w / 2.0,
                version_y,
                version_fg,
                bg,
            );

            // --- "Create terminal" CTA button ---
            // Full-width button spanning the card container for commanding visual
            // weight. Professional welcome pages use full-width primary actions.
            let cta_label = "Create terminal";
            let cta_text_w = ui_text_handle.text_width_ui(cta_label);
            let cta_icon_sz = ch * 0.75;
            let cta_icon_gap = s(6.0);
            let _cta_pad_h = s(16.0);
            let cta_pad_v = s(6.0);
            // Full-width: match the card container (with container_pad inset)
            let cta_w = container_rect.width;
            let cta_h = ch + cta_pad_v * 2.0;
            let cta_x = container_rect.x;
            let cta_y = version_y + ch + s(14.0);
            let cta_rect = ui::widget::Rect {
                x: cta_x,
                y: cta_y,
                width: cta_w,
                height: cta_h,
            };
            let cta_r = s(6.0);
            // Filled accent background — stronger accent presence (30% blend)
            // so the CTA reads as the obvious primary action in the welcome screen.
            let breath = 0.92 + 0.08 * self.tab_bar.glow_phase().sin();
            let cta_fill = [
                active_accent[0] * 0.30 + ui::builder::colors::BG_SURFACE[0] * 0.70,
                active_accent[1] * 0.30 + ui::builder::colors::BG_SURFACE[1] * 0.70,
                active_accent[2] * 0.30 + ui::builder::colors::BG_SURFACE[2] * 0.70,
                0.95,
            ];
            let cta_fill_top = [
                cta_fill[0] * 1.10,
                cta_fill[1] * 1.10,
                cta_fill[2] * 1.10,
                cta_fill[3],
            ];
            let cta_border = [
                active_accent[0] * 0.45,
                active_accent[1] * 0.45,
                active_accent[2] * 0.45,
                0.55 * breath,
            ];
            // Drop shadow for floating depth (slightly stronger)
            ui_builder.fill_shadow(
                ui::widget::Rect {
                    x: cta_x + s(2.0),
                    y: cta_y + s(3.0),
                    width: cta_w - s(4.0),
                    height: cta_h,
                },
                [0.0, 0.0, 0.0, 0.20],
                cta_r,
                s(8.0),
            );
            ui_builder.fill_rounded_gradient(cta_rect, cta_fill_top, cta_fill, cta_r);
            ui_builder.stroke_rounded(cta_rect, cta_r, 0.5, cta_border);
            // Inner top highlight for physical button depth
            ui_builder.hline_fade(
                cta_x + cta_r,
                cta_y + 1.0,
                cta_w - cta_r * 2.0,
                1.0,
                [1.0, 1.0, 1.0, 0.06],
                s(8.0),
            );
            // Plus icon + label — centered within the full-width button
            let content_w = cta_icon_sz + cta_icon_gap + cta_text_w;
            let content_x = cta_x + (cta_w - content_w) / 2.0;
            let icon_rect = ui::widget::Rect {
                x: content_x,
                y: cta_y + (cta_h - cta_icon_sz) / 2.0,
                width: cta_icon_sz,
                height: cta_icon_sz,
            };
            let icon_t = (1.0 * ui_text_handle.scale).max(0.8);
            let icon_fg = [active_accent[0], active_accent[1], active_accent[2], 0.85];
            ui_builder.icon_plus(icon_rect, icon_t, cta_icon_sz * 0.3, icon_fg);
            // Label text — brighter with more accent tint for readable CTA
            let label_fg = [
                ui::builder::colors::FG_PRIMARY[0] * 0.55 + active_accent[0] * 0.45,
                ui::builder::colors::FG_PRIMARY[1] * 0.55 + active_accent[1] * 0.45,
                ui::builder::colors::FG_PRIMARY[2] * 0.55 + active_accent[2] * 0.45,
                0.92,
            ];
            ui_builder.text_ui_scaled(
                &ui_text_handle,
                cta_label,
                content_x + cta_icon_sz + cta_icon_gap,
                cta_y + (cta_h - ch) / 2.0,
                label_fg,
                cta_fill,
                1.0,
            );
        }

        // Scrollbar (rendered before chrome so it layers under borders)
        // Hover proximity: scrollbar widens and brightens when mouse is near.
        if !reference_crop {
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
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
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
                        ui_builder.fill_shadow(
                            thumb_rect,
                            [0.0, 0.0, 0.0, shadow_alpha],
                            bar_w / 2.0,
                            s(3.0),
                        );
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
                    let thumb_top = [
                        thumb_r,
                        thumb_g,
                        thumb_b,
                        scroll_alpha * (1.0 + 0.15 * hover_t),
                    ];
                    let thumb_bottom = [
                        thumb_r,
                        thumb_g,
                        thumb_b,
                        scroll_alpha * (1.0 - 0.1 * hover_t),
                    ];
                    let base_border = if is_scrolled { 0.12 } else { 0.08 };
                    let border_alpha = base_border + 0.10 * hover_t;
                    let thumb_border = [thumb_r, thumb_g, thumb_b, border_alpha];
                    ui_builder.fill_rounded_gradient(
                        thumb_rect,
                        thumb_top,
                        thumb_bottom,
                        bar_w / 2.0,
                    );
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
                        ui_builder.fill_gradient(
                            fog_rect,
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
                        ui_builder.fill_gradient(
                            fog_rect,
                            [0.0, 0.0, 0.0, 0.10],
                            [0.0, 0.0, 0.0, 0.0],
                        );
                    }
                }
            }
        }

        // Tab bar now serves as title bar (full width at top, includes window buttons)
        self.tab_bar.sidebar_width = layout.sidebar.width;
        self.tab_bar
            .build(&mut ui_builder, layout.tab_bar, &ui_text_handle);
        self.sidebar
            .build(&mut ui_builder, layout.sidebar, &ui_text_handle);
        if !reference_crop {
            self.right_panel.build(
                &mut ui_builder,
                layout.right_panel,
                layout.right_panel_status,
                &ui_text_handle,
            );
        }

        // Resize handles — 3px invisible zones at panel boundaries, visible on hover
        if !reference_crop {
            let handle_w = 3.0 * self.scale_factor;
            let handle_y = layout.tab_bar.bottom();
            let handle_h = layout.terminal.bottom() - handle_y;
            let hover_color = ui::builder::colors::BG_HOVER;

            // Left handle (between sidebar and terminal)
            if layout.sidebar.width > 0.0 {
                let left_t = self.left_resize_anim.value();
                if left_t > 0.005 {
                    let handle_rect = ui::widget::Rect {
                        x: layout.sidebar.width - handle_w / 2.0,
                        y: handle_y,
                        width: handle_w,
                        height: handle_h,
                    };
                    ui_builder.fill(
                        handle_rect,
                        [hover_color[0], hover_color[1], hover_color[2], left_t * 0.8],
                    );
                }
            }

            // Right handle (between terminal and right panel)
            if self.right_panel.visible && layout.right_panel.width > 0.0 {
                let right_t = self.right_resize_anim.value();
                if right_t > 0.005 {
                    let handle_rect = ui::widget::Rect {
                        x: layout.right_panel.x - handle_w / 2.0,
                        y: handle_y,
                        width: handle_w,
                        height: handle_h,
                    };
                    ui_builder.fill(
                        handle_rect,
                        [
                            hover_color[0],
                            hover_color[1],
                            hover_color[2],
                            right_t * 0.8,
                        ],
                    );
                }
            }
        }

        if !reference_crop {
            self.status_bar.sidebar_width = layout.sidebar.width;
            self.status_bar.build(
                &mut ui_builder,
                layout.status_bar,
                &ui_text_handle,
                self.tab_bar.glow_phase(),
                self.active_accent(),
            );
        }

        // Progress bar — 2px gradient bar between terminal and status bar (when streaming)
        if !reference_crop && self.status_bar.streaming {
            let s = |v: f32| ui_text_handle.s(v);
            let progress_y = layout.status_bar.y - s(2.0);
            let terminal_x = layout.terminal.x;
            let terminal_w = layout.terminal.width;
            // Background track — web: backgroundColor "#1e2128"
            ui_builder.fill(
                ui::widget::Rect {
                    x: terminal_x,
                    y: progress_y,
                    width: terminal_w,
                    height: s(2.0),
                },
                [0.118, 0.129, 0.157, 1.0], // #1e2128
            );
            // Animated fill — width driven by progress_pct (0.05 → 0.90)
            // Web: linear-gradient(90deg, #6366f1, #8b5cf6, #6366f1) — 3-stop gradient
            let fill_w = terminal_w * self.progress_pct;
            ui_builder.fill_gradient_3stop_h(
                ui::widget::Rect {
                    x: terminal_x,
                    y: progress_y,
                    width: fill_w,
                    height: s(2.0),
                },
                ui::builder::colors::ACCENT_BLUE,
                ui::builder::colors::ACCENT_MAUVE,
                s(1.0), // small radius for the thin progress bar
                0.5,
            );
        }

        // Breadcrumb/path bar — thin bar between tab bar and content showing
        // the current working directory as segmented path with chevron separators.
        // Skipped when BREADCRUMB_HEIGHT is 0.
        if !reference_crop && layout.breadcrumb.height > 0.0 {
            let bc = &layout.breadcrumb;
            let s = |v: f32| ui_text_handle.s(v);
            let ch = ui_text_handle.cell_height;
            let active_accent = self.active_accent();

            // Background: subtle gradient — slightly darker at top (near tab bar)
            // fading to content-adjacent tone at bottom for smooth transition.
            // Kept very subtle (92%→98%) to avoid visible banding in the compact bar.
            let bc_bg_top = [
                ui::builder::colors::BG_BASE[0] * 0.92,
                ui::builder::colors::BG_BASE[1] * 0.92,
                ui::builder::colors::BG_BASE[2] * 0.92,
                1.0,
            ];
            let bc_bg = [
                ui::builder::colors::BG_BASE[0] * 0.98,
                ui::builder::colors::BG_BASE[1] * 0.98,
                ui::builder::colors::BG_BASE[2] * 0.98,
                1.0,
            ];
            ui_builder.fill_gradient(*bc, bc_bg_top, bc_bg);

            // Bottom separator — near-invisible hairline; the gradient background
            // difference between breadcrumb and content area provides primary separation.
            ui_builder.hline_aa(
                bc.x,
                bc.bottom() - 1.0,
                bc.width,
                1.0,
                [
                    ui::builder::colors::BORDER[0],
                    ui::builder::colors::BORDER[1],
                    ui::builder::colors::BORDER[2],
                    0.20,
                ],
            );
            // Left inner shadow for sidebar-cast depth
            ui_builder.fill_gradient_h(
                ui::widget::Rect {
                    x: bc.x,
                    y: bc.y,
                    width: s(6.0),
                    height: bc.height,
                },
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

                // Small folder icon at start (secondary color for better presence)
                ui_builder.icon_folder(
                    ui::widget::Rect {
                        x,
                        y: bc.y + (bc.height - icon_sz) / 2.0,
                        width: icon_sz,
                        height: icon_sz,
                    },
                    icon_t,
                    [
                        ui::builder::colors::FG_SECONDARY[0],
                        ui::builder::colors::FG_SECONDARY[1],
                        ui::builder::colors::FG_SECONDARY[2],
                        0.75,
                    ],
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
                let chevron_fg = [
                    ui::builder::colors::FG_MUTED[0],
                    ui::builder::colors::FG_MUTED[1],
                    ui::builder::colors::FG_MUTED[2],
                    0.62,
                ];
                let segment_fg = [
                    ui::builder::colors::FG_MUTED[0] * 0.35
                        + ui::builder::colors::FG_SECONDARY[0] * 0.65,
                    ui::builder::colors::FG_MUTED[1] * 0.35
                        + ui::builder::colors::FG_SECONDARY[1] * 0.65,
                    ui::builder::colors::FG_MUTED[2] * 0.35
                        + ui::builder::colors::FG_SECONDARY[2] * 0.65,
                    0.82,
                ];
                let last_fg = [
                    ui::builder::colors::FG_SECONDARY[0] * 0.55
                        + ui::builder::colors::FG_PRIMARY[0] * 0.45,
                    ui::builder::colors::FG_SECONDARY[1] * 0.55
                        + ui::builder::colors::FG_PRIMARY[1] * 0.45,
                    ui::builder::colors::FG_SECONDARY[2] * 0.55
                        + ui::builder::colors::FG_PRIMARY[2] * 0.45,
                    0.92,
                ];

                // SDF chevron icon dimensions — matches text line height for alignment
                let chevron_sz = ch * 0.55;
                let chevron_t = (0.8 * ui_text_handle.scale).max(0.5);

                if show_ellipsis {
                    ui_builder.text_ui(
                        &ui_text_handle,
                        "\u{2026}",
                        x,
                        y_center,
                        chevron_fg,
                        bc_bg,
                    );
                    x += ui_text_handle.text_width_ui("\u{2026}") + s(2.0);
                    let chev_rect = ui::widget::Rect {
                        x,
                        y: bc.y + (bc.height - chevron_sz) / 2.0,
                        width: chevron_sz,
                        height: chevron_sz,
                    };
                    ui_builder.icon_chevron_right(chev_rect, chevron_t, chevron_fg);
                    x += chevron_sz + s(4.0);
                }
                for (i, seg) in segments.iter().enumerate() {
                    if i > 0 {
                        // SDF chevron separator — crisp at any scale
                        let chev_rect = ui::widget::Rect {
                            x,
                            y: bc.y + (bc.height - chevron_sz) / 2.0,
                            width: chevron_sz,
                            height: chevron_sz,
                        };
                        ui_builder.icon_chevron_right(chev_rect, chevron_t, chevron_fg);
                        x += chevron_sz + s(4.0);
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
                            x: x - pill_pad,
                            y: pill_y,
                            width: seg_w + pill_pad * 2.0,
                            height: pill_h,
                        };
                        let pill_r = s(3.0);
                        // Faint accent tint in the last-segment pill for color
                        // continuity with tab bar and sidebar accent language.
                        let aa = active_accent;
                        ui_builder.fill_rounded(
                            pill_rect,
                            [
                                ui::builder::colors::BG_SURFACE[0] * 0.92 + aa[0] * 0.08,
                                ui::builder::colors::BG_SURFACE[1] * 0.92 + aa[1] * 0.08,
                                ui::builder::colors::BG_SURFACE[2] * 0.92 + aa[2] * 0.08,
                                0.38,
                            ],
                            pill_r,
                        );
                        ui_builder.stroke_rounded(
                            pill_rect,
                            pill_r,
                            0.5,
                            [
                                ui::builder::colors::BORDER[0] * 0.85 + aa[0] * 0.15,
                                ui::builder::colors::BORDER[1] * 0.85 + aa[1] * 0.15,
                                ui::builder::colors::BORDER[2] * 0.85 + aa[2] * 0.15,
                                0.18,
                            ],
                        );
                    }
                    ui_builder.text_ui(&ui_text_handle, seg, x, y_center, fg, bc_bg);
                    x += ui_text_handle.text_width_ui(seg) + s(4.0);
                }
            }
        }
        // Window outer border — multi-layer shadow + border for professional depth.
        // When maximized, shadows are invisible (window fills screen) so skip them
        // to save GPU work.  Borders and accent top edge still render for polish.
        if !reference_crop {
            let r = if self.is_maximized {
                0.0
            } else {
                ui_text_handle.s(3.0)
            };
            let full = ui::widget::Rect {
                x: 0.0,
                y: 0.0,
                width: vw,
                height: vh,
            };

            if !self.is_maximized {
                // Two-layer shadow: far shadow (wide, faint) + near shadow (tight, darker).
                let far_shadow = ui::widget::Rect {
                    x: -2.0,
                    y: 0.0,
                    width: vw + 4.0,
                    height: vh + 4.0,
                };
                ui_builder.fill_shadow(
                    far_shadow,
                    [0.0, 0.0, 0.0, 0.12],
                    r + 2.0,
                    ui_text_handle.s(10.0),
                );
                ui_builder.fill_shadow(full, [0.0, 0.0, 0.0, 0.30], r, ui_text_handle.s(3.0));
                // Outer border: darker edge against desktop
                ui_builder.stroke_rounded(full, r, 1.0, [0.05, 0.05, 0.08, 0.9]);
                // Inner highlight: subtle bright edge just inside for depth
                let inner = ui::widget::Rect {
                    x: 1.0,
                    y: 1.0,
                    width: vw - 2.0,
                    height: vh - 2.0,
                };
                ui_builder.stroke_rounded(inner, r.max(1.0) - 1.0, 1.0, [1.0, 1.0, 1.0, 0.04]);
            }

            // Accent-tinted top edge: picks up the active tab's accent color.
            // 2px height for visibility; stronger alpha when focused for a
            // prominent colored "brand bar" at the top of the window (like VS Code).
            let active_accent = self.active_accent();
            let breath = 0.92 + 0.08 * self.tab_bar.glow_phase().sin();
            let accent_alpha = if self.window_focused {
                0.20 * breath
            } else {
                0.06
            };
            let accent_fade = ui_text_handle.s(40.0);
            let accent_h = if self.is_maximized { 2.0 } else { 2.0 };
            let accent_full = [
                active_accent[0],
                active_accent[1],
                active_accent[2],
                accent_alpha,
            ];
            let accent_zero = [active_accent[0], active_accent[1], active_accent[2], 0.0];
            let top_w = vw - r * 2.0;
            ui_builder.fill_gradient_h(
                ui::widget::Rect {
                    x: r,
                    y: 0.0,
                    width: accent_fade,
                    height: accent_h,
                },
                accent_zero,
                accent_full,
            );
            ui_builder.fill(
                ui::widget::Rect {
                    x: r + accent_fade,
                    y: 0.0,
                    width: top_w - accent_fade * 2.0,
                    height: accent_h,
                },
                accent_full,
            );
            ui_builder.fill_gradient_h(
                ui::widget::Rect {
                    x: vw - r - accent_fade,
                    y: 0.0,
                    width: accent_fade,
                    height: accent_h,
                },
                accent_full,
                accent_zero,
            );
            // Glow spill below the accent bar for soft light emission
            let glow_below = [
                active_accent[0],
                active_accent[1],
                active_accent[2],
                accent_alpha * 0.3,
            ];
            let glow_below_zero = [active_accent[0], active_accent[1], active_accent[2], 0.0];
            ui_builder.fill_gradient(
                ui::widget::Rect {
                    x: r + accent_fade,
                    y: accent_h,
                    width: top_w - accent_fade * 2.0,
                    height: ui_text_handle.s(4.0),
                },
                glow_below,
                glow_below_zero,
            );
        }

        let (chrome_quads, text_commands) = ui_builder.finish();

        // Prepare atlas pipeline with terminal grid + UI text
        if let Some(renderer) = &mut self.renderer {
            renderer.prepare(
                &gpu.device,
                &gpu.queue,
                if reference_crop {
                    None
                } else {
                    self.current_grid.as_ref()
                },
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
                        // GitHub Dark BG_DARK (#0b0d12) in linear space
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0015,
                            g: 0.0021,
                            b: 0.0042,
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
                            let first_row_px =
                                (sel_start.row as f32 * ch).round() + layout.terminal_content.y;
                            let last_row_py =
                                ((sel_end.row + 1) as f32 * ch).round() + layout.terminal_content.y;
                            let bbox_x = layout.terminal_content.x;
                            let bbox_w = layout.terminal_content.width;
                            let bbox_h = last_row_py - first_row_px;
                            if bbox_h > 0.0 {
                                sel_verts.extend_from_slice(&ui::quad_renderer::quad_vertices_sdf(
                                    bbox_x,
                                    first_row_px,
                                    bbox_w,
                                    bbox_h,
                                    vw,
                                    vh,
                                    [accent[0], accent[1], accent[2], 0.04],
                                    [radius; 4],
                                    0.0,
                                    [0.0; 4],
                                    6.0 * self.scale_factor,
                                    0.0,
                                    0.0,
                                ));
                            }
                        }

                        for row in sel_start.row..=sel_end.row {
                            if row >= grid.rows.len() {
                                break;
                            }

                            // Determine column range for this row
                            let col_start = if row == sel_start.row {
                                sel_start.col
                            } else {
                                0
                            };
                            let col_end = if row == sel_end.row {
                                sel_end.col
                            } else {
                                cols.saturating_sub(1)
                            };
                            if col_start > col_end {
                                continue;
                            }

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

                            sel_verts.extend_from_slice(&ui::quad_renderer::quad_vertices_sdf(
                                px, py, pw, ph, vw, vh, sel_color, radii, 0.5, sel_border, 0.0, 0.0, 0.0,
                            ));
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
                            let cpx =
                                (grid.cursor.col as f32 * cw).round() + layout.terminal_content.x;
                            let cpy =
                                (grid.cursor.row as f32 * ch).round() + layout.terminal_content.y;

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

                                let is_block = matches!(
                                    grid.cursor.cursor_style,
                                    CursorShape::BlinkBlock | CursorShape::SteadyBlock
                                );

                                let mut cursor_verts = Vec::new();

                                if focused {
                                    // Focused cursor: accent-tinted body with gradient for 3D depth.
                                    // Blends white toward the active tab accent for visual coherence
                                    // with selection highlights, glow, and tab chrome.
                                    let accent_blend = 0.15;
                                    let base_r =
                                        1.0 * (1.0 - accent_blend) + accent[0] * accent_blend;
                                    let base_g =
                                        1.0 * (1.0 - accent_blend) + accent[1] * accent_blend;
                                    let base_b =
                                        1.0 * (1.0 - accent_blend) + accent[2] * accent_blend;
                                    let base_a = 0.85 * blink_t;

                                    // Glow behind cursor (accent-colored Gaussian emission)
                                    let glow_color =
                                        [accent[0], accent[1], accent[2], 0.14 * blink_t];
                                    cursor_verts.extend_from_slice(
                                        &ui::quad_renderer::quad_vertices_sdf(
                                            cx,
                                            cy,
                                            cwidth,
                                            cheight,
                                            vw,
                                            vh,
                                            glow_color,
                                            [radius; 4],
                                            0.0,
                                            [0.0; 4],
                                            4.0 * self.scale_factor,
                                            0.0,
                                            0.0,
                                        ),
                                    );

                                    // Cursor body: SDF gradient (brighter top → slightly darker bottom)
                                    // for consistent 3D depth with the rest of the UI chrome.
                                    let cursor_top = [base_r, base_g, base_b, base_a];
                                    let cursor_bot =
                                        [base_r * 0.92, base_g * 0.92, base_b * 0.92, base_a];
                                    cursor_verts.extend_from_slice(
                                        &ui::quad_renderer::quad_vertices_sdf_gradient(
                                            cx,
                                            cy,
                                            cwidth,
                                            cheight,
                                            vw,
                                            vh,
                                            cursor_top,
                                            cursor_bot,
                                            [radius; 4],
                                            0.0,
                                            [0.0; 4],
                                            0.0,
                                            0.0,
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
                                    let glow_color =
                                        [accent[0], accent[1], accent[2], 0.06 * blink_t];
                                    cursor_verts.extend_from_slice(
                                        &ui::quad_renderer::quad_vertices_sdf(
                                            cx,
                                            cy,
                                            cwidth,
                                            cheight,
                                            vw,
                                            vh,
                                            glow_color,
                                            [radius; 4],
                                            0.0,
                                            [0.0; 4],
                                            3.0 * self.scale_factor,
                                            0.0,
                                            0.0,
                                        ),
                                    );
                                    // Hollow outline (transparent fill + border)
                                    cursor_verts.extend_from_slice(
                                        &ui::quad_renderer::quad_vertices_sdf(
                                            cx,
                                            cy,
                                            cwidth,
                                            cheight,
                                            vw,
                                            vh,
                                            [0.0, 0.0, 0.0, 0.0],
                                            [radius; 4],
                                            outline_w,
                                            outline_color,
                                            0.0,
                                            0.0,
                                            0.0,
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
                                            cx,
                                            cy,
                                            cwidth,
                                            cheight,
                                            vw,
                                            vh,
                                            dim_color,
                                            [radius; 4],
                                            0.0,
                                            [0.0; 4],
                                            0.0,
                                            0.0,
                                            0.0,
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
                        0.0,
                        0.0,
                        vw,
                        vh,
                        vw,
                        vh,
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
                        vw - vig_w,
                        0.0,
                        vig_w,
                        vh,
                        vw,
                        vh,
                        vig_zero,
                        vig_full,
                    ));
                    // Top edge
                    dim_verts.extend_from_slice(&ui::quad_renderer::quad_vertices_gradient(
                        0.0, 0.0, vw, vig_h, vw, vh, vig_full, vig_zero,
                    ));
                    // Bottom edge
                    dim_verts.extend_from_slice(&ui::quad_renderer::quad_vertices_gradient(
                        0.0,
                        vh - vig_h,
                        vw,
                        vig_h,
                        vw,
                        vh,
                        vig_zero,
                        vig_full,
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

        if !self.is_web_reference_crop() {
            self.connect_daemon();
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AsyncEvent) {
        match event {
            AsyncEvent::Heartbeat => {
                // Re-fetch grid periodically to catch output we missed
                if !self.is_web_reference_crop() && self.active_session.is_some() {
                    self.fetch_grid();
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            AsyncEvent::TerminalOutput { .. } => {
                log::debug!("TerminalOutput event — fetching grid");
                if !self.is_web_reference_crop() {
                    self.fetch_grid();
                }
            }
            AsyncEvent::GridFetched { grid, .. } => {
                // Log first row content for debugging
                if let Some(row) = grid.rows.first() {
                    let text: String = row.cells.iter().map(|c| c.content.as_str()).collect();
                    let trimmed = text.trim_end();
                    if !trimmed.is_empty() {
                        log::info!(
                            "GridFetched: {}x{}, first row: {:?}",
                            grid.dimensions.cols,
                            grid.dimensions.rows,
                            &trimmed[..trimmed.len().min(80)]
                        );
                    } else {
                        log::info!(
                            "GridFetched: {}x{}, first row empty",
                            grid.dimensions.cols,
                            grid.dimensions.rows
                        );
                    }
                }
                if !self.is_web_reference_crop() {
                    self.current_grid = Some(*grid);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
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

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                // Track maximized state to skip outer shadow rendering
                if let Some(w) = &self.window {
                    self.is_maximized = w.is_maximized();
                    self.scale_factor = w.scale_factor() as f32;
                }
                if let Some(gpu) = &mut self.gpu {
                    // Use winit's reported size as the authoritative surface
                    // dimension. The GetClientRect-based hwnd-client size can
                    // become stale for borderless windows when un-maximising,
                    // causing the render surface to be larger than the visible
                    // window and pushing bottom-anchored UI (agent panel,
                    // status bar) off screen.
                    let surface_size = size;
                    gpu.config.width = surface_size.width.max(1);
                    gpu.config.height = surface_size.height.max(1);
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
                self.status_bar.window_focused = focused;
                self.focus_dim_anim.set(if focused { 0.0 } else { 1.0 });
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
                let ui_scale = self.ui_scale();
                let gpu = self.gpu.as_ref();
                let (vw, vh) = gpu
                    .map(|g| (g.config.width as f32, g.config.height as f32))
                    .unwrap_or((1200.0, 800.0));
                let layout = self.shell_layout(vw, vh);

                // Resize handle dragging
                if self.left_resize_dragging {
                    let min_w = 150.0 * ui_scale;
                    let max_w = 320.0 * ui_scale;
                    self.sidebar_width = (px / ui_scale).clamp(min_w / ui_scale, max_w / ui_scale);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }
                if self.right_resize_dragging {
                    let min_w = 250.0 * ui_scale;
                    let max_w = 550.0 * ui_scale;
                    let new_w = (vw - px) / ui_scale;
                    self.right_panel_width = new_w.clamp(min_w / ui_scale, max_w / ui_scale);
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }

                // Resize handle hover detection (3px zones at panel boundaries)
                let handle_zone = 5.0 * ui_scale;
                let handle_y_min = layout.tab_bar.bottom();
                let handle_y_max = layout.status_bar.y;
                let in_handle_y = py >= handle_y_min && py <= handle_y_max;

                let left_edge = layout.sidebar.width;
                self.left_resize_hover = in_handle_y
                    && layout.sidebar.width > 0.0
                    && (px - left_edge).abs() < handle_zone;

                let right_edge = layout.right_panel.x;
                self.right_resize_hover = in_handle_y
                    && self.right_panel.visible
                    && layout.right_panel.width > 0.0
                    && (px - right_edge).abs() < handle_zone;

                // Set cursor icon
                if let Some(w) = &self.window {
                    if self.left_resize_hover || self.right_resize_hover {
                        w.set_cursor(winit::window::CursorIcon::EwResize);
                    } else {
                        w.set_cursor(winit::window::CursorIcon::Default);
                    }
                }

                // Route mouse to UI chrome
                let me = ui::widget::MouseEvent::Move { x: px, y: py };
                self.tab_bar.on_mouse(me, layout.tab_bar, ui_scale);
                self.sidebar.on_mouse(me, layout.sidebar, ui_scale);
                if let Some(renderer) = &self.renderer {
                    let m = if self.is_web_reference_crop() {
                        *renderer.font_metrics()
                    } else {
                        renderer.font_metrics().scaled_for_render()
                    };
                    let mut ui_text = ui::builder::UiTextRenderer::new(
                        m.cell_width,
                        m.cell_height,
                        m.font_size,
                        ui_scale,
                    );
                    ui_text.ui_avg_advance = renderer.ui_avg_advance();
                    ui_text.layout_engine = self.ui_text_layout.clone();
                    ui_text.raster_scale = self.ui_raster_scale();
                    self.status_bar.sidebar_width = layout.sidebar.width;
                    self.status_bar.on_mouse(me, layout.status_bar, &ui_text);
                    self.right_panel.on_mouse(me, layout.right_panel, &ui_text);
                }

                // Selection drag in terminal area
                if self.selection.active && layout.terminal.contains(px, py) {
                    let pos = self.pixel_to_grid(
                        (px - layout.terminal.x) as f64,
                        (py - layout.terminal.y) as f64,
                    );
                    self.selection.update(pos);
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                use winit::event::MouseButton;
                if button == MouseButton::Left {
                    if let Some((x, y)) = self.mouse_position {
                        let (px, py) = (x as f32, y as f32);
                        let ui_scale = self.ui_scale();
                        let gpu = self.gpu.as_ref();
                        let (vw, vh) = gpu
                            .map(|g| (g.config.width as f32, g.config.height as f32))
                            .unwrap_or((1200.0, 800.0));
                        let layout = self.shell_layout(vw, vh);

                        if state == ElementState::Pressed {
                            // Check resize handles first (highest priority for drag start)
                            if self.left_resize_hover {
                                self.left_resize_dragging = true;
                                return;
                            }
                            if self.right_resize_hover {
                                self.right_resize_dragging = true;
                                return;
                            }

                            // Check tab bar (which includes window buttons + drag)
                            let me = ui::widget::MouseEvent::Press { x: px, y: py };
                            if let Some(action) =
                                self.tab_bar.on_mouse(me, layout.tab_bar, ui_scale)
                            {
                                self.handle_ui_action(action);
                                return;
                            }
                            // Check right panel close button
                            if let Some(renderer) = &self.renderer {
                                let m = if self.is_web_reference_crop() {
                                    *renderer.font_metrics()
                                } else {
                                    renderer.font_metrics().scaled_for_render()
                                };
                                let mut ui_text = ui::builder::UiTextRenderer::new(
                                    m.cell_width,
                                    m.cell_height,
                                    m.font_size,
                                    ui_scale,
                                );
                                ui_text.ui_avg_advance = renderer.ui_avg_advance();
                                ui_text.layout_engine = self.ui_text_layout.clone();
                                ui_text.raster_scale = self.ui_raster_scale();
                                if let Some(ui::right_panel::RightPanelAction::Close) =
                                    self.right_panel.on_mouse(me, layout.right_panel, &ui_text)
                                {
                                    self.right_panel.visible = false;
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                    return;
                                }
                            }
                        }

                        // Release resize drag on mouse up
                        if state == ElementState::Released {
                            if self.left_resize_dragging || self.right_resize_dragging {
                                self.left_resize_dragging = false;
                                self.right_resize_dragging = false;
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
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
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
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
                    log::info!(
                        "KEY: {:?} mods={:?}",
                        event.logical_key,
                        self.modifiers.state()
                    );
                    let mods = self.modifiers.state();
                    let adapter_mods = convert_modifiers(mods);
                    let adapter_key = convert_key(&event.logical_key);

                    // Check app shortcuts first
                    if let Some(action) =
                        godly_app_adapter::shortcuts::check_app_shortcut(&adapter_key, adapter_mods)
                    {
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
                                            let _ = daemon.send_request(&Request::CloseSession {
                                                session_id: id,
                                            });
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
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn convert_modifiers(
    state: winit::keyboard::ModifiersState,
) -> godly_app_adapter::keyboard::Modifiers {
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

struct TerminalFontSetup {
    family: String,
    font_metrics: FontMetrics,
    rasterizer: Box<dyn godly_terminal_surface::glyph_rasterizer::GlyphRasterizer>,
}

#[cfg(windows)]
fn create_terminal_font_setup(font_size: f32, scale_factor: f32) -> TerminalFontSetup {
    use godly_terminal_surface::directwrite_rasterizer::DirectWriteRasterizer;
    use godly_terminal_surface::glyph_rasterizer::GlyphRasterizer;
    use godly_terminal_surface::swash_rasterizer::SwashRasterizer;

    for family in TERMINAL_FONT_CANDIDATES {
        match DirectWriteRasterizer::new() {
            Ok(mut dw) => {
                if dw.load_system_font(family).is_ok() {
                    return TerminalFontSetup {
                        family: (*family).to_string(),
                        font_metrics: FontMetrics::from_system_font(font_size, family)
                            .with_scale_factor(scale_factor),
                        rasterizer: Box::new(dw),
                    };
                }
            }
            Err(e) => {
                log::warn!("[FONT] DirectWrite init failed for terminal font {family} ({e:?})");
                break;
            }
        }
    }

    log::warn!(
        "[FONT] No browser-matching terminal font available from candidates: {}. Falling back to bundled Geist Mono.",
        TERMINAL_FONT_CANDIDATES.join(", ")
    );
    let font_data: &[u8] = include_bytes!("../../iced-shell/fonts/GeistMono-Regular.ttf");
    let mut rasterizer = SwashRasterizer::new();
    rasterizer.load_font(font_data, 0);
    TerminalFontSetup {
        family: "Geist Mono".into(),
        font_metrics: FontMetrics::from_font_bytes(font_size, font_data)
            .with_scale_factor(scale_factor),
        rasterizer: Box::new(rasterizer),
    }
}

#[cfg(not(windows))]
fn create_terminal_font_setup(font_size: f32, scale_factor: f32) -> TerminalFontSetup {
    use godly_terminal_surface::glyph_rasterizer::GlyphRasterizer;
    let font_data: &[u8] = include_bytes!("../../iced-shell/fonts/GeistMono-Regular.ttf");
    let mut r = godly_terminal_surface::swash_rasterizer::SwashRasterizer::new();
    r.load_font(font_data, 0);
    TerminalFontSetup {
        family: "Geist Mono".into(),
        font_metrics: FontMetrics::from_font_bytes(font_size, font_data)
            .with_scale_factor(scale_factor),
        rasterizer: Box::new(r),
    }
}

struct UiFontBundle {
    family: String,
    rasterizer: Box<dyn godly_terminal_surface::glyph_rasterizer::GlyphRasterizer>,
}

/// Composite rasterizer that falls back to a secondary font when the primary
/// font does not contain a glyph. Used to supplement Segoe UI Variable with
/// Segoe UI Symbol for emoji/symbol characters like ⚡.
struct FallbackRasterizer {
    primary: Box<dyn godly_terminal_surface::glyph_rasterizer::GlyphRasterizer>,
    fallback: Box<dyn godly_terminal_surface::glyph_rasterizer::GlyphRasterizer>,
}

impl godly_terminal_surface::glyph_rasterizer::GlyphRasterizer for FallbackRasterizer {
    fn rasterize(
        &mut self,
        ch: char,
        font_size_px: f32,
        weight: u16,
        italic: bool,
    ) -> Option<godly_terminal_surface::glyph_rasterizer::RasterizedGlyph> {
        self.primary
            .rasterize(ch, font_size_px, weight, italic)
            .or_else(|| self.fallback.rasterize(ch, font_size_px, weight, italic))
    }

    fn measure(
        &mut self,
        font_size_px: f32,
    ) -> godly_terminal_surface::glyph_rasterizer::MeasuredFontMetrics {
        self.primary.measure(font_size_px)
    }

    fn has_glyph(&self, ch: char) -> bool {
        self.primary.has_glyph(ch) || self.fallback.has_glyph(ch)
    }

    fn load_font(&mut self, data: &[u8], index: u32) -> bool {
        self.primary.load_font(data, index)
    }

    fn set_scale_factor(&mut self, scale: f32) {
        self.primary.set_scale_factor(scale);
        self.fallback.set_scale_factor(scale);
    }
}

#[cfg(windows)]
fn create_ui_font_bundle(candidates: &[&str], label: &str) -> Option<UiFontBundle> {
    use godly_terminal_surface::directwrite_rasterizer::DirectWriteRasterizer;

    for family in candidates {
        let mut dw = DirectWriteRasterizer::new_grayscale().ok()?;
        if dw.load_system_font(family).is_ok() {
            log::info!("[FONT] {label}: {family}");
            return Some(UiFontBundle {
                family: (*family).to_string(),
                rasterizer: Box::new(dw),
            });
        }
    }
    log::warn!(
        "[FONT] No {label} font available from candidates: {}",
        candidates.join(", ")
    );
    None
}

#[cfg(windows)]
fn create_ui_sans_font() -> Option<UiFontBundle> {
    use godly_terminal_surface::directwrite_rasterizer::DirectWriteRasterizer;

    let primary =
        create_ui_font_bundle(&["Segoe UI Variable", "Segoe UI"], "UI sans font (primary)")?;

    // Try to load a symbol font as fallback for emoji/symbol glyphs (e.g. ⚡)
    // that are missing from the primary UI font.
    let fallback_families = ["Segoe UI Symbol", "Segoe UI Emoji"];
    for family in fallback_families {
        let mut dw = match DirectWriteRasterizer::new_grayscale() {
            Ok(dw) => dw,
            Err(_) => continue,
        };
        if dw.load_system_font(family).is_ok() {
            log::info!("[FONT] UI sans fallback: {family}");
            return Some(UiFontBundle {
                family: primary.family,
                rasterizer: Box::new(FallbackRasterizer {
                    primary: primary.rasterizer,
                    fallback: Box::new(dw),
                }),
            });
        }
    }

    // No fallback available — use primary alone.
    Some(primary)
}

#[cfg(windows)]
fn create_ui_serif_font() -> Option<UiFontBundle> {
    create_ui_font_bundle(&["Georgia", "Cambria", "Times New Roman"], "UI serif font")
}

#[cfg(windows)]
fn create_ui_mono_font(family: &str) -> Option<UiFontBundle> {
    use godly_terminal_surface::directwrite_rasterizer::DirectWriteRasterizer;

    let mut dw = DirectWriteRasterizer::new_grayscale().ok()?;
    if dw.load_system_font(family).is_ok() {
        Some(UiFontBundle {
            family: family.to_string(),
            rasterizer: Box::new(dw),
        })
    } else {
        log::warn!("[FONT] No UI mono font available for {}", family);
        None
    }
}

#[cfg(not(windows))]
fn create_ui_sans_font() -> Option<UiFontBundle> {
    None
}

#[cfg(not(windows))]
fn create_ui_serif_font() -> Option<UiFontBundle> {
    None
}

#[cfg(not(windows))]
fn create_ui_mono_font(_family: &str) -> Option<UiFontBundle> {
    None
}

#[cfg(windows)]
fn create_ui_text_layout_engine(
    families: ui::text_layout::UiFontFamilies,
) -> Option<ui::text_layout::UiTextLayoutEngine> {
    ui::text_layout::UiTextLayoutEngine::new(families).ok()
}

#[cfg(not(windows))]
fn create_ui_text_layout_engine(
    _families: ui::text_layout::UiFontFamilies,
) -> Option<ui::text_layout::UiTextLayoutEngine> {
    None
}

fn window_surface_size(window: &Window) -> winit::dpi::PhysicalSize<u32> {
    #[cfg(windows)]
    if let Some(size) = win32_client_size(window) {
        return size;
    }

    window.inner_size()
}

#[cfg(windows)]
fn win32_client_size(window: &Window) -> Option<winit::dpi::PhysicalSize<u32>> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    let handle = window.window_handle().ok()?;
    let hwnd = match handle.as_raw() {
        RawWindowHandle::Win32(handle) => HWND(handle.hwnd.get() as *mut core::ffi::c_void),
        _ => return None,
    };

    let mut rect = RECT::default();
    unsafe {
        if GetClientRect(hwnd, &mut rect).is_err() {
            return None;
        }
    }

    let width = (rect.right - rect.left).max(0) as u32;
    let height = (rect.bottom - rect.top).max(0) as u32;
    Some(winit::dpi::PhysicalSize::new(width.max(1), height.max(1)))
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let event_loop = EventLoop::<AsyncEvent>::with_user_event().build().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let mut app = App::new(proxy, scene_mode_from_env_and_args());
    event_loop.run_app(&mut app).unwrap();
}

fn scene_mode_from_env_and_args() -> SceneMode {
    let cli_reference = std::env::args().any(|arg| arg == "--web-reference-crop");
    let env_reference = matches!(
        std::env::var("GODLY_SHELL_REFERENCE_MODE").as_deref(),
        Ok("web_reference_crop") | Ok("web-reference-crop") | Ok("1") | Ok("true")
    );

    if cli_reference || env_reference {
        SceneMode::WebReferenceCrop
    } else {
        SceneMode::LiveShell
    }
}
