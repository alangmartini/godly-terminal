### Fixed
- **MCP write_to_terminal timeout under load** — switched MCP handler from blocking `send_request()` to `send_fire_and_forget()` for write operations, preventing 15-second timeouts when the bridge I/O thread is congested (#534)
