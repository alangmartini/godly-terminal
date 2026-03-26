### Fixed
- **Quick Claude trust prompt not auto-accepted** — switched from `ReadRichGrid` + naive cell concatenation to `ReadGrid` which uses the VT library's correct text extraction with proper empty-cell gap filling (#820)
