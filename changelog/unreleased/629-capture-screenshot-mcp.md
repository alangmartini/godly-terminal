### Added
- **Native `capture_screenshot` MCP tool** — godly-mcp can now capture native iced shell window screenshots via the `capture_screenshot` command, saving PNG-encoded RGBA data to disk for inspection/testing

### Changed
- **MCP request routing** — `CaptureScreenshot` requests now route through explicit handler instead of catch-all, allowing native-specific implementation without affecting other shells
