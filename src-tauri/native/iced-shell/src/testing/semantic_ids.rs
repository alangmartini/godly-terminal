/// Stable semantic IDs for testable UI surfaces in the native Iced frontend.
/// These mirror the web frontend's semantic-ids.ts so test contracts can
/// reference the same IDs regardless of frontend type.

/// All known static semantic IDs. Used for both public constants and validation.
pub const ALL_STATIC_IDS: &[&str] = &[
    // Workspace
    "workspace.sidebar",
    "workspace.sidebar.toggle",
    "workspace.list",
    "workspace.active",
    "workspace.add",
    // Tabs
    "tab.bar",
    "tab.active",
    "tab.add",
    // Panes
    "pane.active",
    "pane.container",
    // Terminal
    "terminal.surface",
    // Settings
    "settings.dialog",
    "settings.theme.select",
    // Quick Claude
    "quick-claude.prompt",
];

/// Dynamic ID prefixes for parameterized elements.
const DYNAMIC_PREFIXES: &[&str] = &["terminal.surface:", "tab.close:", "pane.divider:"];

/// Generate a dynamic terminal surface ID for a specific terminal.
pub fn terminal_surface_id(terminal_id: &str) -> String {
    format!("terminal.surface:{terminal_id}")
}

/// Generate a dynamic tab close button ID.
pub fn tab_close_id(terminal_id: &str) -> String {
    format!("tab.close:{terminal_id}")
}

/// Generate a dynamic pane divider ID.
pub fn pane_divider_id(workspace_id: &str) -> String {
    format!("pane.divider:{workspace_id}")
}

/// Check if a semantic ID is valid (known static ID or valid dynamic pattern).
pub fn is_valid_semantic_id(id: &str) -> bool {
    ALL_STATIC_IDS.contains(&id) || DYNAMIC_PREFIXES.iter().any(|prefix| id.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_ids_are_valid() {
        for &id in ALL_STATIC_IDS {
            assert!(is_valid_semantic_id(id), "static ID should be valid: {id}");
        }
    }

    #[test]
    fn dynamic_ids_are_valid() {
        assert!(is_valid_semantic_id(&terminal_surface_id("abc-123")));
        assert!(is_valid_semantic_id(&tab_close_id("def-456")));
        assert!(is_valid_semantic_id(&pane_divider_id("ws-789")));
    }

    #[test]
    fn unknown_ids_are_invalid() {
        assert!(!is_valid_semantic_id("unknown.id"));
        assert!(!is_valid_semantic_id(""));
        assert!(!is_valid_semantic_id("random"));
    }

    #[test]
    fn all_static_ids_use_dot_notation() {
        for &id in ALL_STATIC_IDS {
            assert!(
                id.contains('.') || id.contains('-'),
                "static ID should use dot/dash notation: {id}"
            );
        }
    }
}
