# Risks & Decision Log

## Risk 1: Cell Size Increase

**Risk:** Our extensible `CellContent` enum makes cells larger than vt100's 32-byte fixed layout (~40-48 bytes). With large scrollback buffers, this increases memory usage.

**Impact:** Medium. 10,000 lines of scrollback at 80 cols: 3.8MB vs 2.5MB. For 100,000 lines: 38MB vs 25MB.

**Mitigation:**
- Monitor memory usage in benchmarks
- Consider a compact representation for the common case (`CellContent::Text(char)`) using enum niche optimization
- If needed, use a separate sparse map for image cells instead of per-cell enum
- Scrollback compression (Phase 5+) would offset the size increase

**Decision:** Accept the tradeoff. Extensibility is more valuable than saving 13MB at 100K scrollback.

---

## Risk 2: VT Compliance Gaps

**Risk:** vt100 doesn't handle all sequences (no strikethrough, no hyperlinks, no custom tab stops, no character sets). Our fork inherits these gaps.

**Impact:** Low for now. These features are not commonly used by PowerShell or typical CLI tools on Windows.

**Mitigation:**
- Track gaps in a VT compliance matrix
- Add features incrementally as users hit them
- The compliance test suite (Phase 1) serves as the safety net

**Decision:** Ship with vt100's existing coverage. Add strikethrough (SGR 9) and hyperlinks (OSC 8) in Phase 1 since they're common in modern terminals. Others can wait.

---

## Risk 3: SIMD Correctness on Edge Cases

**Risk:** SIMD scanners could miss control characters or misidentify printable bytes, causing parsing errors.

**Impact:** High if it happens (corrupted terminal state).

**Mitigation:**
- Exhaustive unit tests covering all byte values 0x00-0xFF
- Fuzz testing with arbitrary byte sequences (vt100 already has fuzz tests)
- Property-based tests: SIMD scanner result == scalar scanner result for all inputs
- Test with partial buffers (1 byte, 15 bytes, 17 bytes, etc.)

**Decision:** Invest heavily in SIMD correctness tests before any optimization is considered "done."

---

## Risk 4: Frontend Renderer Complexity (Phase 4)

**Risk:** Building a custom terminal renderer (text selection, cursor, font rendering, ligatures, scrolling) is a massive undertaking that could delay other work.

**Impact:** High. Phase 4 is estimated at 3-6 weeks but could easily be longer.

**Mitigation:**
- Phase 4 is optional — Phases 0-3 deliver full value with xterm.js still as the renderer
- Consider a hybrid approach: keep xterm.js as renderer, feed it from godly-vt grid (serialize ANSI back to xterm.js)
- If going native, start with Canvas2D (simpler) before attempting wgpu
- Could use an existing Rust terminal renderer crate if one matures

**Decision:** Defer Phase 4 decision until after Phase 3. By then we'll have a better sense of whether xterm.js-as-renderer is good enough.

---

## Risk 5: Scrollback Format Migration

**Risk:** Existing users have scrollback saved in ANSI-encoded format (via xterm.js serialize addon). Switching to godly-vt's binary format could break restore.

**Impact:** Medium (users lose scrollback history from before the migration).

**Mitigation:**
- godly-vt can accept ANSI bytes as input (just `parser.process()`) — this is backward compatible
- Support both formats: try binary first, fall back to ANSI re-parse
- One-time migration: on first load, parse ANSI scrollback, save as binary

**Decision:** Support both formats indefinitely. ANSI input is "free" since the parser handles it by design.

---

## Risk 6: Image Memory Pressure

**Risk:** Users with many inline images could exhaust memory despite the ImageStore quota.

**Impact:** Medium (OOM or degraded performance).

**Mitigation:**
- Default quota of 256MB with LRU eviction (matching industry standard)
- Content-hash deduplication prevents storing identical images twice
- Single image cap at 100M pixels (~400MB) prevents allocation attacks
- Configurable quota in settings

**Decision:** Use Kitty's proven limits (320MB) and WezTerm's pixel cap (100M). Monitor real-world usage.

---

## Risk 7: Daemon Performance with Dual Output

**Risk:** Feeding PTY output to both ring buffer AND godly-vt parser doubles the processing work in the daemon's reader thread.

**Impact:** Low-Medium. The parser is fast, but it adds CPU work per output byte.

**Mitigation:**
- godly-vt with SIMD should parse at > 100 MB/s — negligible compared to PTY I/O speeds
- Ring buffer write is a simple memcpy, already fast
- Profile after Phase 1 to measure actual overhead
- If needed, only update grid on-demand (e.g., when MCP queries it)

**Decision:** Accept the dual-write overhead. If profiling shows problems, switch to on-demand grid updates.

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-02-15 | Fork vt100 + vendor vte (not use as dependency) | Full ownership, avoid upstream breakage |
| 2026-02-15 | Extensible CellContent enum (not fixed byte array) | Enables images, AI annotations, future features |
| 2026-02-15 | SIMD via `std::arch` + `memchr` + `simdutf8` | Maximum performance with minimal dependencies |
| 2026-02-15 | Image priority: Kitty > iTerm2 > Sixel | Kitty is most capable and fits daemon arch |
| 2026-02-15 | APC support via streaming callbacks (not buffered) | Large payloads (megabytes of base64) shouldn't be buffered |
| 2026-02-15 | Phase 4 (renderer) deferred until Phase 3 complete | xterm.js works as renderer; focus on testability + perf first |
