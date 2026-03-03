use std::sync::Arc;

use futures_channel::mpsc;
use iced::keyboard;
use iced::widget::{canvas, center, column, text};
use iced::{event, window, Element, Length, Subscription, Task};

use godly_app_adapter::clipboard;
use godly_app_adapter::commands;
use godly_app_adapter::daemon_client::NativeDaemonClient;
use godly_app_adapter::keys::key_to_pty_bytes;
use godly_app_adapter::shortcuts::{self, AppAction};
use godly_protocol::types::RichGridData;

use godly_terminal_surface::{FontMetrics, GridPos as SurfaceGridPos, TerminalCanvas};

use crate::selection::{GridPos, SelectionState};
use crate::split_pane::{view_layout, LayoutNode, SplitDirection};
use crate::subscription::{daemon_events, ChannelEventSink, DaemonEventMsg};
use crate::tab_bar::{self, TAB_BAR_HEIGHT};
use crate::terminal_state::TerminalCollection;

/// Main Iced application state — multi-terminal with event-driven updates.
pub struct GodlyApp {
    /// Daemon client (shared with bridge thread).
    client: Option<Arc<NativeDaemonClient>>,
    /// All terminal sessions.
    terminals: TerminalCollection,
    /// Error message to display if initialization failed.
    init_error: Option<String>,
    /// Event receiver for the daemon subscription (taken once by the subscription).
    event_receiver: Arc<parking_lot::Mutex<Option<mpsc::UnboundedReceiver<DaemonEventMsg>>>>,
    /// Window dimensions in logical pixels.
    window_width: f32,
    window_height: f32,
    /// Font metrics for cell sizing and grid dimension calculations.
    font_metrics: FontMetrics,
    /// Layout tree for split panes. A single leaf when no splits exist.
    layout: LayoutNode,
    /// ID of the focused terminal pane (receives keyboard input).
    /// This may differ from the "active" tab when splits are present.
    focused_terminal: Option<String>,
    /// Mouse text selection state.
    selection: SelectionState,
}

impl Default for GodlyApp {
    fn default() -> Self {
        Self {
            client: None,
            terminals: TerminalCollection::new(),
            init_error: None,
            event_receiver: Arc::new(parking_lot::Mutex::new(None)),
            window_width: 1200.0,
            window_height: 800.0,
            font_metrics: FontMetrics::default(),
            layout: LayoutNode::Leaf {
                terminal_id: String::new(),
            },
            focused_terminal: None,
            selection: SelectionState::default(),
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
    pub fn title(&self) -> String {
        if let Some(active) = self.terminals.active() {
            let label: &str = active.tab_label();
            if label != "Terminal" {
                return format!("{} — Godly Terminal (Native)", label);
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
                        self.terminals.add(session_id.clone(), rows, cols);
                        self.layout = LayoutNode::Leaf {
                            terminal_id: session_id.clone(),
                        };
                        self.focused_terminal = Some(session_id.clone());
                        return self.fetch_grid(&session_id);
                    }
                    InitResult::Recovered {
                        session_ids,
                        first_id,
                    } => {
                        for id in &session_ids {
                            self.terminals.add(id.clone(), rows, cols);
                        }
                        self.terminals.set_active(&first_id);
                        // Set layout to first session (flat, no splits on recovery).
                        self.layout = LayoutNode::Leaf {
                            terminal_id: first_id.clone(),
                        };
                        self.focused_terminal = Some(first_id.clone());
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
                // Scroll up (negative delta) increases offset, scroll down decreases.
                let lines = -(delta_y * 3.0) as isize; // 3 lines per scroll notch
                return self.scroll_active(lines);
            }

            // --- Tab management ---
            Message::TabClicked(id) => {
                self.terminals.set_active(&id);
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
                self.terminals.add(session_id.clone(), rows, cols);

                // If this terminal is already in the layout (from a split), just focus it.
                // Otherwise it's a new tab — set layout to it.
                if self.layout.find_leaf(&session_id) {
                    self.focused_terminal = Some(session_id.clone());
                } else {
                    self.terminals.set_active(&session_id);
                    self.layout = LayoutNode::Leaf {
                        terminal_id: session_id.clone(),
                    };
                    self.focused_terminal = Some(session_id.clone());
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

        let active_id = self.terminals.active_id();

        // Tab bar.
        let tab_bar = tab_bar::view_tab_bar(
            self.terminals.as_slice(),
            active_id,
            |id| Message::TabClicked(id),
            |id| Message::CloseTabRequested(id),
            Message::NewTabRequested,
        );

        // Render the layout tree — each leaf becomes a terminal canvas.
        let focused_id = self.focused_terminal.as_deref();
        let terminal_view: Element<'_, Message> =
            if self.layout.leaf_count() > 0 && self.terminals.count() > 0 {
                view_layout(&self.layout, &|terminal_id: &str| {
                    self.render_terminal_pane(terminal_id, focused_id)
                })
            } else {
                center(text("No active terminal").size(16)).into()
            };

        column![tab_bar, terminal_view].into()
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
                // NOTE: Iced's listen_with doesn't provide cursor position in
                // ButtonPressed events. We start at (0,0) and the first CursorMoved
                // event (which fires before any visible render) corrects the position.
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

    /// Handle an app-level shortcut action.
    fn handle_app_action(&mut self, action: AppAction) -> Task<Message> {
        match action {
            AppAction::NewTab => self.create_new_terminal(),
            AppAction::CloseTab => {
                if let Some(id) = self.terminals.active_id().map(str::to_string) {
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
        }
    }

    // -----------------------------------------------------------------------
    // Scrollback
    // -----------------------------------------------------------------------

    /// Scroll the active terminal by delta lines (negative = up, positive = down).
    fn scroll_active(&mut self, delta: isize) -> Task<Message> {
        let Some(active_id) = self.terminals.active_id().map(str::to_string) else {
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

    /// Scroll to the top of scrollback history.
    fn scroll_to_top(&mut self) -> Task<Message> {
        let Some(active_id) = self.terminals.active_id().map(str::to_string) else {
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

    /// Scroll to the bottom (live view).
    fn scroll_to_bottom(&mut self) -> Task<Message> {
        let Some(active_id) = self.terminals.active_id().map(str::to_string) else {
            return Task::none();
        };

        if let Some(term) = self.terminals.get_mut(&active_id) {
            term.scrollback_offset = 0;
        }

        self.scroll_fetch(&active_id, 0)
    }

    /// Set scrollback offset and fetch the grid snapshot for a session.
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

    /// Fetch the grid snapshot for a specific session.
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

    /// Create a new terminal session via the daemon.
    fn create_new_terminal(&self) -> Task<Message> {
        self.create_terminal_task(uuid::Uuid::new_v4().to_string())
    }

    /// Create a terminal session with the given ID asynchronously.
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

    /// Close a terminal session.
    fn close_terminal(&mut self, session_id: &str) -> Task<Message> {
        self.terminals.remove(session_id);

        // Close on daemon (fire-and-forget).
        if let Some(client) = &self.client {
            let _ = commands::close_terminal(client, session_id);
        }

        Task::none()
    }

    // -----------------------------------------------------------------------
    // Resize
    // -----------------------------------------------------------------------

    /// Resize the active terminal on the daemon.
    fn resize_active_terminal(&mut self, rows: u16, cols: u16) -> Task<Message> {
        let Some(active_id) = self.terminals.active_id().map(str::to_string) else {
            return Task::none();
        };

        if let Some(term) = self.terminals.get_mut(&active_id) {
            term.rows = rows;
            term.cols = cols;
        }

        if let Some(client) = &self.client {
            let _ = commands::resize_terminal(client, &active_id, rows, cols);
        }

        // Fetch updated grid after resize.
        self.fetch_grid(&active_id)
    }

    /// Resize all terminals after a font size change.
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

        // Fetch grid for active terminal.
        if let Some(active_id) = self.terminals.active_id().map(str::to_string) {
            self.fetch_grid(&active_id)
        } else {
            Task::none()
        }
    }

    // -----------------------------------------------------------------------
    // Target terminal resolution
    // -----------------------------------------------------------------------

    /// Get the target terminal ID — prefer focused pane, fall back to active tab.
    fn target_terminal_id(&self) -> Option<&str> {
        self.focused_terminal
            .as_deref()
            .or_else(|| self.terminals.active_id())
    }

    // -----------------------------------------------------------------------
    // Grid dimension calculations
    // -----------------------------------------------------------------------

    /// Calculate terminal columns from window width and font metrics.
    fn calculate_cols(&self) -> u16 {
        (self.window_width / self.font_metrics.cell_width).max(1.0) as u16
    }

    /// Calculate terminal rows from window height (minus tab bar) and font metrics.
    fn calculate_rows(&self) -> u16 {
        let available = (self.window_height - TAB_BAR_HEIGHT).max(0.0);
        (available / self.font_metrics.cell_height).max(1.0) as u16
    }

    // -----------------------------------------------------------------------
    // Split pane operations
    // -----------------------------------------------------------------------

    /// Split the focused pane, creating a new terminal in the given direction.
    fn split_focused_pane(&mut self, direction: SplitDirection) -> Task<Message> {
        let Some(focused) = self.focused_terminal.clone() else {
            return Task::none();
        };

        let new_id = uuid::Uuid::new_v4().to_string();

        // Split the layout tree immediately (optimistic).
        self.layout
            .split_leaf(&focused, new_id.clone(), direction);

        // Create the terminal on the daemon.
        self.create_terminal_task(new_id)
    }

    /// Remove the focused pane from its split, promoting the sibling.
    fn unsplit_focused_pane(&mut self) -> Task<Message> {
        let Some(focused) = self.focused_terminal.clone() else {
            return Task::none();
        };

        if let Some(removed_id) = self.layout.unsplit_leaf(&focused) {
            // Close the removed terminal on the daemon.
            self.terminals.remove(&removed_id);
            if let Some(client) = &self.client {
                let _ = commands::close_terminal(client, &removed_id);
            }

            // Update focus to the next available leaf.
            let leaf_ids = self.layout.all_leaf_ids();
            if let Some(first) = leaf_ids.first() {
                self.focused_terminal = Some(first.to_string());
            }
        }

        Task::none()
    }

    /// Cycle focus to the next pane in depth-first order.
    fn cycle_focus(&mut self) {
        let Some(focused) = &self.focused_terminal else {
            return;
        };

        if let Some(next_id) = self.layout.next_leaf_id(focused) {
            self.focused_terminal = Some(next_id.to_string());
        }
    }

    // -----------------------------------------------------------------------
    // Clipboard
    // -----------------------------------------------------------------------

    /// Copy the current selection text to the system clipboard.
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

    /// Paste text from the system clipboard into the focused terminal.
    fn paste_from_clipboard(&self) -> Task<Message> {
        let Some(tid) = self.target_terminal_id() else {
            return Task::none();
        };
        let Some(client) = &self.client else {
            return Task::none();
        };

        match clipboard::paste_from_clipboard() {
            Ok(text) if !text.is_empty() => {
                let _ = commands::write_to_terminal(client, &tid, text.as_bytes());
            }
            Ok(_) => {}
            Err(e) => {
                log::error!("Clipboard paste failed: {}", e);
            }
        }
        Task::none()
    }

    // -----------------------------------------------------------------------
    // Selection
    // -----------------------------------------------------------------------

    /// Select all text in the focused terminal's grid.
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

    /// Convert pixel coordinates to grid position.
    fn pixel_to_grid(&self, x: f32, y: f32) -> GridPos {
        // Subtract tab bar height from y.
        let adjusted_y = (y - TAB_BAR_HEIGHT).max(0.0);
        let row = (adjusted_y / self.font_metrics.cell_height) as usize;
        let col = (x / self.font_metrics.cell_width) as usize;
        GridPos { row, col }
    }

    // -----------------------------------------------------------------------
    // Rendering helpers
    // -----------------------------------------------------------------------

    /// Render a single terminal pane for the layout tree.
    fn render_terminal_pane<'a>(
        &'a self,
        terminal_id: &str,
        focused_id: Option<&str>,
    ) -> Element<'a, Message> {
        let Some(term) = self.terminals.get(terminal_id) else {
            return center(text("Session not found").size(14)).into();
        };

        // Pass selection only if this is the focused pane and a selection exists.
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

    /// Returns true if there is a non-empty selection.
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
    // Connect to daemon.
    let client = match NativeDaemonClient::connect_or_launch() {
        Ok(c) => Arc::new(c),
        Err(e) => {
            return Task::done(Message::Initialized(Err(e)));
        }
    };

    // Create the event channel.
    let (tx, rx) = mpsc::unbounded();

    // Store receiver for the subscription to pick up.
    *app.event_receiver.lock() = Some(rx);

    // Set up bridge with channel event sink.
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
                // Try to recover existing daemon sessions.
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

                // No sessions to recover — create a new one.
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

