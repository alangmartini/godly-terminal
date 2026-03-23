### Added
- **Crash handler for iced-shell** — Panics, wgpu failures, and Windows structured exceptions (access violations, stack overflows) now log to `iced-crash.log` in the app data directory instead of vanishing silently.
