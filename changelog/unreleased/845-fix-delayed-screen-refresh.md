### Fixed

- **Delayed screen refresh** — Terminal output now renders immediately instead of batching at 100ms intervals. Added Win32 wake-up signal from bridge thread and adaptive heartbeat polling (16ms during output, 100ms idle). (#845)
