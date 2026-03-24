### Added
- **GPU crash observability** — iced-shell now logs GPU device loss detection via D3D11 watchdog thread, expanded SEH exception handler to catch GPU/driver-related exceptions (TDR, device removal, driver crashes), and atexit handler to catch silent process exits. Enables diagnosis of crashes that occur while terminal is in background state.
