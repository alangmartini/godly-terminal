### Added

- **Bell burst suppression** — Rapid-fire bell notifications from the same terminal (e.g., during Claude Code `/batch`) are now suppressed after the first bell. A single "Activity Settled" notification fires once the burst goes quiet (10s of silence), preventing notification spam while still confirming when batch work completes.
