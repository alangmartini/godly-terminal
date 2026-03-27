### Fixed
- **Eliminate micro-blinking during typing and screen updates** — skip pixel renders for stale grid snapshots when a refetch is already pending, avoiding intermediate frames that cause visible flicker (#845)
