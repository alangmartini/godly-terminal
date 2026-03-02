### Changed
- **Background rect merging** — Canvas2D renderer now merges adjacent same-colored background cells into single fillRect calls, reducing draw calls by 5-10x for typical terminal content (refs #511)
