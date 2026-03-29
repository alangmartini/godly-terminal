# Drop Iced: Migrate to winit + wgpu + taffy

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Iced UI framework with a thin custom shell built on winit (event loop / window), wgpu (GPU rendering), and taffy (flexbox layout), reusing 91% of the existing rendering code and 100% of the core state machine crates.

**Architecture:** The new `godly-shell` crate owns the Win32 window via winit, runs its own event loop, manages a retained widget tree laid out by taffy, and renders everything through the existing wgpu glyph atlas pipeline. All core state machines (tabs, layout, workspaces, features-shell) and the daemon IPC layer are reused unchanged.

**Tech Stack:** winit 0.30+, wgpu 27, taffy 0.7+, cosmic-text (for UI text shaping), existing DirectWrite/swash rasterizers

---

## Context for the Implementing Agent

### Why This Migration

Iced has five fundamental problems that cannot be fixed without owning the platform layer:

1. **Event loop dies on minimize** (Windows) — Iced stops polling subscriptions when the window is invisible. Current workaround: Win32 `SetTimer` hack + heartbeat daemon thread.
2. **Async waker broken on Windows** — `Task::perform` doesn't reliably wake the Win32 event loop. Current workaround: bypass entirely with manual `PostMessageW(hwnd, WM_APP, ...)`.
3. **Focus detection unreliable** — Iced's `Focused`/`Unfocused` events fire incorrectly. Current workaround: raw `GetForegroundWindow()` calls.
4. **No zero-copy rendering path** — Terminal frames must be cloned through `image::Handle::from_rgba()`. The GPU atlas shader works around this but still goes through Iced's `Shader` widget abstraction.
5. **Split pane state complexity** — Iced's Elm architecture (Model/Message/Update → full view rebuild) makes layout tree management error-prone. 15+ split-related bugs trace to this.

### What Stays (Copy As-Is, Zero Changes)

These crates have **zero Iced dependencies** and are pure state machines:

| Crate | Path | Purpose |
|-------|------|---------|
| `godly-tabs-core` | `src-tauri/native/tabs-core/` | Tab ordering, active tab selection |
| `godly-layout-core` | `src-tauri/native/layout-core/` | Binary tree split layout (LayoutNode, SplitDirection, FocusDirection) |
| `godly-workspaces-core` | `src-tauri/native/workspaces-core/` | Workspace collection (generic over layout type `L`) |
| `godly-features-shell` | `src-tauri/native/features-shell/` | Pure reducer functions for layout/tabs/workspaces |
| `godly-protocol` | `src-tauri/protocol/` | IPC message types (Request, Response, DaemonMessage, RichGridData) |
| `godly-ports` | `src-tauri/native/ports/` | Side-effect trait definitions (DaemonPort, ClipboardPort, etc.) |

These terminal-surface files have **zero Iced dependencies**:

| File | LOC | Purpose |
|------|-----|---------|
| `glyph_cache.rs` | 301 | HashMap glyph deduplication |
| `glyph_atlas.rs` | 293 | GPU texture atlas packing |
| `glyph_rasterizer.rs` | 52 | Abstract rasterizer trait |
| `font_loader.rs` | 41 | fontdb system font lookup |
| `font_metrics.rs` | 372 | Font table parsing, cell sizing |
| `swash_rasterizer.rs` | 249 | CPU grayscale rasterizer |
| `directwrite_rasterizer.rs` | 548 | Windows ClearType RGB rasterizer |
| `render_stats.rs` | 52 | Timing metrics |

These files need only `iced::Color` replaced (1-line change each):

| File | LOC | Iced Usage |
|------|-----|------------|
| `pixel_renderer.rs` | 1056 | `use iced::Color;` on line 5 |
| `atlas_vertex_builder.rs` | 249 | `use iced::Color;` on line 4 |
| `colors.rs` | 105 | `use iced::Color;` on line 1 |

### What Gets Extracted (Keep Logic, Replace Iced Traits)

These files implement Iced's `shader::Pipeline/Primitive/Program` traits but the underlying wgpu code is 85% reusable:

| File | LOC | Iced Traits | Reusable wgpu Code |
|------|-----|-------------|-------------------|
| `atlas_shader.rs` | 404 | `shader::Pipeline`, `shader::Primitive`, `shader::Program` | WGSL shader (lines 36-96), pipeline creation, vertex/texture upload, draw calls |
| `shader_surface.rs` | 334 | Same 3 traits | WGSL shader (lines 44-67), fullscreen quad pipeline |

### What Gets Deleted

| File | Reason |
|------|--------|
| `surface.rs` (311 lines) | Canvas CPU fallback — going GPU-only |
| Entire `iced-shell/` crate | Replaced by `godly-shell/` |

### What Gets Rewritten

The `iced-shell/` crate (app.rs alone is 12,000+ lines) is replaced by a new `godly-shell/` crate. However, most of the *logic* in iced-shell is reusable — the Message enum variants map to event handler functions, and the view() widget tree maps to the new widget tree. The rewrite is structural (new framework glue), not logical (new behavior).

### Daemon Communication

The daemon runs as a separate process. Communication is via Windows named pipes:
- Pipe: `\\.\pipe\godly-terminal-daemon{-suffix}`
- Protocol: JSON for control messages, binary-tagged frames for high-frequency data (GridDiff, Output)
- Client: `NativeDaemonClient` in `godly-app-adapter/src/daemon_client.rs`
- Events: `FrontendEventSink` trait with callbacks: `on_terminal_output`, `on_session_closed`, `on_process_changed`, `on_grid_diff`, `on_bell`
- The daemon, protocol, and client code are all reusable. Only the `ChannelEventSink` implementation (which routes events into Iced's subscription system) needs replacement.

### Key Types You'll Work With

```rust
// From godly-protocol — terminal grid data
pub struct RichGridData {
    pub rows: Vec<RichRow>,
    pub cursor: CursorInfo,
    pub cols: usize,
    pub rows_count: usize,
    // ... attributes, colors, etc.
}

// From godly-layout-core — split tree
pub enum LayoutNode {
    Leaf { terminal_id: String },
    ContentPane { content: PaneContent },
    Split { direction: SplitDirection, ratio: f32, first: Box<LayoutNode>, second: Box<LayoutNode> },
}

// From godly-workspaces-core — workspace state
pub struct WorkspaceCollection<L> {
    workspaces: Vec<WorkspaceInfo<L>>,
    active_id: Option<String>,
}

// From godly-tabs-core — tab state
pub struct TabState {
    order: Vec<String>,
    active_id: Option<String>,
}
```

### Build & Run

```bash
# From src-tauri/
cargo build -p godly-shell --release

# Run (daemon must be running or will auto-launch)
cargo run -p godly-shell
```

The workspace Cargo.toml is at `src-tauri/Cargo.toml`. Add the new crate as a workspace member.

---

## Phase 0: Decouple terminal-surface from Iced

**Goal:** Remove `iced` from `terminal-surface`'s Cargo.toml so it can be used by any renderer.

### Task 0.1: Define a framework-agnostic Color type

**Files:**
- Create: `src-tauri/native/terminal-surface/src/color.rs`
- Modify: `src-tauri/native/terminal-surface/src/lib.rs`

- [ ] **Step 1: Create `color.rs` with a simple Color struct**

```rust
/// Framework-agnostic RGBA color (0.0–1.0 per channel).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: Self = Self { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const TRANSPARENT: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_rgba8(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a,
        }
    }

    pub fn into_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}
```

- [ ] **Step 2: Export from lib.rs**

Add `pub mod color;` and `pub use color::Color;` to `src-tauri/native/terminal-surface/src/lib.rs`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/native/terminal-surface/src/color.rs src-tauri/native/terminal-surface/src/lib.rs
git commit -m "refactor: add framework-agnostic Color type to terminal-surface"
```

### Task 0.2: Replace `iced::Color` in terminal-surface files

**Files:**
- Modify: `src-tauri/native/terminal-surface/src/colors.rs` (line 1)
- Modify: `src-tauri/native/terminal-surface/src/pixel_renderer.rs` (line 5)
- Modify: `src-tauri/native/terminal-surface/src/atlas_vertex_builder.rs` (line 4)

- [ ] **Step 1: Replace `use iced::Color;` with `use crate::Color;` in all three files**

In each file, change:
```rust
use iced::Color;
```
to:
```rust
use crate::Color;
```

Check that `Color` field access patterns match (both `iced::Color` and our `Color` use `.r`, `.g`, `.b`, `.a` fields). If `iced::Color` was constructed with `Color::from_rgba()` or `Color { r, g, b, a }`, verify the new type supports the same patterns.

- [ ] **Step 2: Search for any other `iced::Color` usage in terminal-surface**

```bash
cd src-tauri && grep -rn "iced::Color\|use iced" native/terminal-surface/src/ --include="*.rs"
```

Fix any remaining references. The only files that should still reference `iced` after this are `surface.rs`, `atlas_shader.rs`, and `shader_surface.rs` (which get handled in Task 0.3).

- [ ] **Step 3: Verify it compiles**

```bash
cd src-tauri && cargo check -p godly-terminal-surface 2>&1 | head -30
```

This will fail because `atlas_shader.rs`, `shader_surface.rs`, and `surface.rs` still import Iced. That's expected — they're addressed next.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor: replace iced::Color with crate::Color in terminal-surface"
```

### Task 0.3: Extract GPU pipelines from Iced shader traits

**Files:**
- Modify: `src-tauri/native/terminal-surface/src/atlas_shader.rs`
- Modify: `src-tauri/native/terminal-surface/src/shader_surface.rs`
- Delete: `src-tauri/native/terminal-surface/src/surface.rs`
- Modify: `src-tauri/native/terminal-surface/src/lib.rs`
- Modify: `src-tauri/native/terminal-surface/Cargo.toml`

The goal is to replace Iced's `shader::Pipeline/Primitive/Program` trait impls with standalone structs that directly own wgpu resources and expose simple `new()`, `prepare()`, `draw()` methods.

- [ ] **Step 1: Refactor `atlas_shader.rs` — remove Iced traits**

The file currently has three Iced trait impls. Replace them with plain methods:

**Current structure (Iced-dependent):**
```rust
impl shader::Pipeline for AtlasPipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self { ... }
}
impl shader::Primitive for AtlasPrimitive {
    fn prepare(&self, pipeline: &mut AtlasPipeline, device: &wgpu::Device, queue: &wgpu::Queue, ...) { ... }
    fn draw(&self, pipeline: &AtlasPipeline, render_pass: &mut wgpu::RenderPass<'_>) { ... }
}
impl<Message> shader::Program<Message> for AtlasShaderProgram {
    fn draw(&self, ...) -> Self::Primitive { ... }
}
```

**New structure (standalone):**
```rust
impl AtlasPipeline {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self { ... }
    // Same body as shader::Pipeline::new()
}

impl AtlasPrimitive {
    pub fn prepare(&self, pipeline: &mut AtlasPipeline, device: &wgpu::Device, queue: &wgpu::Queue) { ... }
    // Same body as shader::Primitive::prepare(), remove `bounds`/`viewport` params not used
    pub fn draw(&self, pipeline: &AtlasPipeline, render_pass: &mut wgpu::RenderPass<'_>) { ... }
    // Same body as shader::Primitive::draw()
}
```

Remove `use iced::widget::shader;` and `use iced::{mouse, Rectangle};`. Keep ALL wgpu code unchanged — only the trait wrappers change.

Keep `AtlasShaderProgram` as a data carrier struct (it holds the frame data that gets turned into an `AtlasPrimitive`), but remove its `shader::Program` impl. Instead, add a method:
```rust
impl AtlasShaderProgram {
    pub fn build_primitive(&self) -> AtlasPrimitive { ... }
    // Move the body from shader::Program::draw() here
}
```

- [ ] **Step 2: Refactor `shader_surface.rs` — same pattern**

Apply the exact same transformation: remove Iced trait impls, keep as plain struct methods.

- [ ] **Step 3: Delete `surface.rs` (canvas fallback)**

```bash
rm src-tauri/native/terminal-surface/src/surface.rs
```

Remove `pub mod surface;` and any `pub use surface::*;` from `lib.rs`. Remove `TerminalCanvas` and `TerminalCanvasState` from public exports.

- [ ] **Step 4: Remove `iced` from `terminal-surface/Cargo.toml`**

In `src-tauri/native/terminal-surface/Cargo.toml`, remove:
```toml
iced.workspace = true
```

Keep `wgpu = "27"` and all other deps.

- [ ] **Step 5: Fix compilation in `iced-shell`**

The `iced-shell` crate imports `TerminalCanvas` from terminal-surface. Update `iced-shell` to handle the removed canvas. For now, stub it — the iced-shell crate will be deleted later. The important thing is that `terminal-surface` compiles independently:

```bash
cd src-tauri && cargo check -p godly-terminal-surface
```

If `iced-shell` breaks, that's acceptable — it will be replaced. But `terminal-surface` must compile cleanly.

- [ ] **Step 6: Verify terminal-surface compiles without Iced**

```bash
cd src-tauri && cargo check -p godly-terminal-surface 2>&1
```

Should succeed with zero Iced references.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: extract GPU pipelines from Iced shader traits, delete canvas fallback"
```

### Task 0.4: Decouple `godly-app-adapter` from Iced

**Files:**
- Modify: `src-tauri/native/app-adapter/Cargo.toml`
- Modify: files in `src-tauri/native/app-adapter/src/` that import Iced

- [ ] **Step 1: Find Iced usage in app-adapter**

```bash
cd src-tauri && grep -rn "use iced\|iced::" native/app-adapter/src/ --include="*.rs"
```

The daemon client, clipboard, notifications, and shortcuts modules should be mostly Iced-free. The `keys.rs` module may import `iced::keyboard` types for key mapping. Identify all imports.

- [ ] **Step 2: Replace Iced key types with winit equivalents or custom types**

If `keys.rs` uses `iced::keyboard::Key` or `iced::keyboard::Modifiers`, replace with equivalent types from `winit::keyboard` (same underlying winit types that Iced wraps). Add `winit` as a dependency to app-adapter if needed, or define simple custom key enums.

- [ ] **Step 3: Remove `iced` from app-adapter's Cargo.toml**

- [ ] **Step 4: Verify compilation**

```bash
cd src-tauri && cargo check -p godly-app-adapter
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: decouple godly-app-adapter from Iced"
```

---

## Phase 1: Platform Shell — winit + wgpu Window

**Goal:** A new `godly-shell` binary that opens a borderless window, initializes wgpu, and renders a solid background color. This proves the platform layer works.

### Task 1.1: Create the `godly-shell` crate

**Files:**
- Create: `src-tauri/native/godly-shell/Cargo.toml`
- Create: `src-tauri/native/godly-shell/src/main.rs`
- Modify: `src-tauri/Cargo.toml` (workspace members)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "godly-shell"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "godly-native"
path = "src/main.rs"

[dependencies]
winit = { version = "0.30", features = ["rwh_06"] }
wgpu = "27"
pollster = "0.4"
log = "0.4"
env_logger = "0.11"
raw-window-handle = "0.6"
```

- [ ] **Step 2: Create `main.rs` — minimal winit + wgpu window**

```rust
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes, WindowId},
};
use wgpu::SurfaceTargetUnsafe;
use std::sync::Arc;

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
}

impl App {
    fn new() -> Self {
        Self { window: None, gpu: None }
    }

    fn init_gpu(&mut self, window: Arc<Window>) {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12 | wgpu::Backends::VULKAN,
            ..Default::default()
        });

        // SAFETY: window lives as long as surface (Arc-held)
        let surface = unsafe {
            instance.create_surface_unsafe(
                SurfaceTargetUnsafe::from_window(&*window).unwrap()
            ).unwrap()
        };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })).expect("No suitable GPU adapter found");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("godly-shell"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        )).expect("Failed to create device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter()
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

        self.gpu = Some(GpuState { surface, device, queue, config });
        self.window = Some(window);
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

        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render"),
        });

        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.071, g: 0.071, b: 0.082, a: 1.0, // Dark theme bg
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() { return; }
        let attrs = WindowAttributes::default()
            .with_title("Godly Terminal")
            .with_inner_size(winit::dpi::LogicalSize::new(1200.0, 800.0))
            .with_min_inner_size(winit::dpi::LogicalSize::new(400.0, 300.0))
            .with_decorations(false); // Custom title bar

        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        self.init_gpu(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width = size.width.max(1);
                    gpu.config.height = size.height.max(1);
                    gpu.surface.configure(&gpu.device, &gpu.config);
                }
                if let Some(w) = &self.window { w.request_redraw(); }
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Request continuous redraws during active output
        if let Some(w) = &self.window { w.request_redraw(); }
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
```

- [ ] **Step 3: Add to workspace members**

In `src-tauri/Cargo.toml`, add `"native/godly-shell"` to the `[workspace] members` array.

- [ ] **Step 4: Build and run**

```bash
cd src-tauri && cargo run -p godly-shell
```

Expected: A 1200x800 borderless dark window appears. Close with Alt+F4.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: create godly-shell crate with winit + wgpu window"
```

### Task 1.2: Integrate async event delivery

**Goal:** Set up a channel-based event system that works correctly during minimize (solving Iced bug #1).

**Files:**
- Create: `src-tauri/native/godly-shell/src/event_bus.rs`
- Modify: `src-tauri/native/godly-shell/src/main.rs`

- [ ] **Step 1: Create `event_bus.rs`**

```rust
use std::sync::{Arc, mpsc};
use winit::event_loop::EventLoopProxy;

/// Events that can arrive from background threads (daemon, MCP, timers).
#[derive(Debug)]
pub enum AsyncEvent {
    TerminalOutput { session_id: String },
    SessionClosed { session_id: String, exit_code: Option<i64> },
    ProcessChanged { session_id: String, process_name: String },
    GridDiff { session_id: String, diff_bytes: Vec<u8> },
    Bell { session_id: String },
    GridFetched { session_id: String, grid: Box<godly_protocol::types::RichGridData>, is_scroll_fetch: bool },
    GridFetchFailed { session_id: String, error: String },
    Heartbeat,
}

/// Wakes the winit event loop from any thread.
/// Unlike Iced's broken waker, winit's EventLoopProxy reliably posts
/// a user event that wakes GetMessage() on Windows.
#[derive(Clone)]
pub struct EventSender {
    proxy: EventLoopProxy<AsyncEvent>,
}

impl EventSender {
    pub fn new(proxy: EventLoopProxy<AsyncEvent>) -> Self {
        Self { proxy }
    }

    pub fn send(&self, event: AsyncEvent) {
        let _ = self.proxy.send_event(event);
    }
}
```

- [ ] **Step 2: Update main.rs to use `EventLoop::with_user_event()`**

Change:
```rust
let event_loop = EventLoop::new().unwrap();
```
to:
```rust
let event_loop = EventLoop::<AsyncEvent>::with_user_event().build().unwrap();
```

Add a `user_event()` handler to the `ApplicationHandler` impl:
```rust
fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AsyncEvent) {
    match event {
        AsyncEvent::Heartbeat => {
            if let Some(w) = &self.window { w.request_redraw(); }
        }
        // Other events handled later
        _ => { log::debug!("Async event: {event:?}"); }
    }
}
```

This solves the minimize-freeze problem: `EventLoopProxy::send_event()` calls `PostMessageW` internally via winit, which wakes the event loop even when the window is invisible.

- [ ] **Step 3: Spawn a test heartbeat thread**

In `resumed()`, after window creation:
```rust
let sender = EventSender::new(event_loop.create_proxy());
std::thread::spawn(move || {
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        sender.send(AsyncEvent::Heartbeat);
    }
});
```

- [ ] **Step 4: Build, run, minimize for 60 seconds, restore — verify no freeze**

```bash
cd src-tauri && cargo run -p godly-shell
```

Minimize the window, wait 60+ seconds, restore. The window should redraw immediately (no freeze, no black screen).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: async event bus with winit EventLoopProxy (no minimize-freeze)"
```

---

## Phase 2: Terminal Grid Rendering

**Goal:** Render actual terminal text using the existing GPU glyph atlas pipeline, connected to a live daemon session.

### Task 2.1: Wire up the atlas pipeline

**Files:**
- Modify: `src-tauri/native/godly-shell/Cargo.toml`
- Create: `src-tauri/native/godly-shell/src/terminal_renderer.rs`
- Modify: `src-tauri/native/godly-shell/src/main.rs`

- [ ] **Step 1: Add terminal-surface dependency**

Add to `godly-shell/Cargo.toml`:
```toml
godly-terminal-surface = { path = "../terminal-surface" }
godly-protocol = { path = "../../protocol" }
```

- [ ] **Step 2: Create `terminal_renderer.rs`**

This wraps the existing `AtlasPipeline` + `GlyphAtlas` + `GlyphCache` + `PixelRenderer` into a simple API:

```rust
use godly_terminal_surface::{
    atlas_shader::{AtlasPipeline, AtlasPrimitive, AtlasShaderProgram},
    atlas_vertex_builder,
    glyph_atlas::GlyphAtlas,
    glyph_cache::GlyphCache,
    glyph_rasterizer::GlyphRasterizer,
    font_metrics::FontMetrics,
    Color,
};
use godly_protocol::types::RichGridData;

pub struct TerminalRenderer {
    pipeline: AtlasPipeline,
    glyph_atlas: GlyphAtlas,
    glyph_cache: GlyphCache,
    rasterizer: Box<dyn GlyphRasterizer>,
    font_metrics: FontMetrics,
}

impl TerminalRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        font_metrics: FontMetrics,
        rasterizer: Box<dyn GlyphRasterizer>,
    ) -> Self {
        let pipeline = AtlasPipeline::new(device, queue, format);
        let glyph_atlas = GlyphAtlas::new();
        let glyph_cache = GlyphCache::new();

        Self { pipeline, glyph_atlas, glyph_cache, rasterizer, font_metrics }
    }

    /// Render a grid to the given render pass.
    /// Returns the number of cells rendered.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'_>,
        grid: &RichGridData,
        viewport_width: f32,
        viewport_height: f32,
    ) -> usize {
        // Build vertices from grid data using the existing atlas_vertex_builder
        let vertices = atlas_vertex_builder::build_vertices(
            grid,
            &mut self.glyph_cache,
            &mut self.glyph_atlas,
            &*self.rasterizer,
            &self.font_metrics,
            viewport_width,
            viewport_height,
        );

        if vertices.is_empty() { return 0; }

        // Create primitive and prepare/draw
        let primitive = AtlasPrimitive::new(vertices, self.glyph_atlas.generation());
        primitive.prepare(&mut self.pipeline, device, queue);
        primitive.draw(&self.pipeline, render_pass);

        grid.rows.len() * grid.cols
    }
}
```

**Note:** The exact API of `atlas_vertex_builder::build_vertices()` may differ from the above. Read the actual function signature in `src-tauri/native/terminal-surface/src/atlas_vertex_builder.rs` and adapt. The key point is: grid data in → vertex buffer out → GPU draw.

- [ ] **Step 3: Integrate into render loop**

In `main.rs`, after the clear pass, add a terminal render pass using the same `view` and `encoder`. Store a `TerminalRenderer` in the `App` struct. For now, render a fake `RichGridData` with a few test cells to verify the pipeline works.

- [ ] **Step 4: Verify colored cells render**

```bash
cd src-tauri && cargo run -p godly-shell
```

Expected: Colored rectangles appear in the terminal area (text may not render yet if glyph rasterization needs tuning).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: wire atlas GPU pipeline into godly-shell render loop"
```

### Task 2.2: Connect to daemon and render live terminal

**Files:**
- Modify: `src-tauri/native/godly-shell/Cargo.toml`
- Create: `src-tauri/native/godly-shell/src/daemon_bridge.rs`
- Modify: `src-tauri/native/godly-shell/src/main.rs`

- [ ] **Step 1: Add app-adapter dependency**

```toml
godly-app-adapter = { path = "../app-adapter" }
```

- [ ] **Step 2: Create `daemon_bridge.rs`**

Implement `FrontendEventSink` that routes events through the `EventSender`:

```rust
use godly_app_adapter::daemon_client::FrontendEventSink;
use crate::event_bus::{AsyncEvent, EventSender};

pub struct ShellEventSink {
    sender: EventSender,
}

impl ShellEventSink {
    pub fn new(sender: EventSender) -> Self {
        Self { sender }
    }
}

impl FrontendEventSink for ShellEventSink {
    fn on_terminal_output(&self, session_id: &str) {
        self.sender.send(AsyncEvent::TerminalOutput {
            session_id: session_id.to_string(),
        });
    }

    fn on_session_closed(&self, session_id: &str, exit_code: Option<i64>) {
        self.sender.send(AsyncEvent::SessionClosed {
            session_id: session_id.to_string(),
            exit_code,
        });
    }

    fn on_process_changed(&self, session_id: &str, process_name: &str) {
        self.sender.send(AsyncEvent::ProcessChanged {
            session_id: session_id.to_string(),
            process_name: process_name.to_string(),
        });
    }

    fn on_grid_diff(&self, session_id: &str, diff_bytes: &[u8]) {
        self.sender.send(AsyncEvent::GridDiff {
            session_id: session_id.to_string(),
            diff_bytes: diff_bytes.to_vec(),
        });
    }

    fn on_bell(&self, session_id: &str) {
        self.sender.send(AsyncEvent::Bell {
            session_id: session_id.to_string(),
        });
    }
}
```

- [ ] **Step 3: In `main.rs`, connect to daemon on startup**

In `resumed()`, after GPU init:
1. Create `EventSender` from `EventLoopProxy`
2. Create `NativeDaemonClient::connect_or_launch()`
3. Call `client.setup_bridge(ShellEventSink::new(sender.clone()))`
4. Create a session: `client.send_request(Request::CreateSession { ... })`
5. Store the client and session ID in `App`

- [ ] **Step 4: Handle `AsyncEvent::TerminalOutput`**

When output arrives:
1. Fetch the grid: `client.send_request(Request::ReadRichGrid { session_id })`
2. This is blocking, so spawn on a thread and send result via `EventSender`
3. On `AsyncEvent::GridFetched`, store the grid and request a redraw

- [ ] **Step 5: Render the stored grid in the render loop**

Pass the stored `RichGridData` to `TerminalRenderer::render()`.

- [ ] **Step 6: Build and run — verify live terminal output renders**

```bash
cd src-tauri && cargo run -p godly-shell
```

Expected: A terminal session starts, shell prompt appears, but you can't type yet (keyboard input is Phase 2.3).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: connect to daemon and render live terminal grid"
```

### Task 2.3: Keyboard input to PTY

**Files:**
- Create: `src-tauri/native/godly-shell/src/input.rs`
- Modify: `src-tauri/native/godly-shell/src/main.rs`

- [ ] **Step 1: Create `input.rs` — map winit key events to PTY bytes**

Use `godly_app_adapter::keys::key_to_pty_bytes()` (or equivalent) to convert winit `KeyEvent` into bytes for the PTY. The existing implementation in app-adapter handles escape sequences for arrow keys, function keys, modifiers, etc.

winit provides:
- `event.logical_key` — the logical key (Key::Named or Key::Character)
- `event.text` — the composed text (handles dead keys, IME)
- `event.state` — pressed or released
- Modifiers via `window_event(Modifiers(...))`

Map these to the existing PTY byte encoding.

- [ ] **Step 2: In `window_event()`, handle `KeyboardInput`**

```rust
WindowEvent::KeyboardInput { event, .. } => {
    if event.state == ElementState::Pressed {
        if let Some(bytes) = input::key_to_pty_bytes(&event, &self.modifiers) {
            // Send to daemon
            if let Some(client) = &self.client {
                let session_id = self.active_session.clone();
                client.send_request(Request::Write { session_id, data: bytes });
            }
        }
    }
}
```

- [ ] **Step 3: Track modifier state**

Handle `WindowEvent::ModifiersChanged` to track current Ctrl/Alt/Shift state.

- [ ] **Step 4: Build and run — verify typing works**

```bash
cd src-tauri && cargo run -p godly-shell
```

Expected: You can type in the terminal, run commands, see output. This is the first "usable terminal" milestone.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: keyboard input forwarding to PTY"
```

### Task 2.4: Mouse selection and scrolling

**Files:**
- Create: `src-tauri/native/godly-shell/src/selection.rs`
- Modify: `src-tauri/native/godly-shell/src/main.rs`

- [ ] **Step 1: Implement text selection**

Port the selection logic from `iced-shell/src/selection.rs`. The core logic is:
1. On mouse down: record anchor position (convert pixel coords to grid row/col using font_metrics)
2. On mouse move (while button held): update selection end
3. On mouse up: finalize selection, copy to clipboard via `arboard`
4. Render selection overlay (semi-transparent blue rectangles) in the GPU pipeline

- [ ] **Step 2: Implement scroll wheel**

On `WindowEvent::MouseWheel`:
1. Send `Request::ScrollAndReadRichGrid { session_id, offset }` to daemon
2. Update displayed grid with scrolled content

- [ ] **Step 3: Build and run — verify selection and scrolling work**

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: mouse text selection and scroll wheel"
```

---

## Phase 3: Widget System

**Goal:** Build a lightweight retained widget tree for the UI chrome (title bar, tab bar, sidebar, status bar) using taffy for layout.

### Task 3.1: Taffy-based layout engine

**Files:**
- Modify: `src-tauri/native/godly-shell/Cargo.toml`
- Create: `src-tauri/native/godly-shell/src/ui/mod.rs`
- Create: `src-tauri/native/godly-shell/src/ui/layout.rs`
- Create: `src-tauri/native/godly-shell/src/ui/widget.rs`

- [ ] **Step 1: Add taffy dependency**

```toml
taffy = "0.7"
```

- [ ] **Step 2: Design the widget trait**

```rust
/// A lightweight retained widget. Widgets are cheap structs that describe
/// what to render. Layout is computed by taffy. Hit testing uses the
/// computed layout rectangles.
pub trait Widget {
    /// Return taffy style for this widget (dimensions, flex, padding, etc.)
    fn style(&self) -> taffy::Style;

    /// Draw this widget into the render pass at the computed position.
    fn draw(&self, ctx: &mut DrawContext, rect: Rect);

    /// Handle a mouse event at the given position. Return an action if consumed.
    fn on_mouse(&mut self, event: MouseEvent, rect: Rect) -> Option<Action>;

    /// Children (for container widgets).
    fn children(&self) -> &[Box<dyn Widget>] { &[] }
    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] { &mut [] }
}
```

- [ ] **Step 3: Implement layout computation**

```rust
pub struct LayoutEngine {
    taffy: taffy::TaffyTree<WidgetId>,
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self { taffy: taffy::TaffyTree::new() }
    }

    /// Build the taffy tree from the widget tree, compute layout,
    /// return a flat list of (widget_id, computed_rect) pairs.
    pub fn compute(&mut self, root: &dyn Widget, viewport: Size) -> Vec<(WidgetId, Rect)> {
        // Walk widget tree → create taffy nodes → compute layout → extract rects
        todo!()
    }
}
```

- [ ] **Step 4: Implement basic widgets: `Box`, `Text`, `Row`, `Column`**

These are the primitives everything else is built from:
- `Box` — colored rectangle with optional border
- `Text` — single line of text (rendered via a separate text pipeline — cosmic-text or the existing glyph rasterizer)
- `Row` — horizontal flex container
- `Column` — vertical flex container

- [ ] **Step 5: Build and test — render a colored box with text**

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: taffy-based widget system with Box, Text, Row, Column"
```

### Task 3.2: UI text rendering pipeline

**Files:**
- Create: `src-tauri/native/godly-shell/src/ui/text_renderer.rs`

- [ ] **Step 1: Implement UI text rendering**

For UI chrome text (tab labels, status bar, button text), you need a text rendering pipeline separate from the terminal grid renderer. Options:

**Option A (recommended): Reuse the existing glyph atlas pipeline.**
Create a second `AtlasPipeline` instance with a UI font (sans-serif like Segoe UI). Build vertices for UI text the same way as terminal cells, but with variable-width character spacing.

**Option B: Use cosmic-text for shaping + swash for rasterization.**
cosmic-text handles text shaping, line breaking, and bidirectional text. This is more correct for complex scripts but adds a dependency.

Start with Option A (simpler, reuses existing code). Switch to B later if needed.

- [ ] **Step 2: Test — render "Godly Terminal" title text**

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: UI text rendering via glyph atlas reuse"
```

### Task 3.3: Title bar, tab bar, sidebar, status bar

**Files:**
- Create: `src-tauri/native/godly-shell/src/ui/title_bar.rs`
- Create: `src-tauri/native/godly-shell/src/ui/tab_bar.rs`
- Create: `src-tauri/native/godly-shell/src/ui/sidebar.rs`
- Create: `src-tauri/native/godly-shell/src/ui/status_bar.rs`

- [ ] **Step 1: Implement title bar**

Custom title bar with:
- Drag-to-move (use `window.drag_window()` from winit)
- Minimize/Maximize/Close buttons
- Window title text

Layout: `Row [ Title(flex:1) | MinBtn | MaxBtn | CloseBtn ]`

- [ ] **Step 2: Implement tab bar**

Horizontal tab bar with:
- Tab buttons (click to switch)
- Close button per tab
- "+" button to create new tab
- Horizontal scrolling for overflow

Layout: `Row [ Tab1 | Tab2 | ... | TabN | AddBtn ]`

State comes from `godly-tabs-core::TabState`.

- [ ] **Step 3: Implement sidebar**

Vertical workspace list:
- Workspace name + terminal count badge
- Click to switch workspace
- Context menu (rename, delete)
- Collapsible

State comes from `godly-workspaces-core::WorkspaceCollection`.

- [ ] **Step 4: Implement status bar**

Bottom bar showing:
- Current process name
- Working directory
- Terminal dimensions (cols x rows)

- [ ] **Step 5: Compose the full layout**

```
Column [
    TitleBar (height: 32px)
    Row [
        Sidebar (width: 200px, collapsible)
        Column [
            TabBar (height: 36px)
            TerminalContent (flex: 1)  ← rendered by TerminalRenderer
            StatusBar (height: 24px)
        ]
    ]
]
```

- [ ] **Step 6: Build and run — verify full chrome renders around terminal**

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: title bar, tab bar, sidebar, status bar widgets"
```

### Task 3.4: Hit testing and mouse interaction

**Files:**
- Create: `src-tauri/native/godly-shell/src/ui/hit_test.rs`
- Modify: `src-tauri/native/godly-shell/src/main.rs`

- [ ] **Step 1: Implement hit testing**

After taffy computes layout, each widget has a bounding rect. On mouse events:
1. Walk the widget tree in reverse paint order (front-to-back)
2. Find the deepest widget whose rect contains the mouse position
3. Deliver the event to that widget

- [ ] **Step 2: Wire up mouse events from winit**

`WindowEvent::CursorMoved`, `MouseInput`, `MouseWheel` → hit test → widget handler or terminal selection/scroll.

- [ ] **Step 3: Implement tab click, sidebar click, button press**

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: hit testing and mouse interaction for UI widgets"
```

---

## Phase 4: State Management

**Goal:** Wire up the core state machines and achieve functional parity with the basic terminal workflow (create/close tabs, switch workspaces, split panes).

### Task 4.1: App state struct

**Files:**
- Create: `src-tauri/native/godly-shell/src/app_state.rs`

- [ ] **Step 1: Define the central app state**

```rust
use godly_tabs_core::TabState;
use godly_layout_core::LayoutNode;
use godly_workspaces_core::{WorkspaceCollection, WorkspaceInfo};
use godly_protocol::types::RichGridData;
use std::collections::HashMap;

pub struct AppState {
    pub client: Arc<NativeDaemonClient>,
    pub workspaces: WorkspaceCollection<LayoutNode>,
    pub grids: HashMap<String, RichGridData>,  // session_id → latest grid
    pub focused_terminal: Option<String>,
    pub window_size: (u32, u32),
    pub scale_factor: f64,
    pub font_metrics: FontMetrics,
    pub sidebar_visible: bool,
    pub sidebar_width: f32,
}
```

- [ ] **Step 2: Implement action dispatch**

Instead of Iced's Message enum, use a simpler action enum:
```rust
pub enum Action {
    NewTab,
    CloseTab(String),
    SwitchTab(String),
    SplitPane(SplitDirection),
    UnsplitPane,
    FocusPane(FocusDirection),
    NewWorkspace,
    SwitchWorkspace(String),
    ToggleSidebar,
    // ... etc
}
```

Each action modifies `AppState` using the existing reducer functions from `godly-features-shell`.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: central AppState with action dispatch via existing reducers"
```

### Task 4.2: Split pane rendering

**Files:**
- Create: `src-tauri/native/godly-shell/src/split_renderer.rs`

- [ ] **Step 1: Implement recursive split layout rendering**

Walk the `LayoutNode` tree:
- `Leaf { terminal_id }` → render that terminal's grid using `TerminalRenderer`
- `Split { direction, ratio, first, second }` → subdivide the rect, recurse

```rust
pub fn render_layout(
    node: &LayoutNode,
    rect: Rect,
    renderers: &mut HashMap<String, TerminalRenderer>,
    grids: &HashMap<String, RichGridData>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    render_pass: &mut wgpu::RenderPass<'_>,
) {
    match node {
        LayoutNode::Leaf { terminal_id } => {
            if let Some(grid) = grids.get(terminal_id) {
                if let Some(renderer) = renderers.get_mut(terminal_id) {
                    renderer.render(device, queue, render_pass, grid, rect.width, rect.height);
                }
            }
        }
        LayoutNode::Split { direction, ratio, first, second } => {
            let (r1, r2) = rect.split(*direction, *ratio);
            render_layout(first, r1, renderers, grids, device, queue, render_pass);
            render_layout(second, r2, renderers, grids, device, queue, render_pass);
        }
        LayoutNode::ContentPane { .. } => {
            // File viewer pane — implement later
        }
    }
}
```

- [ ] **Step 2: Render split divider lines**

Draw 2px lines at split boundaries (use a simple quad shader or the existing pipeline).

- [ ] **Step 3: Handle split resize (drag divider)**

On mouse drag over a divider, update the `ratio` field in the `LayoutNode`.

- [ ] **Step 4: Build and run — verify splits render and resize**

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: recursive split pane rendering from LayoutNode tree"
```

### Task 4.3: Keyboard shortcut routing

**Files:**
- Create: `src-tauri/native/godly-shell/src/shortcuts.rs`
- Modify: `src-tauri/native/godly-shell/src/main.rs`

- [ ] **Step 1: Reuse existing shortcut resolver**

`godly_app_adapter::shortcuts::ShortcutResolver` maps key combinations to actions. Reuse it with the new `Action` enum.

- [ ] **Step 2: Implement the routing pipeline**

Same pipeline as iced-shell, but simpler without Iced's interception layers:
1. Check against shortcut resolver → `Action` (app-level shortcut like Ctrl+T for new tab)
2. If no match → check for dialog/modal interception (settings open? Quick Claude open?)
3. If no interception → forward to PTY as terminal input

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: keyboard shortcut routing with existing ShortcutResolver"
```

### Task 4.4: Session persistence (save/restore)

**Files:**
- Create: `src-tauri/native/godly-shell/src/persistence.rs`

- [ ] **Step 1: Port session persistence**

Reuse the logic from `iced-shell/src/session_persistence.rs` to:
- Save workspace layout, tab order, terminal sessions on exit
- Restore on startup (reconnect to daemon sessions that survived)
- Autosave every 60 seconds

The persistence format is JSON files in `%APPDATA%/com.godly.terminal/`. The save/load logic is mostly string manipulation and serde — no Iced types involved.

- [ ] **Step 2: Commit**

```bash
git add -A
git commit -m "feat: session persistence (save/restore workspaces on restart)"
```

---

## Phase 5: Feature Parity

**Goal:** Implement remaining features to match the Iced shell. These can be done in parallel by independent subagents.

### Task 5.1: Settings dialog

Port `settings_dialog.rs` as a modal widget. Tabs: Appearance, Shortcuts, Notifications, Quick Claude, AI Tools, Plugins, Remote.

### Task 5.2: Quick Claude dialog

Port `quick_claude_dialog.rs`. Text input, workspace/model/mode dropdowns, skill autocomplete, image attachments.

### Task 5.3: Notifications system

Port `notifications.rs` + `notification_state.rs`. Toast stack, bell sounds, desktop notifications via `notify-rust`.

### Task 5.4: Search in terminal

Port `search.rs`. Ctrl+F search bar, regex toggle, next/prev navigation.

### Task 5.5: Terminal context menu

Port `terminal_context_menu.rs`. Right-click menu with copy, paste, select all, split, etc.

### Task 5.6: Theme system

Port `theme.rs`. Color constants, custom theme import/export.

### Task 5.7: MCP integration

Port `mcp_handler.rs`. Named pipe server for MCP tools (read_terminal, write_to_terminal, etc.)

### Task 5.8: Phone remote + Cloudflare tunnel

Port `phone_remote.rs` + `cf_tunnel.rs`. Remote access via phone, Cloudflare tunnel management.

### Task 5.9: Voice input (Whisper)

Port `whisper_ui.rs`. Microphone button, audio level meter, transcription display.

### Task 5.10: Worktree mode

Port `git_worktree.rs`. Git worktree lifecycle management per workspace.

### Task 5.11: Performance overlay

Port `perf_overlay.rs`. FPS counter, render timing graph.

### Task 5.12: CLAUDE.md editor

Port `claude_md_editor.rs`. In-app text editor for CLAUDE.md files.

---

## Verification Milestones

| Milestone | Phase | What to Verify |
|-----------|-------|---------------|
| **Window opens** | 1.1 | Borderless dark window, closes with Alt+F4 |
| **No minimize freeze** | 1.2 | Minimize 60s+ → restore → immediate redraw |
| **Terminal renders** | 2.1-2.2 | Shell prompt visible, live output |
| **Typing works** | 2.3 | Can type commands, run programs |
| **Mouse works** | 2.4 | Text selection, scroll wheel |
| **UI chrome** | 3.3 | Title bar, tab bar, sidebar, status bar visible |
| **Tabs work** | 4.1 | Create/close/switch tabs |
| **Splits work** | 4.2 | H/V splits, resize, directional focus |
| **Shortcuts work** | 4.3 | Ctrl+T new tab, Ctrl+W close, Ctrl+\` toggle sidebar |
| **Persist/restore** | 4.4 | Close app → reopen → same layout |
| **Feature complete** | 5.* | All settings, Quick Claude, notifications, search |

---

## Architecture Diagram

```
godly-shell (new crate)
├── main.rs              ← winit event loop, ApplicationHandler
├── event_bus.rs          ← AsyncEvent enum, EventSender (winit proxy)
├── daemon_bridge.rs      ← FrontendEventSink → EventSender
├── terminal_renderer.rs  ← AtlasPipeline wrapper
├── input.rs              ← winit KeyEvent → PTY bytes
├── selection.rs          ← Text selection state
├── app_state.rs          ← Central state (workspaces, tabs, grids)
├── split_renderer.rs     ← Recursive LayoutNode rendering
├── shortcuts.rs          ← Shortcut resolver integration
├── persistence.rs        ← Save/restore state
└── ui/
    ├── mod.rs
    ├── layout.rs         ← Taffy integration
    ├── widget.rs         ← Widget trait
    ├── text_renderer.rs  ← UI text via glyph atlas
    ├── hit_test.rs       ← Mouse → widget routing
    ├── title_bar.rs
    ├── tab_bar.rs
    ├── sidebar.rs
    └── status_bar.rs

Reused crates (unchanged):
├── godly-tabs-core       ← Tab state machine
├── godly-layout-core     ← Split layout tree
├── godly-workspaces-core ← Workspace collection
├── godly-features-shell  ← Reducer functions
├── godly-protocol        ← IPC types
├── godly-ports           ← Side-effect traits
├── godly-app-adapter     ← Daemon client, clipboard, shortcuts
└── godly-terminal-surface ← GPU rendering (decoupled from Iced in Phase 0)
```

---

## Dependencies Summary

```toml
# godly-shell/Cargo.toml
[dependencies]
winit = { version = "0.30", features = ["rwh_06"] }
wgpu = "27"
taffy = "0.7"
pollster = "0.4"
raw-window-handle = "0.6"
log = "0.4"
env_logger = "0.11"
parking_lot = "0.12"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
arboard = "3"                  # Clipboard
uuid = { version = "1", features = ["v4"] }

# Internal
godly-protocol = { path = "../../protocol" }
godly-app-adapter = { path = "../app-adapter" }
godly-terminal-surface = { path = "../terminal-surface" }
godly-layout-core = { path = "../layout-core" }
godly-tabs-core = { path = "../tabs-core" }
godly-workspaces-core = { path = "../workspaces-core" }
godly-features-shell = { path = "../features-shell" }

[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["winuser", "shellapi"] }
```

---

## Important Notes for the Implementing Agent

1. **Do NOT delete `iced-shell/` until `godly-shell` has feature parity.** Both should coexist in the workspace during migration. The Iced shell remains the production binary while the new shell is developed.

2. **Test each phase independently.** Each phase produces a runnable binary. Don't move to the next phase until the current one works.

3. **The daemon is a separate process.** You don't need to modify the daemon, PTY shim, protocol, or any backend code. The frontend is the only thing changing.

4. **winit's `EventLoopProxy` solves the minimize-freeze.** This is the single most important architectural improvement. Iced's broken waker required `PostMessageW` hacks. winit's proxy does the same thing correctly.

5. **The GPU rendering code is already written.** The atlas shader, vertex builder, glyph cache, glyph atlas, DirectWrite rasterizer — all of this is reusable. You're wiring existing components into a new shell, not rewriting the renderer.

6. **Read the actual function signatures.** The code examples in this plan are approximate. Always read the real source files before implementing. The terminal-surface API may have changed since this plan was written.

7. **Iced's binary is called `godly-native`.** The new shell should also produce a binary called `godly-native` (configured in Cargo.toml `[[bin]]`). During development, use a different name to avoid conflicts, then rename when ready to swap.

8. **The MCP server in the daemon expects the frontend to provide a named pipe at `\\.\pipe\godly-terminal-mcp`.** The MCP handler from `iced-shell/src/mcp_handler.rs` sets this up. Port it early if you need MCP tools to work.
