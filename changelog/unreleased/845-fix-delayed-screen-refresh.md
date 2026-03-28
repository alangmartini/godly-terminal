### Fixed

- **Delayed screen refresh** — Terminal output now renders immediately instead of waiting up to 1 second. Wake the iced event loop via Win32 `PostThreadMessageW` from all critical background threads (grid fetch, terminal creation, scroll, init), bypassing iced's unreliable internal waker. Reduced Win32 safety-net timer to 50ms and enriched the grid fingerprint to detect in-place content updates. (#845)
