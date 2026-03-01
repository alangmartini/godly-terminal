### Added

- **Godly Flows — Visual Node-Graph Automation System (Phase 1)** — A flow engine that lets users compose terminal operations into reusable, triggerable workflows without writing code. Includes:
  - Flow store with localStorage persistence and full CRUD (create, duplicate, import/export JSON)
  - DAG-based execution engine with layered topological sort and cancellation support
  - 12 built-in node types across 3 categories: Triggers (hotkey, manual), Terminal (create, close, write, read, execute, focus, rename, wait-for-text), Data (constant, template)
  - Hotkey trigger system for binding flows to keyboard shortcuts
  - Settings tab UI with flow list, step-based editor, chord picker, and import/export
  - Node type registry for extensible node definitions
  - Exposed via `window.__FLOW_ENGINE__` for MCP integration (refs #459)
