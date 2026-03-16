### Fixed
- **Daemon and MCP debug log disk leaks** — Added runtime log rotation at 2MB with `.prev.log` backup, preventing unbounded log file growth. Previously logs only rotated at startup. Both daemon and MCP processes now track bytes written and rotate proactively.
- **Screenshot temp file accumulation** — Added cleanup of screenshot temp files older than 1 hour at the start of screenshot encoding. Temp directory (`%TEMP%/godly-screenshots/`) is now cleaned best-effort, preventing disk space exhaustion from long-running instances.
