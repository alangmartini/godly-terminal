### Added
- **Frontend mode dispatch** — The Tauri app now checks `GODLY_FRONTEND_MODE` and routes to the native Iced shell when mode is `Native`. Falls back to the web frontend if the native binary is not found.
