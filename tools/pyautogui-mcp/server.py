"""
PyAutoGUI MCP Server — OS-level UI automation for testing Godly Terminal.

Exposes mouse, keyboard, screenshot, and window management tools via MCP
so that Claude Code can drive the app like a real user.

Transports:
  stdio   (default)   — used by Claude Code directly
  --sse               — persistent HTTP server (default port 9742)
  --ensure            — spawn SSE server in background if not running, then exit
"""

from __future__ import annotations

import argparse
import os
import platform
import subprocess
import sys
import tempfile
import time
from datetime import datetime
from pathlib import Path
from typing import Optional

import pyautogui
from mcp.server.fastmcp import FastMCP

# ---------------------------------------------------------------------------
# Safety: moving mouse to any corner aborts the current pyautogui action.
# ---------------------------------------------------------------------------
pyautogui.FAILSAFE = True

# Slightly lower the default pause between pyautogui calls so automation
# feels snappy but still has a tiny breathing room.
pyautogui.PAUSE = 0.03

# ---------------------------------------------------------------------------
# MCP server instance
# ---------------------------------------------------------------------------
mcp = FastMCP(
    "pyautogui",
    description="OS-level UI automation via PyAutoGUI — screenshots, mouse, keyboard, window management",
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
TEMP_DIR = Path(tempfile.gettempdir()) / "pyautogui-mcp-screenshots"
TEMP_DIR.mkdir(parents=True, exist_ok=True)


def _timestamp() -> str:
    return datetime.now().strftime("%Y%m%d_%H%M%S_%f")


def _parse_hotkey(key: str) -> list[str]:
    """Split a combo like 'ctrl+shift+t' into ['ctrl', 'shift', 't']."""
    return [k.strip() for k in key.split("+")]


def _find_window(title_substring: str):
    """Return the first window whose title contains *title_substring* (case-insensitive).

    Uses pygetwindow on Windows, which is bundled with pyautogui.
    """
    import pygetwindow as gw  # type: ignore[import-untyped]

    needle = title_substring.lower()
    for win in gw.getAllWindows():
        if needle in (win.title or "").lower():
            return win
    return None


# ---------------------------------------------------------------------------
# Tools
# ---------------------------------------------------------------------------


@mcp.tool()
def screenshot(
    region_x: Optional[int] = None,
    region_y: Optional[int] = None,
    region_width: Optional[int] = None,
    region_height: Optional[int] = None,
) -> str:
    """Take a full-screen screenshot (or a specific region) and return the file path.

    To capture a region, pass all four region_* parameters.
    """
    region = None
    if all(v is not None for v in (region_x, region_y, region_width, region_height)):
        region = (region_x, region_y, region_width, region_height)

    filename = f"screenshot_{_timestamp()}.png"
    filepath = TEMP_DIR / filename

    img = pyautogui.screenshot(region=region)
    img.save(str(filepath))
    return str(filepath)


@mcp.tool()
def move_mouse(x: int, y: int, duration: float = 0.0) -> str:
    """Move the mouse cursor to (x, y). Set duration > 0 for smooth movement."""
    pyautogui.moveTo(x, y, duration=duration)
    return f"Mouse moved to ({x}, {y})"


@mcp.tool()
def click(
    x: Optional[int] = None,
    y: Optional[int] = None,
    button: str = "left",
    clicks: int = 1,
) -> str:
    """Click at (x, y) — or at the current position if coordinates are omitted.

    button: 'left', 'right', or 'middle'.
    clicks: number of consecutive clicks (2 for double-click).
    """
    kwargs: dict = {"button": button, "clicks": clicks}
    if x is not None and y is not None:
        kwargs["x"] = x
        kwargs["y"] = y
    pyautogui.click(**kwargs)
    pos = pyautogui.position()
    return f"Clicked {button}x{clicks} at ({pos.x}, {pos.y})"


@mcp.tool()
def drag(x: int, y: int, duration: float = 0.5, button: str = "left") -> str:
    """Drag from the current mouse position to (x, y) over *duration* seconds."""
    start = pyautogui.position()
    pyautogui.mouseDown(button=button, _pause=False)
    pyautogui.moveTo(x, y, duration=duration, _pause=False)
    pyautogui.mouseUp(button=button, _pause=False)
    return f"Dragged from ({start.x}, {start.y}) to ({x}, {y})"


@mcp.tool()
def drag_from_to(
    start_x: int,
    start_y: int,
    end_x: int,
    end_y: int,
    duration: float = 0.5,
    button: str = "left",
) -> str:
    """Drag from (start_x, start_y) to (end_x, end_y) over *duration* seconds."""
    pyautogui.moveTo(start_x, start_y, duration=0)
    pyautogui.mouseDown(button=button, _pause=False)
    pyautogui.moveTo(end_x, end_y, duration=duration, _pause=False)
    pyautogui.mouseUp(button=button, _pause=False)
    return f"Dragged from ({start_x}, {start_y}) to ({end_x}, {end_y})"


@mcp.tool()
def press_key(key: str) -> str:
    """Press a key or key combination.

    Examples: 'enter', 'tab', 'ctrl+c', 'alt+tab', 'ctrl+shift+t'.
    """
    parts = _parse_hotkey(key)
    if len(parts) == 1:
        pyautogui.press(parts[0])
    else:
        pyautogui.hotkey(*parts)
    return f"Pressed {key}"


@mcp.tool()
def type_text(text: str, interval: float = 0.02) -> str:
    """Type text character by character with *interval* seconds between keystrokes."""
    pyautogui.typewrite(text, interval=interval)
    return f"Typed {len(text)} characters"


@mcp.tool()
def get_mouse_position() -> dict:
    """Return the current mouse cursor position as {x, y}."""
    pos = pyautogui.position()
    return {"x": pos.x, "y": pos.y}


@mcp.tool()
def get_screen_size() -> dict:
    """Return the primary screen resolution as {width, height}."""
    w, h = pyautogui.size()
    return {"width": w, "height": h}


@mcp.tool()
def get_window_rect(title_substring: str) -> Optional[dict]:
    """Get the position and size of a window whose title contains *title_substring*.

    Returns {x, y, width, height, title} or null if not found.
    """
    win = _find_window(title_substring)
    if win is None:
        return None
    return {
        "x": win.left,
        "y": win.top,
        "width": win.width,
        "height": win.height,
        "title": win.title,
    }


@mcp.tool()
def focus_window(title_substring: str) -> str:
    """Bring a window to the foreground by title substring."""
    win = _find_window(title_substring)
    if win is None:
        return f"No window found matching '{title_substring}'"
    try:
        if win.isMinimized:
            win.restore()
        win.activate()
        return f"Focused window: {win.title}"
    except Exception as exc:
        return f"Found window '{win.title}' but could not activate: {exc}"


@mcp.tool()
def locate_on_screen(
    image_path: str,
    confidence: float = 0.9,
) -> Optional[dict]:
    """Find an image on screen using template matching.

    Returns {x, y, width, height} of the first match, or null if not found.
    confidence: matching threshold (0.0 - 1.0). Requires opencv-python for
    values < 1.0.
    """
    try:
        location = pyautogui.locateOnScreen(image_path, confidence=confidence)
    except pyautogui.ImageNotFoundException:
        return None
    if location is None:
        return None
    return {
        "x": int(location.left),
        "y": int(location.top),
        "width": int(location.width),
        "height": int(location.height),
    }


# ---------------------------------------------------------------------------
# Transport / CLI
# ---------------------------------------------------------------------------
DEFAULT_SSE_PORT = 9742


def _is_sse_server_running(port: int) -> bool:
    """Quick check whether the SSE server is listening on *port*."""
    import socket

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.settimeout(1)
        return s.connect_ex(("127.0.0.1", port)) == 0


def _ensure_sse_server(port: int) -> None:
    """Spawn the SSE server as a detached background process if it isn't already running."""
    if _is_sse_server_running(port):
        print(f"SSE server already running on port {port}")
        return

    cmd = [sys.executable, __file__, "--sse", "--port", str(port)]

    if platform.system() == "Windows":
        DETACHED_PROCESS = 0x00000008
        CREATE_NO_WINDOW = 0x08000000
        subprocess.Popen(
            cmd,
            creationflags=DETACHED_PROCESS | CREATE_NO_WINDOW,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            stdin=subprocess.DEVNULL,
        )
    else:
        subprocess.Popen(
            cmd,
            start_new_session=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            stdin=subprocess.DEVNULL,
        )

    # Wait briefly for it to come up.
    for _ in range(20):
        time.sleep(0.25)
        if _is_sse_server_running(port):
            print(f"SSE server started on port {port}")
            return

    print(f"Warning: SSE server may not have started on port {port}", file=sys.stderr)


def main() -> None:
    parser = argparse.ArgumentParser(description="PyAutoGUI MCP Server")
    parser.add_argument(
        "--sse",
        action="store_true",
        help="Run as a persistent SSE/HTTP server instead of stdio",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=DEFAULT_SSE_PORT,
        help=f"Port for SSE server (default: {DEFAULT_SSE_PORT})",
    )
    parser.add_argument(
        "--ensure",
        action="store_true",
        help="Ensure the SSE server is running (spawn if needed), then exit",
    )
    args = parser.parse_args()

    if args.ensure:
        _ensure_sse_server(args.port)
        return

    if args.sse:
        mcp.run(transport="sse", sse_params={"host": "127.0.0.1", "port": args.port})
    else:
        mcp.run(transport="stdio")


if __name__ == "__main__":
    main()
