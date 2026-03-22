### Fixed
- **Quick Claude quote escaping** — single quotes in prompts (e.g., "isn't") are now correctly escaped for PowerShell using doubled quotes ('') instead of POSIX escaping, allowing prompts with contractions to work properly
