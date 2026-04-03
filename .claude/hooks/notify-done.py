"""Stop hook: play a notification sound when Claude finishes a task.

Skips notification for subagent and agent-team completions — only plays
when the top-level assistant turn ends.
"""
import json
import sys
import winsound

# The Stop hook receives {"session_id", "stop_reason", ...} on stdin.
# For subagents / agent-team members the hook runs in the *child* process,
# but those processes set CLAUDE_AGENT=1 in the environment.  The main
# conversation process does NOT set that variable — so we can use its
# absence as the signal.
import os

if os.environ.get("CLAUDE_AGENT"):
    sys.exit(0)

# Play a short, pleasant notification sound (non-blocking flag not used
# so the hook finishes quickly — SND_ASYNC lets the sound play while
# control returns immediately).
SOUND = r"C:\Windows\Media\Windows Notify.wav"
winsound.PlaySound(SOUND, winsound.SND_FILENAME | winsound.SND_ASYNC)
