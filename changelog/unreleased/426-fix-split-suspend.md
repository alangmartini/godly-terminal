### Fixed
- **Split view lost on tab switch** — split views are now suspended (not destroyed) when `addTerminal()` clears the active layout, and `clearLayoutTree()` no longer deletes suspended splits as a side effect (#426)
