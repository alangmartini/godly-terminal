### Fixed
- **Quick Claude multi-line input** — fixed a bug where prompts containing newlines (from multi-line text input or pasted content) would break the shell command sent to the PTY. Newlines are now collapsed to spaces before embedding in the CLI argument, preventing them from being interpreted as Enter keypresses.
