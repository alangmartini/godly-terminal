# Native (Iced) Parity Plan

Tracks all work needed to bring Godly Terminal Native (Iced shell) to feature parity with the TypeScript/Tauri frontend.

## Parallelization Strategy

Tasks are grouped into **independent work streams** that can run simultaneously. Each stream touches different files/crates with minimal overlap. Agents should claim one stream at a time.

---

## Stream A: Session Persistence & Recovery (CRITICAL)

**Files**: `iced-shell/src/lib.rs`, `app-adapter/src/`, `features-shell/`
**Depends on**: Nothing
**Priority**: P0

- [x] A1. Serialize layout tree + workspace state on app close
- [x] A2. Detect existing daemon sessions on startup (`InitResult::Recovered`)
- [x] A3. Restore terminals, workspaces, splits, and focused state from saved session
- [x] A4. Scrollback restoration from daemon on reconnect
- [x] A5. Auto-save interval (5 min) for crash recovery
- [x] A6. Handle stale sessions (daemon alive but terminals dead)

---

## Stream B: Mouse Interactions — Drag & Drop + Divider Resize

**Files**: `iced-shell/src/tab_bar.rs`, `iced-shell/src/lib.rs` (layout rendering), `iced-shell/src/sidebar.rs`
**Depends on**: Nothing
**Priority**: P0

- [x] B1. Tab drag-drop reorder in tab bar
- [x] B2. Split divider drag to resize panes (replace fixed 0.5 ratio)
- [x] B3. Drag tab onto split zones (left/right/top/bottom overlay)
- [x] B4. Drag tab onto sidebar workspace to move terminal cross-workspace
- [x] B5. Tab overflow: horizontal scroll when many tabs + mouse wheel scroll
- [x] B6. File drag-drop onto terminal (paste quoted paths)

---

## Stream C: Settings Dialog (Parity Tabs)

**Files**: `iced-shell/src/settings_dialog.rs` (new subtabs), `app-adapter/src/`
**Depends on**: Nothing
**Priority**: P1

- [x] C1. **Terminal tab**: default shell picker (Windows/PS7/Cmd/WSL/Custom), WSL distro selector, font size slider, auto-scroll toggle, confirm-quit toggle
- [x] C2. **Themes tab**: theme list with color previews, click-to-apply, custom theme import/export
- [x] C3. **Notifications tab**: enable/disable sounds, volume slider, sound preset picker, test button, idle detection settings, workspace mute patterns
- [x] C4. **Custom keybindings tab**: make shortcuts editable (capture key combo), reset to defaults, search/filter
- [x] C5. **Quick Claude tab**: preset editor with launch step sequences, layout picker
- [x] C6. **AI Tools tab**: register custom AI tools, display name/icon/launch config
- [x] C7. **Plugins tab**: install/remove/enable/disable plugins
- [x] C8. **Flows tab**: automation workflow editor
- [x] C9. **Remote tab**: SSH connection settings

**Note**: C1-C4 were high priority. C5-C9 are now complete.

---

## Stream D: Notification System (Audio + Visual)

**Files**: `iced-shell/src/tab_bar.rs` (badges), `iced-shell/src/sidebar.rs` (badges), `app-adapter/src/` (audio), new `notifications.rs`
**Depends on**: Nothing
**Priority**: P1

- [x] D1. Render notification red dot badges on inactive tabs
- [x] D2. Render notification indicator on workspace items in sidebar
- [x] D3. Audio playback for bell events (use rodio or system beep)
- [x] D4. Sound preset support (chime, bell, ping, etc.)
- [x] D5. Per-workspace mute patterns
- [x] D6. Toast notification overlay (auto-dismiss after 4s)
- [x] D7. Native Windows taskbar flash on notification when unfocused
- [x] D8. Sound debounce (2s per terminal, 500ms global)

---

## Stream E: Tab Bar Polish

**Files**: `iced-shell/src/tab_bar.rs`
**Depends on**: Nothing
**Priority**: P1

- [x] E1. Tab renaming dialog (F2 / double-click, wire up existing stub)
- [x] E2. Tab pinning (context menu, prevent close, visual indicator)
- [x] E3. Dead terminal indicator (exit code overlay, dimmed styling)
- [x] E4. Process icon detection (Claude 💬 / Codex 🤖 icons in tab)
- [x] E5. Tab context menu (right-click: rename, pin, split, copy info, close)
- [x] E6. MRU tab switcher popup (visual popup + keyboard MRU cycling via Ctrl+Tab/Ctrl+Shift+Tab)

---

## Stream F: Theme System

**Files**: new `iced-shell/src/theme.rs` (expand), `iced-shell/src/` (all UI files use colors)
**Depends on**: Nothing (but integrates with C2 for settings UI)
**Priority**: P1

- [ ] F1. Extract all hardcoded Dusk colors into a Theme trait/struct
- [ ] F2. Implement 11 built-in themes (Tokyo Night, Dracula, Nord, etc.)
- [ ] F3. Dynamic theme switching without restart
- [ ] F4. Terminal palette colors (16 ANSI + bright variants) per theme
- [x] F5. Custom theme JSON import/export
- [x] F6. CSS-variable-like design token system for UI + terminal colors (Done — Rust `BG_PRIMARY()`, `TEXT_PRIMARY()`, spacing constants are the Iced equivalent of CSS variables)

---

## Stream G: Terminal Pane Features

**Files**: `iced-shell/src/lib.rs` (terminal view), `terminal-surface/src/surface.rs`, `iced-shell/src/selection.rs`
**Depends on**: Nothing
**Priority**: P2

- [ ] G1. Terminal right-click context menu (copy, paste, select all, clear)
- [ ] G2. Copy clean mode (strip ANSI codes)
- [ ] G3. URL detection + hover highlighting + click-to-open
- [ ] G4. Search/find in terminal (Ctrl+F, regex support)
- [ ] G5. Cursor style rendering (block, underline, bar)
- [ ] G6. Visual scrollbar on right edge (draggable)
- [ ] G7. Performance overlay toggle (Ctrl+Shift+O)

---

## Stream H: Shell Type & Workspace Creation

**Files**: `iced-shell/src/sidebar.rs`, `iced-shell/src/lib.rs`, `app-adapter/src/`
**Depends on**: Nothing (but integrates with C1)
**Priority**: P1

- [ ] H1. Shell type selection dialog on workspace creation (Windows/PS7/Cmd/WSL/Custom)
- [ ] H2. WSL distribution picker (enumerate installed distros)
- [ ] H3. Custom shell program + arguments input
- [ ] H4. AI tool mode per workspace (none/claude/codex/both)
- [ ] H5. Auto-launch Claude/Codex on terminal create when mode set
- [ ] H6. Workspace icon in sidebar reflecting mode (💬/🤖/⚡)

---

## Stream I: AI & Automation Features

**Files**: new modules in `iced-shell/src/`, `app-adapter/src/`
**Depends on**: H4-H5 (AI tool mode)
**Priority**: P2

- [ ] I1. Quick Claude multi-agent launcher (preset system)
- [ ] I2. Launch step sequences (create terminal, run cmd, wait, send prompt)
- [ ] I3. Layout options for Quick Claude (single, vsplit, hsplit, 2x2)
- [x] I4. Voice input (Whisper integration, Ctrl+Shift+M)
- [x] I5. Recording UI (level bar, timer, transcription toast)
- [ ] ~~I6. Figma pane embedding (iframe-like webview in split)~~ — Skipped (not worth implementing)

---

## Stream J: MCP Event Integration

**Files**: `app-adapter/src/`, `iced-shell/src/lib.rs`
**Depends on**: Core features (A, B, E)
**Priority**: P2

- [ ] J1. Handle `focus-terminal` event
- [ ] J2. Handle `switch-workspace` event
- [ ] J3. Handle `terminal-renamed` event
- [ ] J4. Handle `mcp-terminal-created` (add to Agent workspace)
- [ ] J5. Handle `mcp-terminal-closed`
- [ ] J6. Handle `mcp-terminal-moved`
- [ ] J7. Handle `mcp-notify` (trigger notification)
- [ ] J8. Handle `mcp-split-terminal` / `mcp-unsplit-terminal`
- [ ] J9. Handle `mcp-swap-panes` / `mcp-zoom-pane`

---

## Stream L: UI Polish & Visual Quality (UGLY → PRETTY)

**Files**: All `iced-shell/src/*.rs`, `theme.rs`
**Depends on**: Nothing (can start immediately, F enhances it later)
**Priority**: P0 — the app looks rough right now

### Tab Bar
- [x] L1. Rounded tab shapes with smooth hover/active transitions (not flat rectangles)
- [x] L2. Active tab glow/accent underline (match TypeScript's gold accent bar)
- [x] L3. Close button (×) hover highlight, only visible on hover or active tab
- [x] L4. Tab icon spacing and alignment (process icon + title + close button)
- [x] L5. "+" button styling with hover state
- [x] L6. Tab separator lines between inactive tabs
- [x] L7. Smooth tab width transitions when opening/closing tabs

### Sidebar
- [x] L8. Workspace item padding, rounded corners, subtle hover background
- [x] L9. Active workspace: left accent border + background highlight (not just border)
- [x] L10. Settings gear icon + "+" button proper icon rendering (not text)
- [x] L11. "WORKSPACES" header typography (smaller, uppercase, letter-spaced, muted color)
- [x] L12. Terminal count badge: pill shape, muted background, small font
- [x] L13. Sidebar resize handle (or at minimum consistent width)
- [x] L14. Smooth sidebar collapse/expand animation (Ctrl+B)

### Title Bar
- [x] L15. Proper window title bar styling (process name — Godly Terminal)
- [x] L16. Window control buttons (minimize, maximize, close) matching theme
- [x] L17. Title bar drag area for window move

### Settings Dialog
- [x] L18. Rounded modal with backdrop blur/dim
- [x] L19. Tab navigation styled as pill buttons or underlined tabs
- [x] L20. Form inputs: styled text fields, dropdowns, sliders, toggles
- [x] L21. Consistent padding/margins throughout dialog
- [x] L22. Scrollable content area within dialog

### Terminal Area
- [x] L23. Split divider: thin line with grab cursor, highlight on hover
- [x] L24. Focused pane: subtle border or glow to indicate which pane is active
- [x] L25. Empty state: "No terminals open" placeholder with create hint
- [x] L26. Terminal padding (small inset from edges, like real terminal apps)

### General
- [x] L27. Consistent font family across UI (Inter/system-ui for chrome, monospace for terminal)
- [x] L28. Color contrast audit: ensure all text meets WCAG AA on its background
- [x] L29. Spacing system: consistent 4/8/12/16/24px spacing scale
- [x] L30. Border radius system: consistent 4/6/8px radii
- [x] L31. Shadow/elevation for floating elements (context menus, dialogs, toasts)
- [x] L32. Transition/animation timing: 150ms ease for hovers, 200ms for state changes

---

## Stream K: CLAUDE.md Editor & Misc Dialogs

**Files**: `iced-shell/src/` (new dialog modules)
**Depends on**: Nothing
**Priority**: P3

- [ ] K1. CLAUDE.md editor dialog (sidebar buttons for project + user)
- [ ] K2. Quit confirmation dialog (active sessions warning)
- [ ] K3. Copy dialog (preview with clean/normal mode toggle)
- [ ] K4. Figma URL prompt dialog

---

## Suggested Agent Assignment (max parallelism)

| Agent | Streams | Rationale |
|---|---|---|
| Agent 1 | **A** (Session Persistence) | Critical path, complex async work |
| Agent 2 | **B** (Drag & Drop) | Heavy Iced widget work, independent |
| Agent 3 | **C1-C4** (Settings core tabs) | UI-heavy, self-contained |
| Agent 4 | **D** (Notifications) | Audio + visual badges, independent |
| Agent 5 | **E + F** (Tab polish + Themes) | Related visual work, shared tab_bar.rs |
| Agent 6 | **G** (Terminal pane features) | Surface rendering, selection |
| Agent 7 | **H** (Shell type + workspace) | Sidebar + adapter, fast wins |
| Agent 8 | **L** (UI Polish) | Visual quality pass, touches all UI files |

Streams I, J, K are lower priority and can be picked up after the above complete.
Stream L (UI polish) should start early — it's P0 and highly visible.

## Completion Criteria

Parity is achieved when a user cannot distinguish the Iced shell from the TypeScript frontend for all daily workflows:
- Create/manage workspaces with shell type selection
- Tab management (create, close, rename, reorder, pin)
- Split panes with mouse resize
- Session survives app restart
- Notifications with sound
- Theme switching
- Customizable keybindings
- All MCP events handled

---

## Progress Log — 2026-03-04 (P0 Sprint)

### Parallel execution summary
- Orchestrated implementation across **6 parallel agents** (platform capped concurrent worker creation; attempted 8).
- All agent outputs were integrated into `src-tauri/native/iced-shell` and compiled together.

### Completed in this sprint
- **Stream A**: A1, A2, A3, A5, A6 completed.
- **Stream B**: B2 completed.
- **Stream C**: C1, C2, C3, C4 completed.
- **Stream D**: D1, D8 completed.
- **Stream E**: E2, E3, E4 completed.
- **Stream L**: L1, L2, L8, L9, L11, L12, L23 completed.

### Validation performed
- Native crate validation:
  - `cargo check -p godly-iced-shell --manifest-path src-tauri/Cargo.toml` passed.
  - `cargo test -p godly-iced-shell --manifest-path src-tauri/Cargo.toml` passed (`142` tests).
- Staging build + install:
  - `pnpm run staging:build` passed.
  - `pnpm run staging:install` passed.
  - Artifacts produced:
    - `installations/staging/Godly Terminal (Staging)_0.12.0_x64-setup.exe`
    - `installations/staging/Godly Terminal (Staging)_0.12.0_x64_en-US.msi`
  - Installed to:
    - `C:\Users\alanm\AppData\Local\Godly Terminal (Staging)\`

### Important integration notes for next session
- Sidebar resize is now wired in `view_sidebar` + app state (`sidebar_width`, drag start/end messages), with min/max clamping and resize-on-release behavior.
- Notification/audio parity path is implemented (workspace indicators + sound presets + debounce + playback bridge + toast overlay).
- Scrollback recovery now restores persisted offsets on reconnect and prunes stale entries.
- Some non-blocking warnings remain from test enums and unused theme constants; build/test is green.

### Recommended next P0/P1 pickup order
1. L14 + L24/L25/L26 (sidebar animation and pane polish are still pending on `master` as of 2026-03-06)
2. L7, L15-L17, L27-L32 (remaining high-visibility UI polish backlog after the 2026-03-05 tab bar/E6 batch)

## Progress Log — 2026-03-05 (Checklist Sync)

### Completed in this update
- **Stream D**: D7 completed (native window attention request now uses critical attention on Windows for taskbar flash behavior when unfocused).
- **Stream E**: E6 MRU parity completed end-to-end (`Ctrl+Tab` / `Ctrl+Shift+Tab` keyboard semantics plus visual popup switcher).
- **Stream L**: L4, L5, L6, L10, L18, L19, L20, L21, L22 completed.

## Progress Log — 2026-03-06 (I1-I3 Quick Claude Preset Launcher)

### Completed in this update
- **Stream I**: I1, I2, I3 completed (Quick Claude preset launcher with multi-agent support, launch step sequences, and layout arrangements).
- New module `quick_claude.rs` with LaunchStep enum, default_launch_steps builder, and step execution logic.
- Launch button on preset cards, status indicator during launch, cancel support.
- Layout finalization: Single, VSplit (horizontal), HSplit (vertical), Grid2x2.

## Progress Log — 2026-03-06 (L14 + L24-L32 UI Polish)

### Completed in this update
- **Stream L**: L7, L14, L15-L17, L24-L32 completed (tab animation, title bar, sidebar animation, pane borders, empty state, design tokens).

## Progress Log — 2026-03-06 (F5+F6 Custom Theme Import/Export)

### Completed in this update
- **F5**: Custom theme JSON import/export with serde support for `iced::Color`, file dialog via `rfd`, persistence to `custom-themes.json`, and full UI in Appearance settings tab.
- **F6**: Marked done — Rust `BG_PRIMARY()`, `TEXT_PRIMARY()`, spacing/radius constants are the Iced equivalent of CSS variables.
## Progress Log — 2026-03-06 (I4-I5 Voice/Whisper Integration)

### Completed in this update
- **Stream I**: I4, I5 completed (WhisperService sidecar in app-adapter, recording overlay UI with level meter/timer/stop/cancel, mic button in tab bar, Ctrl+Shift+M shortcut, toast on transcription).
- **Stream I**: I6 skipped (Figma pane embedding not worth implementing).
