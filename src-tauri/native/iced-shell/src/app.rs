use std::sync::Arc;

use futures_channel::mpsc;
use iced::keyboard;
use iced::widget::{canvas, center, column, row, stack, text};
use iced::{event, window, Element, Length, Subscription, Task};

use godly_app_adapter::clipboard;
use godly_app_adapter::commands;
use godly_app_adapter::daemon_client::NativeDaemonClient;
use godly_app_adapter::keys::key_to_pty_bytes;
use godly_app_adapter::shortcuts::{self, AppAction};
use godly_protocol::types::RichGridData;

use godly_terminal_surface::{FontMetrics, GridPos as SurfaceGridPos, TerminalCanvas};

use crate::notification_state::NotificationTracker;
use crate::selection::{GridPos, SelectionState};
use crate::settings_dialog::{self, SettingsTab};
use crate::shortcuts_tab;
use crate::sidebar::{self, SIDEBAR_WIDTH};
use crate::split_pane::{view_layout, LayoutNode, SplitDirection};
use crate::subscription::{daemon_events, ChannelEventSink, DaemonEventMsg};
use crate::tab_bar::{self, TAB_BAR_HEIGHT};
use crate::terminal_state::TerminalCollection;
use crate::workspace_state::WorkspaceCollection;

/// Main Iced application state — multi-terminal with event-driven updates.
pub struct GodlyApp {
    /// Daemon client (shared with bridge thread).
    client: Option<Arc<NativeDaemonClient>>,
    /// All terminal sessions (global, with workspace_id tracking).
    terminals: TerminalCollection,
    /// Workspace collection — each workspace owns its layout tree and focused terminal.
    workspaces: WorkspaceCollection,
    /// Error message to display if initialization failed.
    init_error: Option<String>,
    /// Event receiver for the daemon subscription (taken once by the subscription).
    event_receiver: Arc<parking_lot::Mutex<Option<mpsc::UnboundedReceiver<DaemonEventMsg>>>>,
    /// Window dimensions in logical pixels.
    window_width: f32,
    window_height: f32,
    /// Font metrics for cell sizing and grid dimension calculations.
    font_metrics: FontMetrics,
    /// Mouse text selection state.
    selection: SelectionState,
    /// Whether the workspace sidebar is visible.
    sidebar_visible: bool,
    /// Whether the settings dialog is open.
    settings_open: bool,
    /// Active tab in the settings dialog.
    settings_tab: String,
    /// Notification tracker for terminals.
    notifications: NotificationTracker,
    /// Counter for generating workspace names.
    next_workspace_num: u32,
}

impl Default for GodlyApp {
    fn default() -> Self {
        Self {
            client: None,
            terminals: TerminalCollection::new(),
            workspaces: WorkspaceCollection::new(),
            init_error: None,
            event_receiver: Arc::new(parking_lot::Mutex::new(None)),
            window_width: 1200.0,
            window_height: 800.0,
            font_metrics: FontMetrics::default(),
            selection: SelectionState::default(),
            sidebar_visible: false,
            settings_open: false,
            settings_tab: "shortcuts".to_string(),
            notifications: NotificationTracker::new(),
            next_workspace_num: 2, // First workspace is "Workspace 1"
        }
    }
}

/// Application messages.
#[derive(Debug, Clone)]
pub enum Message {
    /// Daemon event received from bridge thread.
    DaemonEvent(DaemonEventMsg),
    /// Grid snapshot fetched for a specific session.
    GridFetched {
        session_id: String,
        grid: RichGridData,
    },
    /// Grid fetch failed for a specific session.
    GridFetchFailed { session_id: String, error: String },
    /// Keyboard event from iced.
    KeyboardEvent(keyboard::Event),
    /// Initialization complete.
    Initialized(Result<InitResult, String>),
    /// User clicked a tab.
    TabClicked(String),
    /// User wants a new terminal.
    NewTabRequested,
    /// User wants to close a terminal.
    CloseTabRequested(String),
    /// New terminal created by daemon.
    TerminalCreated(Result<String, String>),
    /// Window was resized.
    WindowResized { width: f32, height: f32 },
    /// Mouse wheel scrolled (for scrollback).
    MouseWheel { delta_y: f32 },
    /// Mouse button pressed at pixel position (starts selection).
    SelectionStart { x: f32, y: f32 },
    /// Mouse dragged to pixel position (updates selection).
    SelectionUpdate { x: f32, y: f32 },
    /// Mouse button released (finishes selection).
    SelectionEnd,
    /// Split the focused pane in a direction, creating a new terminal.
    SplitPane { direction: SplitDirection },
    /// Remove the focused pane from its split, promoting its sibling.
    UnsplitPane,
    /// Cycle focus to the next pane in the layout tree.
    FocusNextPane,
    /// User clicked a workspace in the sidebar.
    WorkspaceClicked(String),
    /// User requested a new workspace.
    NewWorkspaceRequested,
    /// Toggle the sidebar visibility.
    ToggleSidebar,
    /// Toggle the settings dialog.
    ToggleSettings,
    /// User clicked a settings tab.
    SettingsTabClicked(String),
    /// Clipboard text read successfully in background — write to terminal.
    ClipboardPasted {
        terminal_id: String,
        text: String,
    },
    /// Clipboard read failed in background.
    ClipboardPasteFailed(String),
}

/// Result of initialization — either a fresh terminal or recovered sessions.
#[derive(Debug, Clone)]
pub enum InitResult {
    /// A brand new session was created.
    Fresh { session_id: String },
    /// Existing daemon sessions were recovered (app restart / reconnect).
    Recovered {
        session_ids: Vec<String>,
        first_id: String,
    },
}

impl GodlyApp {
    // -----------------------------------------------------------------------
    // Workspace-aware helpers
    // -----------------------------------------------------------------------

    /// Get the active workspace's layout, if any.
    fn active_layout(&self) -> Option<&LayoutNode> {
        self.workspaces.active().map(|ws| &ws.layout)
    }

    /// Get the active workspace's focused terminal ID.
    fn active_focused(&self) -> Option<&str> {
        self.workspaces.active().map(|ws| ws.focused_terminal.as_str())
    }

    /// Get the target terminal ID — prefer workspace's focused pane, fall back to active tab.
    fn target_terminal_id(&self) -> Option<&str> {
        self.active_focused()
            .or_else(|| self.terminals.active_id())
    }

    /// Get terminals filtered to the active workspace.
    fn active_workspace_terminals(&self) -> Vec<&crate::terminal_state::TerminalInfo> {
        if let Some(ws) = self.workspaces.active() {
            self.terminals.terminals_for_workspace(&ws.id)
        } else {
            // Fallback: show all terminals
            self.terminals.iter().collect()
        }
    }

    pub fn title(&self) -> String {
        if let Some(tid) = self.active_focused() {
            if let Some(term) = self.terminals.get(tid) {
                let label = term.tab_label();
                if label != "Terminal" {
                    return format!("{} — Godly Terminal (Native)", label);
                }
            }
        }
        format!(
            "Godly Terminal (Native) — contract v{}",
            godly_protocol::FRONTEND_CONTRACT_VERSION
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // --- Initialization ---
            Message::Initialized(Ok(result)) => {
                let rows = self.calculate_rows();
                let cols = self.calculate_cols();

                match result {
                    InitResult::Fresh { session_id } => {
                        self.terminals.add_to_workspace(
                            session_id.clone(),
                            rows,
                            cols,
                            "w-default".to_string(),
                        );
                        self.workspaces.add(
                            "w-default".to_string(),
                            "Workspace 1".to_string(),
                            session_id.clone(),
                        );
                        self.terminals.set_active(&session_id);
                        return self.fetch_grid(&session_id);
                    }
                    InitResult::Recovered {
                        session_ids,
                        first_id,
                    } => {
                        for id in &session_ids {
                            self.terminals.add_to_workspace(
                                id.clone(),
                                rows,
                                cols,
                                "w-default".to_string(),
                            );
                        }
                        self.terminals.set_active(&first_id);
                        // Create default workspace with first session's layout.
                        self.workspaces.add(
                            "w-default".to_string(),
                            "Workspace 1".to_string(),
                            first_id.clone(),
                        );
                        // Add remaining sessions to the workspace's layout.
                        if let Some(ws) = self.workspaces.get_mut("w-default") {
                            for id in session_ids.iter().skip(1) {
                                ws.layout.split_leaf(
                                    &first_id,
                                    id.clone(),
                                    SplitDirection::Vertical,
                                );
                            }
                        }
                        // Fetch grids for all recovered sessions.
                        let tasks: Vec<Task<Message>> =
                            session_ids.iter().map(|id| self.fetch_grid(id)).collect();
                        return Task::batch(tasks);
                    }
                }
            }
            Message::Initialized(Err(e)) => {
                log::error!("Initialization failed: {}", e);
                self.init_error = Some(e);
            }

            // --- Daemon events (channel-driven, no polling) ---
            Message::DaemonEvent(DaemonEventMsg::TerminalOutput { session_id }) => {
                if let Some(term) = self.terminals.get_mut(&session_id) {
                    term.dirty = true;
                }
                // Track notifications for non-focused terminals.
                let is_focused = self.active_focused() == Some(session_id.as_str());
                if !is_focused {
                    self.notifications.record_output(&session_id);
                }
                return self.fetch_grid(&session_id);
            }
            Message::DaemonEvent(DaemonEventMsg::SessionClosed {
                session_id,
                exit_code,
            }) => {
                if let Some(term) = self.terminals.get_mut(&session_id) {
                    term.exited = true;
                    term.exit_code = exit_code;
                }
                log::info!(
                    "Session {} closed (exit_code={:?})",
                    session_id,
                    exit_code
                );
            }
            Message::DaemonEvent(DaemonEventMsg::ProcessChanged {
                session_id,
                process_name,
            }) => {
                if let Some(term) = self.terminals.get_mut(&session_id) {
                    term.process_name = process_name;
                }
            }
            Message::DaemonEvent(DaemonEventMsg::Bell { session_id }) => {
                self.notifications.record_bell(&session_id);
                log::debug!("Bell from session {}", session_id);
            }

            // --- Grid fetch results ---
            Message::GridFetched { session_id, grid } => {
                if let Some(term) = self.terminals.get_mut(&session_id) {
                    term.fetching = false;
                    term.dirty = false;
                    term.title = grid.title.clone();
                    term.total_scrollback = grid.total_scrollback;
                    term.scrollback_offset = grid.scrollback_offset;
                    term.grid = Some(grid);
                }
            }
            Message::GridFetchFailed { session_id, error } => {
                if let Some(term) = self.terminals.get_mut(&session_id) {
                    term.fetching = false;
                }
                log::error!("Grid fetch failed for {}: {}", session_id, error);
            }

            // --- Clipboard paste (background result) ---
            Message::ClipboardPasted { terminal_id, text } => {
                if let Some(client) = &self.client {
                    let _ = commands::write_to_terminal(client, &terminal_id, text.as_bytes());
                }
            }
            Message::ClipboardPasteFailed(e) => {
                // "Clipboard empty" is normal — only log actual errors.
                if e != "Clipboard empty" {
                    log::error!("Clipboard paste failed: {}", e);
                }
            }

            // --- Keyboard input (shortcut-first, then forward to PTY) ---
            Message::KeyboardEvent(keyboard::Event::KeyPressed {
                key, modifiers, ..
            }) => {
                // Check app shortcuts first.
                if let Some(action) = shortcuts::check_app_shortcut(&key, modifiers) {
                    return self.handle_app_action(action);
                }

                // Any keypress clears selection.
                self.selection.clear();

                // Forward to PTY — send to focused terminal, not just active tab.
                if let Some(bytes) = key_to_pty_bytes(&key, modifiers) {
                    if let (Some(tid), Some(client)) =
                        (self.target_terminal_id(), &self.client)
                    {
                        let _ = commands::write_to_terminal(client, tid, &bytes);
                    }
                }
            }
            Message::KeyboardEvent(_) => {}

            // --- Mouse wheel scrollback ---
            Message::MouseWheel { delta_y } => {
                let lines = -(delta_y * 3.0) as isize;
                return self.scroll_active(lines);
            }

            // --- Tab management ---
            Message::TabClicked(id) => {
                self.terminals.set_active(&id);
                // Update workspace's focused terminal.
                if let Some(ws) = self.workspaces.active_mut() {
                    if ws.layout.find_leaf(&id) {
                        ws.focused_terminal = id.clone();
                    }
                }
                self.notifications.mark_read(&id);
            }
            Message::NewTabRequested => {
                return self.create_new_terminal();
            }
            Message::CloseTabRequested(id) => {
                return self.close_terminal(&id);
            }
            Message::TerminalCreated(Ok(session_id)) => {
                let rows = self.calculate_rows();
                let cols = self.calculate_cols();
                let ws_id = self.workspaces.active_id().map(str::to_string);

                if let Some(ws_id) = &ws_id {
                    self.terminals.add_to_workspace(
                        session_id.clone(),
                        rows,
                        cols,
                        ws_id.clone(),
                    );
                } else {
                    self.terminals.add(session_id.clone(), rows, cols);
                }

                // If this terminal is already in the layout (from a split), just focus it.
                let in_layout = self
                    .active_layout()
                    .map(|l| l.find_leaf(&session_id))
                    .unwrap_or(false);

                if in_layout {
                    if let Some(ws) = self.workspaces.active_mut() {
                        ws.focused_terminal = session_id.clone();
                    }
                } else {
                    // New tab — update the workspace layout.
                    self.terminals.set_active(&session_id);
                    if let Some(ws) = self.workspaces.active_mut() {
                        ws.layout = LayoutNode::Leaf {
                            terminal_id: session_id.clone(),
                        };
                        ws.focused_terminal = session_id.clone();
                    }
                }
                return self.fetch_grid(&session_id);
            }
            Message::TerminalCreated(Err(e)) => {
                log::error!("Failed to create terminal: {}", e);
            }

            // --- Window resize ---
            Message::WindowResized { width, height } => {
                let old_cols = self.calculate_cols();
                let old_rows = self.calculate_rows();

                self.window_width = width;
                self.window_height = height;

                let new_cols = self.calculate_cols();
                let new_rows = self.calculate_rows();

                if new_cols != old_cols || new_rows != old_rows {
                    return self.resize_active_terminal(new_rows, new_cols);
                }
            }

            // --- Mouse selection ---
            Message::SelectionStart { x, y } => {
                let pos = self.pixel_to_grid(x, y);
                self.selection.start(pos);
            }
            Message::SelectionUpdate { x, y } => {
                if self.selection.active {
                    let pos = self.pixel_to_grid(x, y);
                    self.selection.update(pos);
                }
            }
            Message::SelectionEnd => {
                self.selection.finish();
            }

            // --- Split pane operations ---
            Message::SplitPane { direction } => {
                return self.split_focused_pane(direction);
            }
            Message::UnsplitPane => {
                return self.unsplit_focused_pane();
            }
            Message::FocusNextPane => {
                self.cycle_focus();
            }

            // --- Workspace operations ---
            Message::WorkspaceClicked(id) => {
                self.workspaces.set_active(&id);
                // Mark the new workspace's focused terminal as read.
                if let Some(ws) = self.workspaces.active() {
                    self.notifications.mark_read(&ws.focused_terminal);
                }
            }
            Message::NewWorkspaceRequested => {
                return self.create_new_workspace();
            }

            // --- Sidebar + Settings ---
            Message::ToggleSidebar => {
                self.sidebar_visible = !self.sidebar_visible;
            }
            Message::ToggleSettings => {
                self.settings_open = !self.settings_open;
            }
            Message::SettingsTabClicked(tab_id) => {
                self.settings_tab = tab_id;
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        if let Some(ref err) = self.init_error {
            return center(text(format!("Initialization error: {}", err)).size(18)).into();
        }

        if self.client.is_none() && self.terminals.count() == 0 {
            return center(text("Connecting to daemon...").size(18)).into();
        }

        // Tab bar — show terminals for the active workspace.
        let active_id = self.active_focused();
        let ordered = self.terminals.ordered_terminals();
        let tab_bar = tab_bar::view_tab_bar(
            &ordered,
            active_id,
            |id| Message::TabClicked(id),
            |id| Message::CloseTabRequested(id),
            Message::NewTabRequested,
        );

        // Render the layout tree from active workspace.
        let focused_id = self.active_focused();
        let terminal_view: Element<'_, Message> = if let Some(layout) = self.active_layout() {
            if layout.leaf_count() > 0 && self.terminals.count() > 0 {
                view_layout(layout, &|terminal_id: &str| {
                    self.render_terminal_pane(terminal_id, focused_id)
                })
            } else {
                center(text("No active terminal").size(16)).into()
            }
        } else {
            center(text("No active terminal").size(16)).into()
        };

        // Compose main content with optional sidebar.
        let main_area = column![tab_bar, terminal_view];

        let main_content: Element<'_, Message> = if self.sidebar_visible {
            let sidebar = sidebar::view_sidebar(
                self.workspaces.as_slice(),
                self.workspaces.active_id(),
                |id| Message::WorkspaceClicked(id),
                Message::NewWorkspaceRequested,
            );
            row![sidebar, main_area].into()
        } else {
            main_area.into()
        };

        // Overlay settings dialog if open.
        if self.settings_open {
            let tabs = &[
                SettingsTab {
                    id: "shortcuts",
                    label: "Shortcuts",
                },
            ];
            let tab_content = shortcuts_tab::view_shortcuts_tab();
            let settings_overlay = settings_dialog::view_settings_dialog(
                tabs,
                &self.settings_tab,
                tab_content,
                |id| Message::SettingsTabClicked(id),
                Message::ToggleSettings,
            );
            stack![main_content, settings_overlay].into()
        } else {
            main_content
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            // Keyboard events.
            keyboard::listen().map(Message::KeyboardEvent),
            // Daemon events via channel.
            daemon_events(Arc::clone(&self.event_receiver)).map(Message::DaemonEvent),
            // Window resize + mouse events.
            event::listen_with(|ev, _status, _window_id| match ev {
                event::Event::Window(window::Event::Resized(size)) => {
                    Some(Message::WindowResized {
                        width: size.width,
                        height: size.height,
                    })
                }
                event::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => {
                    let delta_y = match delta {
                        iced::mouse::ScrollDelta::Lines { y, .. } => y,
                        iced::mouse::ScrollDelta::Pixels { y, .. } => y / 20.0,
                    };
                    Some(Message::MouseWheel { delta_y })
                }
                event::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Left,
                )) => Some(Message::SelectionStart { x: 0.0, y: 0.0 }),
                event::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Some(Message::SelectionEnd),
                event::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Some(Message::SelectionUpdate {
                        x: position.x,
                        y: position.y,
                    })
                }
                _ => None,
            }),
        ])
    }

    // -----------------------------------------------------------------------
    // App action dispatch
    // -----------------------------------------------------------------------

    fn handle_app_action(&mut self, action: AppAction) -> Task<Message> {
        match action {
            AppAction::NewTab => self.create_new_terminal(),
            AppAction::CloseTab => {
                if let Some(id) = self.active_focused().map(str::to_string) {
                    self.close_terminal(&id)
                } else {
                    Task::none()
                }
            }
            AppAction::NextTab => {
                self.terminals.next();
                Task::none()
            }
            AppAction::PreviousTab => {
                self.terminals.previous();
                Task::none()
            }
            AppAction::ZoomIn => {
                self.font_metrics =
                    FontMetrics::from_font_size(self.font_metrics.font_size + 1.0);
                self.resize_all_terminals()
            }
            AppAction::ZoomOut => {
                let new_size = (self.font_metrics.font_size - 1.0).max(8.0);
                self.font_metrics = FontMetrics::from_font_size(new_size);
                self.resize_all_terminals()
            }
            AppAction::ZoomReset => {
                self.font_metrics = FontMetrics::default();
                self.resize_all_terminals()
            }
            AppAction::ScrollPageUp => {
                let page = self.calculate_rows() as isize;
                self.scroll_active(-page)
            }
            AppAction::ScrollPageDown => {
                let page = self.calculate_rows() as isize;
                self.scroll_active(page)
            }
            AppAction::ScrollToTop => self.scroll_to_top(),
            AppAction::ScrollToBottom => self.scroll_to_bottom(),
            AppAction::Copy => {
                self.copy_selection();
                Task::none()
            }
            AppAction::Paste => self.paste_from_clipboard(),
            AppAction::SplitRight => self.split_focused_pane(SplitDirection::Horizontal),
            AppAction::SplitDown => self.split_focused_pane(SplitDirection::Vertical),
            AppAction::Unsplit => self.unsplit_focused_pane(),
            AppAction::FocusNextPane => {
                self.cycle_focus();
                Task::none()
            }
            AppAction::SelectAll => {
                self.select_all();
                Task::none()
            }
            AppAction::NextWorkspace => {
                self.workspaces.next();
                if let Some(ws) = self.workspaces.active() {
                    self.notifications.mark_read(&ws.focused_terminal);
                }
                Task::none()
            }
            AppAction::PrevWorkspace => {
                self.workspaces.previous();
                if let Some(ws) = self.workspaces.active() {
                    self.notifications.mark_read(&ws.focused_terminal);
                }
                Task::none()
            }
            AppAction::ToggleSidebar => {
                self.sidebar_visible = !self.sidebar_visible;
                Task::none()
            }
            AppAction::OpenSettings => {
                self.settings_open = !self.settings_open;
                Task::none()
            }
            AppAction::RenameTab => {
                // TODO: Open rename dialog for focused terminal (future PR).
                Task::none()
            }
        }
    }

    // -----------------------------------------------------------------------
    // Workspace operations
    // -----------------------------------------------------------------------

    /// Create a new workspace with a fresh terminal.
    fn create_new_workspace(&mut self) -> Task<Message> {
        let ws_id = uuid::Uuid::new_v4().to_string();
        let session_id = uuid::Uuid::new_v4().to_string();
        let ws_name = format!("Workspace {}", self.next_workspace_num);
        self.next_workspace_num += 1;

        let rows = self.calculate_rows();
        let cols = self.calculate_cols();

        self.terminals.add_to_workspace(
            session_id.clone(),
            rows,
            cols,
            ws_id.clone(),
        );
        self.workspaces.add(ws_id.clone(), ws_name, session_id.clone());
        self.workspaces.set_active(&ws_id);
        self.terminals.set_active(&session_id);

        self.create_terminal_task(session_id)
    }

    // -----------------------------------------------------------------------
    // Scrollback
    // -----------------------------------------------------------------------

    fn scroll_active(&mut self, delta: isize) -> Task<Message> {
        let Some(active_id) = self.active_focused().map(str::to_string) else {
            return Task::none();
        };

        let Some(term) = self.terminals.get_mut(&active_id) else {
            return Task::none();
        };

        let new_offset = if delta < 0 {
            term.scrollback_offset
                .saturating_add((-delta) as usize)
                .min(term.total_scrollback)
        } else {
            term.scrollback_offset.saturating_sub(delta as usize)
        };

        term.scrollback_offset = new_offset;

        self.scroll_fetch(&active_id, new_offset)
    }

    fn scroll_to_top(&mut self) -> Task<Message> {
        let Some(active_id) = self.active_focused().map(str::to_string) else {
            return Task::none();
        };

        let max = self
            .terminals
            .get(&active_id)
            .map(|t| t.total_scrollback)
            .unwrap_or(0);

        if let Some(term) = self.terminals.get_mut(&active_id) {
            term.scrollback_offset = max;
        }

        self.scroll_fetch(&active_id, max)
    }

    fn scroll_to_bottom(&mut self) -> Task<Message> {
        let Some(active_id) = self.active_focused().map(str::to_string) else {
            return Task::none();
        };

        if let Some(term) = self.terminals.get_mut(&active_id) {
            term.scrollback_offset = 0;
        }

        self.scroll_fetch(&active_id, 0)
    }

    fn scroll_fetch(&self, session_id: &str, offset: usize) -> Task<Message> {
        let Some(client) = &self.client else {
            return Task::none();
        };

        let client = Arc::clone(client);
        let sid = session_id.to_string();
        let sid_ok = sid.clone();
        let sid_err = sid.clone();

        Task::perform(
            async move {
                let (tx, rx) = futures_channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = commands::scroll_and_get_snapshot(&client, &sid, offset);
                    let _ = tx.send(result);
                });
                rx.await
                    .unwrap_or_else(|_| Err("Background thread panicked".into()))
            },
            move |result| match result {
                Ok(grid) => Message::GridFetched {
                    session_id: sid_ok,
                    grid,
                },
                Err(e) => Message::GridFetchFailed {
                    session_id: sid_err,
                    error: e,
                },
            },
        )
    }

    // -----------------------------------------------------------------------
    // Terminal lifecycle
    // -----------------------------------------------------------------------

    fn fetch_grid(&mut self, session_id: &str) -> Task<Message> {
        let Some(client) = &self.client else {
            return Task::none();
        };

        if let Some(term) = self.terminals.get_mut(session_id) {
            if term.fetching {
                return Task::none();
            }
            term.fetching = true;
        } else {
            return Task::none();
        }

        let client = Arc::clone(client);
        let sid = session_id.to_string();
        let sid_ok = sid.clone();
        let sid_err = sid.clone();

        Task::perform(
            async move {
                let (tx, rx) = futures_channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = commands::get_grid_snapshot(&client, &sid);
                    let _ = tx.send(result);
                });
                rx.await
                    .unwrap_or_else(|_| Err("Background thread panicked".into()))
            },
            move |result| match result {
                Ok(grid) => Message::GridFetched {
                    session_id: sid_ok,
                    grid,
                },
                Err(e) => Message::GridFetchFailed {
                    session_id: sid_err,
                    error: e,
                },
            },
        )
    }

    fn create_new_terminal(&self) -> Task<Message> {
        self.create_terminal_task(uuid::Uuid::new_v4().to_string())
    }

    fn create_terminal_task(&self, session_id: String) -> Task<Message> {
        let Some(client) = &self.client else {
            return Task::done(Message::TerminalCreated(Err(
                "No daemon connection".to_string(),
            )));
        };

        let client = Arc::clone(client);
        let rows = self.calculate_rows();
        let cols = self.calculate_cols();

        Task::perform(
            async move {
                let (tx, rx) = futures_channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = commands::create_terminal(
                        &client,
                        &session_id,
                        godly_protocol::ShellType::Windows,
                        None,
                        rows,
                        cols,
                    )
                    .map(|_| session_id);
                    let _ = tx.send(result);
                });
                rx.await
                    .unwrap_or_else(|_| Err("Background thread panicked".into()))
            },
            Message::TerminalCreated,
        )
    }

    fn close_terminal(&mut self, session_id: &str) -> Task<Message> {
        // Remove from workspace layout.
        if let Some(ws) = self.workspaces.active_mut() {
            ws.layout.unsplit_leaf(session_id);
            // If the focused terminal was closed, update focus.
            if ws.focused_terminal == session_id {
                let leaf_ids = ws.layout.all_leaf_ids();
                ws.focused_terminal = leaf_ids
                    .first()
                    .map(|id| id.to_string())
                    .unwrap_or_default();
            }
        }

        self.terminals.remove(session_id);
        self.notifications.clear(session_id);

        if let Some(client) = &self.client {
            let _ = commands::close_terminal(client, session_id);
        }

        Task::none()
    }

    // -----------------------------------------------------------------------
    // Resize
    // -----------------------------------------------------------------------

    fn resize_active_terminal(&mut self, rows: u16, cols: u16) -> Task<Message> {
        let Some(active_id) = self.active_focused().map(str::to_string) else {
            return Task::none();
        };

        if let Some(term) = self.terminals.get_mut(&active_id) {
            term.rows = rows;
            term.cols = cols;
        }

        if let Some(client) = &self.client {
            let _ = commands::resize_terminal(client, &active_id, rows, cols);
        }

        self.fetch_grid(&active_id)
    }

    fn resize_all_terminals(&mut self) -> Task<Message> {
        let new_rows = self.calculate_rows();
        let new_cols = self.calculate_cols();

        let ids: Vec<String> = self.terminals.iter().map(|t| t.id.clone()).collect();

        for id in &ids {
            if let Some(term) = self.terminals.get_mut(id) {
                term.rows = new_rows;
                term.cols = new_cols;
            }
            if let Some(client) = &self.client {
                let _ = commands::resize_terminal(client, id, new_rows, new_cols);
            }
        }

        if let Some(active_id) = self.active_focused().map(str::to_string) {
            self.fetch_grid(&active_id)
        } else {
            Task::none()
        }
    }

    // -----------------------------------------------------------------------
    // Grid dimension calculations
    // -----------------------------------------------------------------------

    fn calculate_cols(&self) -> u16 {
        let sidebar_offset = if self.sidebar_visible {
            SIDEBAR_WIDTH
        } else {
            0.0
        };
        ((self.window_width - sidebar_offset) / self.font_metrics.cell_width).max(1.0) as u16
    }

    fn calculate_rows(&self) -> u16 {
        let available = (self.window_height - TAB_BAR_HEIGHT).max(0.0);
        (available / self.font_metrics.cell_height).max(1.0) as u16
    }

    // -----------------------------------------------------------------------
    // Split pane operations (workspace-aware)
    // -----------------------------------------------------------------------

    fn split_focused_pane(&mut self, direction: SplitDirection) -> Task<Message> {
        let Some(focused) = self.active_focused().map(str::to_string) else {
            return Task::none();
        };

        let new_id = uuid::Uuid::new_v4().to_string();

        // Split the active workspace's layout tree.
        if let Some(ws) = self.workspaces.active_mut() {
            ws.layout.split_leaf(&focused, new_id.clone(), direction);
        }

        self.create_terminal_task(new_id)
    }

    fn unsplit_focused_pane(&mut self) -> Task<Message> {
        let Some(focused) = self.active_focused().map(str::to_string) else {
            return Task::none();
        };

        if let Some(ws) = self.workspaces.active_mut() {
            if let Some(removed_id) = ws.layout.unsplit_leaf(&focused) {
                // Close the removed terminal.
                self.terminals.remove(&removed_id);
                self.notifications.clear(&removed_id);
                if let Some(client) = &self.client {
                    let _ = commands::close_terminal(client, &removed_id);
                }

                // Update focus.
                let leaf_ids = ws.layout.all_leaf_ids();
                if let Some(first) = leaf_ids.first() {
                    ws.focused_terminal = first.to_string();
                }
            }
        }

        Task::none()
    }

    fn cycle_focus(&mut self) {
        let Some(ws) = self.workspaces.active_mut() else {
            return;
        };

        if let Some(next_id) = ws.layout.next_leaf_id(&ws.focused_terminal) {
            let next_id = next_id.to_string();
            self.notifications.mark_read(&next_id);
            ws.focused_terminal = next_id;
        }
    }

    // -----------------------------------------------------------------------
    // Clipboard
    // -----------------------------------------------------------------------

    fn copy_selection(&mut self) {
        let Some(tid) = self.target_terminal_id() else {
            return;
        };

        let Some(term) = self.terminals.get(tid) else {
            return;
        };
        let Some(grid) = &term.grid else { return };

        let text = self.selection.selected_text(grid);
        if text.is_empty() {
            return;
        }

        if let Err(e) = clipboard::copy_to_clipboard(&text) {
            log::error!("Clipboard copy failed: {}", e);
        }
    }

    fn paste_from_clipboard(&self) -> Task<Message> {
        let Some(tid) = self.target_terminal_id() else {
            return Task::none();
        };

        let tid = tid.to_string();

        // Read clipboard on a background thread to avoid blocking the UI.
        // Local clipboard is fast (~1-5ms) but RDP/WSL forwarding can stall.
        Task::perform(
            async move {
                let (tx, rx) = futures_channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = clipboard::paste_from_clipboard();
                    let _ = tx.send(result);
                });
                rx.await
                    .unwrap_or_else(|_| Err("Clipboard background thread panicked".into()))
            },
            move |result| match result {
                Ok(text) if !text.is_empty() => Message::ClipboardPasted {
                    terminal_id: tid.clone(),
                    text,
                },
                Ok(_) => Message::ClipboardPasteFailed("Clipboard empty".into()),
                Err(e) => Message::ClipboardPasteFailed(e),
            },
        )
    }

    // -----------------------------------------------------------------------
    // Selection
    // -----------------------------------------------------------------------

    fn select_all(&mut self) {
        let Some(tid) = self.target_terminal_id() else {
            return;
        };
        let Some(term) = self.terminals.get(tid) else {
            return;
        };
        let Some(grid) = &term.grid else { return };

        if grid.rows.is_empty() {
            return;
        }

        let last_row = grid.rows.len() - 1;
        let last_col = grid.rows[last_row].cells.len().saturating_sub(1);
        self.selection.start(GridPos { row: 0, col: 0 });
        self.selection.update(GridPos {
            row: last_row,
            col: last_col,
        });
        self.selection.finish();
    }

    fn pixel_to_grid(&self, x: f32, y: f32) -> GridPos {
        let sidebar_offset = if self.sidebar_visible {
            SIDEBAR_WIDTH
        } else {
            0.0
        };
        let adjusted_x = (x - sidebar_offset).max(0.0);
        let adjusted_y = (y - TAB_BAR_HEIGHT).max(0.0);
        let row = (adjusted_y / self.font_metrics.cell_height) as usize;
        let col = (adjusted_x / self.font_metrics.cell_width) as usize;
        GridPos { row, col }
    }

    // -----------------------------------------------------------------------
    // Rendering helpers
    // -----------------------------------------------------------------------

    fn render_terminal_pane<'a>(
        &'a self,
        terminal_id: &str,
        focused_id: Option<&str>,
    ) -> Element<'a, Message> {
        let Some(term) = self.terminals.get(terminal_id) else {
            return center(text("Session not found").size(14)).into();
        };

        let is_focused = focused_id == Some(terminal_id);
        let selection = if is_focused && (self.selection.active || self.has_selection()) {
            let (start, end) = self.selection.normalized();
            Some((
                SurfaceGridPos {
                    row: start.row,
                    col: start.col,
                },
                SurfaceGridPos {
                    row: end.row,
                    col: end.col,
                },
            ))
        } else {
            None
        };

        let tc = TerminalCanvas {
            grid: term.grid.clone(),
            metrics: self.font_metrics,
            selection,
        };
        canvas(tc)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn has_selection(&self) -> bool {
        let (start, end) = self.selection.normalized();
        start != end
    }
}

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

/// Initialize the app: connect to daemon, set up bridge, recover or create sessions.
pub fn initialize(app: &mut GodlyApp) -> Task<Message> {
    let client = match NativeDaemonClient::connect_or_launch() {
        Ok(c) => Arc::new(c),
        Err(e) => {
            return Task::done(Message::Initialized(Err(e)));
        }
    };

    let (tx, rx) = mpsc::unbounded();
    *app.event_receiver.lock() = Some(rx);

    let sink = Arc::new(ChannelEventSink::new(tx));
    if let Err(e) = client.setup_bridge(sink) {
        return Task::done(Message::Initialized(Err(e)));
    }

    app.client = Some(Arc::clone(&client));

    let rows = app.calculate_rows();
    let cols = app.calculate_cols();

    Task::perform(
        async move {
            let (tx, rx) = futures_channel::oneshot::channel();
            std::thread::spawn(move || {
                let sessions =
                    match client.send_request(&godly_protocol::Request::ListSessions) {
                        Ok(godly_protocol::Response::SessionList { sessions }) => sessions,
                        _ => vec![],
                    };

                let live_sessions: Vec<_> =
                    sessions.into_iter().filter(|s| s.running).collect();

                if !live_sessions.is_empty() {
                    let mut recovered_ids = Vec::new();
                    for session in &live_sessions {
                        match commands::attach_session(&client, &session.id) {
                            Ok(()) => {
                                log::info!("Recovered session: {}", session.id);
                                recovered_ids.push(session.id.clone());
                            }
                            Err(e) => {
                                log::warn!(
                                    "Failed to recover session {}: {}",
                                    session.id,
                                    e
                                );
                            }
                        }
                    }

                    if !recovered_ids.is_empty() {
                        let first_id = recovered_ids[0].clone();
                        let _ = tx.send(Ok(InitResult::Recovered {
                            session_ids: recovered_ids,
                            first_id,
                        }));
                        return;
                    }
                }

                let session_id = uuid::Uuid::new_v4().to_string();
                let sid = session_id.clone();
                let result = commands::create_terminal(
                    &client,
                    &sid,
                    godly_protocol::ShellType::Windows,
                    None,
                    rows,
                    cols,
                )
                .map(|_| InitResult::Fresh { session_id: sid });
                let _ = tx.send(result);
            });
            rx.await
                .unwrap_or_else(|_| Err("Background thread panicked".into()))
        },
        Message::Initialized,
    )
}
