### Fixed
- **Micro-blinking during typing** — coalesced terminal output events now trigger a follow-up grid fetch so stale snapshots don't cause visible flicker (#845)
