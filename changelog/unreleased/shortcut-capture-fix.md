### Fixed

- **Shortcut rebinding in native Iced shell** — The settings dialog now properly intercepts key presses when rebinding shortcuts. Previously, pressing a key during capture mode did nothing, and there was no way to cancel. Now escape cancels capture, and valid key combos (Ctrl+key or Alt+key) update the binding display.
