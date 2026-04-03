### Added
- **godly-shell crate** — new winit + wgpu native shell replacing Iced framework
- **Async event bus** — EventLoopProxy-based event delivery solving Iced's minimize-freeze bug
- **GPU terminal rendering** — direct atlas pipeline integration without Iced shader widget wrapper
- **UI widget system** — quad renderer, title bar, tab bar, status bar chrome
- **Keyboard shortcut routing** — app shortcuts checked before PTY forwarding

### Changed
- **terminal-surface decoupled from Iced** — framework-agnostic Color type, standalone GPU pipelines
- **app-adapter decoupled from Iced** — custom keyboard types replacing iced::keyboard

### Removed
- **Canvas CPU fallback** — surface.rs deleted, going GPU-only
