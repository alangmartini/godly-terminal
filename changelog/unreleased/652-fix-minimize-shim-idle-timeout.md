### Fixed

- **Minimize freeze root cause** — daemon now sends periodic keepalive to PTY shim, preventing the shim's 60-second idle timeout from disconnecting the pipe during window minimize (#652)
