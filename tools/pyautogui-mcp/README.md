# PyAutoGUI MCP Server

OS-level UI automation MCP server for testing Godly Terminal like a real user.
Provides screenshot capture, mouse control, keyboard input, and window management
via PyAutoGUI, exposed as MCP tools that Claude Code can call directly.

## Tools

| Tool | Description |
|---|---|
| `screenshot` | Capture full screen or a region, returns PNG file path |
| `move_mouse` | Move cursor to absolute coordinates |
| `click` | Click at position (left/right/middle, single/double) |
| `drag` | Drag from current position to target |
| `drag_from_to` | Drag between two absolute positions |
| `press_key` | Press a key or combo (`enter`, `ctrl+c`, `alt+tab`) |
| `type_text` | Type text character by character |
| `get_mouse_position` | Return current cursor coordinates |
| `get_screen_size` | Return primary monitor resolution |
| `get_window_rect` | Get window position/size by title substring |
| `focus_window` | Bring a window to the foreground by title |
| `locate_on_screen` | Find an image on screen (template matching) |

## Install

```bash
cd tools/pyautogui-mcp
pip install -r requirements.txt
```

For `locate_on_screen` with confidence < 1.0, also install OpenCV:

```bash
pip install opencv-python
```

## Usage

### stdio (default, for Claude Code)

```bash
python server.py
```

### SSE server (persistent HTTP, port 9742)

```bash
python server.py --sse
python server.py --sse --port 9800
```

### Ensure mode (spawn SSE if not running, then exit)

```bash
python server.py --ensure
```

## Register in Claude Code

Add to `~/.claude/mcp.json`:

```json
{
  "mcpServers": {
    "pyautogui": {
      "command": "bash",
      "args": ["tools/pyautogui-mcp/scripts/start-with-sse.sh"]
    }
  }
}
```

Or for direct stdio usage without the SSE background server:

```json
{
  "mcpServers": {
    "pyautogui": {
      "command": "python",
      "args": ["tools/pyautogui-mcp/server.py"]
    }
  }
}
```

## Safety

`pyautogui.FAILSAFE` is enabled: moving the mouse to any screen corner will
abort the current automation action. This is a safety net during testing.
