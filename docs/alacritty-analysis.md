# Alacritty Analysis: Lessons for Godly Terminal

## Context

Alacritty is a GPU-accelerated single-terminal emulator optimized for raw rendering speed. Godly Terminal is a multi-workspace terminal manager optimized for running 10-20+ concurrent AI tool sessions (Claude Code, etc.) with only 1-2 visible at a time.

**The use cases are fundamentally different.** Alacritty optimizes for one terminal being fast. We optimize for many terminals being manageable. This reframes which Alacritty patterns matter.

## What Matters Most (Multi-Session Efficiency)

### 1. Cell Memory Layout — `Arc<CellExtra>` for Rare Attributes

Alacritty's `Cell` struct puts rare attributes (hyperlinks, zero-width chars, underline colors) behind `Option<Arc<CellExtra>>`. Most cells are `None`.

**Why it matters for us**: With 20 sessions x 10,000 scrollback lines x 200 cols = 40M cells. Saving 16 bytes/cell on rare attributes = ~640MB saved. Our `godly-vt` Cell struct should be audited for size — every byte multiplied by 40M matters.

**Action**: Audit `godly-vt` Cell size. Move rare fields behind `Option<Arc<>>`.

### 2. Row `occ` (Occupied Extent) Tracking

Each Alacritty row tracks `occ: usize` — the rightmost occupied column. `is_clear()` is O(1) instead of scanning all columns.

**Why it matters for us**: Background sessions often have mostly-empty rows (Claude Code output has lots of blank lines, short status messages). O(1) empty-row detection helps skip work during serialization, dirty tracking, and scrollback operations across all 20 sessions.

**Action**: Add `occ` tracking to `godly-vt` Row struct.

### 3. Daemon-Side Visibility Awareness (NOT from Alacritty)

Alacritty doesn't have this problem (single terminal = always visible). But it's our biggest gap.

**Current state**: The daemon streams `Event::Output` for ALL sessions regardless of frontend visibility. The bridge processes them all in FIFO order. Background session output burns CPU in the daemon, serialization in the bridge, and event delivery in the frontend — all for terminals nobody is looking at.

**Action**: Add `Pause`/`Resume` protocol messages. When a terminal becomes invisible (workspace switch, tab switch), the frontend sends `Pause`. The daemon stops streaming output for that session (still parses PTY output into the ring buffer, but doesn't serialize and send events). On `Resume`, replay any missed output from the ring buffer.

### 4. FairMutex / Read Batching — Amplified by Many Sessions

Alacritty's FairMutex ensures the PTY reader and renderer take turns. Their PTY reader batches up to 1MB before acquiring the terminal lock.

**Why it matters for us**: With 20 sessions, the bridge I/O bottleneck (#151) is 20x worse than with one terminal. If session A is dumping a large code block, sessions B-T queue behind it. The bridge needs per-session output coalescing and prioritization of visible sessions.

**Action**:
- Visible session output gets priority in the bridge event queue
- Background sessions batch output into larger chunks (lower frequency, higher throughput)
- Per-session read batching (accumulate before lock acquisition)

### 5. Scrollback Memory Budget

Alacritty caps scrollback at 100,000 lines (configurable). With one terminal, that's ~150MB max.

**Why it matters for us**: 20 sessions x 100,000 lines = 3GB. Claude Code sessions produce enormous output (full file contents, diffs, test results). We need per-session scrollback limits AND a global memory budget.

**Action**:
- Configurable per-session scrollback limit (default lower than single-terminal apps)
- Global memory budget across all sessions — when exceeded, oldest scrollback from background sessions gets evicted first
- Consider compressed scrollback for background sessions (zstd on inactive rows)

## What Matters Less Than Expected

### GPU Rendering / Glyph Atlas

Alacritty's biggest claim to fame — GPU instanced rendering with glyph atlas — is less relevant for us. The bottleneck isn't "paint 10,000 cells fast." It's "manage 200,000 cells across 20 sessions while painting only 10,000." Canvas2D `fillText()` is fast enough for a single visible terminal. A glyph atlas would help, but it's optimization #6, not #1.

### Vi Mode / Inline Search

Nice power-user features but orthogonal to the core multi-Claude workflow. Add later.

### URL Hints with Keyboard Activation

Alacritty's DFA-based regex hint system is clever, but Claude Code already provides clickable file paths and URLs in its own output. Lower priority for our use case.

## What We Already Do Better Than Alacritty

1. **Session persistence** — Alacritty sessions die with the window. Our daemon survives app restarts, which is critical for long-running Claude Code sessions.
2. **Multi-session architecture** — Alacritty bolted on multi-window support. Our daemon was designed for it.
3. **Workspace organization** — Alacritty has no concept of grouping terminals.
4. **MCP integration** — AI tools can interact with terminal state programmatically.

## Prioritized Feature Backlog (From This Analysis)

| Priority | Feature | Source | Impact |
|----------|---------|--------|--------|
| P0 | Daemon-side pause/resume for invisible sessions | Original (not Alacritty) | Eliminates wasted work for 80%+ of sessions |
| P0 | Visible-session priority in bridge event queue | Inspired by FairMutex | Fixes input latency under multi-session load |
| P1 | Cell struct size optimization (`Arc<CellExtra>`) | Alacritty | Memory savings scale with session count |
| P1 | Per-session + global scrollback memory budget | Alacritty's limit, extended | Prevents OOM with many Claude Code sessions |
| P1 | Background session output batching | Alacritty's 1MB batching, adapted | Reduces bridge throughput waste |
| P2 | Row `occ` tracking for O(1) empty detection | Alacritty | Faster serialization, dirty tracking |
| P2 | Synchronized update support (DCS) | Alacritty | Better TUI rendering (htop, vim) |
| P3 | Glyph atlas cache | Alacritty | Faster rendering for visible terminal |
| P3 | Column-range damage tracking | Alacritty | Partial redraws for visible terminal |
| P4 | Vi mode + inline search | Alacritty | Power-user feature |
| P4 | Regex URL hints | Alacritty | Nice-to-have |
