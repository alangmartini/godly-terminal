# godly-vt Migration

This folder tracks the migration from xterm.js to **godly-vt** — our own Rust terminal state engine, forked from [vt100-rust](https://github.com/doy/vt100-rust) with the [vte](https://github.com/alacritty/vte) parser vendored in.

## Objectives

### 1. Full Testability (Primary)
- **Terminal rendering tests without a running app** — assert on grid cells in pure Rust unit tests
- **Daemon-to-terminal bridge tests** — Rust-to-Rust, mock pipes, no E2E required
- **MCP automated tests** — read terminal content deterministically from the Rust grid

### 2. Full Ownership
- No upstream dependency on alacritty/vte API stability
- No dependency on doy/vt100-rust release schedule
- Custom Cell type with extensible metadata fields
- Custom grid with our own scrollback strategy

### 3. Performance Advantage
- SIMD-accelerated scanning for printable character runs
- Batch print (slice of chars, not char-by-char)
- ASCII fast path (skip UTF-8 decode when all bytes are 0x20-0x7E)
- Target: measurably faster than alacritty_terminal for bulk output

### 4. Image Display
- Sixel graphics (DCS-based, most widespread)
- iTerm2 inline images (OSC 1337, widely adopted)
- Kitty graphics protocol (APC-based, most capable — requires parser extension)

### 5. Future Differentiation
- Custom cell metadata (semantic zones, AI annotations, session source)
- Memory-mapped scrollback (not in-memory VecDeque)
- Grid-level features no other terminal has

## Documents

| File | Purpose |
|------|---------|
| [OBJECTIVES.md](OBJECTIVES.md) | Detailed goals with success criteria |
| [ARCHITECTURE.md](ARCHITECTURE.md) | godly-vt crate design and integration plan |
| [PHASES.md](PHASES.md) | Phased migration plan with tasks per phase |
| [RESEARCH.md](RESEARCH.md) | Research findings from vt100, vte, SIMD, image protocols |
| [INTEGRATION-MAP.md](INTEGRATION-MAP.md) | Current xterm.js touchpoints and what changes |
| [RISKS.md](RISKS.md) | Known risks, mitigations, and decision log |

## Current Status

- [x] Phase 0: Fork & Setup (godly-vt crate in workspace) — PR #96
- [x] Phase 1: Wire into daemon as state backend + unit tests — PR #96
- [x] Phase 2: SIMD + batch print performance — PR #97 (80-125 MB/s, SIMD scanner 25+ GiB/s)
- [x] Phase 3: Image protocol support — PR #98 (Kitty, iTerm2, Sixel, APC parser)
- [ ] Phase 4: Frontend renderer migration
- [ ] Phase 5: Remove xterm.js dependency
