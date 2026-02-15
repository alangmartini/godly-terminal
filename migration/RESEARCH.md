# Research Findings

## 1. vt100-rust Source Analysis

**Repo:** https://github.com/doy/vt100-rust | **License:** MIT | **Version:** 0.16.2 | **Total:** ~3,978 lines

### File Breakdown

| File | Lines | Purpose |
|------|------:|---------|
| `screen.rs` | 1,354 | Top-level terminal state: grid + alternate screen + modes + attrs |
| `grid.rs` | 742 | Core grid: rows, cursor, scroll regions, scrollback (VecDeque) |
| `term.rs` | 551 | ANSI escape code generation (BufWrite trait) |
| `row.rs` | 474 | Single row: cells, wrapping, formatted/diff output |
| `perform.rs` | 277 | `vte::Perform` impl: bridges parser actions to Screen commands |
| `cell.rs` | 179 | Single cell: 32 bytes (22-byte UTF-8 content + attrs). Compile-time size assert |
| `attrs.rs` | 144 | Text attributes: fgcolor, bgcolor, bold/dim/italic/underline/inverse |
| `parser.rs` | 96 | Public entry: wraps vte::Parser + WrappedScreen, exposes `process()` / `screen()` |
| `callbacks.rs` | 69 | Trait for non-screen events: bell, title, clipboard, unhandled sequences |
| `lib.rs` | 64 | Re-exports public API |

### Key Types

**Cell** (32 bytes, compile-time enforced):
- `contents: [u8; 22]` — inline UTF-8 storage (no heap allocation)
- `len: u8` — packed: 5-bit length + `IS_WIDE` + `IS_WIDE_CONTINUATION` flags
- `attrs: Attrs` — fgcolor + bgcolor + mode bitfield

**Grid:**
- `rows: Vec<Row>` — active screen rows
- `scrollback: VecDeque<Row>` — only captures rows during full-screen scroll (not custom regions)
- `scroll_top/scroll_bottom: u16` — scroll region bounds
- O(n) scroll ops due to Vec::insert/remove

**Screen:**
- Holds `grid` + `alternate_grid` + current `attrs` + mode bitfield
- Modes: application keypad/cursor, hide cursor, alternate screen, bracketed paste
- Mouse: None/Press/PressRelease/ButtonMotion/AnyMotion + Default/Utf8/Sgr encoding
- Testability API: `cell(row, col)`, `contents()`, `contents_between()`, `contents_formatted()`, `contents_diff()`

### VT Sequences Handled
- **CSI:** CUP, CUU/D/F/B, CNL/CPL, CHA, VPA, ICH, DCH, IL, DL, SU, SD, ECH, ED, EL, DECSED, DECSEL, SGR, DECSTBM, DECSET/DECRST, window resize
- **ESC:** DECSC, DECRC, DECKPAM, DECKPNM, RI, RIS
- **OSC:** 0/1/2 (title), 52 (clipboard)
- **DECSET modes:** 1, 6, 9, 25, 47, 1000, 1002, 1003, 1005, 1006, 1049, 2004
- **SGR:** 0-4, 7, 22-24, 27, 30-37, 38;5/38;2, 39, 40-47, 48;5/48;2, 49, 90-97, 100-107

### Limitations for Our Fork
1. **No strikethrough** (SGR 9)
2. **No hyperlinks** (OSC 8)
3. **No custom tab stops** (HTS/TBC) — hardcoded at 8 columns
4. **No character sets** (SI/SO silently ignored)
5. **No image support** (sixel, kitty, iTerm2)
6. **O(n) scroll** — Vec::insert/remove for row operations
7. **Tab stops hardcoded** at every 8 columns
8. All escape code generation hardcoded (TODO: terminfo)
9. Row output code intentionally duplicated for performance

### Dependencies
- `vte 0.15.0` — parser
- `unicode-width 0.2.1` — character width
- `itoa 1.0.15` — fast integer formatting

---

## 2. vte Parser Internals

**Repo:** https://github.com/alacritty/vte | **License:** Apache-2.0 OR MIT | **Version:** 0.15.0 | **Total:** ~4,178 lines

### Architecture

14-state machine based on Paul Williams' model. Key deviation: `SosPmApcString` is a shared "black hole" state for SOS, PM, and APC — all data silently discarded.

### File Breakdown

| File | Lines | Purpose |
|------|------:|---------|
| `ansi.rs` | ~2,458 | High-level ANSI processor + Handler trait (61 methods). Behind `ansi` feature flag |
| `lib.rs` | ~1,543 | Core parser: state machine, Perform trait, UTF-8 handling |
| `params.rs` | ~144 | Fixed-size CSI/DCS params with subparameter support (max 32) |

### The Hot Path (Ground State)

```
advance() → state == Ground? → advance_ground()
  → memchr(0x1B, bytes) → find next ESC
  → str::from_utf8() → validate UTF-8
  → ground_dispatch() → for c in text.chars() { performer.print(c) }  ← BOTTLENECK
```

**`memchr`** already uses SIMD to find ESC bytes. But `ground_dispatch()` calls `print(char)` one character at a time — 65,536 calls for a 64KB ASCII buffer.

### DCS Flow
`ESC P` → DcsEntry → params → final byte → `hook()` → DcsPassthrough → `put(byte)` per data byte → ST → `unhook()`. Also byte-by-byte.

### OSC Flow
`ESC ]` → OscString → accumulate in `Vec<u8>` → split on `;` → BEL/ST → `osc_dispatch(&[&[u8]])`. Buffered, dispatched as slices.

### APC Gap
`ESC _` → SosPmApcString (shared with SOS/PM) → all data silently discarded. No Perform callbacks. Fixing requires:
1. Add `ApcString` state variant
2. Route `0x5F` to it instead of shared state
3. Add streaming callbacks: `apc_start()` / `apc_put(u8)` / `apc_end()`
4. ~30-50 lines of new code

### UTF-8 Handling
- Delegates to `core::str::from_utf8()` (SIMD-accelerated in std)
- Cross-buffer partial sequences stored in `partial_utf8: [u8; 4]`
- Invalid UTF-8: emits U+FFFD replacement character
- C1 controls (0x80-0x9F) dispatched via `execute()`

---

## 3. SIMD Terminal Parsing

### What Each Terminal Does

| Terminal | Approach | ASCII Speedup | Source |
|----------|----------|--------------|--------|
| **Ghostty** | SIMD UTF-8 decode + control scan in single pass + CSI fast-path | 16.6x (UTF-8), 2-5x (e2e) | [Devlog 006](https://mitchellh.com/writing/ghostty-devlog-006) |
| **Kitty** | AVX2/SSE4.2/NEON for escape boundary + UTF-8 + base64 | 2.6x ASCII, 5.1x images | [RFC #7005](https://github.com/kovidgoyal/kitty/issues/7005) |
| **Alacritty/vte** | `memchr(0x1B)` only | ~2.2x | [PR #8347](https://github.com/alacritty/alacritty/pull/8347) |
| **WezTerm** | No SIMD | Baseline | — |

### The Three Optimizations

**1. SIMD Control Character Scanner**
- SSE2: load 16 bytes, `_mm_subs_epu8(0x20, chunk)` to detect bytes < 0x20, `_mm_cmpeq_epi8` for 0x7F, OR masks, `_mm_movemask_epi8` → bitmask → `trailing_zeros()` for first match
- AVX2: same but 32 bytes per iteration
- SSE2 is always available on x86-64. AVX2 needs runtime detection via `is_x86_feature_detected!`

**2. Batch Print**
- Replace `print(char)` with `print_str(&str)` in Perform trait
- Default impl falls back to char-by-char for backward compat
- Parser emits entire printable runs as string slices

**3. ASCII Fast Path**
- `_mm_movemask_epi8(chunk)` — if all zero, every byte < 0x80 (ASCII)
- Skip full UTF-8 decoding, use `from_utf8_unchecked()`
- Combined with `simdutf8` crate for non-ASCII validation (23x faster than std)

### Implementation Phases for godly-vt
1. **Quick win:** `memchr` + `simdutf8` + `print_str()` → ~2x
2. **Full SIMD:** `std::arch` SSE2/AVX2 scanner → ~3-5x
3. **CSI fast-path:** Inline CSI parsing for common sequences → additional 1.4-2x
4. **Combined pass:** Ghostty-style single-pass UTF-8+control scan → approaching memcpy

### Target Throughput
- **> 100 MB/s ASCII** — competitive with Kitty/Ghostty
- **> 200 MB/s ASCII** — exceeding current leaders
- **> 50 MB/s CSI-heavy** — top tier for real terminal usage

### Rust Crates
- `std::arch` — raw SIMD intrinsics (stable, zero deps)
- `memchr` — SIMD byte search (Phase 1)
- `simdutf8` — SIMD UTF-8 validation (always use)
- `criterion` — benchmarking

---

## 4. Image Protocols

### Protocol Comparison

| Feature | Sixel | iTerm2 (OSC 1337) | Kitty Graphics |
|---------|-------|--------------------|----------------|
| Sequence | DCS q...ST | OSC 1337;File=...BEL | APC G...ST |
| Color | 256 palette, no alpha | Full (file format) | RGBA, full alpha |
| Transmission | Inline only | Inline (base64) | Inline, file, shared memory |
| Image reuse | None (re-send) | None | Upload once, place many |
| Z-index | No | No | Yes (above/below text) |
| Animation | No | GIF passthrough | Native frame composition |
| Max size | Unbounded | Unbounded | Configurable quota |
| tmux support | Yes (passthrough) | Limited | No |
| Adoption | ~45% of terminals | ~15% | ~25% and growing |

### Sixel
- Encoded as 6-pixel-tall horizontal strips, each byte = column of 6 vertical pixels
- Multi-color via overprinting (draw each color, `$` to reset position)
- Rust crate: **`icy_sixel`** (pure Rust, SIMD decoder, recommended)

### Kitty Graphics
- APC-based (requires parser extension since vte ignores APC)
- Key=value control data + base64/file/shm payload
- Upload → Place → Delete lifecycle with explicit IDs
- Unicode placeholder U+10EEEE for TUI integration
- File/SHM modes bypass PTY bandwidth (perfect for daemon architecture)

### iTerm2
- `OSC 1337 ; File=inline=1;width=auto : <base64-of-original-file> BEL`
- Terminal decodes the image format (PNG, JPEG, etc.)
- Simplest to implement, widely emitted by CLI tools

### Storage Architecture (Recommended)

```
ImageStore (separate from grid)
├── images: HashMap<hash, Arc<DecodedImage>>  — deduplicated
├── uploads: HashMap<id, ImageUpload>          — Kitty staging
├── quota: 256-320MB, LRU eviction
└── max_single_image: 100M pixels

Cell grid
├── CellContent::Character(char)
├── CellContent::ImageFragment(ImageCellRef)  — 36 bytes
└── CellContent::Empty
```

Images scroll naturally with cells. Arc refcount frees data when rows exit scrollback.

### Implementation Priority
1. **Kitty** — most capable, fits daemon arch, growing adoption
2. **iTerm2** — simple, broad CLI tool support
3. **Sixel** — tmux compatibility, legacy tools

### Crates Needed
- `image` — PNG/JPEG/GIF decoding
- `icy_sixel` — sixel decoding
- `base64` — payload decoding
- `flate2` — Kitty `o=z` compression
