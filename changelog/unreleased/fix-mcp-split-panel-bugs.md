### Fixed

- **MCP split panel tools don't update UI** — `self_split`, `split_terminal`, `unsplit_terminal`, `swap_panes`, and `zoom_pane` MCP tools now visually update the frontend by adding missing event listeners for `mcp-split-terminal`, `mcp-unsplit-terminal`, `mcp-swap-panes`, and `mcp-zoom-pane` events.
- **MCP `get_split_state` and `get_layout_tree` fail with "Pipe closed"** — Fixed serde tag collision between `McpResponse` and `LayoutNode` (both used `#[serde(tag = "type")]`), which caused serialization failure and pipe disconnect. Changed `LayoutTree` from tuple variant to struct variant to avoid the collision.
