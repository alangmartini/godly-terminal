### Changed
- **Background clipboard paste** — Move clipboard read to background thread via Task::perform(), preventing frame jank on slow clipboard access (RDP, WSL forwarding).
