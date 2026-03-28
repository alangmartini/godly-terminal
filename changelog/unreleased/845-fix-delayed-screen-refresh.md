### Fixed

- **Delayed screen refresh** — Terminal output now renders immediately instead of waiting up to 1 second. Wake the iced event loop via Win32 `PostThreadMessageW` from both bridge I/O and grid-fetch threads, bypassing iced's unreliable internal waker. Reduced Win32 timer interval to 100ms as safety net. (#845)
