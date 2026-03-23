### Fixed
- **Quick Claude image paste via Ctrl+V** — the text_editor widget now correctly triggers clipboard image detection when pasting, fixing an issue where `keyboard::listen()` was never reached due to the widget capturing Ctrl+V events
