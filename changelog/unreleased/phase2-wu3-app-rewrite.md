### Added
- **Multi-terminal support** — multiple terminal sessions with tab switching in the native Iced shell
- **Event-driven rendering** — replaced 60fps AtomicBool polling with channel-based daemon event delivery
- **Window resize handling** — terminal grid auto-resizes when the window is resized
- **Tab bar UI** — clickable tabs with add/close terminal support

### Changed
- **App architecture rewrite** — `GodlyApp` now manages `TerminalCollection` instead of a single session
