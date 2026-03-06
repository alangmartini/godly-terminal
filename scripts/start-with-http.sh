#!/bin/bash
# Wrapper script for Claude Code MCP integration.
# 1. Ensures the HTTP server is running in background (--ensure)
# 2. Falls back to stdio for this session (Claude Code needs stdio for MCP handshake)
EXE="C:/Users/alanm/Documents/dev/godly-claude/godly-terminal/src-tauri/target/release/godly-mcp.exe"
"$EXE" --ensure 2>/dev/null
exec "$EXE"
