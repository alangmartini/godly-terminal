#!/usr/bin/env python3
"""
GUI Bridge for demo recording.
Accepts JSON commands via stdin, executes pyautogui actions.

Protocol: one JSON object per line, responds with one JSON per line on stdout.

Commands:
  move(x, y, duration?)       - smooth mouse movement
  click(x, y, clicks?, button?)- click at position
  type(text, interval?)       - human-like typing (with random jitter)
  hotkey(keys)                - keyboard shortcut (e.g. ["ctrl", "shift", "t"])
  press(key)                  - single key press
  scroll(clicks, x?, y?)     - mouse scroll
  screenshot(path?)           - take screenshot, return path
  locate(image, confidence?)  - find image on screen, return {x, y, w, h}
  window(title, action?)      - find/focus/move window
  cursor()                    - return current cursor position
  ping()                      - health check
"""

import json
import sys
import time
import random
import os
import threading
import ctypes

import pyautogui

# Safety: no pause between actions (we control timing from the orchestrator)
pyautogui.PAUSE = 0
# Don't move to corner to abort
pyautogui.FAILSAFE = False

# Stop hotkey: F10
STOP_REQUESTED = False

def _hotkey_listener():
    """Background thread that listens for F10 (VK 0x79) via GetAsyncKeyState."""
    global STOP_REQUESTED
    user32 = ctypes.windll.user32
    VK_F10 = 0x79
    while not STOP_REQUESTED:
        # High bit set = key is currently pressed
        if user32.GetAsyncKeyState(VK_F10) & 0x8000:
            STOP_REQUESTED = True
            # Push a stop event to stdout so Node.js knows immediately
            sys.stdout.write(json.dumps({"stop": True, "reason": "F10 pressed"}) + "\n")
            sys.stdout.flush()
            break
        time.sleep(0.1)

threading.Thread(target=_hotkey_listener, daemon=True).start()


def jittered_interval(base=0.06):
    """Human-like typing interval with gaussian jitter."""
    return max(0.02, random.gauss(base, base * 0.3))


def human_type(text, interval=0.06):
    """Type text with human-like timing — variable speed, occasional pauses."""
    for i, char in enumerate(text):
        pyautogui.press(char) if len(char) == 1 else pyautogui.hotkey(char)
        # Occasional longer pause (thinking)
        if random.random() < 0.05 and i > 0:
            time.sleep(random.uniform(0.15, 0.4))
        else:
            time.sleep(jittered_interval(interval))


def handle_command(cmd):
    action = cmd.get("action")

    if action == "ping":
        size = pyautogui.size()
        return {"ok": True, "screenWidth": size.width, "screenHeight": size.height}

    elif action == "move":
        x, y = cmd["x"], cmd["y"]
        duration = cmd.get("duration", 0.4)
        pyautogui.moveTo(x, y, duration=duration, tween=pyautogui.easeInOutQuad)
        return {"ok": True, "x": x, "y": y}

    elif action == "click":
        x, y = cmd.get("x"), cmd.get("y")
        clicks = cmd.get("clicks", 1)
        button = cmd.get("button", "left")
        if x is not None and y is not None:
            pyautogui.click(x, y, clicks=clicks, button=button)
        else:
            pyautogui.click(clicks=clicks, button=button)
        return {"ok": True}

    elif action == "type":
        text = cmd["text"]
        interval = cmd.get("interval", 0.06)
        human_type(text, interval)
        return {"ok": True, "length": len(text)}

    elif action == "hotkey":
        keys = cmd["keys"]
        pyautogui.hotkey(*keys)
        return {"ok": True, "keys": keys}

    elif action == "press":
        key = cmd["key"]
        presses = cmd.get("presses", 1)
        interval = cmd.get("interval", 0.05)
        pyautogui.press(key, presses=presses, interval=interval)
        return {"ok": True, "key": key}

    elif action == "scroll":
        clicks = cmd["clicks"]
        x, y = cmd.get("x"), cmd.get("y")
        pyautogui.scroll(clicks, x=x, y=y)
        return {"ok": True}

    elif action == "screenshot":
        path = cmd.get("path", f"demo-output/screenshot-{int(time.time())}.png")
        os.makedirs(os.path.dirname(path), exist_ok=True)
        pyautogui.screenshot(path)
        return {"ok": True, "path": path}

    elif action == "locate":
        image = cmd["image"]
        confidence = cmd.get("confidence", 0.8)
        try:
            loc = pyautogui.locateOnScreen(image, confidence=confidence)
            if loc:
                center = pyautogui.center(loc)
                return {"ok": True, "found": True,
                        "x": center.x, "y": center.y,
                        "left": loc.left, "top": loc.top,
                        "width": loc.width, "height": loc.height}
            return {"ok": True, "found": False}
        except Exception as e:
            return {"ok": False, "error": str(e)}

    elif action == "window":
        title = cmd["title"]
        sub = cmd.get("sub_action", "focus")
        try:
            # Use ctypes directly for reliable window activation on Windows.
            # pygetwindow.activate() often fails with cryptic Windows errors.
            user32 = ctypes.windll.user32

            # EnumWindows: collect all matches, prefer exact title start
            candidates = []
            def _enum_cb(hwnd, _):
                length = user32.GetWindowTextLengthW(hwnd)
                if length > 0:
                    buf = ctypes.create_unicode_buffer(length + 1)
                    user32.GetWindowTextW(hwnd, buf, length + 1)
                    wt = buf.value
                    tl = title.lower()
                    wl = wt.lower()
                    if wl == tl:
                        candidates.append((0, hwnd, wt))  # exact match (best)
                    elif wl.startswith(tl):
                        candidates.append((1, hwnd, wt))  # starts with
                    elif tl in wl:
                        candidates.append((2, hwnd, wt))  # substring
                return True

            WNDENUMPROC = ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)
            user32.EnumWindows(WNDENUMPROC(_enum_cb), 0)
            candidates.sort(key=lambda c: c[0])  # best match first

            found_hwnd = candidates[0][1] if candidates else None

            if not found_hwnd:
                return {"ok": False, "error": f"Window '{title}' not found"}

            if sub == "focus":
                # Simulate Alt key press to bypass Windows foreground restriction,
                # then SetForegroundWindow. Do NOT minimize/restore — that
                # un-maximizes the window and changes viewport coordinates.
                VK_MENU = 0x12
                KEYEVENTF_EXTENDEDKEY = 0x0001
                KEYEVENTF_KEYUP = 0x0002
                user32.keybd_event(VK_MENU, 0, KEYEVENTF_EXTENDEDKEY, 0)
                user32.SetForegroundWindow(found_hwnd)
                user32.keybd_event(VK_MENU, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0)

            # Get window rect
            class RECT(ctypes.Structure):
                _fields_ = [("left", ctypes.c_long), ("top", ctypes.c_long),
                            ("right", ctypes.c_long), ("bottom", ctypes.c_long)]
            rect = RECT()
            user32.GetWindowRect(found_hwnd, ctypes.byref(rect))

            # Get window title
            length = user32.GetWindowTextLengthW(found_hwnd)
            buf = ctypes.create_unicode_buffer(length + 1)
            user32.GetWindowTextW(found_hwnd, buf, length + 1)

            return {"ok": True, "title": buf.value,
                    "x": rect.left, "y": rect.top,
                    "width": rect.right - rect.left, "height": rect.bottom - rect.top}
        except Exception as e:
            return {"ok": False, "error": str(e)}

    elif action == "cursor":
        pos = pyautogui.position()
        return {"ok": True, "x": pos.x, "y": pos.y}

    elif action == "sleep":
        time.sleep(cmd.get("ms", 1000) / 1000.0)
        return {"ok": True}

    else:
        return {"ok": False, "error": f"Unknown action: {action}"}


def main():
    # Signal ready
    sys.stdout.write(json.dumps({"ready": True}) + "\n")
    sys.stdout.flush()

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            cmd = json.loads(line)
            result = handle_command(cmd)
        except Exception as e:
            result = {"ok": False, "error": str(e)}

        sys.stdout.write(json.dumps(result) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
