### Changed
- **Concurrent IPC via pending-map** — Daemon client now supports multiple outstanding requests simultaneously using request IDs, eliminating head-of-line blocking for grid snapshot requests (refs #511)
