use std::collections::HashMap;

use godly_protocol::types::RichGridData;
use godly_tabs_core::TabState;

/// Information about a single terminal session.
pub struct TerminalInfo {
    pub id: String,
    pub title: String,
    pub process_name: String,
    pub order: u32,
    pub grid: Option<RichGridData>,
    pub dirty: bool,
    pub fetching: bool,
    /// Set when a `TerminalOutput` event arrives while a grid fetch is already
    /// in-flight. After `GridFetched` completes, this flag triggers a follow-up
    /// fetch so the coalesced output is not lost (prevents micro-blinking).
    pub needs_refetch: bool,
    pub rows: u16,
    pub cols: u16,
    pub exited: bool,
    pub exit_code: Option<i64>,
    /// Current scrollback offset (0 = live view, >0 = scrolled into history).
    pub scrollback_offset: usize,
    /// Total number of scrollback rows available.
    pub total_scrollback: usize,
    /// Workspace this terminal belongs to (None = default workspace).
    pub workspace_id: Option<String>,
    /// User-assigned custom name (overrides title/process_name in tab label).
    pub custom_name: Option<String>,
    /// Path to a git worktree created for this terminal (None = normal terminal).
    pub worktree_path: Option<String>,
    /// Whether this terminal's worktree_path points to a clone (true) or a git worktree (false).
    pub is_clone: bool,
    /// Cached atlas render frame (vertex data + optional atlas update).
    pub cached_frame: Option<godly_terminal_surface::atlas_shader::CachedAtlasFrame>,
    /// Fingerprint of the last rendered grid state. Used to skip redundant
    /// pixel renders when the grid content has not changed (e.g. heartbeat
    /// recovery polls fetching an identical snapshot).
    /// Tuple: (total_scrollback, scrollback_offset, cursor_row, cursor_col,
    ///         cursor_hidden, num_rows, cursor_row_cells, alternate_screen)
    pub last_grid_fingerprint: Option<(usize, usize, u16, u16, bool, usize, usize, bool)>,
}

impl TerminalInfo {
    /// Returns a friendly display name derived from `process_name`.
    ///
    /// Strips directory prefixes and `.exe` suffix, then maps known basenames
    /// to human-readable labels (e.g. `pwsh` → "PowerShell").
    pub fn display_name(&self) -> String {
        let raw = self
            .process_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(&self.process_name);
        let basename = raw
            .strip_suffix(".exe")
            .or_else(|| raw.strip_suffix(".EXE"))
            .unwrap_or(raw);

        if basename.is_empty() {
            return "Terminal".to_string();
        }

        let lower = basename.to_ascii_lowercase();
        match lower.as_str() {
            "pwsh" | "powershell" => "PowerShell".to_string(),
            "cmd" => "Command Prompt".to_string(),
            "bash" => "Bash".to_string(),
            "zsh" => "Zsh".to_string(),
            "fish" => "Fish".to_string(),
            "sh" => "Shell".to_string(),
            "wsl" => "WSL".to_string(),
            "nu" | "nushell" => "Nushell".to_string(),
            "node" => "Node.js".to_string(),
            "python" | "python3" => "Python".to_string(),
            "ruby" | "irb" => "Ruby".to_string(),
            "claude" => "Claude".to_string(),
            "codex" => "Codex".to_string(),
            _ => {
                let mut chars = basename.chars();
                match chars.next() {
                    Some(c) => {
                        let mut name = c.to_uppercase().collect::<String>();
                        name.push_str(chars.as_str());
                        name
                    }
                    None => "Terminal".to_string(),
                }
            }
        }
    }

    /// Extracts a working directory from `title` if it looks like a filesystem path.
    ///
    /// PowerShell and many shells set the OSC window title to the current directory.
    pub fn extract_cwd(&self) -> Option<&str> {
        let t = self.title.trim();
        if t.contains(":\\") || t.starts_with('/') {
            Some(t)
        } else {
            None
        }
    }

    /// Returns the display label for this terminal's tab.
    ///
    /// Priority: custom_name > OSC title (if not a bare path) > display_name > "Terminal"
    pub fn tab_label(&self) -> String {
        if let Some(ref name) = self.custom_name {
            if !name.is_empty() {
                return name.clone();
            }
        }
        // Use the OSC window title when the running program has set one,
        // unless it looks like a bare filesystem path (shells often set
        // the title to the CWD, which is not a useful tab label).
        let t = self.title.trim();
        if !t.is_empty() && self.extract_cwd().is_none() {
            return t.to_string();
        }
        self.display_name()
    }

    /// Returns a small icon glyph representing the process type.
    pub fn tab_icon(&self) -> &'static str {
        let name = self.process_name.to_ascii_lowercase();
        if name.contains("claude") {
            "\u{25C6}" // ◆
        } else if name.contains("codex") {
            "\u{25B6}" // ▶
        } else if name.contains("pwsh") || name.contains("powershell") {
            "\u{276F}" // ❯
        } else if name.contains("cmd") {
            "\u{25BA}" // ►
        } else if name.contains("wsl") {
            "\u{2318}" // ⌘
        } else if name.contains("bash")
            || name.contains("zsh")
            || name.contains("fish")
            || name == "sh"
            || name.ends_with("/sh")
        {
            "\u{25B8}" // ▸
        } else if name.contains("ssh") {
            "\u{2192}" // →
        } else if name.contains("node") || name.contains("npm") || name.contains("pnpm") {
            "\u{25CB}" // ○
        } else if name.contains("python") {
            "\u{25CA}" // ◊
        } else if name.contains("ruby") || name.contains("irb") {
            "\u{25C8}" // ◈
        } else if name.contains("git") {
            "\u{2387}" // ⎇
        } else if name.contains("vim") || name.contains("nvim") {
            "\u{25A0}" // ■
        } else {
            "\u{25B8}" // ▸ (default shell prompt)
        }
    }
}

/// Collection of terminal sessions with active tab tracking.
///
/// Uses a `HashMap` for O(1) lookup by id and delegates ordering/active
/// semantics to the pure `godly-tabs-core` state machine.
pub struct TerminalCollection {
    terminals: HashMap<String, TerminalInfo>,
    tabs: TabState,
    mru: Vec<String>,
    next_order: u32,
}

impl TerminalCollection {
    /// Creates an empty collection.
    pub fn new() -> Self {
        Self {
            terminals: HashMap::new(),
            tabs: TabState::new(),
            mru: Vec::new(),
            next_order: 0,
        }
    }

    /// Adds a new terminal with the given id and grid dimensions.
    ///
    /// Auto-increments the order counter. Sets as active if this is the first terminal.
    /// Returns a mutable reference to the newly created `TerminalInfo`.
    pub fn add(&mut self, id: String, rows: u16, cols: u16) -> &mut TerminalInfo {
        self.add_terminal(id, rows, cols, None)
    }

    /// Removes the terminal with the given id.
    ///
    /// If the removed terminal was active, the next terminal at the same index
    /// (or the previous one if at the end) becomes active.
    pub fn remove(&mut self, id: &str) {
        if !self.tabs.close(id) {
            return;
        }
        self.terminals.remove(id);
        self.remove_from_mru(id);
        self.sync_active_to_mru();
    }

    /// Returns a reference to the active terminal, if any.
    pub fn active(&self) -> Option<&TerminalInfo> {
        let id = self.tabs.active_id()?;
        self.terminals.get(id)
    }

    /// Returns a mutable reference to the active terminal, if any.
    pub fn active_mut(&mut self) -> Option<&mut TerminalInfo> {
        let id = self.tabs.active_id()?.to_owned();
        self.terminals.get_mut(&id)
    }

    /// Finds a terminal by id.
    pub fn get(&self, id: &str) -> Option<&TerminalInfo> {
        self.terminals.get(id)
    }

    /// Finds a terminal by id, mutably.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut TerminalInfo> {
        self.terminals.get_mut(id)
    }

    /// Sets the active terminal by id. No-op if id not found.
    pub fn set_active(&mut self, id: &str) {
        if self.tabs.activate(id) {
            self.touch_mru(id);
        }
    }

    /// Returns the number of terminals in the collection.
    pub fn count(&self) -> usize {
        self.terminals.len()
    }

    /// Iterates over all terminals in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &TerminalInfo> {
        self.tabs
            .ids()
            .iter()
            .filter_map(|id| self.terminals.get(id))
    }

    /// Returns all terminals in insertion order as a `Vec` of references.
    ///
    /// Use this for the tab bar and other ordered displays.
    pub fn ordered_terminals(&self) -> Vec<&TerminalInfo> {
        self.iter().collect()
    }

    /// Returns the active terminal's id, if any.
    pub fn active_id(&self) -> Option<&str> {
        self.tabs.active_id()
    }

    /// Switch to the next terminal (wraps around).
    pub fn next(&mut self) {
        self.tabs.next();
        self.sync_active_to_mru();
    }

    /// Switch to the previous terminal (wraps around).
    pub fn previous(&mut self) {
        self.tabs.previous();
        self.sync_active_to_mru();
    }

    /// Returns terminal ids in most-recently-used order.
    ///
    /// The active terminal is always first when one exists.
    pub fn mru_terminal_ids(&self) -> Vec<&str> {
        self.mru
            .iter()
            .filter_map(|id| self.terminals.contains_key(id).then_some(id.as_str()))
            .collect()
    }

    /// Returns terminal ids in MRU order, limited to a workspace assignment.
    pub fn mru_terminal_ids_for_workspace(&self, workspace_id: Option<&str>) -> Vec<&str> {
        self.mru
            .iter()
            .filter_map(|id| {
                self.terminals.get(id).and_then(|terminal| {
                    (terminal.workspace_id.as_deref() == workspace_id).then_some(id.as_str())
                })
            })
            .collect()
    }

    /// Returns the next MRU terminal after the active one, if any.
    pub fn mru_next_after_active(&self) -> Option<&str> {
        let active_id = self.active_id()?;
        self.mru
            .iter()
            .map(String::as_str)
            .find(|id| *id != active_id && self.terminals.contains_key(*id))
    }

    /// Reorder terminals by tab id.
    ///
    /// Returns false if either id is missing. Returns true with no changes when
    /// from and to resolve to the same tab index.
    pub fn reorder_by_ids(&mut self, from_id: &str, to_id: &str) -> bool {
        let Some(from_index) = self.tabs.index_of(from_id) else {
            return false;
        };
        let Some(to_index) = self.tabs.index_of(to_id) else {
            return false;
        };
        if from_index == to_index {
            return true;
        }

        let active_id = self.tabs.active_id().map(str::to_owned);
        if !self.tabs.reorder(from_index, to_index) {
            return false;
        }

        if let Some(active_id) = active_id {
            let _ = self.tabs.activate(&active_id);
        }

        true
    }

    /// Adds a terminal to a specific workspace.
    ///
    /// Like `add()`, but also sets `workspace_id`. Returns a mutable reference
    /// to the newly created `TerminalInfo`.
    pub fn add_to_workspace(
        &mut self,
        id: String,
        rows: u16,
        cols: u16,
        workspace_id: String,
    ) -> &mut TerminalInfo {
        self.add_terminal(id, rows, cols, Some(workspace_id))
    }

    fn add_terminal(
        &mut self,
        id: String,
        rows: u16,
        cols: u16,
        workspace_id: Option<String>,
    ) -> &mut TerminalInfo {
        if self.terminals.contains_key(&id) {
            return self
                .terminals
                .get_mut(&id)
                .expect("terminal must exist after contains_key");
        }

        let order = self.next_order;
        self.next_order += 1;
        let _ = self.tabs.open(id.clone());
        self.terminals.insert(
            id.clone(),
            TerminalInfo {
                id: id.clone(),
                title: String::new(),
                process_name: String::new(),
                order,
                grid: None,
                dirty: false,
                fetching: false,
                needs_refetch: false,
                rows,
                cols,
                exited: false,
                exit_code: None,
                scrollback_offset: 0,
                total_scrollback: 0,
                workspace_id,
                custom_name: None,
                worktree_path: None,
                is_clone: false,
                cached_frame: None,
                last_grid_fingerprint: None,
            },
        );
        if self.tabs.active_id() == Some(id.as_str()) {
            self.touch_mru(&id);
        } else {
            self.mru.push(id.clone());
        }

        self.terminals
            .get_mut(&id)
            .expect("newly inserted terminal must exist")
    }

    /// Set or clear the custom name for a terminal.
    pub fn rename(&mut self, id: &str, name: Option<String>) {
        if let Some(term) = self.get_mut(id) {
            term.custom_name = name;
        }
    }

    /// Set the worktree path for a terminal.
    pub fn set_worktree_path(&mut self, id: &str, path: String) {
        if let Some(term) = self.get_mut(id) {
            term.worktree_path = Some(path);
        }
    }

    /// Set whether a terminal's worktree_path is a clone (true) or a git worktree (false).
    pub fn set_clone_flag(&mut self, id: &str, is_clone: bool) {
        if let Some(term) = self.terminals.get_mut(id) {
            term.is_clone = is_clone;
        }
    }

    /// Returns terminals belonging to a specific workspace.
    pub fn terminals_for_workspace(&self, workspace_id: &str) -> Vec<&TerminalInfo> {
        self.tabs
            .ids()
            .iter()
            .filter_map(|id| self.terminals.get(id))
            .filter(|t| t.workspace_id.as_deref() == Some(workspace_id))
            .collect()
    }

    /// Move a terminal to a workspace (or unassign with None).
    pub fn set_workspace(&mut self, terminal_id: &str, workspace_id: Option<String>) {
        if let Some(term) = self.get_mut(terminal_id) {
            term.workspace_id = workspace_id;
        }
    }

    fn sync_active_to_mru(&mut self) {
        let Some(active_id) = self.tabs.active_id().map(str::to_owned) else {
            return;
        };
        self.touch_mru(&active_id);
    }

    fn touch_mru(&mut self, id: &str) {
        if self.mru.first().is_some_and(|current| current == id) {
            return;
        }
        self.remove_from_mru(id);
        self.mru.insert(0, id.to_string());
    }

    fn remove_from_mru(&mut self, id: &str) {
        self.mru.retain(|existing| existing != id);
    }
}

impl Default for TerminalCollection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_first_becomes_active() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        assert_eq!(col.active_id(), Some("t1"));
        assert_eq!(col.count(), 1);
    }

    #[test]
    fn test_add_second_does_not_change_active() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        assert_eq!(col.active_id(), Some("t1"));
        assert_eq!(col.count(), 2);
    }

    #[test]
    fn test_remove_active_picks_next() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        // Active is t1 (first added). Remove it -> t2 should become active (same index 0).
        col.remove("t1");
        assert_eq!(col.active_id(), Some("t2"));
        assert_eq!(col.count(), 2);

        // Remove t2 (active) -> t3 should become active.
        col.remove("t2");
        assert_eq!(col.active_id(), Some("t3"));
        assert_eq!(col.count(), 1);

        // Remove last terminal -> no active.
        col.remove("t3");
        assert_eq!(col.active_id(), None);
        assert_eq!(col.count(), 0);
    }

    #[test]
    fn test_remove_last_active_picks_previous() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        // Make t3 active, then remove it -> should pick t2 (previous).
        col.set_active("t3");
        assert_eq!(col.active_id(), Some("t3"));
        col.remove("t3");
        assert_eq!(col.active_id(), Some("t2"));
    }

    #[test]
    fn test_remove_non_active_preserves_active() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        // Active is t1. Remove t2 -> active should still be t1.
        col.remove("t2");
        assert_eq!(col.active_id(), Some("t1"));
        assert_eq!(col.count(), 2);
    }

    #[test]
    fn test_tab_label_priority() {
        let mut col = TerminalCollection::new();

        // No title, no process_name -> "Terminal"
        let info = col.add("t1".into(), 24, 80);
        assert_eq!(info.tab_label(), "Terminal");

        // process_name set -> display_name (friendly label)
        info.process_name = "pwsh".into();
        assert_eq!(info.tab_label(), "PowerShell");

        // title set to a CWD path -> still uses display_name
        info.title = "C:\\Users\\test".into();
        assert_eq!(info.tab_label(), "PowerShell");

        // title set to a non-path string -> uses OSC title
        info.title = "claude: fixing bug #42".into();
        assert_eq!(info.tab_label(), "claude: fixing bug #42");

        // Unix-style CWD path -> still uses display_name
        info.title = "/home/user/project".into();
        assert_eq!(info.tab_label(), "PowerShell");
    }

    #[test]
    fn test_tab_icon_detects_process_type() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);

        info.process_name = "claude".into();
        assert_eq!(info.tab_icon(), "\u{25C6}"); // ◆

        info.process_name = "codex".into();
        assert_eq!(info.tab_icon(), "\u{25B6}"); // ▶

        info.process_name = "pwsh".into();
        assert_eq!(info.tab_icon(), "\u{276F}"); // ❯

        info.process_name = "powershell".into();
        assert_eq!(info.tab_icon(), "\u{276F}"); // ❯

        info.process_name = "cmd.exe".into();
        assert_eq!(info.tab_icon(), "\u{25BA}"); // ►

        info.process_name = "wsl".into();
        assert_eq!(info.tab_icon(), "\u{2318}"); // ⌘

        info.process_name = "bash".into();
        assert_eq!(info.tab_icon(), "\u{25B8}"); // ▸

        info.process_name = "zsh".into();
        assert_eq!(info.tab_icon(), "\u{25B8}"); // ▸

        info.process_name = "fish".into();
        assert_eq!(info.tab_icon(), "\u{25B8}"); // ▸

        info.process_name = "ssh".into();
        assert_eq!(info.tab_icon(), "\u{2192}"); // →

        info.process_name = "node".into();
        assert_eq!(info.tab_icon(), "\u{25CB}"); // ○

        info.process_name = "python3".into();
        assert_eq!(info.tab_icon(), "\u{25CA}"); // ◊

        info.process_name = "ruby".into();
        assert_eq!(info.tab_icon(), "\u{25C8}"); // ◈

        info.process_name = "irb".into();
        assert_eq!(info.tab_icon(), "\u{25C8}"); // ◈

        info.process_name = "git".into();
        assert_eq!(info.tab_icon(), "\u{2387}"); // ⎇

        info.process_name = "vim".into();
        assert_eq!(info.tab_icon(), "\u{25A0}"); // ■

        info.process_name = "some-unknown".into();
        assert_eq!(info.tab_icon(), "\u{25B8}"); // ▸ (default)
    }

    #[test]
    fn test_get_and_get_mut() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 30, 100);

        // get
        let t1 = col.get("t1").unwrap();
        assert_eq!(t1.rows, 24);
        assert_eq!(t1.cols, 80);

        let t2 = col.get("t2").unwrap();
        assert_eq!(t2.rows, 30);
        assert_eq!(t2.cols, 100);

        assert!(col.get("nonexistent").is_none());

        // get_mut
        {
            let t1_mut = col.get_mut("t1").unwrap();
            t1_mut.dirty = true;
        }
        assert!(col.get("t1").unwrap().dirty);

        assert!(col.get_mut("nonexistent").is_none());
    }

    #[test]
    fn test_set_active_nonexistent_is_noop() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.set_active("nonexistent");
        assert_eq!(col.active_id(), Some("t1"));
    }

    #[test]
    fn test_iter() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        let ids: Vec<&str> = col.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["t1", "t2", "t3"]);
    }

    #[test]
    fn test_order_auto_increments() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        let orders: Vec<u32> = col.iter().map(|t| t.order).collect();
        assert_eq!(orders, vec![0, 1, 2]);
    }

    #[test]
    fn test_active_and_active_mut() {
        let mut col = TerminalCollection::new();
        assert!(col.active().is_none());
        assert!(col.active_mut().is_none());

        col.add("t1".into(), 24, 80);
        assert_eq!(col.active().unwrap().id, "t1");

        col.active_mut().unwrap().title = "Hello".into();
        assert_eq!(col.active().unwrap().title, "Hello");
    }

    #[test]
    fn test_next_wraps_around() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        assert_eq!(col.active_id(), Some("t1"));

        col.next();
        assert_eq!(col.active_id(), Some("t2"));

        col.next();
        assert_eq!(col.active_id(), Some("t3"));

        // Wraps around to t1.
        col.next();
        assert_eq!(col.active_id(), Some("t1"));
    }

    #[test]
    fn test_previous_wraps_around() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        assert_eq!(col.active_id(), Some("t1"));

        // Wraps around to t3.
        col.previous();
        assert_eq!(col.active_id(), Some("t3"));

        col.previous();
        assert_eq!(col.active_id(), Some("t2"));

        col.previous();
        assert_eq!(col.active_id(), Some("t1"));
    }

    #[test]
    fn test_next_single_terminal_is_noop() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);

        col.next();
        assert_eq!(col.active_id(), Some("t1"));
    }

    #[test]
    fn test_previous_single_terminal_is_noop() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);

        col.previous();
        assert_eq!(col.active_id(), Some("t1"));
    }

    #[test]
    fn test_next_empty_is_noop() {
        let mut col = TerminalCollection::new();
        col.next();
        assert_eq!(col.active_id(), None);
    }

    #[test]
    fn test_previous_empty_is_noop() {
        let mut col = TerminalCollection::new();
        col.previous();
        assert_eq!(col.active_id(), None);
    }

    #[test]
    fn test_next_previous_round_trip() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        col.set_active("t2");
        assert_eq!(col.active_id(), Some("t2"));

        col.next();
        assert_eq!(col.active_id(), Some("t3"));

        col.previous();
        assert_eq!(col.active_id(), Some("t2"));
    }

    #[test]
    fn test_scrollback_fields_initialized_to_zero() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);
        assert_eq!(info.scrollback_offset, 0);
        assert_eq!(info.total_scrollback, 0);
    }

    #[test]
    fn test_tab_label_custom_name_priority() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);
        info.process_name = "pwsh".into();
        info.title = "My Shell".into();
        info.custom_name = Some("Custom Name".into());
        assert_eq!(info.tab_label(), "Custom Name");
    }

    #[test]
    fn test_tab_label_empty_custom_name_falls_through() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);
        info.custom_name = Some(String::new());
        info.process_name = "bash".into();
        assert_eq!(info.tab_label(), "Bash");

        // With a non-path OSC title, empty custom_name falls through to title
        info.title = "vim main.rs".into();
        assert_eq!(info.tab_label(), "vim main.rs");
    }

    #[test]
    fn test_rename() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.rename("t1", Some("My Terminal".into()));
        assert_eq!(col.get("t1").unwrap().tab_label(), "My Terminal");

        col.rename("t1", None);
        assert_eq!(col.get("t1").unwrap().tab_label(), "Terminal");
    }

    #[test]
    fn test_workspace_filtering() {
        let mut col = TerminalCollection::new();
        col.add_to_workspace("t1".into(), 24, 80, "w1".into());
        col.add_to_workspace("t2".into(), 24, 80, "w1".into());
        col.add_to_workspace("t3".into(), 24, 80, "w2".into());
        col.add("t4".into(), 24, 80); // No workspace

        let w1_terms = col.terminals_for_workspace("w1");
        assert_eq!(w1_terms.len(), 2);

        let w2_terms = col.terminals_for_workspace("w2");
        assert_eq!(w2_terms.len(), 1);

        let w3_terms = col.terminals_for_workspace("w3");
        assert_eq!(w3_terms.len(), 0);
    }

    #[test]
    fn test_set_workspace() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        assert!(col.get("t1").unwrap().workspace_id.is_none());

        col.set_workspace("t1", Some("w1".into()));
        assert_eq!(col.get("t1").unwrap().workspace_id.as_deref(), Some("w1"));

        col.set_workspace("t1", None);
        assert!(col.get("t1").unwrap().workspace_id.is_none());
    }

    #[test]
    fn test_new_fields_default_to_none() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);
        assert!(info.workspace_id.is_none());
        assert!(info.custom_name.is_none());
    }

    #[test]
    fn test_ordered_terminals_preserves_insertion_order() {
        let mut col = TerminalCollection::new();
        col.add("t3".into(), 24, 80);
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);

        let ordered: Vec<&str> = col
            .ordered_terminals()
            .iter()
            .map(|t| t.id.as_str())
            .collect();
        assert_eq!(ordered, vec!["t3", "t1", "t2"]);
    }

    #[test]
    fn test_ordered_terminals_after_removal() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);

        col.remove("t2");

        let ordered: Vec<&str> = col
            .ordered_terminals()
            .iter()
            .map(|t| t.id.as_str())
            .collect();
        assert_eq!(ordered, vec!["t1", "t3"]);
    }

    #[test]
    fn test_reorder_by_ids_rejects_missing_ids() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);

        assert!(!col.reorder_by_ids("missing", "t1"));
        assert!(!col.reorder_by_ids("t1", "missing"));

        let ordered: Vec<&str> = col.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ordered, vec!["t1", "t2"]);
    }

    #[test]
    fn test_reorder_by_ids_same_id_is_noop() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);

        assert!(col.reorder_by_ids("t2", "t2"));
        assert_eq!(col.active_id(), Some("t1"));

        let ordered: Vec<&str> = col.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ordered, vec!["t1", "t2"]);
    }

    #[test]
    fn test_reorder_by_ids_preserves_active_identity() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);
        col.set_active("t2");

        assert!(col.reorder_by_ids("t3", "t1"));
        assert_eq!(col.active_id(), Some("t2"));

        let ordered: Vec<&str> = col.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ordered, vec!["t3", "t1", "t2"]);
    }

    #[test]
    fn test_next_previous_wrap_after_reorder() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);
        assert!(col.reorder_by_ids("t3", "t1"));
        // New order: t3, t1, t2.

        col.set_active("t2");
        col.next();
        assert_eq!(col.active_id(), Some("t3"));

        col.previous();
        assert_eq!(col.active_id(), Some("t2"));
    }

    #[test]
    fn test_mru_tracks_recent_activation_order() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);
        assert_eq!(col.mru_terminal_ids(), vec!["t1", "t2", "t3"]);

        col.set_active("t2");
        assert_eq!(col.mru_terminal_ids(), vec!["t2", "t1", "t3"]);
        assert_eq!(col.mru_next_after_active(), Some("t1"));

        col.next();
        assert_eq!(col.active_id(), Some("t3"));
        assert_eq!(col.mru_terminal_ids(), vec!["t3", "t2", "t1"]);
        assert_eq!(col.mru_next_after_active(), Some("t2"));
    }

    #[test]
    fn test_mru_cleanup_removes_closed_ids_and_promotes_fallback_active() {
        let mut col = TerminalCollection::new();
        col.add("t1".into(), 24, 80);
        col.add("t2".into(), 24, 80);
        col.add("t3".into(), 24, 80);
        col.set_active("t3");
        assert_eq!(col.mru_terminal_ids(), vec!["t3", "t1", "t2"]);

        col.remove("t2");
        assert_eq!(col.mru_terminal_ids(), vec!["t3", "t1"]);

        col.remove("t3");
        assert_eq!(col.active_id(), Some("t1"));
        assert_eq!(col.mru_terminal_ids(), vec!["t1"]);
        assert_eq!(col.mru_next_after_active(), None);
    }

    #[test]
    fn test_mru_terminal_ids_for_workspace_filters_to_active_scope() {
        let mut col = TerminalCollection::new();
        col.add_to_workspace("w1-a".into(), 24, 80, "w1".into());
        col.add_to_workspace("w2-a".into(), 24, 80, "w2".into());
        col.add_to_workspace("w1-b".into(), 24, 80, "w1".into());
        col.add("no-workspace".into(), 24, 80);

        col.set_active("w1-b");
        col.set_active("w2-a");
        col.set_active("no-workspace");

        assert_eq!(
            col.mru_terminal_ids_for_workspace(Some("w1")),
            vec!["w1-b", "w1-a"]
        );
        assert_eq!(col.mru_terminal_ids_for_workspace(Some("w2")), vec!["w2-a"]);
        assert_eq!(
            col.mru_terminal_ids_for_workspace(None),
            vec!["no-workspace"]
        );
    }

    #[test]
    fn test_display_name_strips_path_and_exe() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);

        info.process_name = "C:\\Program Files\\PowerShell\\7\\pwsh.exe".into();
        assert_eq!(info.display_name(), "PowerShell");

        info.process_name = "/usr/bin/bash".into();
        assert_eq!(info.display_name(), "Bash");

        info.process_name = "cmd.exe".into();
        assert_eq!(info.display_name(), "Command Prompt");

        info.process_name = "zsh".into();
        assert_eq!(info.display_name(), "Zsh");

        info.process_name = "fish".into();
        assert_eq!(info.display_name(), "Fish");

        info.process_name = "node".into();
        assert_eq!(info.display_name(), "Node.js");

        info.process_name = "python3".into();
        assert_eq!(info.display_name(), "Python");

        info.process_name = "some-tool".into();
        assert_eq!(info.display_name(), "Some-tool");

        info.process_name = String::new();
        assert_eq!(info.display_name(), "Terminal");
    }

    #[test]
    fn test_extract_cwd_detects_paths() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);

        info.title = "C:\\Users\\test\\project".into();
        assert_eq!(info.extract_cwd(), Some("C:\\Users\\test\\project"));

        info.title = "/home/user/project".into();
        assert_eq!(info.extract_cwd(), Some("/home/user/project"));

        info.title = "My Shell".into();
        assert_eq!(info.extract_cwd(), None);

        info.title = String::new();
        assert_eq!(info.extract_cwd(), None);
    }

    // -- needs_refetch flag tests (issue #845: micro-blinking) --

    #[test]
    fn needs_refetch_defaults_to_false() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);
        assert!(!info.needs_refetch);
    }

    #[test]
    fn needs_refetch_set_during_coalesced_output() {
        // Simulates the race: output arrives while a fetch is in-flight.
        // The TerminalOutput handler sets needs_refetch = true so that
        // GridFetched triggers a follow-up fetch instead of dropping the event.
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);

        // Simulate fetch_grid() starting
        info.dirty = true;
        info.fetching = true;

        // Simulate coalesced TerminalOutput: dirty && fetching → set needs_refetch
        assert!(info.dirty && info.fetching);
        info.needs_refetch = true;

        // Simulate GridFetched arriving
        info.fetching = false;
        info.dirty = false;
        let should_refetch = info.needs_refetch;
        info.needs_refetch = false;

        assert!(
            should_refetch,
            "needs_refetch must trigger a follow-up fetch"
        );
    }

    #[test]
    fn needs_refetch_not_set_without_coalescing() {
        // When no output events arrive during a fetch, needs_refetch stays false.
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);

        info.dirty = true;
        info.fetching = true;

        // GridFetched arrives with no coalesced events
        info.fetching = false;
        info.dirty = false;

        assert!(
            !info.needs_refetch,
            "no coalesced events → no refetch needed"
        );
    }

    // -- last_grid_fingerprint tests (issue #845: heartbeat render loop) --

    #[test]
    fn last_grid_fingerprint_defaults_to_none() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);
        assert!(info.last_grid_fingerprint.is_none());
    }

    #[test]
    fn fingerprint_match_skips_render() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);

        let fp = (100_usize, 0_usize, 5_u16, 10_u16, false, 24_usize, 80_usize, false);
        info.last_grid_fingerprint = Some(fp);

        // Simulate GridFetched with identical fingerprint
        let mut should_render = true;
        let new_fp = fp;
        if should_render && info.last_grid_fingerprint == Some(new_fp) {
            should_render = false;
        }
        info.last_grid_fingerprint = Some(new_fp);

        assert!(!should_render, "unchanged grid should skip render");
    }

    #[test]
    fn fingerprint_mismatch_allows_render() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);

        let old_fp = (100_usize, 0_usize, 5_u16, 10_u16, false, 24_usize, 80_usize, false);
        info.last_grid_fingerprint = Some(old_fp);

        // Simulate GridFetched with changed scrollback
        let mut should_render = true;
        let new_fp = (101_usize, 0_usize, 5_u16, 10_u16, false, 24_usize, 80_usize, false);
        if should_render && info.last_grid_fingerprint == Some(new_fp) {
            should_render = false;
        }
        info.last_grid_fingerprint = Some(new_fp);

        assert!(should_render, "changed grid should allow render");
    }

    #[test]
    fn fingerprint_none_allows_render() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);

        assert!(info.last_grid_fingerprint.is_none());

        // Simulate first GridFetched (no prior fingerprint)
        let mut should_render = true;
        let new_fp = (0_usize, 0_usize, 0_u16, 0_u16, false, 24_usize, 0_usize, false);
        if should_render && info.last_grid_fingerprint == Some(new_fp) {
            should_render = false;
        }
        info.last_grid_fingerprint = Some(new_fp);

        assert!(should_render, "first fetch must always render");
    }

    #[test]
    fn fingerprint_reset_forces_render() {
        let mut col = TerminalCollection::new();
        let info = col.add("t1".into(), 24, 80);

        let fp = (50_usize, 0_usize, 3_u16, 7_u16, false, 24_usize, 80_usize, false);
        info.last_grid_fingerprint = Some(fp);

        // Simulate resize clearing the fingerprint
        info.last_grid_fingerprint = None;

        // Same content as before resize — should still render because
        // fingerprint was reset
        let mut should_render = true;
        let new_fp = fp;
        if should_render && info.last_grid_fingerprint == Some(new_fp) {
            should_render = false;
        }
        info.last_grid_fingerprint = Some(new_fp);

        assert!(should_render, "render must proceed after fingerprint reset");
    }
}
