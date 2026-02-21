# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2026-02-21

### Added
- **Plugin system with external GitHub repos** — install, uninstall, and manage community plugins from a registry with plugin cards UI (#226)
- **Split terminal keyboard shortcuts** — Ctrl+Shift+H/V/U for horizontal/vertical split and unsplit (#224)

### Fixed
- **Selection auto-scroll** — drag-selecting text beyond viewport edges now auto-scrolls in all directions (#228)

### Changed
- Training data generation switched from Anthropic to OpenAI API for branch name model

## [0.4.1] - 2026-02-20

### Fixed
- **Typing rollback** — diffSeq staleness guard prevents input from being rolled back during concurrent output (#222)
- **Notification storm on startup** — added startup grace period and per-terminal cooldown for idle notifications (#210)

## [0.4.0] - 2026-02-20

### Added
- **Ultrafast I/O pipeline** — Tauri custom protocol for streaming terminal output bytes + frontend stream consumer for direct byte passthrough (#215, #220)
- **MCP multi-agent orchestration tools** — `send_keys`, `erase_content`, `execute_command` for coordinating parallel agents (#221)
- **Per-workspace notification muting** with glob pattern matching in Settings (#219)
- **Tiny branch name generator** — training pipeline + engine integration into quick_claude flow (#214)
- **Terminal input notification system** with bell detection and idle monitoring (#195)
- **Plugin system** with Peon-Ping sound pack plugin (#192)
- **SmolLM2-135M local LLM plugin** for AI branch name generation (#194)
- **Push grid diffs** from daemon instead of pull snapshots (#177)
- **Terminal exit code propagation** to frontend UI (#191)
- Drag-and-drop reordering for settings dialog tabs (#188)
- Search bar for keyboard shortcuts settings (#186)
- UI refinements for minimal aesthetic (#193)

### Fixed
- **Scroll auto-snap during sustained output** — terminal no longer jumps to bottom while user is scrolled up (#211)
- **MCP WebView crash** — removed second WebView window that caused crash under heavy output (#206)
- **Scroll position preservation** during output and alternate screen transitions (#205)
- **SmolLM2 download** — hf-hub URL parsing fix + retry button + root cause visibility (#203, #208)
- Terminal stealing focus when dialog overlays are open (#198)
- Plugins tab missing from settings dialog default tab order (#196)
- Quick Claude prompt not submitting to Claude Code (#185)
- OSC title propagation from daemon to frontend (#184)
- Home/End keys now move cursor instead of scrolling (#183)
- Infinite dev build loop from .taurignore and release binary stubs (#178, #179)

### Changed
- CI hang detection with timeouts and guardrail enforcement (#180)

## [0.3.0] - 2026-02-19

### Fixed
- **Text selection freeze** — terminal display now freezes while dragging to select text, preventing output from breaking the selection; catches up on mouseup (#176)
- **Ctrl key snapping to bottom** — modifier-only keypresses (Ctrl, Shift, Alt) no longer trigger snap-to-bottom, so Ctrl+C copy works while scrolled up (#176)
- Remote HTTP API uses correct axum 0.7 `:id` path param syntax (#175)

## [0.2.0] - 2026-02-19

### Added
- **godly-vt terminal engine** — custom VT parser forked from vt100-rust with SIMD-accelerated parsing, image protocol support (Kitty/iTerm2/Sixel), and 10K-line scrollback (#96–#100)
- **Canvas2D renderer** — replaced xterm.js with a pure Canvas2D rendering pipeline backed by godly-vt; frontend is now a stateless display layer (#99, #101)
- **godly-remote** — HTTP/WebSocket bridge crate for remote terminal access (#172)
- **Quick Claude** — instant idea-capture dialog with workspace selector (#162, #169)
- **Split-pane terminal view** with unsplit shortcut and tab-switch support (#74, #117)
- **Theme system** with settings UI and eye-saver theme; Tokyo Night visual redesign (#103, #112)
- **Performance profiling HUD** — always-on overlay toggled with Ctrl+Shift+P (#166)
- **Binary framing** for hot-path IPC messages — faster daemon-to-app communication (#136)
- **GitHub Actions CI pipeline** with full Rust + TypeScript checks on every PR (#135)
- **Copy (Clean) dialog** for terminal text selection (#134)
- **Toast notifications** with workspace name and click-to-navigate (#75, #168)
- **CMD aliases editor** in Settings with AutoRun registry setup (#121)
- **Shell selection** in Settings dialog with Custom shell type support (#119, #126)
- **Figma embed pane** with godly-figma-mCP integration (#122)
- **Frontend file-based logger** with log rotation (#109)
- **godly-notify** — lightweight CLI for fast hook notifications (#62)
- **Daemon direct fallback** for godly-mcp (#94)
- **MCP tools**: `read_terminal`, `WaitForIdle`, `WaitForText`, `strip_ansi` (#69, #79, #80)
- F2 hotkey for renaming the active terminal tab (#116)
- Workspace toggle shortcuts: Ctrl+Shift+W, Ctrl+Shift+E (#83)
- Version display in Settings (#118)
- Worktree panel with open-terminal button (#69)
- Scrollbar drag and PageUp/PageDown navigation (#164)

### Fixed
- **Ctrl+Arrow word navigation** now sends correct CSI modifier sequences (#174)
- **Quick Claude focus stealing** — terminal no longer grabs focus from other panes (#173)
- **OSC tab titles** no longer wiped by process-changed events (#171)
- **Production build** no longer requires closing the running app (#170)
- **Terminal freeze under heavy output** — resolved mutex starvation, DOM thrashing, and output flood bottleneck (#55, #57, #60, #102)
- **Input latency** — eliminated Windows timer resolution penalty, reduced arrow key lag (#71, #73, #77, #81)
- **Memory leak** — reader_master Arc dropped after first PTY read (#131)
- **Scroll position** preserved when new output arrives; no more rollbacks (#163, #164)
- Dead keys (quotes/accents) on ABNT2 keyboards (#132)
- Ctrl+Alt+letter combos no longer leak bare characters (#161)
- Workspace shell type no longer overridden by global default (#160)
- Tab rename no longer steals terminal focus (#167)
- Tab name updates correctly when Claude Code process starts (#165)
- Dead terminal tabs show indicator instead of silently disappearing (#137)
- Paste/drag image freeze from circular deadlock (#125)
- Canvas focus recovery to prevent keyboard input freeze (#127)
- Zoom flash when activating terminal panes (#123)
- Split view state preserved across tab navigation (#120)
- Ctrl+C interrupt in ConPTY sessions (#58)
- Session recovery after daemon crash/restart (#56)
- Drag-drop: pointer-event system replaces HTML5 DnD (#115, #130)
- Daemon logs preserved across restarts (#59)

### Changed
- Build pipeline optimized for faster iteration (#159)
- Tab drag-drop uses pointer events instead of HTML5 DnD API (#115)
- MCP-created terminals open in a separate Agent window (#82)
- Daemon tests use isolated pipe names and PID-based cleanup (#68)

## [0.1.0] - 2025-01-01

### Added
- Initial release with workspace management, terminal tabs, and tmux-style session persistence
- Background daemon (godly-daemon) for PTY session management via named pipes
- WSL and PowerShell support
- Layout and scrollback persistence with autosave
- MCP server (godly-mcp) for external tool integration
