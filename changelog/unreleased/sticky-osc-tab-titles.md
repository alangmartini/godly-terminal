### Fixed

- **Tab names from session hooks overwritten by CWD** — when a session hook sets the OSC window title (e.g., `\033]0;My Tab Name\007`), the shell no longer resets it back to the current working directory, which would cause the tab to display as "Claude" instead of the intended name
