#!/usr/bin/env bash
# Wrapper script for Claude Code MCP registration.
# 1. Ensures the SSE background server is running (spawns if needed).
# 2. Execs the stdio transport so Claude Code can talk to it directly.
SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
python "$SCRIPT_DIR/server.py" --ensure
exec python "$SCRIPT_DIR/server.py"
