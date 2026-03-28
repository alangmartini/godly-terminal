### Fixed

- **Delayed screen refresh** — Terminal output now renders immediately instead of waiting for the next timer tick. Uses `PostMessageW(hwnd, WM_APP)` to wake winit's event loop from background threads (grid fetch, terminal creation, scroll, init). Previous `PostThreadMessageW` approach didn't work because winit only dispatches window messages. Also enriched grid fingerprint with row/cell count to detect in-place content changes. (#845)
