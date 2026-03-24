### Fixed
- **Workspace persistence regression** — workspace mutations (create, delete, rename) now immediately persist session state to disk so crashes within the autosave window don't lose changes (#619)
- **Missing field in persistence test** — added missing `terminal_clone_ids` field in existing test case

### Tests
- Added 3 regression tests verifying workspace mutations survive crash recovery with immediate persistence
