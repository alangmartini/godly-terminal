# Objectives & Success Criteria

## Objective 1: Full Testability

**Why:** Today we cannot test terminal rendering without a running app, the daemon-app IPC bridge has zero isolated tests, and MCP tools rely on a 34-step manual procedure.

### Success Criteria

- [ ] **Grid unit tests**: Can feed VT sequences into godly-vt and assert on cell content, colors, attributes, cursor position — all in `cargo test`, no window or app required
- [ ] **Bridge integration tests**: Can test daemon→godly-vt data flow with mock named pipes, asserting grid state after output
- [ ] **MCP automated tests**: Can test `read_terminal` by querying the godly-vt grid directly, with deterministic output — no ANSI stripping hacks
- [ ] **Scrollback round-trip tests**: Can serialize/deserialize grid state and verify content preservation
- [ ] **VT compliance test suite**: A suite of tests covering the most common escape sequences (SGR, cursor movement, erase, scroll, alternate screen) that serves as a regression safety net

### Measurable target
- 0 manual test steps for MCP tools (down from 34)
- 100% of terminal output parsing covered by unit tests
- Bridge tests runnable in CI without a display server

---

## Objective 2: Full Ownership

**Why:** Depending on alacritty_terminal locks us to their Cell struct, their API churn, and their priorities. We need to move fast on features they don't care about.

### Success Criteria

- [ ] **Single crate**: godly-vt is one crate in our workspace with vte parser vendored as a module — no external terminal state dependencies
- [ ] **Custom Cell type**: Our Cell struct has extensible fields beyond what vt100/alacritty offer
- [ ] **No upstream breakage risk**: Updating Rust toolchain or other deps never breaks our terminal state engine
- [ ] **MIT license compliance**: Attribution preserved, license file included in crate

### Measurable target
- 0 external crate dependencies for terminal parsing/state (only std + unicode-width)
- Can add a new Cell metadata field in < 1 hour of work

---

## Objective 3: Performance Advantage (High-Throughput)

**Why:** xterm.js and vte both process printable characters one at a time. For build logs, large file dumps, and CI output, this is the bottleneck.

### Success Criteria

- [ ] **SIMD scanner**: Scans 16+ bytes at once to find control character boundaries
- [ ] **Batch print**: Printable runs written to the grid in a single operation, not char-by-char
- [ ] **ASCII fast path**: When a run is all ASCII (0x20-0x7E), skip UTF-8 decoding entirely
- [ ] **Benchmark suite**: Reproducible benchmarks comparing godly-vt throughput against vte + vt100 baseline

### Measurable target
- `cat 100MB_file` parsed at > 1 GB/s (throughput)
- 5-10x improvement over baseline vte char-by-char path
- Benchmark results tracked in `migration/benchmarks/`

---

## Objective 4: Image Display

**Why:** Terminal image display is increasingly common (AI tools, dev tools, TUIs). No Tauri-based terminal supports it. This is differentiation.

### Success Criteria

- [ ] **Sixel decoding**: DCS sixel sequences decoded and stored in grid as image cell fragments
- [ ] **iTerm2 inline images**: OSC 1337 File= sequences parsed, base64 decoded, stored as image data
- [ ] **Kitty graphics protocol**: APC-based graphics parsed (requires parser extension for APC support)
- [ ] **Image cell type**: Cell enum variant for image fragments with reference to shared image cache
- [ ] **Scrollback interaction**: Images survive scrollback correctly (fragments scroll with text)

### Measurable target
- Can display a PNG inline via each of the 3 protocols
- Image data correctly persists through scroll operations
- Unit tests verify image cell placement for all 3 protocols

---

## Objective 5: Future Differentiation

**Why:** Owning the terminal state engine means we can add capabilities no other terminal has.

### Not in initial scope, but enabled by godly-vt:

- **Semantic zones**: Mark regions as "command", "output", "prompt" for AI integration
- **AI annotations**: Attach metadata to cell ranges for context-aware features
- **Session source tracking**: Know which daemon session produced each line
- **Memory-mapped scrollback**: Use mmap instead of in-memory VecDeque for massive scrollback
- **Custom serialization**: Efficient binary format for scrollback persistence (vs ANSI re-parsing)
- **Compressed history**: Delta-encode scrollback for reduced disk/memory usage

These become possible because we own the Cell type and Grid implementation.
