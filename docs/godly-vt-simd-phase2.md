# godly-vt Phase 2: SIMD-Accelerated Parsing

## Summary

Phase 2 adds SIMD-accelerated parsing to the godly-vt terminal state engine, targeting high-throughput terminal output processing.

## Changes Made

### 1. Batch Print API (Perform trait)
- Added `print_str(&str)` to the `Perform` trait with default char-by-char fallback
- Added `put_slice(&[u8])` for DCS batch optimization
- Modified `ground_dispatch()` to accumulate printable runs and dispatch via `print_str()`
- Updated `WrappedScreen` to implement `print_str`

### 2. SIMD Scanner Module (`simd/`)
- `scalar.rs` — reference implementation for non-x86 platforms
- `sse2.rs` — SSE2 implementation (16 bytes/iter), always available on x86-64
- `avx2.rs` — AVX2 implementation (32 bytes/iter) with runtime detection
- `mod.rs` — runtime dispatch: AVX2 > SSE2 > scalar
- Functions: `scan_for_control()` finds first C0/DEL byte, `is_all_ascii()` checks high bits

### 3. Parser Hot Path Rewrite (`advance_ground`)
- Replaced `memchr(0x1B)` with `scan_for_control()` to find ALL control characters
- ASCII fast path: `is_all_ascii()` + `from_utf8_unchecked()` skips UTF-8 validation
- Non-ASCII path: `simdutf8::basic::from_utf8()` for faster validation (vs std)
- Dispatches entire printable runs via `print_str()` without per-char control checks

### 4. CSI Fast-Path
- When ESC is encountered in ground state, looks ahead for `[`
- Parses simple CSI sequences (digits + semicolons + final byte) inline
- Avoids state machine transitions for common sequences (SGR, CUP, ED, EL, cursor)
- Falls back to normal state machine for private markers, intermediates, subparameters

### 5. Benchmark Suite (`benches/throughput.rs`)
- Criterion-based benchmarks with throughput measurements
- Scenarios: ascii_1mb, unicode_1mb, csi_heavy, mixed_realistic
- SIMD scanner microbenchmarks

## Benchmark Results

### Parser Throughput (end-to-end, includes grid operations)

| Scenario | Throughput | Description |
|----------|-----------|-------------|
| ASCII 1MB | 79.8 MiB/s | Pure ASCII printable text with newlines |
| Unicode 1MB | 74.7 MiB/s | Mixed UTF-8 with CJK and emoji |
| CSI-heavy | 125.4 MiB/s | 10K SGR color changes + text |
| Mixed realistic | 90.3 MiB/s | Simulated cargo build output |

### SIMD Scanner (raw scanning speed)

| Operation | Throughput | Description |
|-----------|-----------|-------------|
| scan_for_control (1MB, no match) | 25.4 GiB/s | Full buffer scan, no control chars |
| scan_for_control (with newlines) | 198 GiB/s | Early match (first-byte hit) |
| is_all_ascii (1MB pure) | 45.8 GiB/s | Full buffer ASCII check |

### Analysis

The SIMD scanner operates at near-memory-bandwidth speeds (25-46 GiB/s). The parser throughput bottleneck is now in the grid operations (`screen.text()`) which does per-character work including unicode width lookup, wrapping checks, and cell mutations. The parsing layer itself (scanning + UTF-8 validation + dispatch) adds negligible overhead.

CSI-heavy workloads show the highest throughput (125 MiB/s) because the CSI fast-path avoids state machine overhead and the sequences are short, meaning less grid work per byte.

## Test Coverage

- 57 unit tests (26 new SIMD tests + 31 existing state machine tests)
- 55 vt_compliance integration tests
- 6 basic, 9 CSI, 6 escape, 2 mode, 3 OSC, 3 processing, 5 scroll integration tests
- Total: 146 tests, 144 pass (2 pre-existing failures in ri and scroll_regions)
- SIMD tests cover: all 256 byte values, boundary conditions (1-256 byte buffers), property tests (SIMD matches scalar for all inputs)

## Dependencies Added

- `simdutf8 = "0.1"` — SIMD-accelerated UTF-8 validation
- `criterion = "0.5"` (dev) — benchmarking framework
