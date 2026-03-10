/// Stable semantic IDs for testable UI surfaces in the native Iced frontend.
/// These mirror the web frontend's semantic-ids.ts so test contracts can
/// reference the same IDs regardless of frontend type.

// Workspace IDs
pub const WORKSPACE_SIDEBAR: &str = "workspace.sidebar";
pub const WORKSPACE_SIDEBAR_TOGGLE: &str = "workspace.sidebar.toggle";
pub const WORKSPACE_LIST: &str = "workspace.list";
pub const WORKSPACE_ACTIVE: &str = "workspace.active";
pub const WORKSPACE_ADD: &str = "workspace.add";

// Tab IDs
pub const TAB_BAR: &str = "tab.bar";
pub const TAB_ACTIVE: &str = "tab.active";
pub const TAB_ADD: &str = "tab.add";

// Pane IDs
pub const PANE_ACTIVE: &str = "pane.active";
pub const PANE_CONTAINER: &str = "pane.container";

// Terminal IDs
pub const TERMINAL_SURFACE: &str = "terminal.surface";

// Settings IDs
pub const SETTINGS_DIALOG: &str = "settings.dialog";
pub const SETTINGS_THEME_SELECT: &str = "settings.theme.select";

// Quick Claude IDs
pub const QUICK_CLAUDE_PROMPT: &str = "quick-claude.prompt";

/// Generate a dynamic terminal surface ID for a specific terminal
pub fn terminal_surface_id(terminal_id: &str) -> String {
    format!("terminal.surface:{}", terminal_id)
}

/// Generate a dynamic tab close button ID
pub fn tab_close_id(terminal_id: &str) -> String {
    format!("tab.close:{}", terminal_id)
}

/// Generate a dynamic pane divider ID
pub fn pane_divider_id(workspace_id: &str) -> String {
    format!("pane.divider:{}", workspace_id)
}

/// Check if a semantic ID is valid (known static ID or valid dynamic pattern)
pub fn is_valid_semantic_id(id: &str) -> bool {
    // Check static IDs
    let static_ids = [
        WORKSPACE_SIDEBAR,
        WORKSPACE_SIDEBAR_TOGGLE,
        WORKSPACE_LIST,
        WORKSPACE_ACTIVE,
        WORKSPACE_ADD,
        TAB_BAR,
        TAB_ACTIVE,
        TAB_ADD,
        PANE_ACTIVE,
        PANE_CONTAINER,
        TERMINAL_SURFACE,
        SETTINGS_DIALOG,
        SETTINGS_THEME_SELECT,
        QUICK_CLAUDE_PROMPT,
    ];

    if static_ids.contains(&id) {
        return true;
    }

    // Check dynamic patterns
    id.starts_with("terminal.surface:")
        || id.starts_with("tab.close:")
        || id.starts_with("pane.divider:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_ids_are_valid() {
        assert!(is_valid_semantic_id(WORKSPACE_SIDEBAR));
        assert!(is_valid_semantic_id(TAB_ACTIVE));
        assert!(is_valid_semantic_id(PANE_ACTIVE));
        assert!(is_valid_semantic_id(SETTINGS_DIALOG));
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
}
