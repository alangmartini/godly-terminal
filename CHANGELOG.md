# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.11.1] - 2026-03-01

### Fixed
- **Terminal text invisible after typing (root cause)** — The `stream://` custom protocol URLs for real-time terminal updates never worked on Windows (WebView2 requires `http://{scheme}.localhost/` format). Changed `stream://localhost/` to `http://stream.localhost/` so diff and output streams actually connect. Also eliminated the 1s polling delay on successful responses and switched empty responses to 16ms polling for near-instant updates ([#486](https://github.com/alangmartini/godly-terminal/issues/486), [#492](https://github.com/alangmartini/godly-terminal/pull/492))
- **Recovery fetch livelock under sustained diff traffic** — `fetchFullSnapshot()` unconditionally checked `diffSeqAtStart`, discarding every recovery snapshot when diffs arrived during the IPC roundtrip. The fix skips the staleness check for recovery fetches where `cachedSnapshot` is null ([#486](https://github.com/alangmartini/godly-terminal/issues/486), [#490](https://github.com/alangmartini/godly-terminal/pull/490))

## [0.11.0] - 2026-03-01

### Added
- **Quick Claude AI tool selector** — Quick Claude dialog now includes a dropdown to choose between Claude Code and Codex, defaulting to the workspace's AI tool mode (#480)
- **Quick Claude "Both" mode** — New "Both (Claude + Codex)" option in the Quick Claude dialog (Ctrl+Shift+Q) that creates a vertical split with both tools side-by-side, sending the same prompt to each. Worktree branches use `-cc` / `-c` suffixes (#487)

### Fixed
- **AI Tool Mode via Ctrl+T** — Codex mode now executes `codex --yolo` and Both mode creates a vertical split with Claude + Codex when using the Ctrl+T keyboard shortcut (#480)
- **Binary framing for GridDiff on daemon→bridge pipe** — GridDiff events now use compact binary encoding (tag 0x04) instead of JSON serialization on the daemon→bridge named pipe, reducing per-diff payload by ~10x. Fixes low FPS (~18) and high input latency caused by JSON serialization saturating the bridge I/O loop (#481)
- **Terminal text invisible until tab switch** — Fixed a race condition where binary diff stream data arriving during the initial snapshot fetch created a deadlock, leaving `cachedSnapshot` permanently null (#486)

### Changed
- **Voice/Whisper decoupled from main installer** — godly-whisper is no longer built or bundled with the main app. Users who want voice-to-text can download a separate standalone installer from GitHub Releases (#484)

## [0.10.0] - 2026-03-01

### Added
- **Godly Flows — Visual Node-Graph Automation System (Phase 1)** — A flow engine that lets users compose terminal operations into reusable, triggerable workflows without writing code. Includes flow store with persistence, DAG-based execution engine, 12 built-in node types, hotkey triggers, settings tab UI, and MCP integration via `window.__FLOW_ENGINE__` (#459)
- **AI Tool Mode** — Workspaces now support multiple AI tool modes: None, Claude, Codex, and Both. Replaces the binary Claude Code toggle with a cycle button and right-click submenu. "Both" mode creates a vertical split with Claude and Codex terminals side by side (#467)
- **Binary GridDiff streaming** — Stream binary-encoded grid diffs via `stream://` custom protocol, eliminating Tauri JSON serialization overhead. Binary payload is ~10x smaller than JSON, with adaptive diff rate (3ms interactive / 16ms bulk) (#477)
- **Voice vocabulary hints** — Added initial_prompt with domain-specific terms and a custom vocabulary editor in settings to improve Whisper recognition of technical terms (#363)
- **MCP tab navigation tools** — `next_tab`, `previous_tab`, and `go_to_tab` for programmatic tab switching (#462)
- **MCP workspace mode tools** — `toggle_worktree_mode`, `toggle_claude_code_mode`, and `get_workspace_modes` (#463)
- **MCP notification settings tools** — `get_notification_config`, `set_notification_sound`, `add_mute_pattern`, `remove_mute_pattern`, and `list_mute_patterns` (#464)
- **MCP font/zoom tools** — `zoom_in`, `zoom_out`, `zoom_reset`, and `get_font_size` (#465)
- **MCP app control tools** — `open_settings`, `save_layout`, and `get_app_info` (#466)
- **MCP scrollback control tools** — `scroll_page_up`, `scroll_page_down`, `scroll_to_top`, `scroll_to_bottom`, and `get_scroll_position` (#468)
- **MCP workspace management tools** — `rename_workspace`, `reorder_workspaces`, `get_workspace_details`, and `open_in_explorer` (#469)
- **MCP split pane tools** — `focus_pane`, `focus_other_pane`, `resize_pane`, `set_split_ratio`, and `rotate_split` (#470)
- **MCP tab reorder and clipboard tools** — `reorder_tabs`, `get_tab_order`, `copy_to_clipboard`, and `get_selected_text` (#472)
- **MCP theme management tools** — `list_themes`, `get_active_theme`, and `set_theme` (#473)
- **MCP shell settings tools** — `list_available_shells`, `get_default_shell`, and `set_default_shell` (#474)

### Fixed
- **Split view lost on tab switch** — split views are now suspended (not destroyed) when `addTerminal()` clears the active layout, and `clearLayoutTree()` no longer deletes suspended splits (#426)
- **Gemini 3 empty content parse failure** — Branch Name AI no longer fails when gemini-3-flash-preview returns empty content after spending all tokens on thinking (#457)
- **MCP split panel tools don't update UI** — Added missing event listeners for split/unsplit/swap/zoom MCP tools (#455)
- **MCP `get_split_state` fails with "Pipe closed"** — Fixed serde tag collision between `McpResponse` and `LayoutNode` (#455)
- **Whisper test recording shows [object Object]** — The "Test Recording" button displayed `[object Object]` instead of transcribed text (#443)
- **Scrollback dirty-flag row-index mismatch** — diff snapshots no longer send garbled data when scrolled into history (#448)
- **Gemini 3 Flash Preview response parsing** — branch name AI handles optional candidate content, thinking parts, and non-JSON errors (#449)
- **Stale terminal IDs in persisted layout** — Added centralized pruning after terminal restoration (#450)

### Changed
- **Voice default model upgraded to Large v3 Turbo** — Switched from Base (142 MB) to Large v3 Turbo (1.5 GB) for significantly better transcription accuracy (#454)
- **Refactored daemon handlers** — Extracted 19 request handlers from `server.rs` into individual files (#453)
- **Refactored settings dialog** — Extracted 6 settings tabs into separate files with a registry pattern (#451)

## [0.9.0] - 2026-02-25

### Added
- **GPU terminal renderer** — new `godly-renderer` crate with wgpu-based GPU rendering pipeline, Tauri integration, and frontend renderer switching (#330, #343, #344, #345)
- **Branch name quality gate** — rejects garbage LLM-generated branch names with pattern validation (#342)

### Fixed
- **Selection grows when scrolling during active drag** — selection no longer expands incorrectly while scrolling (#340, #341)
- **WebGL context pool leak** — pool slots released on demote, promotes routed through pool to prevent ~1.6GB RAM leak (#339)
- **Glyph atlas overflow** — capped at 2048px and resets when full instead of overflowing (#337)
- **Phone UI scroll and touch UX** — improved scroll behavior and touch interactions on mobile (#338)
- **CI nextest config** — removed stale `zombie_tabs` binary reference that broke all CI jobs

## [0.8.0] - 2026-02-24

### Added
- **User-like testing framework** — JS bridge (`execute_js`), canvas screenshot capture, and split MCP tools for automated UI testing (#332)
- **Auto-focus terminal on MCP write operations** — terminal tab switches automatically when MCP sends input (#313)
- **PyAutoGUI MCP server** — OS-level mouse/keyboard automation for testing (#332)

### Fixed
- **Idle notification false positives** — added output volume threshold to prevent notifications during active output (#334)
- **Idle re-notification after cooldown** — allow re-notification when output→idle transitions occur after cooldown expires (#311)
- **WebView2 zoom black border** — disabled native Ctrl+scroll zoom that caused rendering artifacts (#328)
- **Stream reconnection reliability** — circuit-breaker pattern prevents thundering herd on reconnect failures (#315)
- **Stream reconnection jitter** — added randomized backoff to prevent synchronized reconnection storms (#320)
- **Stream:// cascade failure** — non-blocking protocol handler isolates stream handling from IPC thread pool (#326, #321)
- **OutputStreamRegistry contention** — sharded to per-session locks, eliminating cross-session mutex blocking (#322, #324)
- **Split view persistence** — split view stays alive during tab navigation (#310)

### Changed
- **Lazy WebGL context allocation** — WebGL contexts created only for visible terminals, reducing GPU memory (#323)
- **WebGL context pooling** — contexts recycled across terminal lifecycle to prevent exhaustion (#314)
- Consolidated screenshot callback into existing JsCallbackState channel
- Removed flaky `zombie_tabs` daemon tests pending CI reliability fix (#335)

### Tests
- Reproduction tests for stream:// cascade failure (#325)
- Split tab grouping regression tests (#310)
- Native zoom prevention tests (#328)
- Lazy WebGL allocation tests (#323)
- WebGL context pool tests (#314)
- Stream cascade failure tests (#325)
- Idle notification service tests (#334)

## [0.7.0] - 2026-02-24

### Added
- **Touch-to-scroll** for mobile terminal scrolling (#307)
- **Demo recording system** — fully automatic demo capture (#305)
- `no_worktree` option for Quick Claude spawning (#306)

### Fixed
- **Idle re-notification** — allow re-notification after cooldown expires for output-to-idle transitions
- **OSC titles** exposed to remote web console

### Changed
- Comprehensive README.md with full architecture documentation

## [0.6.0] - 2026-02-23

### Added
- **Phone remote access** — full mobile terminal control with godly-remote HTTP/WebSocket bridge, device lock, QR code setup, session death detection, and Remote settings tab (#240, #261, #270, #271, #282, #299)
- **Multi-session scalability** — pause/resume for invisible sessions, global scrollback budget, and canvas/WebGL resource release for hidden terminals (#243, #285, #287)
- **PTY shim crash isolation** — separate `godly-pty-shim` binary per session with bounded MPSC channels and orphan process cleanup (#231, #257, #268)
- **MCP transport upgrade** — SSE and Streamable HTTP transports for godly-mcp (#251, #254)
- **Terminal zoom** — keyboard (Ctrl+=/−) and Ctrl+scroll zoom in/out (#300)
- **Clipboard image paste** — paste images from clipboard as temp file paths (#298)
- **Mobile navigation buttons** with smart select menu detection (#291)
- **Peon Ping full Orc Peon sounds** + PeonPing registry browser (#275)
- **User-selectable models** for SmolLM2 plugin (#286)
- **Skill autocomplete** in Quick Claude dialog with workspace-aware refresh (#253, #277, #302)
- Integration tests for adaptive output batching (#264)
- Automated phone setup script with QR code sharing (#270)

### Fixed
- **Shim metadata isolation** — scoped by `GODLY_INSTANCE` to prevent test daemons killing production shims (#304)
- **Orphaned pty-shim process leak** — 200+ zombies and 10GB+ RAM prevented (#257)
- **CPU-burning spin loops** removed from bridge and daemon (#247, #248)
- **Resize freeze** — made resize fire-and-forget during maximize (#255)
- **Selection scroll anchor** — anchored to content position when scrolling (#252)
- **Viewport snap on typing** — scrolls to live view when typing while scrolled up (#241)
- **Multi-screen selection copy** fixed (#301)
- **Notification sound overlap** from multiple sources prevented (#296)
- **Tab bar wheel scroll** — vertical scroll translated to horizontal (#293)
- **Arrow up history latency** reduced (#229)
- Security hardening for godly-remote (high + medium/low severity) (#274, #280)
- Plugin card settings overlapping body content (#258)
- Skill autocomplete scans `.claude/commands/` directory (#302)

### Changed
- Adopted cargo-nextest with restructured CI and smart test runner (#235)
- Daemon test isolation now requires `GODLY_INSTANCE` env var for shim metadata directory scoping

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
