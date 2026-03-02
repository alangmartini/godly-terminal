### Fixed

- **Split layout persistence** — Split pane layouts are now correctly saved and restored across app restarts. The root cause was a serde case mismatch: Rust serialized `LayoutNode` type tags as PascalCase (`"Leaf"`, `"Split"`) while TypeScript used lowercase (`"leaf"`, `"split"`), causing the save path to silently fail. (fixes #498)
