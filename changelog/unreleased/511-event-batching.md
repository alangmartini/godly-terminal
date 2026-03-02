### Changed
- **4ms event batching window** — Bridge thread now coalesces rapid terminal output events within a 4ms window after the first immediate dispatch, reducing IPC overhead for high-throughput sessions (refs #511)
