# godly-vt Architecture

## Crate Structure

```
src-tauri/
  godly-vt/                    ← new workspace member
    Cargo.toml
    src/
      lib.rs                   ← public API: Parser, Screen, Cell, Perform trait
      parser.rs                ← entry point: wraps state machine + screen
      state_machine/           ← vendored vte parser (modified)
        mod.rs                 ← Parser struct, advance(), state transitions
        states.rs              ← State enum (14 states + ApcString)
        params.rs              ← CSI/DCS parameter accumulation
        utf8.rs                ← UTF-8 handling, partial sequence buffer
      simd/                    ← SIMD acceleration (our addition)
        mod.rs                 ← runtime dispatch (avx2 vs sse2 vs scalar)
        scanner.rs             ← scan_for_control(), is_all_ascii()
        sse2.rs                ← SSE2 implementations
        avx2.rs                ← AVX2 implementations
        scalar.rs              ← fallback for non-x86
      screen.rs                ← terminal state: grid + alternate + modes
      grid.rs                  ← rows, cursor, scroll regions, scrollback
      row.rs                   ← single row: cells, wrapping
      cell.rs                  ← cell with extensible CellContent enum
      attrs.rs                 ← text attributes (fg, bg, bold, etc.)
      callbacks.rs             ← non-screen events (bell, title, clipboard)
      perform.rs               ← bridges state_machine actions → screen commands
      image/                   ← image protocol support (our addition)
        mod.rs                 ← ImageStore, DecodedImage, ImageCellRef
        sixel.rs               ← DCS sixel handler (uses icy_sixel)
        kitty.rs               ← APC Kitty graphics handler
        iterm2.rs              ← OSC 1337 handler
      term.rs                  ← ANSI escape code generation (BufWrite)
    benches/
      throughput.rs            ← criterion benchmarks vs baseline
    tests/
      vt_compliance.rs         ← escape sequence compliance suite
      grid_tests.rs            ← grid operations, scrollback
      image_tests.rs           ← image protocol tests
      simd_tests.rs            ← SIMD scanner correctness
```

## Dependency Graph

```
lib.rs (public API)
  └── parser.rs (Parser)
        ├── state_machine/ (vendored from vte)
        │     ├── mod.rs (state machine core)
        │     ├── states.rs
        │     ├── params.rs
        │     └── utf8.rs
        ├── simd/ (our addition)
        │     ├── scanner.rs
        │     ├── sse2.rs / avx2.rs / scalar.rs
        │     └── mod.rs (dispatch)
        └── perform.rs (WrappedScreen)
              ├── callbacks.rs
              └── screen.rs (Screen)
                    ├── grid.rs (Grid) → row.rs (Row) → cell.rs (Cell) → attrs.rs
                    ├── image/ (our addition)
                    │     ├── mod.rs (ImageStore)
                    │     ├── sixel.rs, kitty.rs, iterm2.rs
                    │     └── (uses: icy_sixel, image, base64, flate2)
                    └── term.rs (escape code generation)
```

## Cargo.toml

```toml
[package]
name = "godly-vt"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "High-performance terminal state engine with SIMD parsing and image support"

[dependencies]
unicode-width = "0.2"
itoa = "1.0"
memchr = "2"          # Phase 1: SIMD byte search
simdutf8 = "0.1"     # Phase 1: fast UTF-8 validation

# Image support (Phase 3, optional)
icy_sixel = { version = "0.1", optional = true }
image = { version = "0.25", optional = true, default-features = false, features = ["png", "jpeg"] }
base64 = { version = "0.22", optional = true }
flate2 = { version = "1.0", optional = true }

[features]
default = ["simd"]
simd = []             # SIMD scanner (SSE2/AVX2)
images = ["icy_sixel", "image", "base64", "flate2"]

[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "throughput"
harness = false
```

## Key Design Decisions

### 1. Cell Type — Extensible via Enum

```rust
/// What a cell contains. Most cells are characters.
pub enum CellContent {
    /// Normal text character (most common case)
    Text(char),
    /// Part of an inline image
    ImageFragment(ImageCellRef),
    /// Empty cell (no content, just background)
    Empty,
}

pub struct Cell {
    pub content: CellContent,
    /// UTF-8 combining characters (accents, emoji modifiers)
    combining: ArrayVec<char, 4>,
    pub attrs: Attrs,
    // Flags
    wide: bool,
    wide_continuation: bool,
}
```

**Why not keep vt100's fixed `[u8; 22]` storage?** Because `CellContent::ImageFragment` doesn't fit in a fixed byte array. The enum lets us add new cell types (semantic zones, AI annotations) without touching the grid.

**Size tradeoff:** vt100's Cell is exactly 32 bytes. Our Cell will be slightly larger (~40-48 bytes depending on enum layout). For an 80x24 grid that's 92KB vs 61KB — negligible. For 10,000 lines of scrollback it's 3.8MB vs 2.5MB — still fine.

### 2. Perform Trait — Batch Print

```rust
pub trait Perform {
    /// Batch of printable characters (hot path).
    /// Default: falls back to print() per char.
    fn print_str(&mut self, text: &str) {
        for c in text.chars() {
            self.print(c);
        }
    }

    /// Single printable character.
    fn print(&mut self, _c: char) {}

    /// C0/C1 control code.
    fn execute(&mut self, _byte: u8) {}

    /// CSI sequence final byte.
    fn csi_dispatch(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}

    /// ESC sequence final byte.
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}

    /// OSC string dispatched.
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}

    /// DCS hook (start of device control string).
    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}
    /// DCS data byte.
    fn put(&mut self, _byte: u8) {}
    /// DCS data slice (batch optimization).
    fn put_slice(&mut self, data: &[u8]) {
        for &b in data { self.put(b); }
    }
    /// DCS end.
    fn unhook(&mut self) {}

    /// APC start (our addition for Kitty graphics).
    fn apc_start(&mut self) {}
    /// APC data byte.
    fn apc_put(&mut self, _byte: u8) {}
    /// APC data slice (batch).
    fn apc_put_slice(&mut self, data: &[u8]) {
        for &b in data { self.apc_put(b); }
    }
    /// APC end.
    fn apc_end(&mut self) {}

    /// Early termination check (for synchronized updates).
    fn terminated(&self) -> bool { false }
}
```

### 3. Image Storage — Separate Cache

```rust
pub struct ImageStore {
    images: HashMap<u64, Arc<DecodedImage>>,
    uploads: HashMap<u32, ImageUpload>,    // Kitty staging
    total_bytes: usize,
    quota: usize,                          // Default 256MB
    lru: VecDeque<u64>,
}

pub struct DecodedImage {
    pub pixels: Vec<u8>,     // RGBA
    pub width: u32,
    pub height: u32,
    pub content_hash: u64,
}

pub struct ImageCellRef {
    pub image_hash: u64,     // Into ImageStore.images
    pub placement_id: u32,
    pub tex_x: f32,          // Normalized 0.0-1.0
    pub tex_y: f32,
    pub tex_w: f32,
    pub tex_h: f32,
    pub z_index: i32,
}
```

### 4. SIMD Scanner — Runtime Dispatch

```rust
// simd/mod.rs
pub fn scan_for_control(data: &[u8]) -> Option<usize> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { avx2::scan_for_control(data) };
        }
        return unsafe { sse2::scan_for_control(data) };
    }
    scalar::scan_for_control(data)
}

pub fn is_all_ascii(data: &[u8]) -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { avx2::is_all_ascii(data) };
        }
        return unsafe { sse2::is_all_ascii(data) };
    }
    scalar::is_all_ascii(data)
}
```

## Integration with Godly Terminal

### Where godly-vt Lives in the Data Flow

```
┌──────────────┐     Named Pipe     ┌──────────────────────────────┐
│  Daemon       │◄─────────────────►│  Tauri App                    │
│  session.rs   │  Event::Output    │                               │
│               │  { data: Vec<u8>} │  DaemonBridge                 │
│  ┌──────────┐ │                   │  ├── emits "terminal-output"  │
│  │ godly-vt │ │                   │  │   (for frontend rendering) │
│  │ Parser   │ │                   │  └── optionally maintains     │
│  │ (grid)   │ │                   │      local godly-vt grid      │
│  └──────────┘ │                   │      (for MCP queries)        │
└──────────────┘                    └──────────────────────────────┘
                                              │
                                    Tauri event: "terminal-output"
                                              │
                                              ▼
                                    ┌──────────────────┐
                                    │  Frontend         │
                                    │  Phase 1-3: xterm │
                                    │  Phase 4+: custom │
                                    └──────────────────┘
```

### Daemon Integration (Phase 1)
The daemon's `Session` already has a ring buffer. Add a `godly_vt::Parser` alongside it:

```rust
pub struct Session {
    // Existing
    pub ring_buffer: VecDeque<u8>,
    pub output_history: Vec<u8>,
    // New
    pub terminal: godly_vt::Parser,
}
```

On PTY output: feed bytes to both ring buffer AND `terminal.process(bytes)`. The grid state is always up-to-date.

### MCP Integration (Phase 1)
`read_terminal` switches from reading `output_history` bytes to reading `terminal.screen()`:

```rust
// Before: raw bytes, needs ANSI stripping
let raw = session.read_output_history();
let text = strip_ansi(&String::from_utf8_lossy(&raw));

// After: structured grid, always clean
let screen = session.terminal.screen();
let text = screen.contents(); // Already plain text, no stripping needed
```

### Frontend Integration (Phase 4)
Replace xterm.js with a renderer that reads godly-vt grid snapshots via IPC:

```
godly-vt grid → serialize to JSON/binary → Tauri command → JS renderer
```

Or embed a wgpu surface in the Tauri window for native GPU rendering.
