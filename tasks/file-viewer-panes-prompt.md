# Prompt: Implement Collaborator-Style Canvas Features for Godly Terminal

You are implementing two features inspired by [Collaborator](https://github.com/collaborator-ai/collab-public) — a spatial canvas app for AI-assisted development. We're adding: **(1) Agent canvas control via new MCP tools** and **(2) non-terminal pane types** (code viewer, markdown preview, image viewer) to Godly Terminal's existing split-pane layout.

This is a Tauri app with a **native Iced frontend** (Rust, not web), a **daemon** managing PTY sessions over named pipes, and an **MCP server** (`godly-mcp` binary) that agents like Claude Code use to control the app.

---

## FEATURE 1: Agent Canvas Control MCP Tools

Add new MCP tools so an AI agent running inside a terminal can programmatically open files as panes beside itself, arrange them, and clean up — enabling the "agent arranges its own workspace" workflow.

### New MCP Tools to Add

**`open_file_pane`** — Open a file as a non-terminal pane (code/markdown/image) split beside a target terminal.
```json
{
  "name": "open_file_pane",
  "description": "Open a file as a viewer pane (code, markdown, or image) split beside a terminal. The file type is auto-detected from extension. Code files get syntax highlighting, markdown gets rendered preview, images get display.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "file_path": { "type": "string", "description": "Absolute path to the file to open" },
      "target_terminal_id": { "type": "string", "description": "Terminal to split beside (optional — defaults to the agent's own terminal via GODLY_SESSION_ID)" },
      "direction": { "type": "string", "enum": ["horizontal", "vertical"], "default": "horizontal", "description": "Split direction" },
      "ratio": { "type": "number", "default": 0.5, "description": "Split ratio (0.0-1.0, proportion given to existing pane)" }
    },
    "required": ["file_path"]
  }
}
```

**`close_pane`** — Close a non-terminal pane by ID.
```json
{
  "name": "close_pane",
  "description": "Close a non-terminal pane (file viewer, markdown preview, or image). Use list_panes to find pane IDs.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "pane_id": { "type": "string", "description": "ID of the pane to close" }
    },
    "required": ["pane_id"]
  }
}
```

**`list_panes`** — List all panes (terminals + non-terminal) in a workspace.
```json
{
  "name": "list_panes",
  "description": "List all panes in a workspace, including terminals and file viewers. Returns pane IDs, types, and metadata.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "workspace_id": { "type": "string", "description": "Workspace to list panes for (optional — defaults to active workspace)" }
    },
    "required": []
  }
}
```

**`update_file_pane`** — Change the file displayed in an existing file pane (reuse the pane, don't create a new split).
```json
{
  "name": "update_file_pane",
  "description": "Update the file shown in an existing file viewer pane. Reuses the pane without changing the layout.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "pane_id": { "type": "string", "description": "ID of the file pane to update" },
      "file_path": { "type": "string", "description": "New file path to display" }
    },
    "required": ["pane_id", "file_path"]
  }
}
```

---

## FEATURE 2: Non-Terminal Pane Types in the Layout Tree

Currently, every leaf in the layout tree is a terminal (`LayoutNode::Leaf { terminal_id }`). We need to extend this to support file viewer panes.

### Architecture Changes

**Phase 1: Extend `LayoutNode` to support content panes**

The layout tree currently only holds terminal IDs in leaves. We need a new leaf variant for non-terminal content. The cleanest approach: add a new variant to the layout-core `LayoutNode` enum.

**In `src-tauri/native/layout-core/src/lib.rs`**, add a new leaf variant:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum PaneContent {
    Terminal { terminal_id: String },
    FileViewer {
        pane_id: String,
        file_path: String,
        file_type: FileViewerType,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileViewerType {
    Code,      // syntax-highlighted source code
    Markdown,  // rendered markdown
    Image,     // image display (png, jpg, gif, svg, webp)
}

#[derive(Debug, Clone, PartialEq)]
pub enum LayoutNode {
    Leaf { terminal_id: String },
    ContentPane { content: PaneContent },  // NEW
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}
```

**Important**: Keep the existing `Leaf { terminal_id }` variant for backward compatibility. The new `ContentPane` variant is for non-terminal content. All existing methods (`find_leaf`, `split_leaf`, `unsplit_leaf`, `next_leaf_id`, `neighbor_in_direction`) should continue to work — they already skip non-matching variants. Add parallel methods for content panes: `find_content_pane`, `all_content_pane_ids`, `unsplit_content_pane`.

**Phase 2: Protocol layer** — Mirror the changes in `src-tauri/protocol/src/layout_tree.rs` and `src-tauri/protocol/src/mcp_messages.rs`.

**Phase 3: MCP request/response types** — Add to `McpRequest` enum:
```rust
OpenFilePane {
    file_path: String,
    target_terminal_id: Option<String>,
    direction: Option<String>,    // "horizontal" | "vertical", default "horizontal"
    ratio: Option<f64>,           // default 0.5
},
ClosePane {
    pane_id: String,
},
ListPanes {
    workspace_id: Option<String>,
},
UpdateFilePane {
    pane_id: String,
    file_path: String,
},
```

Add to `McpResponse` enum:
```rust
PaneCreated {
    pane_id: String,
    file_type: String,  // "code", "markdown", "image"
},
PaneList {
    panes: Vec<PaneInfo>,
},
```

Add `PaneInfo` struct:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneInfo {
    pub id: String,
    pub pane_type: String,       // "terminal" or "file_viewer"
    pub terminal_id: Option<String>,
    pub file_path: Option<String>,
    pub file_type: Option<String>,  // "code", "markdown", "image"
}
```

**Phase 4: MCP tool dispatch** — Add the 4 tools to `src-tauri/mcp/src/tools.rs` in both `list_tools()` and `call_tool()`.

**Phase 5: App-side handler** — Add handlers in `src-tauri/native/iced-shell/src/mcp_handler.rs`:
- `OpenFilePane`: Detect file type from extension, generate a pane ID (`fp-{uuid}`), read the file content, insert a `ContentPane` node into the layout tree beside the target terminal, store the pane state in a new `HashMap<String, FilePaneState>` on `GodlyApp`.
- `ClosePane`: Remove from layout tree + state map.
- `ListPanes`: Iterate layout tree + terminal collection.
- `UpdateFilePane`: Update file path + content in state map, trigger re-render.

**Phase 6: Rendering** — In `src-tauri/native/iced-shell/src/split_pane.rs`, extend `view_layout` to handle `ContentPane`:
- For `ContentPane`, call a new `render_content_pane` function instead of `render_leaf`.
- The render function should display:
  - **Code**: Syntax-highlighted text using iced widgets (scrollable text with monospace font, line numbers). Use the `syntect` crate for highlighting if it's already a dependency, otherwise just render plain monospace text with the terminal's color scheme.
  - **Markdown**: Rendered markdown text. Parse with `pulldown-cmark`, render as styled text widgets (bold, italic, headers with different sizes). Wrap in `scrollable`.
  - **Image**: Use `iced::widget::image::Image` with `Handle::from_path()`. Wrap in `container` with `center_x` + `center_y`.
- All pane types should have a small header bar showing the filename and a close button (X). Use the same styling as terminal tab headers.

**Phase 7: File watching** — Use `notify` crate (if already a dep) or poll-based check to detect when a displayed file changes on disk. When it does, re-read and update the pane content. This is what makes the "agent edits a file → you see changes in real-time" workflow work.

**Phase 8: Session persistence** — Extend `src-tauri/native/iced-shell/src/session_persistence.rs` to save/restore `ContentPane` nodes so file panes survive app restarts.

---

## FILE MODIFICATION GUIDE

Here's the exact chain of files to modify, in order:

### 1. Protocol Layer
| File | Changes |
|------|---------|
| `src-tauri/native/layout-core/src/lib.rs` | Add `PaneContent`, `FileViewerType`, `ContentPane` variant to `LayoutNode`. Add `find_content_pane()`, `all_content_pane_ids()`, `unsplit_content_pane()`. Update existing methods to handle new variant. |
| `src-tauri/protocol/src/layout_tree.rs` | Mirror `ContentPane` variant in the serializable protocol `LayoutNode`. Add `PaneInfo` struct. |
| `src-tauri/protocol/src/mcp_messages.rs` | Add `OpenFilePane`, `ClosePane`, `ListPanes`, `UpdateFilePane` to `McpRequest`. Add `PaneCreated`, `PaneList` to `McpResponse`. Add `PaneInfo` struct. |

### 2. MCP Tool Layer
| File | Changes |
|------|---------|
| `src-tauri/mcp/src/tools.rs` | Add 4 tool definitions to `list_tools()`. Add 4 dispatch cases to `call_tool()`. |

### 3. App Layer
| File | Changes |
|------|---------|
| `src-tauri/native/iced-shell/src/app.rs` | Add `file_panes: HashMap<String, FilePaneState>` to `GodlyApp`. Add `FilePaneState` struct (pane_id, file_path, file_type, content, last_modified). Add file watcher subscription. |
| `src-tauri/native/iced-shell/src/mcp_handler.rs` | Add `McpEvent` variants for file pane operations. Handle `OpenFilePane`, `ClosePane`, `ListPanes`, `UpdateFilePane` in `handle_mcp_request()`. |
| `src-tauri/native/iced-shell/src/split_pane.rs` | Extend `view_layout()` to handle `ContentPane` variant. Add `render_file_pane()` function with code/markdown/image rendering. |
| `src-tauri/native/iced-shell/src/session_persistence.rs` | Extend `PersistedLayoutNode` to include `ContentPane` variant. Handle serialization/deserialization. |

### 4. Cargo.toml Dependencies
| File | Changes |
|------|---------|
| `src-tauri/native/layout-core/Cargo.toml` | Add `serde` feature if not present (for `PaneContent` serialization) |
| `src-tauri/native/iced-shell/Cargo.toml` | Add `pulldown-cmark` for markdown parsing, `notify` for file watching (check if already deps first) |

### 5. MCP Build Constant
| File | Changes |
|------|---------|
| `src-tauri/mcp/src/lib.rs` | Bump the `BUILD` constant (required when adding new MCP tools) |

---

## EXISTING PATTERNS TO FOLLOW

**Adding an MCP tool (4-step pattern):**
1. Add `McpRequest` variant in `protocol/src/mcp_messages.rs`
2. Add `McpResponse` variant if needed
3. Add tool JSON schema to `list_tools()` in `mcp/src/tools.rs`
4. Add dispatch case to `call_tool()` in `mcp/src/tools.rs`
5. Handle in `handle_mcp_request()` in `iced-shell/src/mcp_handler.rs`

**Layout tree manipulation (existing pattern):**
```rust
// Insert a content pane beside terminal "t1" in horizontal split
if let Some(ws) = self.workspaces.get_mut(&workspace_id) {
    // Create content pane node
    let content_node = LayoutNode::ContentPane {
        content: PaneContent::FileViewer {
            pane_id: pane_id.clone(),
            file_path: file_path.clone(),
            file_type: FileViewerType::Code,
        },
    };
    // Split the target terminal's leaf, replacing it with a split containing both
    ws.layout.split_leaf_with_content(&target_terminal_id, content_node, SplitDirection::Horizontal);
}
```

**File type detection:**
```rust
fn detect_file_type(path: &str) -> FileViewerType {
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext.to_lowercase().as_str() {
        "md" | "mdx" | "markdown" => FileViewerType::Markdown,
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp" | "ico" => FileViewerType::Image,
        _ => FileViewerType::Code,  // everything else gets syntax highlighting
    }
}
```

**Pane ID generation:** Use prefix `fp-` + UUID, e.g., `fp-a1b2c3d4`.

---

## RENDERING GUIDELINES

Since this is an **Iced** (native Rust GUI) application, not a web app:

- **Code viewer**: Use `iced::widget::scrollable` containing a `column` of `text` widgets (one per line). Monospace font. Line numbers in a left gutter. Start with plain text — syntax highlighting can come later.
- **Markdown viewer**: Parse with `pulldown-cmark`, render as styled `text` widgets (bold, italic, headers with different sizes). Wrap in `scrollable`.
- **Image viewer**: Use `iced::widget::image::Image` with `Handle::from_path()`. Wrap in `container` with `center_x` + `center_y`.
- All pane types should have a small header bar showing the filename and a close button (X). Use the same styling as terminal tab headers.

---

## WHAT NOT TO DO

- **Don't implement an infinite canvas.** We're using the existing split-pane binary tree layout. No pan/zoom.
- **Don't add a web view or embedded browser.** All rendering is native Iced widgets.
- **Don't modify the daemon.** File panes are app-only — no PTY sessions needed. All file reading happens in the iced-shell process.
- **Don't break existing terminal functionality.** The `Leaf { terminal_id }` variant must continue to work exactly as before.
- **Don't add external editor capabilities.** File panes are read-only viewers. The agent edits files via the terminal; the viewer shows the result.

---

## TESTING

1. **Unit tests** in `layout-core` for the new `ContentPane` variant — split, unsplit, find, persistence round-trip.
2. **Protocol tests** in `protocol` for serialization of `ContentPane` layout nodes.
3. **Integration tests** for the new MCP tools — verify `open_file_pane` creates a pane, `list_panes` returns it, `close_pane` removes it.

---

## CHANGELOG

Create `changelog/unreleased/<PR-number>-file-viewer-panes.md`:
```markdown
### Added
- **File viewer panes** — Open code, markdown, and image files as split panes alongside terminals. Auto-detects file type from extension. (#<PR>)
- **MCP tools for agent workspace control** — `open_file_pane`, `close_pane`, `list_panes`, `update_file_pane` allow AI agents to programmatically arrange file viewers beside their terminals. (#<PR>)
- **File watching** — File viewer panes auto-refresh when the underlying file changes on disk, enabling real-time feedback when agents edit files. (#<PR>)
```

## GIT WORKFLOW

- Branch: `feat/file-viewer-panes`
- Split into atomic commits: protocol changes first, then MCP tools, then app-side handlers, then rendering, then file watching, then tests.
- Open PR against `master`.
