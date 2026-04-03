### Fixed
- **Surface size uses winit dimensions** — use winit's reported size instead of potentially stale HWND GetClientRect for wgpu surface configuration, preventing bottom-anchored UI from being pushed off-screen when un-maximizing borderless windows
