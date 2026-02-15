# godly-vt Phase 3: Image Protocol Support

## Status: Complete

## What was done

### APC Parser Extension (3.2.1-3.2.5)
- Added `ApcString` state to state machine (separate from `SosPmApcString`)
- Routed `ESC _` (0x5F) to `ApcString` with streaming callbacks
- Implemented `advance_apc_string()` handler
- Added `apc_start()`, `apc_put()`, `apc_put_slice()`, `apc_end()` to Perform trait
- SOS/PM still use existing discard behavior (non-regression verified)
- 10 APC parser tests

### Cell Type Refactor (3.1.1)
- Added `CellContent` enum: `Text` (default), `ImageFragment` (behind `images` feature)
- Cell struct now has `content_type: CellContent` field
- All existing text behavior preserved
- `Cell.clear()` and `Cell.set()` reset to Text, clearing any image content
- `Cell.append()` is no-op for image cells
- `Cell.contents()` returns empty string for image cells

### ImageStore (3.1.2-3.1.4)
- Content-hash deduplication (FNV-1a 64-bit)
- LRU eviction with configurable quota (default 256MB)
- Kitty image ID to hash mapping
- Chunked upload staging lifecycle
- `assign_image_to_cells()` maps pixel regions to normalized texture coordinates
- 100M pixel per-image limit

### Kitty Graphics Protocol (3.3.1-3.3.9)
- Key=value parser for APC payload
- Actions: transmit (t), transmit+display (T), place (p), delete (d)
- Formats: RGBA (f=32), RGB (f=24), PNG (f=100)
- Base64 decoding + zlib decompression (o=z)
- Chunked transfer (m=0/m=1)
- Clean-room from spec: https://sw.kovidgoyal.net/kitty/graphics-protocol/

### iTerm2 Inline Images (3.4.1-3.4.5)
- OSC 1337 File= parameter parsing
- Base64 decoding + image format detection via `image` crate
- Clean-room from spec: https://iterm2.com/documentation-images.html

### Sixel (3.5.1-3.5.4)
- DCS accumulator (hook/put/unhook lifecycle)
- Built-in decoder: color defs, repeat, carriage return, newline
- Multi-color overprinting support

## Test Results
- 85 library tests pass without `images` feature
- 121 library tests pass with `images` feature (+36 image tests)
- Pre-existing `ri` escape test failure (not introduced by this work)

## Dependencies Added
- `image 0.25` (optional, PNG/JPEG decode)
- `base64 0.22` (optional, payload decode)
- `flate2 1.0` (optional, zlib decompress)
- All behind `features = ["images"]`

## Architecture Decisions
1. **CellContent as enum field**: Rather than replacing the entire Cell struct, added `content_type` field. Text cells keep the existing compact `[u8; 22]` inline storage. Cell size grows by the size of the enum discriminant but text behavior is identical.
2. **Feature gating**: All image code behind `#[cfg(feature = "images")]`. Default build has zero image overhead.
3. **Clean-room implementation**: Kitty (GPLv3) and iTerm2 (GPLv2) protocols implemented from public specs only, never referencing GPL source code.
4. **Built-in sixel decoder**: Rather than adding icy_sixel dependency, implemented a minimal decoder directly. Can be replaced with icy_sixel later for better performance.
