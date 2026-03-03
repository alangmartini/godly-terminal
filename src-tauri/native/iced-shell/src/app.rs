use std::sync::Arc;

use futures_channel::mpsc;
use iced::keyboard;
use iced::widget::{canvas, center, column, text};
use iced::{event, window, Element, Length, Subscription, Task};

use godly_app_adapter::commands;
use godly_app_adapter::daemon_client::NativeDaemonClient;
use godly_app_adapter::keys::key_to_pty_bytes;
use godly_protocol::types::RichGridData;

use godly_terminal_surface::{TerminalCanvas, TerminalCanvasState};

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
    /// Canvas state for the currently active terminal.
    /// Swapped in/out when switching tabs.
    canvas_state: TerminalCanvasState,
    /// Window dimensions in logical pixels.
    window_width: f32,
    window_height: f32,
    /// Font metrics for grid dimension calculations.
    #[allow(dead_code)]
    font_size: f32,
    cell_width: f32,
    cell_height: f32,
}

impl Default for GodlyApp {
    fn default() -> Self {
        let font_size = 14.0;
        Self {
            client: None,
            terminals: TerminalCollection::new(),
            init_error: None,
            event_receiver: Arc::new(parking_lot::Mutex::new(None)),
            canvas_state: TerminalCanvasState::default(),
            window_width: 1200.0,
            window_height: 800.0,
            font_size,
            cell_width: font_size * 0.6,
            cell_height: font_size * 1.3,
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
    GridFetchFailed {
        session_id: String,
        error: String,
    },
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
    /// Terminal session exited.
    SessionExited {
        session_id: String,
        exit_code: Option<i64>,
    },
    /// Foreground process changed.
    ProcessChanged {
        session_id: String,
        process_name: String,
    },
    /// Window was resized.
    WindowResized { width: f32, height: f32 },
}

#[derive(Debug, Clone)]
pub struct InitResult {
    pub session_id: String,
}

impl GodlyApp {
    pub fn title(&self) -> String {
        if let Some(active) = self.terminals.active() {
            let label = active.tab_label();
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
            Message::Initialized(Ok(result)) => {
                let rows = self.calculate_rows();
                let cols = self.calculate_cols();
                self.terminals
                    .add(result.session_id.clone(), rows, cols);
                return self.fetch_grid(&result.session_id);
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
                // Update the terminal's stored grid.
                let is_active = self.terminals.active_id() == Some(session_id.as_str());
                if let Some(term) = self.terminals.get_mut(&session_id) {
                    term.fetching = false;
                    term.dirty = false;
                    term.title = grid.title.clone();
                    term.grid = Some(grid.clone());
                }
                // If this is the active terminal, update the canvas state.
                if is_active {
                    self.canvas_state.grid = Some(grid);
                }
            }
            Message::GridFetchFailed { session_id, error } => {
                if let Some(term) = self.terminals.get_mut(&session_id) {
                    term.fetching = false;
                }
                log::error!("Grid fetch failed for {}: {}", session_id, error);
            }

            // --- Keyboard input ---
            Message::KeyboardEvent(keyboard::Event::KeyPressed {
                key, modifiers, ..
            }) => {
                if let Some(bytes) = key_to_pty_bytes(&key, modifiers) {
                    if let Some(active_id) = self.terminals.active_id().map(|s| s.to_string()) {
                        if let Some(client) = &self.client {
                            let _ = commands::write_to_terminal(client, &active_id, &bytes);
                        }
                    }
                }
            }
            Message::KeyboardEvent(_) => {}

            // --- Tab management ---
            Message::TabClicked(id) => {
                // Save current active terminal's grid for later.
                if let Some(old_active_id) = self.terminals.active_id().map(|s| s.to_string()) {
                    if old_active_id != id {
                        // Store the current canvas grid back into the old terminal.
                        let current_grid = self.canvas_state.grid.take();
                        if let Some(old_term) = self.terminals.get_mut(&old_active_id) {
                            old_term.grid = current_grid;
                        }
                    }
                }

                self.terminals.set_active(&id);

                // Load the new active terminal's grid into the canvas.
                if let Some(active) = self.terminals.active() {
                    self.canvas_state.grid = active.grid.clone();
                }
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

                // Save current active terminal's grid.
                if let Some(old_active_id) = self.terminals.active_id().map(|s| s.to_string()) {
                    let current_grid = self.canvas_state.grid.take();
                    if let Some(old_term) = self.terminals.get_mut(&old_active_id) {
                        old_term.grid = current_grid;
                    }
                }

                self.terminals.add(session_id.clone(), rows, cols);
                // New terminal becomes active (add sets it for the first, we set_active for rest)
                self.terminals.set_active(&session_id);
                self.canvas_state.grid = None;
                return self.fetch_grid(&session_id);
            }
            Message::TerminalCreated(Err(e)) => {
                log::error!("Failed to create terminal: {}", e);
            }

            Message::SessionExited {
                session_id,
                exit_code,
            } => {
                if let Some(term) = self.terminals.get_mut(&session_id) {
                    term.exited = true;
                    term.exit_code = exit_code;
                }
            }
            Message::ProcessChanged {
                session_id,
                process_name,
            } => {
                if let Some(term) = self.terminals.get_mut(&session_id) {
                    term.process_name = process_name;
                }
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

        // Active terminal canvas.
        let terminal_view: Element<'_, Message> = if self.terminals.active().is_some() {
            canvas(TerminalCanvas)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            center(text("No active terminal").size(16)).into()
        };

        column![tab_bar, terminal_view].into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            // Keyboard events.
            keyboard::listen().map(Message::KeyboardEvent),
            // Daemon events via channel.
            daemon_events(Arc::clone(&self.event_receiver)).map(Message::DaemonEvent),
        ];

        // Window resize events.
        subs.push(
            event::listen_with(|ev, _status, _window_id| {
                if let event::Event::Window(window::Event::Resized(size)) = ev {
                    Some(Message::WindowResized {
                        width: size.width,
                        height: size.height,
                    })
                } else {
                    None
                }
            }),
        );

        Subscription::batch(subs)
    }

    /// Fetch the grid snapshot for a specific session.
    fn fetch_grid(&mut self, session_id: &str) -> Task<Message> {
        if let Some(client) = &self.client {
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
        } else {
            Task::none()
        }
    }

    /// Create a new terminal session via the daemon.
    fn create_new_terminal(&self) -> Task<Message> {
        let Some(client) = &self.client else {
            return Task::done(Message::TerminalCreated(Err(
                "No daemon connection".to_string(),
            )));
        };

        let client = Arc::clone(client);
        let session_id = uuid::Uuid::new_v4().to_string();
        let sid = session_id.clone();
        let rows = self.calculate_rows();
        let cols = self.calculate_cols();

        Task::perform(
            async move {
                let (tx, rx) = futures_channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = commands::create_terminal(
                        &client,
                        &sid,
                        godly_protocol::ShellType::Windows,
                        None,
                        rows,
                        cols,
                    )
                    .map(|_| sid);
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
        // If closing the active terminal, save its grid first.
        let is_active = self.terminals.active_id() == Some(session_id);
        if is_active {
            self.canvas_state.grid = None;
        }

        self.terminals.remove(session_id);

        // Load the new active terminal's grid, if any.
        if let Some(active) = self.terminals.active() {
            self.canvas_state.grid = active.grid.clone();
        }

        // Close on daemon (fire-and-forget).
        if let Some(client) = &self.client {
            let _ = commands::close_terminal(client, session_id);
        }

        Task::none()
    }

    /// Resize the active terminal on the daemon.
    fn resize_active_terminal(&mut self, rows: u16, cols: u16) -> Task<Message> {
        let Some(active_id) = self.terminals.active_id().map(|s| s.to_string()) else {
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

    /// Calculate terminal columns from window width and font metrics.
    fn calculate_cols(&self) -> u16 {
        let cols = (self.window_width / self.cell_width) as u16;
        cols.max(1)
    }

    /// Calculate terminal rows from window height (minus tab bar) and font metrics.
    fn calculate_rows(&self) -> u16 {
        let available_height = (self.window_height - TAB_BAR_HEIGHT).max(0.0);
        let rows = (available_height / self.cell_height) as u16;
        rows.max(1)
    }
}

/// Initialize the app: connect to daemon, set up bridge, create first session.
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

    // Create a session.
    let session_id = uuid::Uuid::new_v4().to_string();
    let sid = session_id.clone();
    let rows = app.calculate_rows();
    let cols = app.calculate_cols();

    Task::perform(
        async move {
            let (tx, rx) = futures_channel::oneshot::channel();
            std::thread::spawn(move || {
                let result = commands::create_terminal(
                    &client,
                    &sid,
                    godly_protocol::ShellType::Windows,
                    None,
                    rows,
                    cols,
                )
                .map(|_| InitResult { session_id: sid });
                let _ = tx.send(result);
            });
            rx.await
                .unwrap_or_else(|_| Err("Background thread panicked".into()))
        },
        Message::Initialized,
    )
}
