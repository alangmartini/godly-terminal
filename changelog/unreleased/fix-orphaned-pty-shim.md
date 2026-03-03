### Fixed

- **Orphaned pty-shim processes** — Fixed two root causes that left shim processes running indefinitely after terminals were closed: (1) daemon `close()` now kills the shim process directly as a safety net when the Shutdown message can't reach it due to an I/O thread race, and (2) the shim's `create_pipe_and_wait` now joins the ConnectNamedPipe thread before returning to prevent handle-reuse races that caused phantom connections resetting the orphan timer. Added a 60-second daemon idle timeout in the shim's inner I/O loop as defense in depth.
