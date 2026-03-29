---
name: test-feature
description: >
  Autonomously test godly-terminal features using PyAutoGUI GUI automation.
  Use this skill when the user asks to "test a feature", "check the UI",
  "verify rendering", "see if it looks right", "check quality", "test the app",
  "validate changes", "smoke test", "try it out", "does it work", "QA this",
  or any request involving visually inspecting or interacting with the running
  godly-terminal application. Also use when the user says "build and test",
  "launch and check", or asks about rendering quality, visual bugs, or UI correctness.
allowed-tools: Bash, Read, Glob, Grep, Write, Edit, Agent
---

# Autonomous Feature Testing for Godly Terminal

You can build, launch, and test godly-terminal like a real user using PyAutoGUI
for OS-level GUI automation (mouse, keyboard, screenshots, window management).

## Environment Facts

- **Platform**: Windows 11, Git Bash shell
- **Screen**: 150% DPI scaling — pyautogui uses logical coordinates (≈1694×843)
- **Python**: 3.12 with pyautogui, pygetwindow, pyperclip, Pillow installed
- **Rust**: 1.94.1, MSVC 14.40 linker

## Build the Project

Every cargo command needs MSVC environment variables. Use this preamble:

```bash
export MSVC_VER="14.40.33807"
export VS_BASE="/c/Program Files/Microsoft Visual Studio/2022/Community"
export MSVC_BASE="$VS_BASE/VC/Tools/MSVC/$MSVC_VER"
export SDK_BASE="/c/Program Files (x86)/Windows Kits/10"
export SDK_VER="10.0.22621.0"
export PATH="$MSVC_BASE/bin/Hostx64/x64:$SDK_BASE/bin/$SDK_VER/x64:$PATH"
export LIB="$MSVC_BASE/lib/x64;$SDK_BASE/Lib/$SDK_VER/ucrt/x64;$SDK_BASE/Lib/$SDK_VER/um/x64"
export INCLUDE="$MSVC_BASE/include;$SDK_BASE/Include/$SDK_VER/ucrt;$SDK_BASE/Include/$SDK_VER/um;$SDK_BASE/Include/$SDK_VER/shared"
```

Build order (all from `/c/Users/User/godly-terminal/src-tauri`):

1. `cargo build -p godly-daemon` — background terminal manager
2. `cargo build -p godly-pty-shim` — per-session PTY wrapper (required!)
3. `cargo build -p godly-mcp` — MCP server (optional, for programmatic testing)
4. `cargo build -p godly-iced-shell` — the GUI app (longest build, ~90s cold)

Binaries land in `src-tauri/target/debug/`:
- `godly-native.exe` (the GUI)
- `godly-daemon.exe`
- `godly-pty-shim.exe`
- `godly-mcp.exe`

## Launch the App

```bash
/c/Users/User/godly-terminal/src-tauri/target/debug/godly-native.exe &
sleep 5
tasklist.exe 2>/dev/null | grep -i godly
```

Expect 3 processes: `godly-native.exe`, `godly-daemon.exe`, `godly-pty-shim.exe`.

## Kill the App

Git Bash mangles `/IM`, so use cmd:

```bash
cmd //C "taskkill /IM godly-native.exe /F" 2>&1
cmd //C "taskkill /IM godly-daemon.exe /F" 2>&1
cmd //C "taskkill /IM godly-pty-shim.exe /F" 2>&1
```

## PyAutoGUI Interaction Patterns

All automation goes through inline Python scripts. Key patterns:

### Take a screenshot and view it

```python
python -c "
import pyautogui, tempfile, os
path = os.path.join(tempfile.gettempdir(), 'godly-screenshot.png')
pyautogui.screenshot().save(path)
print(path)
"
```

Then use the Read tool on the returned path to see the image.

### Focus the Godly Terminal window

```python
python -c "
import pyautogui, pygetwindow as gw, time
wins = [w for w in gw.getAllWindows() if 'Godly Terminal' in (w.title or '')]
if wins:
    win = wins[0]
    win.maximize()
    time.sleep(0.3)
    win.activate()
    time.sleep(0.5)
    print(f'Focused: {win.title} at ({win.left},{win.top}) {win.width}x{win.height}')
else:
    print('Godly Terminal not found')
"
```

### Type text into the terminal (MUST use clipboard paste)

**NEVER use `pyautogui.typewrite()`** — the terminal drops keystrokes. Always use clipboard:

```python
python -c "
import pyautogui, pyperclip, time
pyperclip.copy(r'echo hello world')
pyautogui.hotkey('ctrl', 'v')
time.sleep(0.3)
pyautogui.press('enter')
"
```

### Click a UI element

```python
python -c "
import pyautogui, time
pyautogui.click(265, 75)  # x, y in logical coords
time.sleep(1)
"
```

### Capture a region for precise element location

```python
python -c "
import pyautogui, tempfile, os
path = os.path.join(tempfile.gettempdir(), 'region.png')
pyautogui.screenshot(region=(x, y, width, height)).save(path)
print(path)
"
```

### Keyboard shortcuts

```python
pyautogui.hotkey('ctrl', 'shift', 't')  # example: new tab
pyautogui.press('enter')
pyautogui.hotkey('ctrl', 'c')           # interrupt
```

## Critical Rules

1. **Always maximize the window first** — coordinates shift otherwise
2. **Always use clipboard paste for text input** — `typewrite()` drops characters
3. **Always take a screenshot after each action** and Read the file to verify
4. **Capture small regions** to locate precise button positions
5. **Dismiss taskbar previews** by clicking the app body before clicking small UI elements
6. **Use `time.sleep()`** between actions — minimum 0.3s for clicks, 2s after commands
7. **DPI scaling**: coordinates are logical (150% scaling), not physical pixels
8. **Windows paths in Python**: use raw strings `r'C:\path'` or forward slashes

## Standard UI Layout (when maximized)

The window is maximized to fill the screen (~1694×843 logical pixels):

- **Title bar**: y ≈ 0–25, shows "Godly Terminal — {workspace name}"
- **Sidebar**: x ≈ 0–280
  - **WORKSPACES header**: y ≈ 55–80, gear icon at ~(220, 68), "+" at ~(265, 68)
  - **Workspace items**: y ≈ 85+ (each ~50px tall)
- **Tab bar**: x ≈ 290–1500, y ≈ 55–80
  - **"+" new tab button**: far right at ~(1530, 55)
- **Terminal area**: x ≈ 290–1694, y ≈ 85–680
- **Status bar**: y ≈ 680–700

These are approximate — always screenshot and verify before clicking.

## Testing Workflow

1. **Build** (if needed) — use the MSVC preamble + cargo build
2. **Launch** the app and wait for processes
3. **Maximize and focus** the window
4. **Screenshot** to see initial state
5. **Interact** — click, type, use shortcuts
6. **Screenshot after each action** to verify results
7. **Report** — summarize what you found with screenshots as evidence

## MCP HTTP API (optional, for programmatic verification)

The app exposes 94 MCP tools via HTTP at the port found in:
`%APPDATA%/com.godly.terminal/mcp-http.json`

Useful for reading terminal content, checking workspace state, etc.
without relying on screenshots.

## Interacting with Native Dialogs (e.g. folder picker)

Windows file dialogs appear when creating workspaces. To navigate them:

1. Click the `Folder:` input field at the bottom of the dialog
2. `Ctrl+A` to select all existing text
3. Clipboard-paste the desired path: `pyperclip.copy(r'C:\Users\User\Desktop')` then `Ctrl+V`
4. Press `Tab` then `Enter` — this is more reliable than clicking the "Select Folder" button,
   which can be obscured by taskbar preview popups

## Common Test Scenarios

- **Terminal I/O**: Type a command, verify output appears
- **Workspace CRUD**: Click "+", select folder, verify workspace appears in sidebar
- **Split panes**: Right-click terminal tab or use split shortcut, verify layout changes
- **Theme/rendering**: Switch themes, screenshot, check for visual issues
- **Keyboard shortcuts**: Test hotkeys, verify they trigger correct actions
- **Scrollback**: Run a long command, scroll up/down, verify content
