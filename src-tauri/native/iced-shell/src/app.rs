use std::collections::{HashMap, HashSet};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Lightweight diagnostic logger for freeze debugging.
/// Writes timestamped events to a file in the app data directory.
pub mod diag {
    use std::io::Write;
    use std::sync::Mutex;

    static LOG: Mutex<Option<std::fs::File>> = Mutex::new(None);
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

    /// Maximum log file size before rotation (2MB).
    const MAX_LOG_SIZE: u64 = 2 * 1024 * 1024;

    pub fn init() {
        START.get_or_init(std::time::Instant::now);
        let base = match std::env::var("APPDATA") {
            Ok(v) => std::path::PathBuf::from(v),
            Err(_) => return,
        };
        let dir = base.join("com.godly.terminal");
        let path = dir.join("iced-diag.log");
        let prev_path = dir.join("iced-diag.prev.log");

        // Rotate if the log file exceeds 2MB
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() > MAX_LOG_SIZE {
                let _ = std::fs::copy(&path, &prev_path);
                let _ = std::fs::remove_file(&path);
            }
        }

        if let Ok(f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            *LOG.lock().unwrap() = Some(f);
        }
    }

    pub fn log(msg: &str) {
        if let Ok(mut guard) = LOG.lock() {
            if let Some(f) = guard.as_mut() {
                let elapsed = START.get().map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);
                let _ = writeln!(f, "[{:>8.3}] {}", elapsed, msg);
                let _ = f.flush();
            }
        }
    }
}



use futures_channel::mpsc;
use iced::keyboard;
use iced::widget::{
    button, canvas, center, column, container, mouse_area, row, stack, text, text_input, Space,
};
use iced::{event, window, Color, Element, Font, Length, Padding, Point, Shadow, Subscription, Task, Vector};

/// Proportional UI font for chrome elements (title bar, tab bar, status bar).
/// Uses the system's default sans-serif (Segoe UI on Windows, SF Pro on macOS).
const UI_FONT: Font = Font {
    family: iced::font::Family::SansSerif,
    weight: iced::font::Weight::Normal,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

use godly_app_adapter::clipboard;
use godly_app_adapter::commands;
use godly_app_adapter::daemon_client::NativeDaemonClient;
use godly_app_adapter::keys::key_to_pty_bytes;
use godly_app_adapter::shortcuts::{self, AppAction, ShortcutResolver};
use godly_app_adapter::sound::{self, NotificationSoundPreset};
use godly_features_shell::layout as layout_reducer;
use godly_features_shell::tabs as tab_reducer;
use godly_features_shell::workspaces as workspace_reducer;
use godly_layout_core::{FocusDirection, SplitPlacement};
use godly_protocol::types::RichGridData;
use godly_protocol::McpResponse;

use godly_terminal_surface::{FontMetrics, GridPos as SurfaceGridPos, TerminalCanvas};

use crate::font_enumerator;
use crate::notification_state::NotificationTracker;
use crate::notifications;
use crate::scrollback_restore;
use crate::keybinding_persistence;
use crate::session_persistence;
use crate::selection::{GridPos, SelectionState};
use crate::settings_dialog::{self, SettingsTab};
use crate::shortcuts_tab;
use crate::sidebar::{self, SidebarAction, SIDEBAR_WIDTH};
use crate::status_bar;
use crate::split_pane::{view_layout, LayoutNode, SplitDirection};
use crate::subscription::{daemon_events, ChannelEventSink, DaemonEventMsg};
use crate::tab_bar::{self, TAB_BAR_HEIGHT};
use crate::title_bar;
use crate::terminal_state::TerminalCollection;
use crate::theme::{
    ACCENT, BACKDROP, BG_SECONDARY, BG_TERTIARY, BORDER, DANGER, EMPTY_STATE_BG, PANE_BG,
    PANE_BORDER, PANE_FOCUSED_BORDER, RADIUS_MD, SHADOW_ACCENT, SHADOW_COLOR, TEXT_ACTIVE,
    TEXT_PRIMARY, TEXT_SECONDARY,
};
use crate::url_detector;
use crate::workspace_state::WorkspaceCollection;
use crate::search::SearchState;
use crate::terminal_context_menu::{self, TermCtxAction};
use crate::shell_picker::{self, AiToolMode, ShellPickerState, ShellPickerTab};
use crate::whisper_ui;

/// Default font family name for the bundled Geist Mono font.
const DEFAULT_FONT_FAMILY: &str = "Geist Mono";

/// Bundled Geist Mono Regular font bytes for measured metrics.
/// Same file as the `fonts::REGULAR` constant in `main.rs`.
const GEIST_MONO_REGULAR: &[u8] = include_bytes!("../fonts/GeistMono-Regular.ttf");

/// Create font metrics using the bundled Geist Mono font file for accurate
/// measurements. Falls back to heuristic ratios for non-default font families.
fn measured_font_metrics(font_size: f32, font_family: &str) -> FontMetrics {
    if font_family == DEFAULT_FONT_FAMILY {
        FontMetrics::from_font_bytes(font_size, GEIST_MONO_REGULAR)
    } else {
        FontMetrics::from_font_size(font_size)
    }
}

#[path = "mru_switcher.rs"]
mod mru_switcher;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickClaudeLayout {
    Single,
    VSplit,
    HSplit,
    Grid2x2,
}

impl QuickClaudeLayout {
    fn all() -> [Self; 4] {
        [Self::Single, Self::VSplit, Self::HSplit, Self::Grid2x2]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Single => "Single",
            Self::VSplit => "VSplit",
            Self::HSplit => "HSplit",
            Self::Grid2x2 => "2x2",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuickClaudePreset {
    name: String,
    prompt_template: String,
    layout: QuickClaudeLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiToolEntry {
    display_name: String,
    command: String,
    icon_tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PluginEntry {
    name: String,
    source: String,
    enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FlowEntry {
    name: String,
    trigger: String,
    steps: Vec<String>,
    enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteAuthMode {
    Password,
    SshKey,
}

impl RemoteAuthMode {
    fn all() -> [Self; 2] {
        [Self::Password, Self::SshKey]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Password => "Password",
            Self::SshKey => "SSH Key",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteConnectionEntry {
    name: String,
    host: String,
    port: u16,
    username: String,
    auth_mode: RemoteAuthMode,
    auth_value: Option<String>,
    enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToastNotification {
    id: u64,
    title: String,
    message: String,
    expires_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MruSwitcherState {
    selected_terminal_id: String,
}

const TOAST_TTL_MS: u64 = 4_000;
const TOAST_TICK_INTERVAL_MS: u64 = 250;
const MAX_ACTIVE_TOASTS: usize = 6;
const SIDEBAR_ANIMATION_DURATION_MS: u64 = 200;
const SIDEBAR_ANIMATION_TICK_MS: u64 = 16;
const TERMINAL_VIEWPORT_INSET_X: f32 = 4.0;
const TERMINAL_VIEWPORT_INSET_Y: f32 = 2.0;
const EMPTY_STATE_CARD_WIDTH: f32 = 400.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct SidebarAnimation {
    from_width: f32,
    to_width: f32,
    started_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PaneRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl PaneRect {
    fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width: width.max(1.0),
            height: height.max(1.0),
        }
    }

    fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    fn inset(self, inset_x: f32, inset_y: f32) -> Self {
        let inset_x = inset_x.min(self.width * 0.5);
        let inset_y = inset_y.min(self.height * 0.5);

        Self::new(
            self.x + inset_x,
            self.y + inset_y,
            self.width - inset_x * 2.0,
            self.height - inset_y * 2.0,
        )
    }

    fn clamp_point(self, point: Point) -> Point {
        let max_x = (self.x + self.width - 0.001).max(self.x);
        let max_y = (self.y + self.height - 0.001).max(self.y);

        Point::new(point.x.clamp(self.x, max_x), point.y.clamp(self.y, max_y))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalEmptyState {
    NoTerminalsOpen,
}

/// Main Iced application state — multi-terminal with event-driven updates.
pub struct GodlyApp {
    /// Daemon client (shared with bridge thread).
    pub(crate) client: Option<Arc<NativeDaemonClient>>,
    /// All terminal sessions (global, with workspace_id tracking).
    pub(crate) terminals: TerminalCollection,
    /// Workspace collection — each workspace owns its layout tree and focused terminal.
    pub(crate) workspaces: WorkspaceCollection,
    /// Error message to display if initialization failed.
    init_error: Option<String>,
    /// Event receiver for the daemon subscription (taken once by the subscription).
    event_receiver: Arc<parking_lot::Mutex<Option<mpsc::UnboundedReceiver<DaemonEventMsg>>>>,
    /// Event receiver for MCP pipe events (taken once by the subscription).
    mcp_event_receiver: Arc<parking_lot::Mutex<Option<mpsc::UnboundedReceiver<godly_app_adapter::mcp_pipe::McpEvent>>>>,
    /// Window dimensions in logical pixels.
    window_width: f32,
    window_height: f32,
    /// Native window id captured from runtime events.
    pub(crate) window_id: Option<window::Id>,
    /// Whether the app window is currently focused.
    window_focused: bool,
    /// Font metrics for cell sizing and grid dimension calculations.
    font_metrics: FontMetrics,
    /// Mouse text selection state.
    selection: SelectionState,
    /// Whether the workspace sidebar is visible.
    sidebar_visible: bool,
    /// Current sidebar width in logical pixels.
    sidebar_width: f32,
    /// In-flight sidebar width animation, if any.
    sidebar_animation: Option<SidebarAnimation>,
    /// Whether the sidebar resize handle is currently being dragged.
    sidebar_resizing: bool,
    /// Tabs currently animating their entry (tab_id → started_at_ms).
    entering_tabs: std::collections::HashMap<String, u64>,
    /// Last known global cursor position in logical pixels.
    cursor_position: Option<Point>,
    /// Whether the settings dialog is open.
    pub(crate) settings_open: bool,
    /// Active tab in the settings dialog.
    pub(crate) settings_tab: String,
    /// Which shortcut badge is in capture mode (flat index), if any.
    pub(crate) shortcut_capturing_index: Option<usize>,
    /// Custom shortcut overrides (flat index → display string).
    pub(crate) shortcut_overrides: HashMap<usize, String>,
    /// Resolver that routes key events through custom overrides + defaults.
    shortcut_resolver: ShortcutResolver,
    /// Notification tracker for terminals.
    notifications: NotificationTracker,
    /// Startup timestamp used for test harness uptime reporting.
    pub(crate) started_at_ms: u64,
    /// Counter for generating workspace names.
    pub(crate) next_workspace_num: u32,
    /// Last workspace touched by the semantic test harness.
    pub(crate) last_test_workspace_id: Option<String>,
    /// Last terminal touched by the semantic test harness.
    pub(crate) last_test_terminal_id: Option<String>,
    /// Which workspace currently has its context actions opened.
    workspace_context_menu_id: Option<String>,
    /// Workspace currently being renamed.
    rename_workspace_id: Option<String>,
    /// Current value in the rename workspace input.
    rename_workspace_value: String,
    /// Tab currently showing context menu actions.
    tab_context_menu_id: Option<String>,
    /// Tab currently being renamed.
    rename_tab_id: Option<String>,
    /// Current value in the rename tab input.
    rename_tab_value: String,
    /// Whether audible notification sounds are enabled.
    pub(crate) notification_sounds_enabled: bool,
    /// Active notification sound preset.
    pub(crate) notification_sound_preset: NotificationSoundPreset,
    /// Last terminal-local sound timestamps for debounce.
    last_terminal_sound_ms: HashMap<String, u64>,
    /// Last global sound timestamp for debounce.
    last_global_sound_ms: Option<u64>,
    /// Last native attention request timestamp for debounce.
    last_attention_request_ms: Option<u64>,
    /// Workspace name/id mute patterns for notification sounds.
    pub(crate) workspace_mute_patterns: Vec<String>,
    /// Current input value for adding a workspace mute pattern.
    workspace_mute_pattern_input: String,
    /// Quick Claude preset editor input: preset name.
    quick_claude_name_input: String,
    /// Quick Claude preset editor input: prompt template.
    quick_claude_prompt_input: String,
    /// Quick Claude preset editor selected layout.
    quick_claude_layout: QuickClaudeLayout,
    /// Stored Quick Claude presets for settings/runtime usage.
    quick_claude_presets: Vec<QuickClaudePreset>,
    /// Selected preset index when editing an existing preset.
    quick_claude_edit_index: Option<usize>,
    /// AI Tools editor input: display name.
    ai_tool_name_input: String,
    /// AI Tools editor input: launch command.
    ai_tool_command_input: String,
    /// AI Tools editor input: optional icon tag.
    ai_tool_icon_input: String,
    /// Stored AI tool entries for settings/runtime usage.
    ai_tools: Vec<AiToolEntry>,
    /// Selected AI tool index when editing an existing tool.
    ai_tool_edit_index: Option<usize>,
    /// Plugins editor input: plugin name.
    plugin_name_input: String,
    /// Plugins editor input: plugin source path/url.
    plugin_source_input: String,
    /// Stored plugin entries for settings/runtime usage.
    plugins: Vec<PluginEntry>,
    /// Selected plugin index when editing an existing plugin.
    plugin_edit_index: Option<usize>,
    /// Flows editor input: flow name.
    flow_name_input: String,
    /// Flows editor input: trigger descriptor.
    flow_trigger_input: String,
    /// Flows editor input: list of steps separated by new lines, commas, or semicolons.
    flow_steps_input: String,
    /// Stored flow entries for settings/runtime usage.
    flows: Vec<FlowEntry>,
    /// Selected flow index when editing an existing flow.
    flow_edit_index: Option<usize>,
    /// Remote profile editor input: profile name.
    remote_name_input: String,
    /// Remote profile editor input: SSH host/IP.
    remote_host_input: String,
    /// Remote profile editor input: SSH port.
    remote_port_input: String,
    /// Remote profile editor input: SSH username.
    remote_username_input: String,
    /// Remote profile editor selected auth mode.
    remote_auth_mode: RemoteAuthMode,
    /// Remote profile editor input: password or key path.
    remote_auth_value_input: String,
    /// Stored remote connection profiles for settings/runtime usage.
    remote_connections: Vec<RemoteConnectionEntry>,
    /// Selected remote profile index when editing.
    remote_edit_index: Option<usize>,
    /// Active toast notifications rendered as overlay cards.
    toasts: Vec<ToastNotification>,
    /// Monotonic id source for toast notifications.
    next_toast_id: u64,
    /// Tab ID currently being dragged for reorder.
    dragging_tab_id: Option<String>,
    /// Cursor position when the tab drag started. `Some` means the drag has
    /// not yet "committed" (mouse hasn't moved beyond threshold). Once the
    /// cursor moves ≥5 px, this is cleared to `None`, and the split-zone
    /// drop overlay becomes visible.
    tab_drag_start_pos: Option<Point>,
    /// Ctrl+Tab MRU switcher popup state while modifier is held.
    mru_switcher: Option<MruSwitcherState>,
    // --- Worktree Mode ---
    worktree_close_pending: Option<String>,
    // --- K2/K3: Quit Confirmation + Copy Preview ---
    quit_confirm_pending: bool,
    copy_preview_text: Option<String>,
    // --- F1-F4: Theme System ---
    active_theme: crate::theme::ThemeId,
    // --- F5: Custom Theme Import/Export ---
    custom_themes: Vec<crate::theme::CustomTheme>,
    active_custom_theme_id: Option<String>,
    // --- H1-H6: Shell Picker & Workspace Creation ---
    shell_picker: ShellPickerState,
    workspace_ai_modes: HashMap<String, AiToolMode>,
    // --- I1-I3: Quick Claude Launcher ---
    quick_claude_launch: Option<crate::quick_claude::LaunchState>,
    // --- Quick Claude Dialog (modal) ---
    quick_claude_dialog: Option<crate::quick_claude_dialog::QuickClaudeDialogState>,
    quick_claude_prefs: crate::quick_claude_dialog::QuickClaudePreferences,
    // --- G1/G2: Terminal Context Menu ---
    terminal_context_menu_pos: Option<(f32, f32)>,
    terminal_context_menu_terminal_id: Option<String>,
    // --- G3: URL Detection ---
    hovered_url: Option<String>,
    ctrl_held: bool,
    // --- G4: Find in Terminal ---
    search: SearchState,
    // --- G6/G7: Scrollbar + Performance Overlay ---
    perf_overlay_visible: bool,
    // --- K1: CLAUDE.md Editor ---
    claude_md_editor: Option<crate::claude_md_editor::ClaudeMdEditorState>,
    // --- I4/I5: Voice/Whisper Integration ---
    whisper_available: bool,
    whisper_state: Option<whisper_ui::WhisperState>,
    whisper_service: Option<Arc<parking_lot::Mutex<Option<godly_app_adapter::whisper::WhisperService>>>>,
    /// Pending reply channel for async screenshot capture.
    pub(crate) pending_screenshot_reply: Option<std::sync::mpsc::Sender<McpResponse>>,
    // --- Font Family Selector ---
    /// Currently selected font family name.
    font_family: String,
    /// Computed Iced Font from font_family (cached to avoid re-interning).
    terminal_font: iced::Font,
    /// Available monospace fonts from system (lazy-loaded on Appearance tab open).
    available_fonts: Option<Vec<String>>,
    /// Search/filter query for font list.
    font_filter_query: String,
    // --- Pixel Renderer (glyph atlas pipeline) ---
    /// Glyph cache for the pixel rendering pipeline.
    glyph_cache: godly_terminal_surface::glyph_cache::GlyphCache,
    /// Primary font rasterizer (swash-based).
    glyph_rasterizer: godly_terminal_surface::swash_rasterizer::SwashRasterizer,
    /// Reusable pixel renderer (keeps buffer between frames to avoid reallocation).
    pixel_renderer: godly_terminal_surface::pixel_renderer::PixelRenderer,
    /// Whether to use the new image-based renderer instead of the canvas.
    use_pixel_renderer: bool,
}

impl Default for GodlyApp {
    fn default() -> Self {
        Self {
            client: None,
            terminals: TerminalCollection::new(),
            workspaces: WorkspaceCollection::new(),
            init_error: None,
            event_receiver: Arc::new(parking_lot::Mutex::new(None)),
            mcp_event_receiver: Arc::new(parking_lot::Mutex::new(None)),
            window_width: 1200.0,
            window_height: 800.0,
            window_id: None,
            window_focused: true,
            font_metrics: measured_font_metrics(14.0, DEFAULT_FONT_FAMILY),
            selection: SelectionState::default(),
            sidebar_visible: true,
            sidebar_width: SIDEBAR_WIDTH,
            sidebar_animation: None,
            sidebar_resizing: false,
            entering_tabs: std::collections::HashMap::new(),
            cursor_position: None,
            settings_open: false,
            settings_tab: "shortcuts".to_string(),
            shortcut_capturing_index: None,
            shortcut_overrides: keybinding_persistence::load_keybindings(),
            shortcut_resolver: ShortcutResolver::empty(), // rebuilt below
            notifications: NotificationTracker::new(),
            started_at_ms: Self::now_ms(),
            next_workspace_num: 2, // First workspace is "Workspace 1"
            last_test_workspace_id: None,
            last_test_terminal_id: None,
            workspace_context_menu_id: None,
            rename_workspace_id: None,
            rename_workspace_value: String::new(),
            tab_context_menu_id: None,
            rename_tab_id: None,
            rename_tab_value: String::new(),
            notification_sounds_enabled: true,
            notification_sound_preset: NotificationSoundPreset::Peon,
            last_terminal_sound_ms: HashMap::new(),
            last_global_sound_ms: None,
            last_attention_request_ms: None,
            workspace_mute_patterns: Vec::new(),
            workspace_mute_pattern_input: String::new(),
            quick_claude_name_input: String::new(),
            quick_claude_prompt_input: String::new(),
            quick_claude_layout: QuickClaudeLayout::Single,
            quick_claude_presets: Vec::new(),
            quick_claude_edit_index: None,
            ai_tool_name_input: String::new(),
            ai_tool_command_input: String::new(),
            ai_tool_icon_input: String::new(),
            ai_tools: Vec::new(),
            ai_tool_edit_index: None,
            plugin_name_input: String::new(),
            plugin_source_input: String::new(),
            plugins: Vec::new(),
            plugin_edit_index: None,
            flow_name_input: String::new(),
            flow_trigger_input: String::new(),
            flow_steps_input: String::new(),
            flows: Vec::new(),
            flow_edit_index: None,
            remote_name_input: String::new(),
            remote_host_input: String::new(),
            remote_port_input: "22".to_string(),
            remote_username_input: String::new(),
            remote_auth_mode: RemoteAuthMode::Password,
            remote_auth_value_input: String::new(),
            remote_connections: Vec::new(),
            remote_edit_index: None,
            toasts: Vec::new(),
            next_toast_id: 1,
            dragging_tab_id: None,
            tab_drag_start_pos: None,
            mru_switcher: None,
            worktree_close_pending: None,
            quit_confirm_pending: false,
            copy_preview_text: None,
            active_theme: crate::theme::ThemeId::Dusk,
            custom_themes: crate::theme::load_custom_themes(&crate::theme::custom_themes_dir()),
            active_custom_theme_id: None,
            shell_picker: ShellPickerState::default(),
            workspace_ai_modes: HashMap::new(),
            quick_claude_launch: None,
            quick_claude_dialog: None,
            quick_claude_prefs: crate::quick_claude_dialog::QuickClaudePreferences::default(),
            terminal_context_menu_pos: None,
            terminal_context_menu_terminal_id: None,
            hovered_url: None,
            ctrl_held: false,
            search: SearchState::default(),
            perf_overlay_visible: false,
            claude_md_editor: None,
            whisper_available: godly_app_adapter::whisper::whisper_binary_path().is_some(),
            whisper_state: None,
            whisper_service: None,
            pending_screenshot_reply: None,
            font_family: DEFAULT_FONT_FAMILY.to_string(),
            terminal_font: iced::Font {
                family: iced::font::Family::Name("Geist Mono"),
                weight: iced::font::Weight::Normal,
                stretch: iced::font::Stretch::Normal,
                style: iced::font::Style::Normal,
            },
            available_fonts: None,
            font_filter_query: String::new(),
            glyph_cache: godly_terminal_surface::glyph_cache::GlyphCache::new(),
            glyph_rasterizer: {
                use godly_terminal_surface::glyph_rasterizer::GlyphRasterizer as _;
                let mut r = godly_terminal_surface::swash_rasterizer::SwashRasterizer::new();
                r.load_font(
                    include_bytes!("../fonts/GeistMono-Regular.ttf"),
                    0,
                );
                r
            },
            pixel_renderer: godly_terminal_surface::pixel_renderer::PixelRenderer::new(),
            use_pixel_renderer: false,
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
    /// User opened/closes tab context menu via right click.
    TabContextToggle(String),
    /// Tab context menu action: begin rename.
    TabContextRename(String),
    /// Tab context menu action: split tab in direction.
    TabContextSplit {
        terminal_id: String,
        direction: SplitDirection,
    },
    /// Tab context menu action: copy tab info to clipboard.
    TabContextCopyInfo(String),
    /// Tab context menu action: close tab.
    TabContextClose(String),
    /// User started dragging a tab.
    TabDragStart(String),
    /// User hovered another tab while dragging.
    TabDragHover(String),
    /// User hovered a split drop zone while dragging.
    TabSplitZoneHover {
        target_terminal_id: String,
        placement: SplitPlacement,
    },
    /// User finished dragging a tab.
    TabDragEnd,
    /// User wants a new terminal.
    NewTabRequested,
    /// User wants to close a terminal.
    CloseTabRequested(String),
    /// New terminal created by daemon.
    TerminalCreated(Result<String, String>),
    /// Window opened and delivered a runtime window id.
    WindowOpened(window::Id),
    /// Window was resized.
    WindowResized {
        window_id: window::Id,
        width: f32,
        height: f32,
    },
    /// App window focus changed.
    WindowFocusChanged {
        window_id: window::Id,
        focused: bool,
    },
    /// Start dragging the window from the custom title bar.
    TitleBarDragStart,
    /// Minimize the window via the title bar button.
    TitleBarMinimize,
    /// Toggle maximize/restore via the title bar button.
    TitleBarToggleMaximize,
    /// Close the window via the title bar button.
    TitleBarClose,
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
    /// Move focus to the pane in a specific direction.
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    /// User clicked on a specific pane in the split layout.
    PaneClicked(String),
    /// User clicked a workspace in the sidebar.
    WorkspaceClicked(String),
    /// User requested a new workspace.
    NewWorkspaceRequested,
    /// Sidebar-level action (click / right-click / context command).
    SidebarAction(SidebarAction),
    /// Toggle the sidebar visibility.
    ToggleSidebar,
    /// Begin dragging the sidebar resize handle.
    SidebarResizeStart,
    /// Finish dragging the sidebar resize handle.
    SidebarResizeEnd,
    /// Periodic tick used for sidebar collapse/expand animation.
    SidebarAnimationTick,
    /// Periodic tick used for tab entry width animation.
    TabEntryAnimationTick,
    /// Rename dialog input changed.
    WorkspaceRenameInputChanged(String),
    /// Rename dialog submitted.
    WorkspaceRenameSubmitted,
    /// Rename dialog canceled.
    WorkspaceRenameCancelled,
    /// Tab rename dialog input changed.
    TabRenameInputChanged(String),
    /// Tab rename dialog submitted.
    TabRenameSubmitted,
    /// Tab rename dialog canceled.
    TabRenameCancelled,
    /// Toggle notification sounds enabled/disabled.
    NotificationSoundsToggled,
    /// Select notification sound preset.
    NotificationSoundPresetSelected(NotificationSoundPreset),
    /// Play test notification sound.
    NotificationSoundTest,
    /// Input changed for workspace mute pattern editor.
    WorkspaceMutePatternInputChanged(String),
    /// Add a workspace mute pattern from the current input.
    AddWorkspaceMutePattern,
    /// Remove a workspace mute pattern.
    RemoveWorkspaceMutePattern(String),
    /// Quick Claude preset name input changed.
    QuickClaudeNameInputChanged(String),
    /// Quick Claude prompt template input changed.
    QuickClaudePromptInputChanged(String),
    /// Quick Claude layout selected.
    QuickClaudeLayoutSelected(QuickClaudeLayout),
    /// Save current Quick Claude preset editor values.
    QuickClaudeSavePreset,
    /// Load an existing Quick Claude preset into the editor.
    QuickClaudeEditPreset(usize),
    /// Remove an existing Quick Claude preset.
    QuickClaudeDeletePreset(usize),
    /// Clear Quick Claude preset editor state.
    QuickClaudeClearEditor,
    /// Launch a Quick Claude preset by index.
    QuickClaudeLaunchPreset(usize),
    /// A launch step completed (Ok) or failed (Err).
    QuickClaudeLaunchStepComplete(Result<crate::quick_claude::StepResult, String>),
    /// Cancel a running Quick Claude launch.
    QuickClaudeLaunchCancel,
    // --- Quick Claude Dialog (modal) ---
    /// Open the Quick Claude dialog.
    QuickClaudeDialogOpen,
    /// Close the Quick Claude dialog.
    QuickClaudeDialogClose,
    /// Workspace selected in Quick Claude dialog.
    QuickClaudeDialogWorkspaceSelected(String),
    /// Toggle workspace dropdown in Quick Claude dialog.
    QuickClaudeDialogWorkspaceDropdownToggle,
    /// AI tool selected in Quick Claude dialog.
    QuickClaudeDialogAiToolSelected(String),
    /// Toggle AI tool dropdown in Quick Claude dialog.
    QuickClaudeDialogAiToolDropdownToggle,
    /// Prompt text editor action in Quick Claude dialog.
    QuickClaudeDialogPromptAction(iced::widget::text_editor::Action),
    /// Branch name changed in Quick Claude dialog.
    QuickClaudeDialogBranchChanged(String),
    /// Main branch mode toggled in Quick Claude dialog.
    QuickClaudeDialogMainBranchToggled(bool),
    /// Auto-suggest branch name toggled in Quick Claude dialog.
    QuickClaudeDialogAutoSuggestToggled(bool),
    /// Launch from Quick Claude dialog.
    QuickClaudeDialogLaunch,
    /// Voice input from Quick Claude dialog.
    QuickClaudeDialogVoice,
    /// Skills loaded for Quick Claude dialog autocomplete.
    QuickClaudeDialogSkillsLoaded(Vec<crate::quick_claude_dialog::SkillEntry>),
    /// Skill selected from autocomplete popup.
    QuickClaudeDialogSkillSelected(usize),
    /// Navigate autocomplete popup (delta: -1 up, +1 down).
    QuickClaudeDialogSkillAutocompleteNavigate(i32),
    /// Dismiss skill autocomplete popup.
    QuickClaudeDialogSkillAutocompleteDismiss,
    /// Tab selected in Quick Claude dialog.
    QuickClaudeDialogTabSelected(crate::quick_claude_dialog::QuickClaudeTab),
    /// Session selected in Quick Claude dialog resume tab.
    QuickClaudeDialogSessionSelected(usize),
    /// Sessions loaded for Quick Claude dialog resume tab.
    QuickClaudeDialogSessionsLoaded(Vec<crate::quick_claude_dialog::ClaudeSession>),
    /// Resume a Claude session from Quick Claude dialog.
    QuickClaudeDialogResume,
    /// Model selected in Quick Claude dialog.
    QuickClaudeDialogModelSelected(String),
    /// Toggle model dropdown in Quick Claude dialog.
    QuickClaudeDialogModelDropdownToggle,
    /// Permission mode selected in Quick Claude dialog.
    QuickClaudeDialogModeSelected(String),
    /// Toggle mode dropdown in Quick Claude dialog.
    QuickClaudeDialogModeDropdownToggle,
    /// Models discovered from claude CLI.
    QuickClaudeDialogModelsLoaded(Vec<(String, String)>),
    /// AI tool display name input changed.
    AiToolNameInputChanged(String),
    /// AI tool command input changed.
    AiToolCommandInputChanged(String),
    /// AI tool icon input changed.
    AiToolIconInputChanged(String),
    /// Save current AI tool editor values.
    AiToolSave,
    /// Load an existing AI tool into editor.
    AiToolEdit(usize),
    /// Remove an existing AI tool.
    AiToolDelete(usize),
    /// Clear AI tool editor state.
    AiToolClearEditor,
    /// Plugin name input changed.
    PluginNameInputChanged(String),
    /// Plugin source input changed.
    PluginSourceInputChanged(String),
    /// Save current plugin editor values.
    PluginSave,
    /// Load an existing plugin entry into editor.
    PluginEdit(usize),
    /// Remove an existing plugin entry.
    PluginDelete(usize),
    /// Toggle enabled state for a plugin entry.
    PluginToggleEnabled(usize),
    /// Clear plugin editor state.
    PluginClearEditor,
    /// Flow name input changed.
    FlowNameInputChanged(String),
    /// Flow trigger input changed.
    FlowTriggerInputChanged(String),
    /// Flow steps input changed.
    FlowStepsInputChanged(String),
    /// Save current flow editor values.
    FlowSave,
    /// Load an existing flow entry into editor.
    FlowEdit(usize),
    /// Remove an existing flow entry.
    FlowDelete(usize),
    /// Toggle enabled state for a flow entry.
    FlowToggleEnabled(usize),
    /// Clear flow editor state.
    FlowClearEditor,
    /// Remote profile name input changed.
    RemoteNameInputChanged(String),
    /// Remote host input changed.
    RemoteHostInputChanged(String),
    /// Remote port input changed.
    RemotePortInputChanged(String),
    /// Remote username input changed.
    RemoteUsernameInputChanged(String),
    /// Remote auth mode selected.
    RemoteAuthModeSelected(RemoteAuthMode),
    /// Remote auth value input changed.
    RemoteAuthValueInputChanged(String),
    /// Save current remote profile values.
    RemoteSave,
    /// Load an existing remote profile into editor.
    RemoteEdit(usize),
    /// Remove an existing remote profile.
    RemoteDelete(usize),
    /// Toggle enabled state for a remote profile.
    RemoteToggleEnabled(usize),
    /// Clear remote editor state.
    RemoteClearEditor,
    /// Periodic tick used for toast auto-dismiss.
    ToastTick,
    /// Heartbeat tick — keeps the event loop alive when window is unfocused/minimized
    /// so that the Focused event is reliably delivered on restore.
    Heartbeat,
    /// Toggle the settings dialog.
    ToggleSettings,
    /// User clicked a settings tab.
    SettingsTabClicked(String),
    /// Font family changed from settings.
    FontFamilyChanged(String),
    /// System fonts enumerated in background.
    FontsEnumerated(Vec<String>),
    /// Font filter search query changed.
    FontFilterChanged(String),
    /// Increment font size from settings stepper.
    FontSizeIncrement,
    /// Decrement font size from settings stepper.
    FontSizeDecrement,
    /// User clicked a shortcut badge to enter capture mode.
    ShortcutBadgeClicked(usize),
    /// User cancelled shortcut capture (e.g. pressed Escape).
    ShortcutCaptureCancelled,
    /// Clipboard text read successfully in background — write to terminal.
    ClipboardPasted { terminal_id: String, text: String },
    /// Clipboard read failed in background.
    ClipboardPasteFailed(String),
    // --- G3: URL Detection ---
    /// URL clicked (Ctrl+Click) — open in default browser.
    UrlClicked(String),
    /// File path dropped on window.
    FileDropped(PathBuf),
    // --- K2/K3: Quit Confirmation + Copy Preview ---
    QuitConfirmShow,
    QuitConfirmed,
    QuitCancelled,
    CopyPreviewShow(String),
    CopyPreviewConfirmed,
    CopyPreviewDismissed,
    // --- Worktree Mode ---
    /// A worktree was successfully created for a new terminal.
    WorktreeCreated {
        session_id: String,
        worktree_path: String,
    },
    /// User confirmed worktree cleanup on terminal close.
    WorktreeCloseConfirmed { session_id: String },
    /// User chose to keep worktree on terminal close.
    WorktreeCloseKeep { session_id: String },
    /// Background worktree removal completed.
    WorktreeRemoved {
        session_id: String,
        result: Result<(), String>,
    },
    /// Dismiss the worktree close dialog without closing.
    WorktreeCloseCancelled,
    // --- F1-F4: Theme System ---
    ThemeChanged(crate::theme::ThemeId),
    // --- F5: Custom Theme Import/Export ---
    ThemeImportRequested,
    ThemeImported(Result<crate::theme::CustomTheme, String>),
    ThemeExportRequested(String),
    ThemeExported(Result<String, String>),
    ThemeDeleteCustom(String),
    ThemeSelectCustom(String),
    // --- Folder Picker for New Workspace ---
    FolderPickerResult(Option<std::path::PathBuf>),
    // --- H1-H6: Shell Picker & Workspace Creation ---
    ShellPickerOpen,
    ShellPickerTabClicked(ShellPickerTab),
    ShellPickerDistroSelected(Option<String>),
    ShellPickerCustomProgramChanged(String),
    ShellPickerCustomArgsChanged(String),
    ShellPickerConfirmed,
    ShellPickerCancelled,
    WorkspaceAiModeChanged {
        workspace_id: String,
        mode: AiToolMode,
    },
    // --- G1/G2: Terminal Context Menu ---
    TerminalContextOpen { id: String, x: f32, y: f32 },
    TerminalContextClose,
    TerminalContextAction(TermCtxAction),
    // --- G4: Find in Terminal ---
    SearchOpen,
    SearchClose,
    SearchQueryChanged(String),
    SearchNext,
    SearchPrev,
    SearchToggleRegex,
    // --- G6/G7: Scrollbar + Performance Overlay ---
    TogglePerfOverlay,
    // --- K1: CLAUDE.md Editor ---
    ClaudeMdOpen { path: std::path::PathBuf },
    ClaudeMdLoaded { content: String, path: std::path::PathBuf },
    ClaudeMdLoadFailed(String),
    ClaudeMdEditorAction(iced::widget::text_editor::Action),
    ClaudeMdSave,
    ClaudeMdSaved(Result<(), String>),
    ClaudeMdClose,
    // --- J1-J9: MCP Event Integration ---
    McpEvent(godly_app_adapter::mcp_pipe::McpEvent),
    // --- I4/I5: Voice/Whisper Integration ---
    WhisperToggle,
    WhisperStarted,
    WhisperStopped(Result<(String, u64), String>),
    WhisperLevelUpdate(f32),
    WhisperTimerTick,
    WhisperCancel,
    /// Screenshot captured via iced window API.
    ScreenshotCaptured(iced::window::Screenshot),
}

/// Result of initialization — either a fresh terminal or recovered sessions.
#[derive(Debug, Clone)]
pub enum InitResult {
    /// A brand new session was created.
    Fresh { session_id: String },
    /// Existing daemon sessions were recovered (app restart / reconnect).
    Recovered {
        session_ids: Vec<String>,
        restored_scrollback_offsets: HashMap<String, usize>,
        merged_session_state: Option<session_persistence::MergedSessionState>,
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
        self.workspaces
            .active()
            .map(|ws| ws.focused_terminal.as_str())
    }

    /// Get the target terminal ID — prefer workspace's focused pane, fall back to active tab.
    fn target_terminal_id(&self) -> Option<&str> {
        self.active_focused().or_else(|| self.terminals.active_id())
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

    fn workspace_has_notifications(
        &self,
        workspace: &crate::workspace_state::WorkspaceInfo,
    ) -> bool {
        workspace.layout.all_leaf_ids().iter().any(|terminal_id| {
            self.notifications.unread_count(terminal_id) > 0
                || self.notifications.has_bell(terminal_id)
        })
    }

    fn notified_workspace_ids(&self) -> Vec<String> {
        self.workspaces
            .as_slice()
            .iter()
            .filter(|workspace| self.workspace_has_notifications(workspace))
            .map(|workspace| workspace.id.clone())
            .collect()
    }

    fn workspace_muted_for_terminal(&self, terminal_id: &str) -> bool {
        let Some(terminal) = self.terminals.get(terminal_id) else {
            return false;
        };
        let Some(workspace_id) = terminal.workspace_id.as_deref() else {
            return false;
        };
        let Some(workspace) = self.workspaces.get(workspace_id) else {
            return false;
        };
        workspace_matches_mute_patterns(
            &self.workspace_mute_patterns,
            workspace_id,
            &workspace.name,
        )
    }

    pub(crate) fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    fn current_sidebar_width(&self) -> f32 {
        resolved_sidebar_width(
            self.sidebar_visible,
            self.sidebar_width,
            self.sidebar_animation,
            Self::now_ms(),
        )
    }

    fn terminal_content_area(&self) -> PaneRect {
        terminal_content_rect(
            self.window_width,
            self.window_height,
            self.current_sidebar_width(),
        )
    }

    fn terminal_pane_rect(&self, terminal_id: &str) -> Option<PaneRect> {
        let content_rect = self.terminal_content_area();

        self.workspaces.iter().find_map(|workspace| {
            pane_rect_for_terminal(&workspace.layout, terminal_id, content_rect)
        })
    }

    fn terminal_viewport_rect(&self, terminal_id: &str) -> PaneRect {
        self.terminal_pane_rect(terminal_id)
            .map(inset_terminal_pane_rect)
            .unwrap_or_else(|| inset_terminal_pane_rect(self.terminal_content_area()))
    }

    fn terminal_grid_size(&self, terminal_id: Option<&str>) -> (u16, u16) {
        let viewport = terminal_id
            .map(|id| self.terminal_viewport_rect(id))
            .unwrap_or_else(|| inset_terminal_pane_rect(self.terminal_content_area()));

        grid_dimensions_for_viewport(viewport, self.font_metrics)
    }

    /// G3: Detect if the cursor is currently over a URL in the terminal grid.
    fn detect_url_under_cursor(&self) -> Option<String> {
        let grid_pos = self.active_terminal_pointer_grid(false)?;
        let terminal_id = self.target_terminal_id()?;
        let term = self.terminals.get(terminal_id)?;
        let grid = term.grid.as_ref()?;
        let row_idx = grid_pos.row as usize;
        let row = grid.rows.get(row_idx)?;
        let line: String = row
            .cells
            .iter()
            .map(|c| {
                if c.content.is_empty() {
                    " "
                } else {
                    c.content.as_str()
                }
            })
            .collect();
        url_detector::url_at_col(&line, grid_pos.col as usize)
    }

    /// Find which terminal pane the cursor is currently over.
    fn find_terminal_at_cursor(&self) -> Option<String> {
        let cursor = self.cursor_position?;
        let layout = self.active_layout()?;
        for terminal_id in layout.all_leaf_ids() {
            if let Some(rect) = self.terminal_pane_rect(terminal_id) {
                if rect.contains(cursor) {
                    return Some(terminal_id.to_string());
                }
            }
        }
        None
    }

    fn active_terminal_pointer_grid(&self, clamp_to_viewport: bool) -> Option<GridPos> {
        let cursor = self.cursor_position?;
        let terminal_id = self.target_terminal_id()?;
        let pane_rect = self.terminal_pane_rect(terminal_id)?;

        if !clamp_to_viewport && !pane_rect.contains(cursor) {
            return None;
        }

        let viewport_rect = inset_terminal_pane_rect(pane_rect);
        Some(pointer_to_grid(cursor, viewport_rect, self.font_metrics))
    }

    fn terminal_empty_state(
        &self,
        active_workspace_terminal_count: usize,
    ) -> Option<TerminalEmptyState> {
        resolve_terminal_empty_state(self.active_layout(), active_workspace_terminal_count)
    }

    fn set_sidebar_visible(&mut self, visible: bool) -> Task<Message> {
        self.sidebar_resizing = false;

        let now_ms = Self::now_ms();
        let current_width = resolved_sidebar_width(
            self.sidebar_visible,
            self.sidebar_width,
            self.sidebar_animation,
            now_ms,
        );
        let target_width = if visible { self.sidebar_width } else { 0.0 };

        self.sidebar_visible = visible;
        self.sidebar_animation = begin_sidebar_animation(current_width, target_width, now_ms);

        if self.sidebar_animation.is_none() {
            return self.resize_all_terminals();
        }

        Task::none()
    }

    fn toggle_sidebar_visibility(&mut self) -> Task<Message> {
        self.set_sidebar_visible(!self.sidebar_visible)
    }

    pub(crate) fn enqueue_toast(&mut self, title: String, message: String) {
        let now_ms = Self::now_ms();
        enqueue_toast_entry(
            self.toasts.as_mut(),
            &mut self.next_toast_id,
            title,
            message,
            now_ms,
        );
    }

    fn enqueue_bell_toast(&mut self, terminal_id: &str) {
        let title = "Terminal Bell".to_string();
        let message = if let Some(term) = self.terminals.get(terminal_id) {
            let workspace = term
                .workspace_id
                .as_deref()
                .and_then(|workspace_id| self.workspaces.get(workspace_id))
                .map(|workspace| workspace.name.as_str())
                .unwrap_or("Unknown workspace");
            format!("{} in {}", term.tab_label(), workspace)
        } else {
            format!("Bell event from {}", terminal_id)
        };
        self.enqueue_toast(title, message);
    }

    pub(crate) fn play_notification_sound_if_allowed(&mut self, terminal_id: &str) {
        if !self.notification_sounds_enabled
            || self.notification_sound_preset == NotificationSoundPreset::None
        {
            return;
        }
        if self.workspace_muted_for_terminal(terminal_id) {
            return;
        }

        let now_ms = Self::now_ms();
        let last_terminal_sound_ms = self.last_terminal_sound_ms.get(terminal_id).copied();
        let decision = notifications::decide_sound_playback(
            now_ms,
            last_terminal_sound_ms,
            self.last_global_sound_ms,
        );
        if !decision.should_play_sound {
            return;
        }

        if let Err(e) = sound::play_notification_sound_async(self.notification_sound_preset) {
            log::warn!(
                "Failed to launch notification sound preset '{}': {}",
                self.notification_sound_preset.label(),
                e
            );
            return;
        }

        self.last_terminal_sound_ms
            .insert(terminal_id.to_string(), now_ms);
        self.last_global_sound_ms = Some(now_ms);
    }

    fn request_window_attention_if_allowed(&mut self) -> Task<Message> {
        let now_ms = Self::now_ms();
        let decision = notifications::decide_window_attention_request(
            now_ms,
            self.window_focused,
            self.last_attention_request_ms,
        );
        if !decision.should_request_attention {
            return Task::none();
        }

        let Some(window_id) = self.window_id else {
            return Task::none();
        };

        self.last_attention_request_ms = Some(now_ms);
        let attention = if notifications::bell_attention_is_critical(cfg!(target_os = "windows")) {
            window::UserAttention::Critical
        } else {
            window::UserAttention::Informational
        };
        window::request_user_attention(window_id, Some(attention))
    }

    fn persist_scrollback_offsets(&self) {
        let offsets: HashMap<String, usize> = self
            .terminals
            .iter()
            .filter(|terminal| terminal.scrollback_offset > 0)
            .map(|terminal| (terminal.id.clone(), terminal.scrollback_offset))
            .collect();
        if let Err(error) = scrollback_restore::save_offsets(&offsets) {
            log::warn!("Failed to persist scrollback offsets: {}", error);
        }
    }

    fn clear_runtime_state(&mut self) {
        self.terminals = TerminalCollection::new();
        self.workspaces = WorkspaceCollection::new();
        self.init_error = None;
        self.selection = SelectionState::default();
        self.sidebar_visible = true;
        self.sidebar_width = SIDEBAR_WIDTH;
        self.sidebar_animation = None;
        self.sidebar_resizing = false;
        self.entering_tabs.clear();
        self.cursor_position = None;
        self.settings_open = false;
        self.settings_tab = "shortcuts".to_string();
        self.shortcut_capturing_index = None;
        self.notifications = NotificationTracker::new();
        self.next_workspace_num = 2;
        self.last_test_workspace_id = None;
        self.last_test_terminal_id = None;
        self.workspace_context_menu_id = None;
        self.rename_workspace_id = None;
        self.rename_workspace_value.clear();
        self.tab_context_menu_id = None;
        self.rename_tab_id = None;
        self.rename_tab_value.clear();
        self.last_terminal_sound_ms.clear();
        self.last_global_sound_ms = None;
        self.last_attention_request_ms = None;
        self.toasts.clear();
        self.dragging_tab_id = None;
        self.tab_drag_start_pos = None;
        self.mru_switcher = None;
        self.worktree_close_pending = None;
        self.quit_confirm_pending = false;
        self.copy_preview_text = None;
        self.terminal_context_menu_pos = None;
        self.terminal_context_menu_terminal_id = None;
        self.hovered_url = None;
        self.ctrl_held = false;
        self.search = SearchState::default();
    }

    fn build_persisted_session_state(&self) -> session_persistence::PersistedSessionState {
        session_persistence::PersistedSessionState {
            version: session_persistence::PERSISTENCE_VERSION,
            sidebar_visible: self.sidebar_visible,
            settings_open: self.settings_open,
            settings_tab: self.settings_tab.clone(),
            font_size: self.font_metrics.font_size,
            font_family: self.font_family.clone(),
            next_workspace_num: self.next_workspace_num,
            active_workspace_id: self.workspaces.active_id().map(str::to_string),
            active_terminal_id: self.terminals.active_id().map(str::to_string),
            workspaces: self
                .workspaces
                .iter()
                .map(|workspace| session_persistence::PersistedWorkspaceState {
                    id: workspace.id.clone(),
                    name: workspace.name.clone(),
                    folder_path: workspace.folder_path.clone(),
                    worktree_mode: workspace.worktree_mode,
                    focused_terminal: workspace.focused_terminal.clone(),
                    layout: session_persistence::PersistedLayoutNode::from_layout(&workspace.layout),
                })
                .collect(),
            terminal_worktree_paths: self
                .terminals
                .iter()
                .filter_map(|t| {
                    t.worktree_path
                        .as_ref()
                        .map(|path| (t.id.clone(), path.clone()))
                })
                .collect(),
        }
    }

    pub(crate) fn save_layout_for_testing(&self) -> Result<(), String> {
        self.persist_scrollback_offsets();
        session_persistence::save_to_default_path(&self.build_persisted_session_state())
    }

    fn persist_before_close(&self) {
        if let Err(error) = self.save_layout_for_testing() {
            log::warn!("Failed to persist native session state: {}", error);
        }
    }

    pub fn title(&self) -> String {
        let workspace_name = self
            .workspaces
            .active()
            .map(|ws| ws.name.as_str())
            .unwrap_or("Default");

        if let Some(tid) = self.active_focused() {
            if let Some(term) = self.terminals.get(tid) {
                let label = term.tab_label();
                if label != "Terminal" {
                    return format!(
                        "Godly Terminal \u{2014} {} \u{2014} {}",
                        workspace_name, label
                    );
                }
            }
        }
        format!("Godly Terminal \u{2014} {}", workspace_name)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        // Diagnostic: log key messages for freeze debugging
        match &message {
            Message::DaemonEvent(DaemonEventMsg::TerminalOutput { session_id }) => {
                diag::log(&format!("UPDATE: TerminalOutput({})", &session_id[..8.min(session_id.len())]));
            }
            Message::GridFetched { session_id, .. } => {
                diag::log(&format!("UPDATE: GridFetched({})", &session_id[..8.min(session_id.len())]));
            }
            Message::GridFetchFailed { session_id, ref error } => {
                diag::log(&format!("UPDATE: GridFetchFailed({}) err={}", &session_id[..8.min(session_id.len())], error));
            }
            Message::WindowFocusChanged { focused, .. } => {
                diag::log(&format!("UPDATE: WindowFocusChanged(focused={})", focused));
            }
            Message::Heartbeat => {
                diag::log("UPDATE: Heartbeat(RedrawRequested)");
            }
            Message::KeyboardEvent(_) => {
                diag::log("UPDATE: KeyboardEvent");
            }
            _ => {}
        }

        match message {
            // --- Initialization ---
            Message::Initialized(Ok(result)) => {
                return self.apply_init_result(result);
            }
            Message::Initialized(Err(e)) => {
                log::error!("Initialization failed: {}", e);
                self.init_error = Some(e);
            }

            // --- Daemon events (channel-driven, no polling) ---
            Message::DaemonEvent(DaemonEventMsg::TerminalOutput { session_id }) => {
                if let Some(term) = self.terminals.get_mut(&session_id) {
                    // Already dirty + fetching → this event is redundant, skip all work.
                    // This coalesces bursts of output events into a single grid fetch.
                    if term.dirty && term.fetching {
                        return Task::none();
                    }
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
                log::info!("Session {} closed (exit_code={:?})", session_id, exit_code);
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
                let is_focused = self.active_focused() == Some(session_id.as_str());
                if !is_focused {
                    self.enqueue_bell_toast(&session_id);
                }
                self.play_notification_sound_if_allowed(&session_id);
                log::debug!("Bell from session {}", session_id);
                return self.request_window_attention_if_allowed();
            }
            Message::DaemonEvent(DaemonEventMsg::Heartbeat) => {
                // Heartbeat from background thread — keeps event loop alive.
                // Also detect focus via Win32 API since Iced events are unreliable.
                #[cfg(windows)]
                {
                    extern "system" {
                        fn GetForegroundWindow() -> *mut std::ffi::c_void;
                        fn GetWindowThreadProcessId(hwnd: *mut std::ffi::c_void, pid: *mut u32) -> u32;
                        fn GetCurrentProcessId() -> u32;
                    }
                    let is_actually_focused = unsafe {
                        let fg = GetForegroundWindow();
                        if fg.is_null() {
                            false
                        } else {
                            let mut pid: u32 = 0;
                            GetWindowThreadProcessId(fg, &mut pid);
                            pid == GetCurrentProcessId()
                        }
                    };
                    if is_actually_focused != self.window_focused {
                        diag::log(&format!(
                            "HEARTBEAT: focus correction — OS={} iced={}",
                            is_actually_focused, self.window_focused
                        ));
                        self.window_focused = is_actually_focused;
                        if is_actually_focused {
                            self.last_attention_request_ms = None;
                        }
                    }
                }
            }

            // --- Grid fetch results ---
            Message::GridFetched { session_id, grid } => {
                let mut should_persist_clamp = false;
                if let Some(term) = self.terminals.get_mut(&session_id) {
                    if grid.scrollback_offset < term.scrollback_offset {
                        should_persist_clamp = true;
                    }
                    term.fetching = false;
                    term.dirty = false;
                    term.title = grid.title.clone();
                    term.total_scrollback = grid.total_scrollback;
                    term.scrollback_offset = grid.scrollback_offset.min(grid.total_scrollback);
                    term.grid = Some(grid);
                }
                if should_persist_clamp {
                    self.persist_scrollback_offsets();
                }
                if self.use_pixel_renderer {
                    self.render_terminal_image(&session_id);
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
            // --- G3: URL Click-to-Open ---
            Message::UrlClicked(url) => {
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "", &url])
                        .creation_flags(0x08000000) // CREATE_NO_WINDOW
                        .spawn();
                }
                #[cfg(not(windows))]
                {
                    let _ = std::process::Command::new("xdg-open")
                        .arg(&url)
                        .spawn();
                }
            }
            Message::FileDropped(path) => {
                let target_terminal = self.target_terminal_id().map(str::to_string);
                if let (Some(terminal_id), Some(client)) = (target_terminal, &self.client) {
                    let payload = format!("{} ", quote_dropped_path(&path));
                    let _ = commands::write_to_terminal(client, &terminal_id, payload.as_bytes());
                }
            }
            // --- H1-H6: Shell Picker & Workspace Creation ---
            Message::ShellPickerOpen => {
                self.shell_picker.open();
            }
            Message::ShellPickerTabClicked(tab) => {
                self.shell_picker.tab = tab;
            }
            Message::ShellPickerDistroSelected(distro) => {
                self.shell_picker.selected_distro = distro;
            }
            Message::ShellPickerCustomProgramChanged(val) => {
                self.shell_picker.custom_program = val;
            }
            Message::ShellPickerCustomArgsChanged(val) => {
                self.shell_picker.custom_args = val;
            }
            Message::ShellPickerConfirmed => {
                self.shell_picker.close();
                return self.create_new_terminal();
            }
            Message::ShellPickerCancelled => {
                self.shell_picker.close();
            }
            Message::WorkspaceAiModeChanged { workspace_id, mode } => {
                if mode == AiToolMode::None {
                    self.workspace_ai_modes.remove(&workspace_id);
                } else {
                    self.workspace_ai_modes.insert(workspace_id, mode);
                }
            }

            // --- Keyboard input (shortcut-first, then forward to PTY) ---
            Message::KeyboardEvent(keyboard::Event::KeyPressed {
                key,
                modifiers,
                physical_key,
                text,
                ..
            }) => {
                let mut capture = CaptureState {
                    capturing_index: self.shortcut_capturing_index,
                    overrides: self.shortcut_overrides.clone(),
                    resolver: self.shortcut_resolver.clone(),
                };
                let result = resolve_key_event(
                    &key,
                    modifiers,
                    &physical_key,
                    &mut capture,
                    self.mru_switcher.is_some(),
                    self.claude_md_editor.is_some(),
                    self.quick_claude_dialog.is_some(),
                );

                // Write capture mutations back.
                let overrides_changed = capture.overrides != self.shortcut_overrides;
                self.shortcut_capturing_index = capture.capturing_index;
                self.shortcut_overrides = capture.overrides;
                self.shortcut_resolver = capture.resolver;
                if overrides_changed {
                    keybinding_persistence::save_keybindings(&self.shortcut_overrides);
                }

                match result {
                    KeyRoutingResult::Action(action) => {
                        return self.handle_app_action(action);
                    }
                    KeyRoutingResult::CapturedBinding => {
                        return Task::none();
                    }
                    KeyRoutingResult::Intercepted => {
                        // Re-run the interceptor side-effects that
                        // resolve_key_event cannot perform (it's pure).
                        if let Some(direction) =
                            mru_cycle_direction_from_shortcut_key(&key, modifiers)
                        {
                            self.open_or_cycle_mru_switcher(direction);
                            return Task::none();
                        }
                        if self.mru_switcher.is_some() {
                            if is_escape_key(&key) {
                                self.cancel_mru_switcher();
                            }
                            return Task::none();
                        }
                        if self.claude_md_editor.is_some() {
                            if modifiers.control()
                                && matches!(key, keyboard::Key::Character(ref ch) if ch.as_str() == "s")
                            {
                                return self.update(Message::ClaudeMdSave);
                            }
                            if is_escape_key(&key) {
                                self.claude_md_editor = None;
                            }
                            return Task::none();
                        }
                        if let Some(ref dlg) = self.quick_claude_dialog {
                            // Skill autocomplete key routing
                            if dlg.skill_autocomplete_open {
                                use iced::keyboard::key::Named;
                                match &key {
                                    keyboard::Key::Named(Named::ArrowUp) => {
                                        return self.update(
                                            Message::QuickClaudeDialogSkillAutocompleteNavigate(
                                                -1,
                                            ),
                                        );
                                    }
                                    keyboard::Key::Named(Named::ArrowDown) => {
                                        return self.update(
                                            Message::QuickClaudeDialogSkillAutocompleteNavigate(
                                                1,
                                            ),
                                        );
                                    }
                                    keyboard::Key::Named(Named::Enter)
                                    | keyboard::Key::Named(Named::Tab) => {
                                        let selected = dlg.skill_autocomplete_selected;
                                        return self.update(
                                            Message::QuickClaudeDialogSkillSelected(selected),
                                        );
                                    }
                                    keyboard::Key::Named(Named::Escape) => {
                                        return self.update(
                                            Message::QuickClaudeDialogSkillAutocompleteDismiss,
                                        );
                                    }
                                    _ => {}
                                }
                            }
                            if is_escape_key(&key) {
                                self.quick_claude_dialog = None;
                                return Task::none();
                            }
                            if modifiers.control()
                                && matches!(key, keyboard::Key::Named(keyboard::key::Named::Enter))
                            {
                                if dlg.active_tab == crate::quick_claude_dialog::QuickClaudeTab::ResumeSession
                                    && dlg.selected_session.is_some()
                                {
                                    return self.update(Message::QuickClaudeDialogResume);
                                }
                                return self.update(Message::QuickClaudeDialogLaunch);
                            }
                            // All other keys are forwarded to the dialog's text_editor
                            return Task::none();
                        }
                        return Task::none();
                    }
                    KeyRoutingResult::ForwardToPty => {
                        // fall through to PTY forwarding below
                    }
                }

                // Any keypress clears selection.
                self.selection.clear();

                // Forward to PTY — send to focused terminal, not just active tab.
                // Use the platform-composed `text` for printable Character keys
                // (handles Shift+key → uppercase/symbols on all keyboard layouts).
                // Named keys (Tab, Enter, arrows, etc.) always go through
                // key_to_pty_bytes which encodes modifiers correctly
                // (e.g. Shift+Tab → ESC[Z, not bare \t).
                let bytes =
                    if !modifiers.control() && matches!(key, keyboard::Key::Character(_)) {
                        // Use platform-composed text for printable characters.
                        // If `text` is None this is a dead-key press — return None
                        // so no bytes are sent until the composition resolves.
                        // Falling through to key_to_pty_bytes here would send
                        // the raw character prematurely, causing duplicates
                        // when the dead key is later cancelled or composed.
                        text.as_ref().map(|t| t.as_bytes().to_vec())
                    } else {
                        key_to_pty_bytes(&key, modifiers)
                    };

                if let Some(bytes) = bytes {
                    if let (Some(tid), Some(client)) = (self.target_terminal_id(), &self.client) {
                        let _ = commands::write_to_terminal(client, tid, &bytes);
                    }
                }
            }
            Message::KeyboardEvent(keyboard::Event::KeyReleased { key, modifiers, .. }) => {
                if should_commit_mru_switcher_on_key_release(
                    self.mru_switcher.is_some(),
                    &key,
                    modifiers,
                ) {
                    return self.commit_mru_switcher();
                }
            }
            Message::KeyboardEvent(keyboard::Event::ModifiersChanged(modifiers)) => {
                self.ctrl_held = modifiers.control();
                if should_commit_mru_switcher_on_modifiers_changed(
                    self.mru_switcher.is_some(),
                    modifiers,
                ) {
                    return self.commit_mru_switcher();
                }
            }

            // --- Mouse wheel scrollback ---
            Message::MouseWheel { delta_y } => {
                let lines = -(delta_y * 3.0) as isize;
                return self.scroll_active(lines);
            }

            // --- Tab management ---
            Message::TabClicked(id) => {
                return self.activate_tab_via_reducer(id);
            }
            Message::TabContextToggle(id) => {
                self.tab_context_menu_id = tab_reducer::reduce_tab_context_toggle(
                    self.tab_context_menu_id.as_deref(),
                    &id,
                );
            }
            Message::TabContextRename(id) => {
                if let Some(term) = self.terminals.get(&id) {
                    self.rename_tab_value = term.custom_name.clone().unwrap_or_default();
                    self.rename_tab_id = Some(id);
                    self.tab_context_menu_id = None;
                }
            }
            Message::TabContextSplit {
                terminal_id,
                direction,
            } => {
                if let Some(ws) = self.workspaces.active_mut() {
                    ws.focused_terminal = terminal_id.clone();
                }
                self.terminals.set_active(&terminal_id);
                self.notifications.mark_read(&terminal_id);
                self.tab_context_menu_id = None;
                return self.split_focused_pane(direction);
            }
            Message::TabContextCopyInfo(id) => {
                if let Some(term) = self.terminals.get(&id) {
                    let info = format!(
                        "Tab: {}\nSession: {}\nWorkspace: {}\nTitle: {}\nProcess: {}\nExited: {}{}\nSize: {}x{}",
                        term.tab_label(),
                        term.id,
                        term.workspace_id.as_deref().unwrap_or("-"),
                        if term.title.is_empty() { "-" } else { &term.title },
                        if term.process_name.is_empty() {
                            "-"
                        } else {
                            &term.process_name
                        },
                        term.exited,
                        term.exit_code
                            .map(|code| format!("\nExit code: {}", code))
                            .unwrap_or_default(),
                        term.cols,
                        term.rows
                    );
                    if let Err(e) = clipboard::copy_to_clipboard(&info) {
                        log::error!("Failed to copy tab info to clipboard: {}", e);
                    }
                }
                self.tab_context_menu_id = None;
            }
            Message::TabContextClose(id) => {
                self.tab_context_menu_id = None;
                return self.close_terminal(&id);
            }
            Message::TabDragStart(tab_id) => {
                self.dragging_tab_id = Some(tab_id);
                self.tab_drag_start_pos = self.cursor_position;
                self.tab_context_menu_id = None;
            }
            Message::TabDragHover(target_id) => {
                if let Some(source_id) = self.dragging_tab_id.clone() {
                    if source_id != target_id {
                        self.tab_drag_start_pos = None; // commit drag
                        let _ = self.terminals.reorder_by_ids(&source_id, &target_id);
                    }
                }
            }
            Message::TabSplitZoneHover {
                target_terminal_id,
                placement,
            } => {
                if let Some(source_id) = self.dragging_tab_id.clone() {
                    let decision = layout_reducer::reduce_drop_tab_into_split_zone(
                        layout_reducer::DropTabIntoSplitZoneInput {
                            layout: self.active_layout().cloned(),
                            source_terminal_id: Some(source_id.clone()),
                            target_terminal_id: Some(target_terminal_id),
                            placement,
                        },
                    );

                    if let Some(decision) = decision {
                        if let Some(ws) = self.workspaces.active_mut() {
                            ws.layout = decision.next_layout;
                            ws.focused_terminal = decision.next_focused_terminal_id.clone();
                        }
                        self.terminals
                            .set_active(&decision.next_focused_terminal_id);
                        self.notifications
                            .mark_read(&decision.next_focused_terminal_id);
                        self.dragging_tab_id = None;
                        self.tab_drag_start_pos = None;
                        return self.fetch_grid(&decision.next_focused_terminal_id);
                    }
                }
            }
            Message::TabDragEnd => {
                self.dragging_tab_id = None;
                self.tab_drag_start_pos = None;
            }
            Message::NewTabRequested => {
                self.shell_picker.open();
            }
            Message::CloseTabRequested(id) => {
                if self.dragging_tab_id.as_deref() == Some(id.as_str()) {
                    self.dragging_tab_id = None;
                    self.tab_drag_start_pos = None;
                }
                return self.close_terminal(&id);
            }
            Message::TerminalCreated(Ok(session_id)) => {
                return self.handle_terminal_created(Ok(session_id));
            }
            Message::TerminalCreated(Err(e)) => {
                return self.handle_terminal_created(Err(e));
            }

            Message::WindowOpened(window_id) => {
                self.window_id = Some(window_id);
            }

            // --- Title bar actions ---
            Message::TitleBarDragStart => {
                if let Some(id) = self.window_id {
                    return window::drag(id);
                }
            }
            Message::TitleBarMinimize => {
                if let Some(id) = self.window_id {
                    return window::minimize(id, true);
                }
            }
            Message::TitleBarToggleMaximize => {
                if let Some(id) = self.window_id {
                    return window::toggle_maximize(id);
                }
            }
            Message::TitleBarClose => {
                let terminal_count = self.terminals.count();
                if terminal_count > 0 {
                    self.quit_confirm_pending = true;
                } else if let Some(id) = self.window_id {
                    self.persist_before_close();
                    return window::close(id);
                }
            }

            // --- Window resize ---
            Message::WindowResized {
                window_id,
                width,
                height,
            } => {
                diag::log(&format!("UPDATE: WindowResized({}x{})", width, height));

                let (old_rows, old_cols) = self.terminal_grid_size(self.target_terminal_id());

                self.window_id = Some(window_id);
                self.window_width = width;
                self.window_height = height;

                let (new_rows, new_cols) = self.terminal_grid_size(self.target_terminal_id());

                // Ignore degenerate grid sizes (e.g., when minimized on Windows
                // Iced may report tiny pixel dimensions that compute to ≤2 rows/cols).
                // Resizing terminals to near-zero destroys viewport content.
                if new_rows <= 2 || new_cols <= 2 {
                    diag::log(&format!("RESIZE: ignored — degenerate grid {}x{} (minimized?)", new_cols, new_rows));
                    // Revert stored dimensions so the next real resize computes correctly.
                    self.window_width = old_cols as f32 * self.font_metrics.cell_width;
                    self.window_height = old_rows as f32 * self.font_metrics.cell_height;
                } else if new_cols != old_cols || new_rows != old_rows {
                    diag::log(&format!("RESIZE: {}x{} -> {}x{}", old_cols, old_rows, new_cols, new_rows));
                    // Resize terminals AND force-fetch all grids. After minimize/restore
                    // the subscription stream may be dead, so this is the only way to
                    // get fresh content. Clear fetching flags to bypass coalescing.
                    let ids: Vec<String> = self.terminals.iter().map(|t| t.id.clone()).collect();
                    for id in &ids {
                        if let Some(t) = self.terminals.get_mut(id) {
                            t.fetching = false;
                            t.dirty = true;
                        }
                    }
                    let resize_task = self.resize_all_terminals();
                    let fetch_tasks: Vec<Task<Message>> = ids
                        .into_iter()
                        .map(|id| self.fetch_grid(&id))
                        .collect();
                    let mut tasks = vec![resize_task];
                    tasks.extend(fetch_tasks);
                    return Task::batch(tasks);
                }
            }
            Message::WindowFocusChanged { window_id, focused } => {
                self.window_id = Some(window_id);
                self.window_focused = focused;
                if !focused {
                    self.cancel_mru_switcher();
                    // Pause background sessions (not the active one) to reduce
                    // event traffic while the window isn't visible.
                    // Only pause if we can identify the active terminal — if we
                    // can't, don't pause anything (safe default).
                    if let (Some(active_id), Some(client)) = (self.active_focused().map(str::to_string), &self.client) {
                        let bg_ids: Vec<String> = self.terminals.iter()
                            .filter(|t| t.id != active_id)
                            .map(|t| t.id.clone())
                            .collect();
                        if !bg_ids.is_empty() {
                            commands::pause_all_sessions(client, &bg_ids);
                        }
                    }
                }
                if focused {
                    self.last_attention_request_ms = None;
                    // Resume all sessions that may have been paused on unfocus.
                    if let Some(client) = &self.client {
                        let ids: Vec<String> = self.terminals.iter().map(|t| t.id.clone()).collect();
                        commands::resume_all_sessions(client, &ids);
                    }
                    return window::request_user_attention(window_id, None);
                }
            }

            // --- Mouse selection ---
            Message::SelectionStart { x, y } => {
                if x != 0.0 || y != 0.0 {
                    self.cursor_position = Some(Point::new(x, y));
                }
                // Switch focus to the pane under the cursor.
                if let Some(clicked_id) = self.find_terminal_at_cursor() {
                    if self.active_focused() != Some(clicked_id.as_str()) {
                        self.notifications.mark_read(&clicked_id);
                        if let Some(ws) = self.workspaces.active_mut() {
                            ws.focused_terminal = clicked_id;
                        }
                    }
                }
                // G3: Ctrl+Click opens URL under cursor
                if self.ctrl_held {
                    if let Some(url) = self.detect_url_under_cursor() {
                        return Task::done(Message::UrlClicked(url));
                    }
                }
                if let Some(pos) = self.active_terminal_pointer_grid(false) {
                    self.selection.start(pos);
                }
            }
            Message::SelectionUpdate { x, y } => {
                self.cursor_position = Some(Point::new(x, y));
                // Commit tab drag after mouse moves ≥5 px from press origin.
                if let Some(start) = self.tab_drag_start_pos {
                    let dx = x - start.x;
                    let dy = y - start.y;
                    if dx * dx + dy * dy > 25.0 {
                        self.tab_drag_start_pos = None;
                    }
                }
                // G3: Track hovered URL for visual feedback
                self.hovered_url = self.detect_url_under_cursor();
                if self.sidebar_resizing {
                    self.sidebar_width = sidebar::clamp_sidebar_width(x);
                    return Task::none();
                }
                if self.selection.active {
                    if let Some(pos) = self.active_terminal_pointer_grid(true) {
                        self.selection.update(pos);
                    }
                }
            }
            Message::SelectionEnd => {
                if self.sidebar_resizing {
                    self.sidebar_resizing = false;
                    return self.resize_all_terminals();
                }
                self.selection.finish();
                self.dragging_tab_id = None;
                self.tab_drag_start_pos = None;
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
            Message::FocusLeft => self.focus_in_direction(FocusDirection::Left),
            Message::FocusRight => self.focus_in_direction(FocusDirection::Right),
            Message::FocusUp => self.focus_in_direction(FocusDirection::Up),
            Message::FocusDown => self.focus_in_direction(FocusDirection::Down),
            Message::PaneClicked(terminal_id) => {
                if let Some(new_focus) =
                    layout_reducer::reduce_pane_clicked(self.active_layout(), &terminal_id)
                {
                    self.notifications.mark_read(&new_focus);
                    if let Some(ws) = self.workspaces.active_mut() {
                        ws.focused_terminal = new_focus;
                    }
                }
            }

            // --- Workspace operations ---
            Message::WorkspaceClicked(id) => {
                let focused_terminal_id = self
                    .workspaces
                    .get(&id)
                    .map(|ws| ws.focused_terminal.clone())
                    .or_else(|| {
                        self.workspaces
                            .active()
                            .map(|ws| ws.focused_terminal.clone())
                    });
                let decision = workspace_reducer::reduce_workspace_selection(
                    workspace_reducer::WorkspaceSelectionInput {
                        workspace_id: id,
                        focused_terminal_id,
                    },
                );

                self.workspaces.set_active(&decision.workspace_id);
                if decision.clear_context_menu {
                    self.workspace_context_menu_id = None;
                }
                if let Some(terminal_id) = decision.mark_terminal_read_id {
                    self.notifications.mark_read(&terminal_id);
                }
                self.tab_context_menu_id = None;
            }
            Message::NewWorkspaceRequested => {
                return self.create_new_workspace();
            }
            Message::SidebarAction(action) => {
                return self.handle_sidebar_action(action);
            }
            Message::WorkspaceRenameInputChanged(value) => {
                self.rename_workspace_value = value;
            }
            Message::WorkspaceRenameSubmitted => {
                if let Some(workspace_id) = self.rename_workspace_id.take() {
                    let next_name = self.rename_workspace_value.trim().to_string();
                    if !next_name.is_empty() {
                        let _ = self.workspaces.rename(&workspace_id, next_name);
                    }
                }
                self.rename_workspace_value.clear();
            }
            Message::WorkspaceRenameCancelled => {
                self.rename_workspace_id = None;
                self.rename_workspace_value.clear();
            }
            Message::TabRenameInputChanged(value) => {
                self.rename_tab_value = value;
            }
            Message::TabRenameSubmitted => {
                if let Some(tab_id) = self.rename_tab_id.take() {
                    let decision = tab_reducer::reduce_tab_rename(tab_reducer::TabRenameInput {
                        terminal_id: tab_id.clone(),
                        raw_name: self.rename_tab_value.clone(),
                        terminal_exists: self.terminals.get(&tab_id).is_some(),
                    });
                    if let Some(decision) = decision {
                        self.terminals
                            .rename(&decision.terminal_id, decision.next_custom_name);
                        if decision.clear_context_menu {
                            self.tab_context_menu_id = None;
                        }
                    }
                }
                self.rename_tab_value.clear();
            }
            Message::TabRenameCancelled => {
                self.rename_tab_id = None;
                self.rename_tab_value.clear();
            }
            Message::NotificationSoundsToggled => {
                self.notification_sounds_enabled = !self.notification_sounds_enabled;
            }
            Message::NotificationSoundPresetSelected(preset) => {
                self.notification_sound_preset = preset;
            }
            Message::NotificationSoundTest => {
                if self.notification_sounds_enabled {
                    if let Err(e) =
                        sound::play_notification_sound_async(self.notification_sound_preset)
                    {
                        log::warn!("Failed to play notification test sound: {}", e);
                    }
                }
            }
            Message::WorkspaceMutePatternInputChanged(value) => {
                self.workspace_mute_pattern_input = value;
            }
            Message::AddWorkspaceMutePattern => {
                if let Some(pattern) =
                    normalize_mute_pattern(self.workspace_mute_pattern_input.as_str())
                {
                    if !self
                        .workspace_mute_patterns
                        .iter()
                        .any(|existing| existing == &pattern)
                    {
                        self.workspace_mute_patterns.push(pattern);
                    }
                    self.workspace_mute_pattern_input.clear();
                }
            }
            Message::RemoveWorkspaceMutePattern(pattern) => {
                self.workspace_mute_patterns
                    .retain(|existing| existing != &pattern);
            }
            Message::QuickClaudeNameInputChanged(value) => {
                self.quick_claude_name_input = value;
            }
            Message::QuickClaudePromptInputChanged(value) => {
                self.quick_claude_prompt_input = value;
            }
            Message::QuickClaudeLayoutSelected(layout) => {
                self.quick_claude_layout = layout;
            }
            Message::QuickClaudeSavePreset => {
                let Some(name) = normalize_quick_claude_text(&self.quick_claude_name_input) else {
                    return Task::none();
                };
                let Some(prompt_template) =
                    normalize_quick_claude_text(&self.quick_claude_prompt_input)
                else {
                    return Task::none();
                };

                let next_preset = QuickClaudePreset {
                    name,
                    prompt_template,
                    layout: self.quick_claude_layout,
                };

                if let Some(edit_index) = self.quick_claude_edit_index {
                    if edit_index < self.quick_claude_presets.len() {
                        self.quick_claude_presets[edit_index] = next_preset;
                    } else {
                        self.quick_claude_presets.push(next_preset);
                    }
                } else {
                    self.quick_claude_presets.push(next_preset);
                }

                self.quick_claude_name_input.clear();
                self.quick_claude_prompt_input.clear();
                self.quick_claude_layout = QuickClaudeLayout::Single;
                self.quick_claude_edit_index = None;
            }
            Message::QuickClaudeEditPreset(index) => {
                if let Some(preset) = self.quick_claude_presets.get(index) {
                    self.quick_claude_name_input = preset.name.clone();
                    self.quick_claude_prompt_input = preset.prompt_template.clone();
                    self.quick_claude_layout = preset.layout;
                    self.quick_claude_edit_index = Some(index);
                }
            }
            Message::QuickClaudeDeletePreset(index) => {
                if index < self.quick_claude_presets.len() {
                    self.quick_claude_presets.remove(index);
                    match self.quick_claude_edit_index {
                        Some(edit_index) if edit_index == index => {
                            self.quick_claude_edit_index = None;
                            self.quick_claude_name_input.clear();
                            self.quick_claude_prompt_input.clear();
                            self.quick_claude_layout = QuickClaudeLayout::Single;
                        }
                        Some(edit_index) if edit_index > index => {
                            self.quick_claude_edit_index = Some(edit_index - 1);
                        }
                        _ => {}
                    }
                }
            }
            Message::QuickClaudeClearEditor => {
                self.quick_claude_edit_index = None;
                self.quick_claude_name_input.clear();
                self.quick_claude_prompt_input.clear();
                self.quick_claude_layout = QuickClaudeLayout::Single;
            }
            Message::QuickClaudeLaunchPreset(index) => {
                if self.quick_claude_launch.is_some() {
                    return Task::none();
                }
                let Some(preset) = self.quick_claude_presets.get(index).cloned() else {
                    return Task::none();
                };
                let num_agents = match preset.layout {
                    QuickClaudeLayout::Single => 1,
                    QuickClaudeLayout::VSplit | QuickClaudeLayout::HSplit => 2,
                    QuickClaudeLayout::Grid2x2 => 4,
                };

                // Use active workspace's folder as CWD for presets
                let cwd = self.workspaces.active()
                    .map(|ws| ws.folder_path.clone())
                    .filter(|p| !p.is_empty());
                let steps = crate::quick_claude::default_launch_steps(
                    num_agents, &preset.prompt_template, "sonnet", "default", cwd.as_deref(),
                );

                let ws_id = uuid::Uuid::new_v4().to_string();
                let ws_name = format!("QC: {}", preset.name);
                let placeholder_id = uuid::Uuid::new_v4().to_string();
                let rows = self.calculate_rows();
                let cols = self.calculate_cols();
                self.terminals.add_to_workspace(
                    placeholder_id.clone(),
                    rows,
                    cols,
                    ws_id.clone(),
                );
                self.workspaces.add(
                    ws_id.clone(),
                    ws_name,
                    placeholder_id.clone(),
                );
                self.workspaces.set_active(&ws_id);
                self.terminals.set_active(&placeholder_id);
                self.next_workspace_num += 1;

                let mut launch_state = crate::quick_claude::LaunchState::new(
                    preset.name.clone(),
                    steps,
                    num_agents,
                    ws_id,
                );
                launch_state.agent_terminal_ids[0] = Some(placeholder_id);

                self.quick_claude_launch = Some(launch_state);
                return self.execute_next_launch_step();
            }
            Message::QuickClaudeLaunchStepComplete(result) => {
                return self.handle_launch_step_result(result);
            }
            Message::QuickClaudeLaunchCancel => {
                self.quick_claude_launch = None;
            }
            // --- Quick Claude Dialog (modal) ---
            Message::QuickClaudeDialogOpen => {
                let ws_id = self.workspaces.active().map(|ws| ws.id.clone());
                let ws_list: Vec<(String, String)> = self
                    .workspaces
                    .iter()
                    .map(|ws| (ws.id.clone(), ws.name.clone()))
                    .collect();
                self.quick_claude_dialog = Some(
                    crate::quick_claude_dialog::QuickClaudeDialogState::new(
                        ws_id,
                        ws_list,
                        &self.quick_claude_prefs,
                    ),
                );

                // Load skills, sessions, and models asynchronously
                let ws_folder: Option<String> = self
                    .workspaces
                    .active()
                    .and_then(|ws| {
                        if ws.folder_path.is_empty() {
                            None
                        } else {
                            Some(ws.folder_path.clone())
                        }
                    });
                let ws_folder2 = ws_folder.clone();
                let skills_task = iced::Task::perform(
                    async move {
                        crate::quick_claude_dialog::discover_skills(ws_folder.as_deref())
                    },
                    Message::QuickClaudeDialogSkillsLoaded,
                );
                let sessions_task = iced::Task::perform(
                    async move {
                        crate::quick_claude_dialog::discover_sessions(ws_folder2.as_deref())
                    },
                    Message::QuickClaudeDialogSessionsLoaded,
                );
                let models_task = iced::Task::perform(
                    async move {
                        crate::quick_claude_dialog::discover_models()
                    },
                    Message::QuickClaudeDialogModelsLoaded,
                );
                return iced::Task::batch([skills_task, sessions_task, models_task]);
            }
            Message::QuickClaudeDialogClose => {
                self.quick_claude_dialog = None;
            }
            Message::QuickClaudeDialogWorkspaceSelected(id) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.selected_workspace_id = Some(id);
                    dlg.workspace_dropdown_open = false;
                }
            }
            Message::QuickClaudeDialogWorkspaceDropdownToggle => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.workspace_dropdown_open = !dlg.workspace_dropdown_open;
                }
            }
            Message::QuickClaudeDialogAiToolSelected(tool) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.selected_ai_tool = tool;
                    dlg.ai_tool_dropdown_open = false;
                }
            }
            Message::QuickClaudeDialogAiToolDropdownToggle => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.ai_tool_dropdown_open = !dlg.ai_tool_dropdown_open;
                }
            }
            Message::QuickClaudeDialogPromptAction(action) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.prompt_content.perform(action);

                    // Detect skill autocomplete trigger
                    if dlg.selected_ai_tool == "Claude Code" {
                        let text = dlg.prompt_content.text();
                        // Find the last '/' in the text and check if it's a skill trigger
                        if let Some(slash_pos) = text.rfind('/') {
                            let after_slash = &text[slash_pos + 1..];
                            // Only trigger if there's no space after the slash
                            if !after_slash.contains(' ') && !after_slash.contains('\n') {
                                dlg.skill_autocomplete_open = true;
                                dlg.skill_autocomplete_filter = after_slash.to_string();
                                dlg.skill_autocomplete_selected = 0;
                            } else {
                                dlg.skill_autocomplete_open = false;
                                dlg.skill_autocomplete_filter.clear();
                            }
                        } else {
                            dlg.skill_autocomplete_open = false;
                            dlg.skill_autocomplete_filter.clear();
                        }
                    }
                }
            }
            Message::QuickClaudeDialogBranchChanged(val) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.branch_name = val;
                }
            }
            Message::QuickClaudeDialogMainBranchToggled(val) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.main_branch_mode = val;
                }
            }
            Message::QuickClaudeDialogAutoSuggestToggled(val) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.auto_suggest_branch = val;
                }
            }
            Message::QuickClaudeDialogModelSelected(model) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.selected_model = model;
                    dlg.model_dropdown_open = false;
                }
            }
            Message::QuickClaudeDialogModelDropdownToggle => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.model_dropdown_open = !dlg.model_dropdown_open;
                }
            }
            Message::QuickClaudeDialogModeSelected(mode) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.selected_mode = mode;
                    dlg.mode_dropdown_open = false;
                }
            }
            Message::QuickClaudeDialogModeDropdownToggle => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.mode_dropdown_open = !dlg.mode_dropdown_open;
                }
            }
            Message::QuickClaudeDialogLaunch => {
                if self.quick_claude_launch.is_some() {
                    return Task::none();
                }
                let Some(dlg) = self.quick_claude_dialog.take() else {
                    return Task::none();
                };
                // Save preferences for next dialog open
                self.quick_claude_prefs = dlg.to_preferences();
                let prompt = dlg.prompt_text();
                let prompt = prompt.trim();
                let model = dlg.selected_model.clone();
                let mode = dlg.selected_mode.clone();
                let num_agents = 1;

                // Resolve workspace: use selected workspace if it exists,
                // otherwise create a new one.
                let selected_ws = dlg
                    .selected_workspace_id
                    .as_ref()
                    .and_then(|id| self.workspaces.get(id))
                    .map(|ws| (ws.id.clone(), ws.name.clone(), ws.folder_path.clone()));

                let placeholder_id = uuid::Uuid::new_v4().to_string();
                let rows = self.calculate_rows();
                let cols = self.calculate_cols();

                let (ws_id, ws_name, cwd) = if let Some((id, name, folder)) = selected_ws {
                    // Add terminal to the EXISTING workspace as a new tab
                    self.terminals.add_to_workspace(
                        placeholder_id.clone(),
                        rows,
                        cols,
                        id.clone(),
                    );
                    // Split the focused terminal's leaf to show the new pane
                    if let Some(ws) = self.workspaces.get_mut(&id) {
                        let focused = ws.focused_terminal.clone();
                        ws.layout.split_leaf(
                            &focused,
                            placeholder_id.clone(),
                            SplitDirection::Horizontal,
                        );
                    }
                    self.workspaces.set_active(&id);
                    self.terminals.set_active(&placeholder_id);
                    let cwd = if folder.is_empty() { None } else { Some(folder) };
                    (id, name, cwd)
                } else {
                    // No workspace selected — create a new one
                    let id = uuid::Uuid::new_v4().to_string();
                    let snippet: String = prompt.chars().take(30).collect();
                    let name = if snippet.is_empty() {
                        "Quick Claude".to_string()
                    } else {
                        format!("QC: {}", snippet.trim())
                    };
                    self.terminals.add_to_workspace(
                        placeholder_id.clone(),
                        rows,
                        cols,
                        id.clone(),
                    );
                    self.workspaces.add(
                        id.clone(),
                        name.clone(),
                        placeholder_id.clone(),
                    );
                    self.workspaces.set_active(&id);
                    self.terminals.set_active(&placeholder_id);
                    self.next_workspace_num += 1;
                    (id, name, None)
                };

                let steps = crate::quick_claude::default_launch_steps(
                    num_agents, prompt, &model, &mode, cwd.as_deref(),
                );

                let mut launch_state = crate::quick_claude::LaunchState::new(
                    ws_name,
                    steps,
                    num_agents,
                    ws_id,
                );
                launch_state.agent_terminal_ids[0] = Some(placeholder_id);

                self.quick_claude_launch = Some(launch_state);
                return self.execute_next_launch_step();
            }
            Message::QuickClaudeDialogVoice => {
                // Voice integration — forward to whisper toggle if available
                return self.update(Message::WhisperToggle);
            }
            Message::QuickClaudeDialogSkillsLoaded(skills) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.skills = skills;
                }
            }
            Message::QuickClaudeDialogSkillSelected(index) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    let filtered: Vec<&crate::quick_claude_dialog::SkillEntry> =
                        dlg.filtered_skills();
                    if let Some(skill) = filtered.get(index) {
                        let skill_name = format!("/{}", skill.name);
                        // Replace the /partial text in prompt with the selected skill name
                        let current_text = dlg.prompt_content.text();
                        if let Some(slash_pos) = current_text.rfind('/') {
                            let new_text =
                                format!("{}{} ", &current_text[..slash_pos], skill_name);
                            dlg.prompt_content =
                                iced::widget::text_editor::Content::with_text(&new_text);
                        }
                        dlg.skill_autocomplete_open = false;
                        dlg.skill_autocomplete_filter.clear();
                    }
                }
            }
            Message::QuickClaudeDialogSkillAutocompleteNavigate(delta) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    let count = dlg.filtered_skills().len().min(8);
                    if count > 0 {
                        let current = dlg.skill_autocomplete_selected as i32;
                        let next = (current + delta).rem_euclid(count as i32);
                        dlg.skill_autocomplete_selected = next as usize;
                    }
                }
            }
            Message::QuickClaudeDialogSkillAutocompleteDismiss => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.skill_autocomplete_open = false;
                    dlg.skill_autocomplete_filter.clear();
                }
            }
            Message::QuickClaudeDialogTabSelected(tab) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.active_tab = tab;
                }
            }
            Message::QuickClaudeDialogSessionSelected(index) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.selected_session = Some(index);
                }
            }
            Message::QuickClaudeDialogModelsLoaded(models) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    if !models.is_empty() {
                        dlg.available_models = models;
                    }
                }
            }
            Message::QuickClaudeDialogSessionsLoaded(sessions) => {
                if let Some(ref mut dlg) = self.quick_claude_dialog {
                    dlg.sessions = sessions;
                }
            }
            Message::QuickClaudeDialogResume => {
                if self.quick_claude_launch.is_some() {
                    return Task::none();
                }
                let Some(dlg) = self.quick_claude_dialog.take() else {
                    return Task::none();
                };
                // Save preferences for next dialog open
                self.quick_claude_prefs = dlg.to_preferences();
                let Some(selected_idx) = dlg.selected_session else {
                    return Task::none();
                };
                let Some(session) = dlg.sessions.get(selected_idx) else {
                    return Task::none();
                };

                // Use selected workspace's folder as CWD for resume
                let cwd = dlg
                    .selected_workspace_id
                    .as_ref()
                    .and_then(|id| self.workspaces.get(id))
                    .map(|ws| ws.folder_path.clone())
                    .filter(|p| !p.is_empty());

                let session_id = session.session_id.clone();
                let steps = crate::quick_claude::resume_launch_steps(
                    &session_id, cwd.as_deref(),
                );

                let ws_id = uuid::Uuid::new_v4().to_string();
                let ws_name = "Quick Claude (Resume)".to_string();
                let placeholder_id = uuid::Uuid::new_v4().to_string();
                let rows = self.calculate_rows();
                let cols = self.calculate_cols();
                self.terminals.add_to_workspace(
                    placeholder_id.clone(),
                    rows,
                    cols,
                    ws_id.clone(),
                );
                self.workspaces.add(
                    ws_id.clone(),
                    ws_name.clone(),
                    placeholder_id.clone(),
                );
                self.workspaces.set_active(&ws_id);
                self.terminals.set_active(&placeholder_id);
                self.next_workspace_num += 1;

                let mut launch_state = crate::quick_claude::LaunchState::new(
                    ws_name,
                    steps,
                    1,
                    ws_id,
                );
                launch_state.agent_terminal_ids[0] = Some(placeholder_id);

                self.quick_claude_launch = Some(launch_state);
                return self.execute_next_launch_step();
            }
            Message::AiToolNameInputChanged(value) => {
                self.ai_tool_name_input = value;
            }
            Message::AiToolCommandInputChanged(value) => {
                self.ai_tool_command_input = value;
            }
            Message::AiToolIconInputChanged(value) => {
                self.ai_tool_icon_input = value;
            }
            Message::AiToolSave => {
                let Some(display_name) = normalize_ai_tool_field(&self.ai_tool_name_input) else {
                    return Task::none();
                };
                let Some(command) = normalize_ai_tool_field(&self.ai_tool_command_input) else {
                    return Task::none();
                };
                let icon_tag = normalize_ai_tool_field(&self.ai_tool_icon_input);

                let next_entry = AiToolEntry {
                    display_name,
                    command,
                    icon_tag,
                };

                if let Some(edit_index) = self.ai_tool_edit_index {
                    if edit_index < self.ai_tools.len() {
                        self.ai_tools[edit_index] = next_entry;
                    } else {
                        self.ai_tools.push(next_entry);
                    }
                } else {
                    self.ai_tools.push(next_entry);
                }

                self.ai_tool_name_input.clear();
                self.ai_tool_command_input.clear();
                self.ai_tool_icon_input.clear();
                self.ai_tool_edit_index = None;
            }
            Message::AiToolEdit(index) => {
                if let Some(entry) = self.ai_tools.get(index) {
                    self.ai_tool_name_input = entry.display_name.clone();
                    self.ai_tool_command_input = entry.command.clone();
                    self.ai_tool_icon_input = entry.icon_tag.clone().unwrap_or_default();
                    self.ai_tool_edit_index = Some(index);
                }
            }
            Message::AiToolDelete(index) => {
                if index < self.ai_tools.len() {
                    self.ai_tools.remove(index);
                    match self.ai_tool_edit_index {
                        Some(edit_index) if edit_index == index => {
                            self.ai_tool_edit_index = None;
                            self.ai_tool_name_input.clear();
                            self.ai_tool_command_input.clear();
                            self.ai_tool_icon_input.clear();
                        }
                        Some(edit_index) if edit_index > index => {
                            self.ai_tool_edit_index = Some(edit_index - 1);
                        }
                        _ => {}
                    }
                }
            }
            Message::AiToolClearEditor => {
                self.ai_tool_edit_index = None;
                self.ai_tool_name_input.clear();
                self.ai_tool_command_input.clear();
                self.ai_tool_icon_input.clear();
            }
            Message::PluginNameInputChanged(value) => {
                self.plugin_name_input = value;
            }
            Message::PluginSourceInputChanged(value) => {
                self.plugin_source_input = value;
            }
            Message::PluginSave => {
                let Some(name) = normalize_plugin_field(&self.plugin_name_input) else {
                    return Task::none();
                };
                let Some(source) = normalize_plugin_field(&self.plugin_source_input) else {
                    return Task::none();
                };

                let next_entry = PluginEntry {
                    name,
                    source,
                    enabled: true,
                };

                if let Some(edit_index) = self.plugin_edit_index {
                    if edit_index < self.plugins.len() {
                        self.plugins[edit_index] = next_entry;
                    } else {
                        self.plugins.push(next_entry);
                    }
                } else {
                    self.plugins.push(next_entry);
                }

                self.plugin_name_input.clear();
                self.plugin_source_input.clear();
                self.plugin_edit_index = None;
            }
            Message::PluginEdit(index) => {
                if let Some(entry) = self.plugins.get(index) {
                    self.plugin_name_input = entry.name.clone();
                    self.plugin_source_input = entry.source.clone();
                    self.plugin_edit_index = Some(index);
                }
            }
            Message::PluginDelete(index) => {
                if index < self.plugins.len() {
                    self.plugins.remove(index);
                    match self.plugin_edit_index {
                        Some(edit_index) if edit_index == index => {
                            self.plugin_edit_index = None;
                            self.plugin_name_input.clear();
                            self.plugin_source_input.clear();
                        }
                        Some(edit_index) if edit_index > index => {
                            self.plugin_edit_index = Some(edit_index - 1);
                        }
                        _ => {}
                    }
                }
            }
            Message::PluginToggleEnabled(index) => {
                let _ = toggle_plugin_enabled(self.plugins.as_mut_slice(), index);
            }
            Message::PluginClearEditor => {
                self.plugin_edit_index = None;
                self.plugin_name_input.clear();
                self.plugin_source_input.clear();
            }
            Message::FlowNameInputChanged(value) => {
                self.flow_name_input = value;
            }
            Message::FlowTriggerInputChanged(value) => {
                self.flow_trigger_input = value;
            }
            Message::FlowStepsInputChanged(value) => {
                self.flow_steps_input = value;
            }
            Message::FlowSave => {
                let Some(name) = normalize_flow_field(&self.flow_name_input) else {
                    return Task::none();
                };
                let trigger = normalize_flow_field(&self.flow_trigger_input)
                    .unwrap_or_else(|| "Manual".to_string());
                let steps = parse_flow_steps(&self.flow_steps_input);
                let enabled = self
                    .flow_edit_index
                    .and_then(|index| self.flows.get(index).map(|flow| flow.enabled))
                    .unwrap_or(true);

                let next_entry = FlowEntry {
                    name,
                    trigger,
                    steps,
                    enabled,
                };

                if let Some(edit_index) = self.flow_edit_index {
                    if edit_index < self.flows.len() {
                        self.flows[edit_index] = next_entry;
                    } else {
                        self.flows.push(next_entry);
                    }
                } else {
                    self.flows.push(next_entry);
                }

                self.flow_name_input.clear();
                self.flow_trigger_input.clear();
                self.flow_steps_input.clear();
                self.flow_edit_index = None;
            }
            Message::FlowEdit(index) => {
                if let Some(entry) = self.flows.get(index) {
                    self.flow_name_input = entry.name.clone();
                    self.flow_trigger_input = entry.trigger.clone();
                    self.flow_steps_input = entry.steps.join("\n");
                    self.flow_edit_index = Some(index);
                }
            }
            Message::FlowDelete(index) => {
                if index < self.flows.len() {
                    self.flows.remove(index);
                    match self.flow_edit_index {
                        Some(edit_index) if edit_index == index => {
                            self.flow_edit_index = None;
                            self.flow_name_input.clear();
                            self.flow_trigger_input.clear();
                            self.flow_steps_input.clear();
                        }
                        Some(edit_index) if edit_index > index => {
                            self.flow_edit_index = Some(edit_index - 1);
                        }
                        _ => {}
                    }
                }
            }
            Message::FlowToggleEnabled(index) => {
                let _ = toggle_flow_enabled(self.flows.as_mut_slice(), index);
            }
            Message::FlowClearEditor => {
                self.flow_edit_index = None;
                self.flow_name_input.clear();
                self.flow_trigger_input.clear();
                self.flow_steps_input.clear();
            }
            Message::RemoteNameInputChanged(value) => {
                self.remote_name_input = value;
            }
            Message::RemoteHostInputChanged(value) => {
                self.remote_host_input = value;
            }
            Message::RemotePortInputChanged(value) => {
                self.remote_port_input = value;
            }
            Message::RemoteUsernameInputChanged(value) => {
                self.remote_username_input = value;
            }
            Message::RemoteAuthModeSelected(mode) => {
                self.remote_auth_mode = mode;
            }
            Message::RemoteAuthValueInputChanged(value) => {
                self.remote_auth_value_input = value;
            }
            Message::RemoteSave => {
                let Some(name) = normalize_remote_field(&self.remote_name_input) else {
                    return Task::none();
                };
                let Some(host) = normalize_remote_field(&self.remote_host_input) else {
                    return Task::none();
                };
                let Some(port) = normalize_remote_port(&self.remote_port_input) else {
                    return Task::none();
                };
                let Some(username) = normalize_remote_field(&self.remote_username_input) else {
                    return Task::none();
                };
                let auth_value = normalize_remote_field(&self.remote_auth_value_input);
                let enabled = self
                    .remote_edit_index
                    .and_then(|index| {
                        self.remote_connections
                            .get(index)
                            .map(|profile| profile.enabled)
                    })
                    .unwrap_or(true);

                let next_profile = RemoteConnectionEntry {
                    name,
                    host,
                    port,
                    username,
                    auth_mode: self.remote_auth_mode,
                    auth_value,
                    enabled,
                };

                if let Some(edit_index) = self.remote_edit_index {
                    if edit_index < self.remote_connections.len() {
                        self.remote_connections[edit_index] = next_profile;
                    } else {
                        self.remote_connections.push(next_profile);
                    }
                } else {
                    self.remote_connections.push(next_profile);
                }

                self.remote_name_input.clear();
                self.remote_host_input.clear();
                self.remote_port_input = "22".to_string();
                self.remote_username_input.clear();
                self.remote_auth_mode = RemoteAuthMode::Password;
                self.remote_auth_value_input.clear();
                self.remote_edit_index = None;
            }
            Message::RemoteEdit(index) => {
                if let Some(profile) = self.remote_connections.get(index) {
                    self.remote_name_input = profile.name.clone();
                    self.remote_host_input = profile.host.clone();
                    self.remote_port_input = profile.port.to_string();
                    self.remote_username_input = profile.username.clone();
                    self.remote_auth_mode = profile.auth_mode;
                    self.remote_auth_value_input = profile.auth_value.clone().unwrap_or_default();
                    self.remote_edit_index = Some(index);
                }
            }
            Message::RemoteDelete(index) => {
                if index < self.remote_connections.len() {
                    self.remote_connections.remove(index);
                    match self.remote_edit_index {
                        Some(edit_index) if edit_index == index => {
                            self.remote_edit_index = None;
                            self.remote_name_input.clear();
                            self.remote_host_input.clear();
                            self.remote_port_input = "22".to_string();
                            self.remote_username_input.clear();
                            self.remote_auth_mode = RemoteAuthMode::Password;
                            self.remote_auth_value_input.clear();
                        }
                        Some(edit_index) if edit_index > index => {
                            self.remote_edit_index = Some(edit_index - 1);
                        }
                        _ => {}
                    }
                }
            }
            Message::RemoteToggleEnabled(index) => {
                let _ = toggle_remote_enabled(self.remote_connections.as_mut_slice(), index);
            }
            Message::RemoteClearEditor => {
                self.remote_edit_index = None;
                self.remote_name_input.clear();
                self.remote_host_input.clear();
                self.remote_port_input = "22".to_string();
                self.remote_username_input.clear();
                self.remote_auth_mode = RemoteAuthMode::Password;
                self.remote_auth_value_input.clear();
            }
            Message::ToastTick => {
                let now_ms = Self::now_ms();
                let _ = prune_expired_toasts(self.toasts.as_mut(), now_ms);
            }
            Message::Heartbeat => {
                // Detect actual focus via Win32 API — Iced's Focused/Unfocused
                // events are unreliable on Windows (missed on minimize, stolen
                // on restore). This runs from RedrawRequested + WM_TIMER.
                #[cfg(windows)]
                {
                    extern "system" {
                        fn GetForegroundWindow() -> *mut std::ffi::c_void;
                        fn GetWindowThreadProcessId(hwnd: *mut std::ffi::c_void, pid: *mut u32) -> u32;
                        fn GetCurrentProcessId() -> u32;
                    }
                    let is_actually_focused = unsafe {
                        let fg = GetForegroundWindow();
                        if fg.is_null() {
                            false
                        } else {
                            let mut pid: u32 = 0;
                            GetWindowThreadProcessId(fg, &mut pid);
                            pid == GetCurrentProcessId()
                        }
                    };
                    if is_actually_focused != self.window_focused {
                        diag::log(&format!(
                            "HEARTBEAT: focus correction — OS={} iced={}",
                            is_actually_focused, self.window_focused
                        ));
                        self.window_focused = is_actually_focused;
                        if is_actually_focused {
                            self.last_attention_request_ms = None;
                            // Resume any paused sessions
                            if let Some(client) = &self.client {
                                let ids: Vec<String> = self.terminals.iter().map(|t| t.id.clone()).collect();
                                commands::resume_all_sessions(client, &ids);
                            }
                        }
                    }
                    // Periodic grid poll: when focused, fetch grids for all terminals.
                    // This compensates for the daemon event subscription dying after
                    // minimize/restore (bridge pipe can break during dormancy).
                    if is_actually_focused && self.client.is_some() {
                        let ids: Vec<String> = self.terminals.iter().map(|t| t.id.clone()).collect();
                        for id in &ids {
                            if let Some(t) = self.terminals.get_mut(id) {
                                t.fetching = false; // Force-allow fetch
                            }
                        }
                        let tasks: Vec<Task<Message>> = ids
                            .into_iter()
                            .map(|id| self.fetch_grid(&id))
                            .collect();
                        if !tasks.is_empty() {
                            return Task::batch(tasks);
                        }
                    }
                }
            }

            // --- Sidebar + Settings ---
            Message::ToggleSidebar => {
                return self.toggle_sidebar_visibility();
            }
            Message::SidebarResizeStart => {
                if self.sidebar_visible && self.sidebar_animation.is_none() {
                    self.sidebar_resizing = true;
                    self.selection.clear();
                }
            }
            Message::SidebarResizeEnd => {
                if self.sidebar_resizing {
                    self.sidebar_resizing = false;
                    return self.resize_all_terminals();
                }
            }
            Message::SidebarAnimationTick => {
                let now_ms = Self::now_ms();
                if let Some(animation) = self.sidebar_animation {
                    if sidebar_animation_finished(animation, now_ms) {
                        self.sidebar_animation = None;
                        return self.resize_all_terminals();
                    }
                }
            }
            Message::TabEntryAnimationTick => {
                let now_ms = Self::now_ms();
                if tab_bar::all_entries_finished(&self.entering_tabs, now_ms) {
                    self.entering_tabs.clear();
                }
            }
            Message::ToggleSettings => {
                self.settings_open = !self.settings_open;
            }
            Message::SettingsTabClicked(tab_id) => {
                self.settings_tab = tab_id.clone();
                if tab_id == "appearance" && self.available_fonts.is_none() {
                    return iced::Task::perform(
                        async { font_enumerator::enumerate_monospace_fonts() },
                        Message::FontsEnumerated,
                    );
                }
            }
            Message::FontFamilyChanged(name) => {
                log::info!("[FONT] family changed: {} -> {}", self.font_family, name);
                self.font_family = name.clone();
                let interned = font_enumerator::intern_font_name(&name);
                self.terminal_font = iced::Font {
                    family: iced::font::Family::Name(interned),
                    weight: iced::font::Weight::Normal,
                    stretch: iced::font::Stretch::Normal,
                    style: iced::font::Style::Normal,
                };
                return self.resize_all_terminals();
            }
            Message::FontsEnumerated(fonts) => {
                self.available_fonts = Some(fonts);
            }
            Message::FontFilterChanged(query) => {
                self.font_filter_query = query;
            }
            Message::FontSizeIncrement => {
                log::info!("[FONT] size increment: {} -> {}", self.font_metrics.font_size, self.font_metrics.font_size + 1.0);
                self.font_metrics = measured_font_metrics(self.font_metrics.font_size + 1.0, &self.font_family);
                return self.resize_all_terminals();
            }
            Message::FontSizeDecrement => {
                let new_size = (self.font_metrics.font_size - 1.0).max(8.0);
                log::info!("[FONT] size decrement: {} -> {}", self.font_metrics.font_size, new_size);
                self.font_metrics = measured_font_metrics(new_size, &self.font_family);
                return self.resize_all_terminals();
            }
            Message::ShortcutBadgeClicked(index) => {
                self.shortcut_capturing_index = Some(index);
            }
            Message::ShortcutCaptureCancelled => {
                self.shortcut_capturing_index = None;
            }
            // --- K2/K3: Quit Confirmation + Copy Preview ---
            Message::QuitConfirmShow => {
                self.quit_confirm_pending = true;
            }
            Message::QuitConfirmed => {
                self.quit_confirm_pending = false;
                self.persist_before_close();
                if let Some(id) = self.window_id {
                    return window::close(id);
                }
            }
            Message::QuitCancelled => {
                self.quit_confirm_pending = false;
            }
            Message::CopyPreviewShow(preview_text) => {
                self.copy_preview_text = Some(preview_text);
            }
            Message::CopyPreviewConfirmed => {
                if let Some(ref text) = self.copy_preview_text {
                    if let Err(e) = clipboard::copy_to_clipboard(text) {
                        log::error!("Clipboard copy failed: {}", e);
                    }
                }
                self.copy_preview_text = None;
            }
            Message::CopyPreviewDismissed => {
                self.copy_preview_text = None;
            }
            // --- Worktree Mode ---
            Message::WorktreeCreated { session_id, worktree_path } => {
                self.terminals.set_worktree_path(&session_id, worktree_path);
                return self.handle_terminal_created(Ok(session_id));
            }
            Message::WorktreeCloseConfirmed { session_id } => {
                self.worktree_close_pending = None;
                let worktree_path = self.terminals.get(&session_id)
                    .and_then(|t| t.worktree_path.clone());
                let repo_root = self.workspaces.active()
                    .map(|ws| ws.folder_path.clone());
                let close_task = self.close_terminal_immediate(&session_id);
                if let (Some(wt_path), Some(root)) = (worktree_path, repo_root) {
                    let sid = session_id.clone();
                    let remove_task = Task::perform(
                        async move {
                            let (tx, rx) = futures_channel::oneshot::channel::<Result<(), String>>();
                            std::thread::spawn(move || {
                                let result = crate::git_worktree::remove_worktree(&root, &wt_path);
                                let _ = tx.send(result);
                            });
                            rx.await.unwrap_or_else(|_| Err("Background thread panicked".into()))
                        },
                        move |result| Message::WorktreeRemoved { session_id: sid, result },
                    );
                    return Task::batch([close_task, remove_task]);
                }
                return close_task;
            }
            Message::WorktreeCloseKeep { session_id } => {
                self.worktree_close_pending = None;
                return self.close_terminal_immediate(&session_id);
            }
            Message::WorktreeRemoved { session_id, result } => {
                match result {
                    Ok(()) => log::info!("Worktree cleaned up for session {session_id}"),
                    Err(ref e) => log::warn!("Failed to remove worktree for {session_id}: {e}"),
                }
            }
            Message::WorktreeCloseCancelled => {
                self.worktree_close_pending = None;
            }
            Message::ThemeChanged(id) => {
                self.active_theme = id;
                self.active_custom_theme_id = None;
                crate::theme::set_active_theme(id);
            }
            // --- F5: Custom Theme Import/Export ---
            Message::ThemeImportRequested => {
                return Task::perform(
                    async {
                        let file = rfd::AsyncFileDialog::new()
                            .add_filter("JSON", &["json"])
                            .set_title("Import Custom Theme")
                            .pick_file()
                            .await;
                        match file {
                            Some(f) => {
                                let bytes = f.read().await;
                                let json = String::from_utf8(bytes)
                                    .map_err(|e| format!("Invalid UTF-8: {e}"))?;
                                crate::theme::validate_custom_theme(&json)
                            }
                            None => Err("Cancelled".into()),
                        }
                    },
                    Message::ThemeImported,
                );
            }
            Message::ThemeImported(Ok(theme)) => {
                self.custom_themes.retain(|t| t.id != theme.id);
                self.custom_themes.push(theme.clone());
                let dir = crate::theme::custom_themes_dir();
                if let Err(e) = crate::theme::save_custom_themes(&dir, &self.custom_themes) {
                    log::error!("Failed to save custom themes: {e}");
                }
                self.enqueue_toast(
                    "Theme Imported".into(),
                    format!("\"{}\" added to custom themes.", theme.name),
                );
            }
            Message::ThemeImported(Err(e)) => {
                if e != "Cancelled" {
                    self.enqueue_toast("Import Failed".into(), e);
                }
            }
            Message::ThemeExportRequested(id) => {
                if let Some(theme) = self.custom_themes.iter().find(|t| t.id == id).cloned() {
                    return Task::perform(
                        async move {
                            let dir = rfd::AsyncFileDialog::new()
                                .set_title("Export Theme - Choose Folder")
                                .pick_folder()
                                .await;
                            match dir {
                                Some(d) => {
                                    let path = d.path().to_path_buf();
                                    crate::theme::export_theme_to_file(&theme, &path)
                                        .map(|p| p.display().to_string())
                                }
                                None => Err("Cancelled".into()),
                            }
                        },
                        Message::ThemeExported,
                    );
                }
            }
            Message::ThemeExported(Ok(path)) => {
                self.enqueue_toast("Theme Exported".into(), format!("Saved to {path}"));
            }
            Message::ThemeExported(Err(e)) => {
                if e != "Cancelled" {
                    self.enqueue_toast("Export Failed".into(), e);
                }
            }
            Message::FolderPickerResult(Some(path)) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| {
                        format!("Workspace {}", self.next_workspace_num)
                    });
                let folder = path.display().to_string();
                return self.create_new_workspace_with_folder(name, folder);
            }
            Message::FolderPickerResult(None) => {
                // User cancelled the folder picker dialog
            }
            Message::ThemeDeleteCustom(id) => {
                self.custom_themes.retain(|t| t.id != id);
                let dir = crate::theme::custom_themes_dir();
                if let Err(e) = crate::theme::save_custom_themes(&dir, &self.custom_themes) {
                    log::error!("Failed to save custom themes after delete: {e}");
                }
                if self.active_custom_theme_id.as_deref() == Some(&id) {
                    self.active_custom_theme_id = None;
                    self.active_theme = crate::theme::ThemeId::Dusk;
                    crate::theme::set_active_theme(crate::theme::ThemeId::Dusk);
                }
            }
            Message::ThemeSelectCustom(id) => {
                if let Some(theme) = self.custom_themes.iter().find(|t| t.id == id) {
                    self.active_custom_theme_id = Some(id);
                    crate::theme::set_active_custom_theme(theme);
                }
            }
            // --- G1/G2: Terminal Context Menu ---
            Message::TerminalContextOpen { id, x, y } => {
                self.terminal_context_menu_pos = Some((x, y));
                self.terminal_context_menu_terminal_id = Some(id);
            }
            Message::TerminalContextClose => {
                self.terminal_context_menu_pos = None;
                self.terminal_context_menu_terminal_id = None;
            }
            Message::TerminalContextAction(action) => {
                self.terminal_context_menu_pos = None;
                self.terminal_context_menu_terminal_id = None;
                match action {
                    TermCtxAction::Copy => {
                        self.copy_selection();
                    }
                    TermCtxAction::CopyClean => {
                        if let Some(tid) = self.target_terminal_id() {
                            if let Some(term) = self.terminals.get(tid) {
                                if let Some(grid) = &term.grid {
                                    let clean = self.selection.selected_text_clean(grid);
                                    if !clean.is_empty() {
                                        if let Err(e) = clipboard::copy_to_clipboard(&clean) {
                                            log::error!("Clipboard copy (clean) failed: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    TermCtxAction::Paste => {
                        return self.paste_from_clipboard();
                    }
                    TermCtxAction::SelectAll => {
                        self.select_all();
                    }
                    TermCtxAction::Clear => {
                        if let (Some(tid), Some(client)) = (self.target_terminal_id(), &self.client) {
                            let _ = commands::write_to_terminal(client, tid, b"clear\r");
                        }
                    }
                    TermCtxAction::SplitRight => {
                        return self.split_focused_pane(SplitDirection::Horizontal);
                    }
                    TermCtxAction::SplitDown => {
                        return self.split_focused_pane(SplitDirection::Vertical);
                    }
                }
            }
            // --- G4: Find in Terminal ---
            Message::SearchOpen => {
                self.search.open();
            }
            Message::SearchClose => {
                self.search.close();
            }
            Message::SearchQueryChanged(query) => {
                let grid = self.target_terminal_id()
                    .and_then(|tid| self.terminals.get(tid))
                    .and_then(|term| term.grid.as_ref());
                self.search.set_query(query, grid);
            }
            Message::SearchNext => {
                self.search.next_match();
            }
            Message::SearchPrev => {
                self.search.prev_match();
            }
            Message::SearchToggleRegex => {
                let grid = self.target_terminal_id()
                    .and_then(|tid| self.terminals.get(tid))
                    .and_then(|term| term.grid.as_ref());
                self.search.toggle_regex(grid);
            }
            // --- G6/G7: Performance Overlay ---
            Message::TogglePerfOverlay => {
                self.perf_overlay_visible = !self.perf_overlay_visible;
            }
            // --- K1: CLAUDE.md Editor ---
            Message::ClaudeMdOpen { path } => {
                let p = path.clone();
                return Task::perform(
                    async move {
                        let (tx, rx) = futures_channel::oneshot::channel();
                        std::thread::spawn(move || {
                            let result = if p.exists() {
                                std::fs::read_to_string(&p)
                            } else {
                                if let Some(parent) = p.parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }
                                let _ = std::fs::write(&p, "");
                                Ok(String::new())
                            };
                            let _ = tx.send((result, p));
                        });
                        rx.await.unwrap_or_else(|_| (Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "Background thread panicked",
                        )), path))
                    },
                    |(result, path)| match result {
                        Ok(content) => Message::ClaudeMdLoaded { content, path },
                        Err(e) => Message::ClaudeMdLoadFailed(e.to_string()),
                    },
                );
            }
            Message::ClaudeMdLoaded { content, path } => {
                self.claude_md_editor =
                    Some(crate::claude_md_editor::ClaudeMdEditorState::new(&content, path));
            }
            Message::ClaudeMdLoadFailed(err) => {
                self.enqueue_toast("CLAUDE.md Error".into(), err);
            }
            Message::ClaudeMdEditorAction(action) => {
                if let Some(ref mut state) = self.claude_md_editor {
                    state.content.perform(action);
                    state.dirty = true;
                }
            }
            Message::ClaudeMdSave => {
                if let Some(ref state) = self.claude_md_editor {
                    let text = state.text();
                    let path = state.file_path.clone();
                    return Task::perform(
                        async move {
                            let (tx, rx) = futures_channel::oneshot::channel();
                            std::thread::spawn(move || {
                                let result = std::fs::write(&path, &text)
                                    .map_err(|e| e.to_string());
                                let _ = tx.send(result);
                            });
                            rx.await.unwrap_or_else(|_| Err("Background thread panicked".into()))
                        },
                        Message::ClaudeMdSaved,
                    );
                }
            }
            Message::ClaudeMdSaved(result) => {
                match result {
                    Ok(()) => {
                        if let Some(ref mut state) = self.claude_md_editor {
                            state.dirty = false;
                        }
                        self.enqueue_toast("Saved".into(), "CLAUDE.md saved successfully".into());
                    }
                    Err(e) => {
                        self.enqueue_toast("Save Failed".into(), e);
                    }
                }
            }
            Message::ClaudeMdClose => {
                self.claude_md_editor = None;
            }
            // --- I4/I5: Voice/Whisper Integration ---
            Message::WhisperToggle => {
                if !self.whisper_available {
                    return Task::none();
                }
                if self.whisper_state.is_some() {
                    return self.whisper_stop();
                }
                return self.whisper_start();
            }
            Message::WhisperStarted => {
                if let Some(ref mut state) = self.whisper_state {
                    state.recording = true;
                }
            }
            Message::WhisperStopped(result) => {
                self.whisper_state = None;
                self.whisper_service = None;
                match result {
                    Ok((text, duration_ms)) => {
                        if !text.is_empty() {
                            if let (Some(client), Some(session_id)) =
                                (&self.client, self.active_focused().map(str::to_string))
                            {
                                let _ = commands::write_to_terminal(client, &session_id, text.as_bytes());
                            }
                            let secs = duration_ms / 1000;
                            self.enqueue_toast(
                                "Voice Input".to_string(),
                                format!("Transcribed {secs}s of audio"),
                            );
                        }
                    }
                    Err(e) => {
                        self.enqueue_toast("Whisper Error".to_string(), e);
                    }
                }
            }
            Message::WhisperLevelUpdate(level) => {
                if let Some(ref mut state) = self.whisper_state {
                    state.level = level;
                }
            }
            Message::WhisperTimerTick => {
                if let Some(ref mut state) = self.whisper_state {
                    state.elapsed_ms = state.elapsed_ms.saturating_add(100);
                }
                if let Some(ref slot) = self.whisper_service {
                    let slot = Arc::clone(slot);
                    return Task::perform(
                        async move {
                            let mut guard = slot.lock();
                            guard.as_mut().map(|svc| svc.get_level().unwrap_or(0.0)).unwrap_or(0.0)
                        },
                        Message::WhisperLevelUpdate,
                    );
                }
            }
            Message::WhisperCancel => {
                if let Some(ref slot) = self.whisper_service {
                    if let Some(svc) = slot.lock().as_mut() {
                        svc.kill();
                    }
                }
                self.whisper_service = None;
                self.whisper_state = None;
            }
            // --- Screenshot captured (async MCP reply) ---
            Message::ScreenshotCaptured(screenshot) => {
                if let Some(reply) = self.pending_screenshot_reply.take() {
                    let response = Self::encode_screenshot_to_file(&screenshot);
                    let _ = reply.send(response);
                }
            }
            // --- J1-J9: MCP Event Integration ---
            Message::McpEvent(event) => {
                return self.handle_mcp_event(event);
            }
        }
        Task::none()
    }

    /// Encode an iced Screenshot as PNG and save to a temp file.
    fn encode_screenshot_to_file(screenshot: &iced::window::Screenshot) -> McpResponse {
        let size = screenshot.size;
        let bytes = &screenshot.rgba;

        let temp_dir = std::env::temp_dir().join("godly-screenshots");
        if let Err(e) = std::fs::create_dir_all(&temp_dir) {
            return McpResponse::Error {
                message: format!("Failed to create screenshot dir: {}", e),
            };
        }

        // Clean up screenshots older than 1 hour (best-effort)
        let one_hour = std::time::Duration::from_secs(3600);
        if let Ok(entries) = std::fs::read_dir(&temp_dir) {
            let now = std::time::SystemTime::now();
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if now.duration_since(modified).unwrap_or_default() > one_hour {
                            let _ = std::fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }

        let id = uuid::Uuid::new_v4();
        let path = temp_dir.join(format!("screenshot-{}.png", &id.to_string()[..8]));

        let file = match std::fs::File::create(&path) {
            Ok(f) => f,
            Err(e) => {
                return McpResponse::Error {
                    message: format!("Failed to create screenshot file: {}", e),
                };
            }
        };
        let writer = std::io::BufWriter::new(file);
        let mut encoder = png::Encoder::new(writer, size.width, size.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        match encoder.write_header() {
            Ok(mut png_writer) => {
                if let Err(e) = png_writer.write_image_data(bytes) {
                    return McpResponse::Error {
                        message: format!("Failed to write PNG data: {}", e),
                    };
                }
            }
            Err(e) => {
                return McpResponse::Error {
                    message: format!("Failed to write PNG header: {}", e),
                };
            }
        }

        McpResponse::Screenshot {
            path: path.to_string_lossy().to_string(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        diag::log(&format!("VIEW: focused={} terminals={}", self.window_focused, self.terminals.count()));
        if let Some(ref err) = self.init_error {
            return center(text(format!("Initialization error: {}", err)).size(18)).into();
        }

        if self.client.is_none() && self.terminals.count() == 0 {
            return center(text("Connecting to daemon...").size(18)).into();
        }

        // Custom title bar — spans full window width above sidebar + main.
        let title = self.title();
        let title_bar_row = title_bar::view_title_bar(
            title,
            UI_FONT,
            Message::TitleBarDragStart,
            Message::TitleBarMinimize,
            Message::TitleBarToggleMaximize,
            Message::TitleBarClose,
        );

        // Tab bar — show terminals for the active workspace.
        let active_id = self.active_focused();
        let ordered = self.active_workspace_terminals();
        let active_workspace_terminal_count = ordered.len();
        let entry_progress = tab_bar::tab_entry_progress(&self.entering_tabs, Self::now_ms());
        let tab_bar = tab_bar::view_tab_bar(
            &ordered,
            active_id,
            &entry_progress,
            UI_FONT,
            |id| Message::TabClicked(id),
            |id| Message::CloseTabRequested(id),
            |id| Message::TabDragStart(id),
            |id| Message::TabDragHover(id),
            |id| Message::TabContextToggle(id),
            Message::TabDragEnd,
            Message::NewTabRequested,
        );

        // Mic button for whisper (I4/I5)
        let tab_bar: Element<'_, Message> = if self.whisper_available {
            let mic = whisper_ui::view_mic_button(Message::WhisperToggle);
            row![
                container(tab_bar).width(Length::Fill),
                container(mic)
                    .padding(Padding::from([0, 4]))
                    .height(Length::Fixed(TAB_BAR_HEIGHT))
            ]
            .align_y(iced::Alignment::Center)
            .height(Length::Fixed(TAB_BAR_HEIGHT))
            .into()
        } else {
            tab_bar
        };

        // Render the layout tree from active workspace.
        let focused_id = self.active_focused();
        let terminal_view: Element<'_, Message> = if let Some(_empty_state) =
            self.terminal_empty_state(active_workspace_terminal_count)
        {
            self.view_terminal_empty_state()
        } else if let Some(layout) = self.active_layout() {
            view_layout(layout, &|terminal_id: &str| {
                self.render_terminal_leaf_with_drop_overlay(terminal_id, focused_id)
            })
        } else {
            self.view_terminal_empty_state()
        };

        let status_info = focused_id
            .and_then(|tid| self.terminals.get(tid))
            .map(|term| status_bar::StatusBarInfo {
                shell_label: status_bar::shell_label(&term.process_name),
                cwd: term.extract_cwd().unwrap_or(""),
                cols: term.cols,
                rows: term.rows,
                font: UI_FONT,
            });
        let status_bar_el: Element<'_, Message> = status_bar::view_status_bar(status_info);

        let main_area = column![
            tab_bar,
            container(terminal_view).height(Length::Fill),
            status_bar_el,
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        let sidebar_width = self.current_sidebar_width();
        let body_content: Element<'_, Message> = if sidebar_width > 0.0 {
            let notified_workspace_ids = self.notified_workspace_ids();
            let sidebar = sidebar::view_sidebar(
                self.workspaces.as_slice(),
                self.workspaces.active_id(),
                (
                    self.workspace_context_menu_id.as_deref(),
                    move |workspace_id: &str| {
                        notified_workspace_ids
                            .iter()
                            .any(|id| id.as_str() == workspace_id)
                    },
                    |workspace_id: &str| -> Option<&'static str> {
                        match self.workspace_ai_modes.get(workspace_id) {
                            Some(AiToolMode::Claude) => Some("[C]"),
                            Some(AiToolMode::Codex) => Some("[X]"),
                            Some(AiToolMode::Both) => Some("[CX]"),
                            _ => None,
                        }
                    },
                ),
                sidebar_width,
                self.terminal_font,
                Message::SidebarAction,
                Message::SidebarResizeStart,
                Message::SidebarResizeEnd,
            );
            row![sidebar, main_area]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            main_area.into()
        };

        let main_content: Element<'_, Message> = column![title_bar_row, body_content]
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        // Overlay settings dialog if open.
        let with_settings: Element<'_, Message> = if self.settings_open {
            let tabs = &[
                SettingsTab {
                    id: "appearance",
                    label: "Appearance",
                },
                SettingsTab {
                    id: "shortcuts",
                    label: "Shortcuts",
                },
                SettingsTab {
                    id: "notifications",
                    label: "Notifications",
                },
                SettingsTab {
                    id: "quick-claude",
                    label: "Quick Prompt",
                },
                SettingsTab {
                    id: "ai-tools",
                    label: "AI Tools",
                },
                SettingsTab {
                    id: "plugins",
                    label: "Plugins",
                },
                SettingsTab {
                    id: "flows",
                    label: "Flows",
                },
                SettingsTab {
                    id: "remote",
                    label: "Remote",
                },
            ];
            let tab_content = match self.settings_tab.as_str() {
                "appearance" => self.view_appearance_tab(),
                "notifications" => self.view_notifications_tab(),
                "quick-claude" => self.view_quick_claude_tab(),
                "ai-tools" => self.view_ai_tools_tab(),
                "plugins" => self.view_plugins_tab(),
                "flows" => self.view_flows_tab(),
                "remote" => self.view_remote_tab(),
                _ => shortcuts_tab::view_shortcuts_tab(
                    self.shortcut_capturing_index,
                    &self.shortcut_overrides,
                    |idx| Message::ShortcutBadgeClicked(idx),
                ),
            };
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
        };

        let with_toasts: Element<'_, Message> = if self.toasts.is_empty() {
            with_settings
        } else {
            stack![with_settings, self.view_toast_overlay()].into()
        };

        let with_tab_context: Element<'_, Message> = if self.tab_context_menu_id.is_some() {
            stack![with_toasts, self.view_tab_context_menu()].into()
        } else {
            with_toasts
        };

        let with_mru_switcher: Element<'_, Message> = if self.mru_switcher.is_some() {
            stack![with_tab_context, self.view_mru_switcher_overlay()].into()
        } else {
            with_tab_context
        };

        let with_workspace_rename: Element<'_, Message> = if self.rename_workspace_id.is_some() {
            stack![with_mru_switcher, self.view_workspace_rename_dialog()].into()
        } else {
            with_mru_switcher
        };

        let with_tab_rename: Element<'_, Message> = if self.rename_tab_id.is_some() {
            stack![with_workspace_rename, self.view_tab_rename_dialog()].into()
        } else {
            with_workspace_rename
        };

        // --- K2/K3: Quit Confirmation + Copy Preview ---
        let with_quit: Element<'_, Message> = if self.quit_confirm_pending {
            let terminal_count = self.terminals.count();
            stack![
                with_tab_rename,
                crate::confirm_dialog::view_quit_confirm(
                    terminal_count,
                    Message::QuitConfirmed,
                    Message::QuitCancelled,
                )
            ]
            .into()
        } else {
            with_tab_rename
        };

        let with_worktree_close: Element<'_, Message> = if let Some(ref pending_id) = self.worktree_close_pending {
            let worktree_path = self
                .terminals
                .get(pending_id)
                .and_then(|t| t.worktree_path.as_deref())
                .unwrap_or("unknown");
            let pid1 = pending_id.clone();
            let pid2 = pending_id.clone();
            stack![
                with_quit,
                crate::confirm_dialog::view_worktree_close_confirm(
                    worktree_path,
                    Message::WorktreeCloseConfirmed { session_id: pid1 },
                    Message::WorktreeCloseKeep { session_id: pid2 },
                    Message::WorktreeCloseCancelled,
                )
            ]
            .into()
        } else {
            with_quit
        };

        let with_copy_preview: Element<'_, Message> = if let Some(ref preview_text) = self.copy_preview_text {
            stack![
                with_worktree_close,
                crate::confirm_dialog::view_copy_preview(
                    preview_text,
                    preview_text.len(),
                    Message::CopyPreviewConfirmed,
                    Message::CopyPreviewDismissed,
                )
            ]
            .into()
        } else {
            with_worktree_close
        };

        // Shell picker overlay (H1-H6)
        let with_shell_picker: Element<'_, Message> = if self.shell_picker.visible {
            let picker = shell_picker::view_shell_picker(
                &self.shell_picker,
                Message::ShellPickerTabClicked,
                Message::ShellPickerDistroSelected,
                Message::ShellPickerCustomProgramChanged,
                Message::ShellPickerCustomArgsChanged,
                Message::ShellPickerConfirmed,
                Message::ShellPickerCancelled,
            );
            stack![with_copy_preview, picker].into()
        } else {
            with_copy_preview
        };

        // --- Quick Claude Dialog overlay ---
        let with_quick_claude: Element<'_, Message> = if let Some(ref dlg_state) = self.quick_claude_dialog {
            let qc_overlay = crate::quick_claude_dialog::view_quick_claude_dialog(
                dlg_state,
                Message::QuickClaudeDialogWorkspaceSelected,
                Message::QuickClaudeDialogWorkspaceDropdownToggle,
                Message::QuickClaudeDialogAiToolSelected,
                Message::QuickClaudeDialogAiToolDropdownToggle,
                Message::QuickClaudeDialogPromptAction,
                Message::QuickClaudeDialogBranchChanged,
                Message::QuickClaudeDialogMainBranchToggled,
                Message::QuickClaudeDialogAutoSuggestToggled,
                Message::QuickClaudeDialogModelSelected,
                Message::QuickClaudeDialogModelDropdownToggle,
                Message::QuickClaudeDialogModeSelected,
                Message::QuickClaudeDialogModeDropdownToggle,
                Message::QuickClaudeDialogLaunch,
                Message::QuickClaudeDialogVoice,
                Message::QuickClaudeDialogClose,
                Message::QuickClaudeDialogSkillSelected,
                Message::QuickClaudeDialogSkillAutocompleteNavigate,
                Message::QuickClaudeDialogSkillAutocompleteDismiss,
                |tab| Message::QuickClaudeDialogTabSelected(tab),
                Message::QuickClaudeDialogSessionSelected,
                Message::QuickClaudeDialogResume,
            );
            stack![with_shell_picker, qc_overlay].into()
        } else {
            with_shell_picker
        };

        // --- K1: CLAUDE.md Editor overlay ---
        let with_claude_md: Element<'_, Message> = if let Some(ref editor_state) = self.claude_md_editor {
            let editor_overlay = crate::claude_md_editor::view_claude_md_editor(
                editor_state,
                Message::ClaudeMdEditorAction,
                Message::ClaudeMdSave,
                Message::ClaudeMdClose,
            );
            stack![with_quick_claude, editor_overlay].into()
        } else {
            with_quick_claude
        };

        // --- G1/G2: Terminal Context Menu overlay ---
        let with_ctx_menu: Element<'_, Message> = if let Some((x, y)) = self.terminal_context_menu_pos {
            let ctx_menu = terminal_context_menu::view_terminal_context_menu(
                x,
                y,
                |action| Message::TerminalContextAction(action),
                Message::TerminalContextClose,
            );
            stack![with_claude_md, ctx_menu].into()
        } else {
            with_claude_md
        };

        // --- G4: Search bar overlay (top-right, non-blocking) ---
        let with_search: Element<'_, Message> = if self.search.active {
            let search_bar = crate::search::view_search_bar(
                &self.search,
                |q| Message::SearchQueryChanged(q),
                Message::SearchNext,
                Message::SearchPrev,
                Message::SearchClose,
                Message::SearchToggleRegex,
            );
            let positioned = container(search_bar)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Right)
                .padding(Padding::from([4, 8]));
            stack![with_ctx_menu, positioned].into()
        } else {
            with_ctx_menu
        };

        // --- G7: Performance overlay (top-right corner) ---
        let with_perf: Element<'_, Message> = if self.perf_overlay_visible {
            let perf: Element<'_, Message> = crate::perf_overlay::view_perf_overlay(
                60.0, // placeholder FPS
                16.6, // placeholder frame_ms
                self.terminals.count(),
            );
            let positioned = container(perf)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Right)
                .padding(Padding::from([30, 8]));
            stack![with_search, positioned].into()
        } else {
            with_search
        };

        // --- I4/I5: Whisper recording overlay ---
        if let Some(ref state) = self.whisper_state {
            let overlay = whisper_ui::view_whisper_overlay(
                state,
                Message::WhisperToggle,
                Message::WhisperCancel,
            );
            stack![with_perf, overlay].into()
        } else {
            with_perf
        }
    }

    fn view_tab_context_menu(&self) -> Element<'_, Message> {
        let Some(tab_id) = self.tab_context_menu_id.as_deref() else {
            return Space::new().into();
        };
        let Some(term) = self.terminals.get(tab_id) else {
            return Space::new().into();
        };

        let title = text(format!("Tab Actions: {}", term.tab_label()))
            .size(14)
            .color(TEXT_ACTIVE());
        let rename_id = tab_id.to_string();
        let split_right_id = tab_id.to_string();
        let split_down_id = tab_id.to_string();
        let copy_info_id = tab_id.to_string();
        let close_id = tab_id.to_string();

        let action_btn = |label: &'static str, msg: Message| {
            button(text(label).size(12).color(TEXT_PRIMARY()))
                .on_press(msg)
                .padding(Padding::from([5, 8]))
                .width(Length::Fill)
                .style(|_theme, status| {
                    let bg = match status {
                        button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                        _ => iced::Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        text_color: TEXT_PRIMARY(),
                        border: iced::Border::default(),
                        ..button::Style::default()
                    }
                })
        };

        let menu = container(
            column![
                title,
                action_btn("Rename", Message::TabContextRename(rename_id)),
                action_btn(
                    "Split Right",
                    Message::TabContextSplit {
                        terminal_id: split_right_id,
                        direction: SplitDirection::Horizontal
                    }
                ),
                action_btn(
                    "Split Down",
                    Message::TabContextSplit {
                        terminal_id: split_down_id,
                        direction: SplitDirection::Vertical
                    }
                ),
                action_btn("Copy Info", Message::TabContextCopyInfo(copy_info_id)),
                action_btn("Close", Message::TabContextClose(close_id)),
                action_btn("Dismiss", Message::TabContextToggle(tab_id.to_string())),
            ]
            .spacing(4),
        )
        .padding(Padding::from([10, 10]))
        .width(Length::Fixed(260.0))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(BG_SECONDARY())),
            border: iced::Border {
                color: BORDER(),
                width: 1.0,
                radius: RADIUS_MD.into(),
            },
            shadow: Shadow {
                color: SHADOW_COLOR,
                offset: Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..container::Style::default()
        });

        container(center(menu))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(BACKDROP())),
                ..container::Style::default()
            })
            .into()
    }

    fn view_tab_rename_dialog(&self) -> Element<'_, Message> {
        let dialog = container(column![
            text("Rename Tab").size(16).color(TEXT_ACTIVE()),
            text_input(
                "Tab name (empty clears custom name)",
                &self.rename_tab_value
            )
            .on_input(Message::TabRenameInputChanged)
            .on_submit(Message::TabRenameSubmitted)
            .padding(Padding::from([6, 8]))
            .size(14),
            row![
                Space::new().width(Length::Fill),
                button(text("Cancel").size(12).color(TEXT_PRIMARY()))
                    .on_press(Message::TabRenameCancelled)
                    .padding(Padding::from([4, 9]))
                    .style(|_theme, status| {
                        let bg = match status {
                            button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                            _ => iced::Color::TRANSPARENT,
                        };
                        button::Style {
                            background: Some(iced::Background::Color(bg)),
                            text_color: TEXT_PRIMARY(),
                            border: iced::Border {
                                color: BORDER(),
                                width: 1.0,
                                radius: 4.0.into(),
                            },
                            ..button::Style::default()
                        }
                    }),
                button(text("Rename").size(12).color(TEXT_ACTIVE()))
                    .on_press(Message::TabRenameSubmitted)
                    .padding(Padding::from([4, 9]))
                    .style(|_theme, status| {
                        let bg = match status {
                            button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                            _ => BG_SECONDARY(),
                        };
                        button::Style {
                            background: Some(iced::Background::Color(bg)),
                            text_color: TEXT_ACTIVE(),
                            border: iced::Border {
                                color: BORDER(),
                                width: 1.0,
                                radius: 4.0.into(),
                            },
                            ..button::Style::default()
                        }
                    }),
            ]
            .spacing(8),
        ])
        .padding(Padding::from([14, 16]))
        .width(Length::Fixed(360.0))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(BG_SECONDARY())),
            border: iced::Border {
                color: BORDER(),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        });

        container(center(dialog))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(BACKDROP())),
                ..container::Style::default()
            })
            .into()
    }

    fn view_mru_switcher_overlay(&self) -> Element<'_, Message> {
        let Some(selected_terminal_id) = self
            .mru_switcher
            .as_ref()
            .map(|state| state.selected_terminal_id.as_str())
        else {
            return Space::new().into();
        };

        let entries: Vec<mru_switcher::MruSwitcherEntry> = self
            .active_workspace_mru_terminal_ids()
            .into_iter()
            .filter_map(|terminal_id| {
                self.terminals.get(terminal_id).map(|terminal| {
                    let detail =
                        (!terminal.process_name.is_empty()).then(|| terminal.process_name.clone());
                    mru_switcher::MruSwitcherEntry {
                        terminal_id: terminal.id.clone(),
                        label: terminal.tab_label().to_string(),
                        detail,
                    }
                })
            })
            .collect();
        if entries.is_empty() {
            return Space::new().into();
        }

        mru_switcher::view_overlay(entries, Some(selected_terminal_id))
    }

    fn active_workspace_mru_terminal_ids(&self) -> Vec<&str> {
        let Some(workspace_id) = self.workspaces.active_id() else {
            return Vec::new();
        };

        self.terminals
            .mru_terminal_ids_for_workspace(Some(workspace_id))
    }

    /// Render the Appearance tab (theme selection grid with preview swatches).
    fn view_appearance_tab(&self) -> Element<'_, Message> {
        use crate::theme::{ThemeId, RADIUS_MD};
        use iced::widget::{button, row, scrollable, text, text_input, Space};
        use iced::Theme;

        // -- Font section --
        let font_header = column![
            text("Font").size(16).color(TEXT_PRIMARY()),
            text("Choose a monospace font for the terminal.")
                .size(13)
                .color(TEXT_SECONDARY()),
        ]
        .spacing(4);

        // Font size stepper row
        let font_size_val = self.font_metrics.font_size;
        let minus_btn = button(text("\u{2212}").size(14).color(TEXT_PRIMARY()))
            .on_press(Message::FontSizeDecrement)
            .padding(Padding::from([4, 10]))
            .style(move |_t: &Theme, _s| button::Style {
                background: Some(iced::Background::Color(BG_TERTIARY())),
                border: iced::Border {
                    color: BORDER(),
                    width: 1.0,
                    radius: RADIUS_MD.into(),
                },
                text_color: TEXT_PRIMARY(),
                ..Default::default()
            });
        let plus_btn = button(text("+").size(14).color(TEXT_PRIMARY()))
            .on_press(Message::FontSizeIncrement)
            .padding(Padding::from([4, 10]))
            .style(move |_t: &Theme, _s| button::Style {
                background: Some(iced::Background::Color(BG_TERTIARY())),
                border: iced::Border {
                    color: BORDER(),
                    width: 1.0,
                    radius: RADIUS_MD.into(),
                },
                text_color: TEXT_PRIMARY(),
                ..Default::default()
            });
        let size_label = text(format!("{:.0}", font_size_val))
            .size(14)
            .color(TEXT_PRIMARY());
        let font_size_row = row![
            text("Size").size(13).color(TEXT_SECONDARY()),
            Space::new().width(Length::Fill),
            minus_btn,
            container(size_label).padding(Padding::from([4, 8])),
            plus_btn,
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);

        // Font search input
        let search_input = text_input("Search fonts...", &self.font_filter_query)
            .on_input(Message::FontFilterChanged)
            .size(13)
            .padding(Padding::from([6, 10]));

        // Font list
        let mut font_list_col = column![].spacing(4);
        if let Some(fonts) = &self.available_fonts {
            let query_lower = self.font_filter_query.to_lowercase();
            for font_name in fonts {
                if !query_lower.is_empty() && !font_name.to_lowercase().contains(&query_lower) {
                    continue;
                }
                let is_active = self.font_family == *font_name;
                let name_clone = font_name.clone();
                let interned = font_enumerator::intern_font_name(font_name);

                let preview_font = iced::Font {
                    family: iced::font::Family::Name(interned),
                    weight: iced::font::Weight::Normal,
                    stretch: iced::font::Stretch::Normal,
                    style: iced::font::Style::Normal,
                };

                let label = text(font_name.as_str())
                    .size(13)
                    .font(preview_font)
                    .color(if is_active { TEXT_ACTIVE() } else { TEXT_PRIMARY() });

                let font_label_row: Element<'_, Message> = if is_active {
                    row![
                        text("\u{2713}").size(13).color(ACCENT()),
                        label,
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center)
                    .into()
                } else {
                    label.into()
                };

                let border_color = if is_active { ACCENT() } else { BORDER() };
                let bg_color = if is_active { BG_TERTIARY() } else { BG_SECONDARY() };

                let font_btn = button(
                    container(font_label_row).padding(Padding::from([6, 10])),
                )
                .on_press(Message::FontFamilyChanged(name_clone))
                .padding(0)
                .width(Length::Fill)
                .style(move |_t: &Theme, _s| button::Style {
                    background: Some(iced::Background::Color(bg_color)),
                    border: iced::Border {
                        color: border_color,
                        width: if is_active { 2.0 } else { 1.0 },
                        radius: RADIUS_MD.into(),
                    },
                    text_color: TEXT_PRIMARY(),
                    ..Default::default()
                });

                font_list_col = font_list_col.push(font_btn);
            }
        } else {
            font_list_col = font_list_col.push(
                text("Loading fonts...").size(13).color(TEXT_SECONDARY()),
            );
        }

        let font_list_scrollable = scrollable(font_list_col)
            .height(Length::Fixed(200.0));

        // Font preview area — shows sample text in the selected font & size
        let preview_font = self.terminal_font;
        let preview_size = self.font_metrics.font_size;

        // Separator between font and theme sections
        let separator = container(Space::new().width(Length::Fill).height(1))
            .style(move |_t: &Theme| container::Style {
                background: Some(iced::Background::Color(BORDER())),
                ..Default::default()
            });

        let has_custom_active = self.active_custom_theme_id.is_some();

        let all_themes = ThemeId::all();
        let mut grid_children: Vec<Element<'_, Message>> = Vec::new();

        for &theme_id in all_themes {
            let colors = theme_id.preview_colors();
            let is_active = !has_custom_active && self.active_theme == theme_id;

            let mut swatches = row![].spacing(2);
            for c in &colors {
                let color_val = *c;
                swatches = swatches.push(
                    container(Space::new().width(16).height(16))
                        .style(move |_t: &Theme| container::Style {
                            background: Some(iced::Background::Color(color_val)),
                            border: iced::Border {
                                radius: 2.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                );
            }

            let label = text(theme_id.label())
                .size(13)
                .color(if is_active { TEXT_ACTIVE() } else { TEXT_PRIMARY() });

            let btn_content = column![label, swatches].spacing(4).padding(8);

            let border_color = if is_active { ACCENT() } else { BORDER() };
            let bg_color = if is_active { BG_TERTIARY() } else { BG_SECONDARY() };

            let theme_button = button(btn_content)
                .on_press(Message::ThemeChanged(theme_id))
                .padding(0)
                .style(move |_t: &Theme, _status| button::Style {
                    background: Some(iced::Background::Color(bg_color)),
                    border: iced::Border {
                        color: border_color,
                        width: if is_active { 2.0 } else { 1.0 },
                        radius: RADIUS_MD.into(),
                    },
                    text_color: TEXT_PRIMARY(),
                    ..Default::default()
                });

            grid_children.push(theme_button.width(Length::FillPortion(1)).into());
        }

        let mut rows: Vec<Element<'_, Message>> = Vec::new();
        for chunk in grid_children.chunks_mut(3) {
            let mut r = row![].spacing(8);
            for child in chunk.iter_mut() {
                let taken = std::mem::replace(child, Space::new().width(0).height(0).into());
                r = r.push(taken);
            }
            rows.push(r.into());
        }

        let mut col = column![
            font_header,
            font_size_row,
            search_input,
            font_list_scrollable,
            Space::new().height(4),
            text("Preview").size(13).color(TEXT_SECONDARY()),
            container(
                column![
                    text("$ echo \"Hello, World!\"").font(preview_font).size(preview_size).color(TEXT_PRIMARY()),
                    text("Hello, World!").font(preview_font).size(preview_size).color(ACCENT()),
                    text("$ git status").font(preview_font).size(preview_size).color(TEXT_PRIMARY()),
                    text("On branch main").font(preview_font).size(preview_size).color(TEXT_SECONDARY()),
                    text("ABCDEFGHIJKLM 0123456789 {}[]()").font(preview_font).size(preview_size).color(TEXT_PRIMARY()),
                ]
                .spacing(2)
            )
            .width(Length::Fill)
            .padding(Padding::from([12, 16]))
            .style(move |_t: &Theme| container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.85))),
                border: iced::Border {
                    color: BORDER(),
                    width: 1.0,
                    radius: RADIUS_MD.into(),
                },
                ..Default::default()
            }),
            Space::new().height(8),
            separator,
            Space::new().height(12),
            text("Theme").size(16).color(TEXT_PRIMARY()),
            text("Choose a color theme for the terminal and UI.")
                .size(13)
                .color(TEXT_SECONDARY()),
        ]
        .spacing(8)
        .padding(16);

        for r in rows {
            col = col.push(r);
        }

        // --- Custom themes section ---
        if !self.custom_themes.is_empty() {
            col = col.push(Space::new().height(12));
            col = col.push(text("Custom Themes").size(14).color(TEXT_PRIMARY()));

            let mut custom_grid: Vec<Element<'_, Message>> = Vec::new();
            for ct in &self.custom_themes {
                let colors = ct.preview_colors();
                let is_active = self.active_custom_theme_id.as_deref() == Some(&ct.id);
                let ct_id = ct.id.clone();
                let ct_id_export = ct.id.clone();
                let ct_id_delete = ct.id.clone();

                let mut swatches = row![].spacing(2);
                for c in &colors {
                    let cv = *c;
                    swatches = swatches.push(
                        container(Space::new().width(16).height(16))
                            .style(move |_t: &Theme| container::Style {
                                background: Some(iced::Background::Color(cv)),
                                border: iced::Border {
                                    radius: 2.0.into(),
                                    ..Default::default()
                                },
                                ..Default::default()
                            }),
                    );
                }

                let label = text(ct.name.as_str())
                    .size(13)
                    .color(if is_active { TEXT_ACTIVE() } else { TEXT_PRIMARY() });

                let action_btns = row![
                    button(text("Export").size(11))
                        .on_press(Message::ThemeExportRequested(ct_id_export))
                        .padding(Padding::from([2, 6]))
                        .style(move |_t: &Theme, _s| button::Style {
                            background: Some(iced::Background::Color(BG_TERTIARY())),
                            text_color: TEXT_SECONDARY(),
                            border: iced::Border {
                                radius: 3.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                    button(text("Delete").size(11))
                        .on_press(Message::ThemeDeleteCustom(ct_id_delete))
                        .padding(Padding::from([2, 6]))
                        .style(move |_t: &Theme, _s| button::Style {
                            background: Some(iced::Background::Color(BG_TERTIARY())),
                            text_color: DANGER(),
                            border: iced::Border {
                                radius: 3.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                ]
                .spacing(4);

                let btn_content = column![label, swatches, action_btns].spacing(4).padding(8);
                let border_color = if is_active { ACCENT() } else { BORDER() };
                let bg_color = if is_active { BG_TERTIARY() } else { BG_SECONDARY() };

                let theme_button = button(btn_content)
                    .on_press(Message::ThemeSelectCustom(ct_id))
                    .padding(0)
                    .style(move |_t: &Theme, _status| button::Style {
                        background: Some(iced::Background::Color(bg_color)),
                        border: iced::Border {
                            color: border_color,
                            width: if is_active { 2.0 } else { 1.0 },
                            radius: RADIUS_MD.into(),
                        },
                        text_color: TEXT_PRIMARY(),
                        ..Default::default()
                    });

                custom_grid.push(theme_button.width(Length::FillPortion(1)).into());
            }

            let mut custom_rows: Vec<Element<'_, Message>> = Vec::new();
            for chunk in custom_grid.chunks_mut(3) {
                let mut r = row![].spacing(8);
                for child in chunk.iter_mut() {
                    let taken = std::mem::replace(child, Space::new().width(0).height(0).into());
                    r = r.push(taken);
                }
                custom_rows.push(r.into());
            }
            for r in custom_rows {
                col = col.push(r);
            }
        }

        // --- Import button ---
        col = col.push(Space::new().height(12));
        let import_btn = button(
            text("Import Theme...").size(13).color(TEXT_PRIMARY()),
        )
        .on_press(Message::ThemeImportRequested)
        .padding(Padding::from([8, 16]))
        .style(move |_t: &Theme, _s| button::Style {
            background: Some(iced::Background::Color(BG_TERTIARY())),
            border: iced::Border {
                color: BORDER(),
                width: 1.0,
                radius: RADIUS_MD.into(),
            },
            text_color: TEXT_PRIMARY(),
            ..Default::default()
        });
        col = col.push(import_btn);

        scrollable(col).height(Length::Fill).into()
    }

    fn view_toast_overlay(&self) -> Element<'_, Message> {
        let mut toasts_column = column![].spacing(8).width(Length::Fixed(320.0));
        for toast in &self.toasts {
            let accent_border = container(Space::new().width(3.0).height(Length::Fill))
                .style(|_theme| container::Style {
                    background: Some(iced::Background::Color(ACCENT())),
                    ..container::Style::default()
                });
            let card_content = column![
                text(&toast.title).size(13).color(TEXT_ACTIVE()),
                text(&toast.message).size(11).color(TEXT_SECONDARY()),
            ]
            .spacing(2);
            let card = container(
                row![accent_border, container(card_content).padding(Padding::from([10, 14]))]
                    .spacing(0),
            )
            .width(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(BG_SECONDARY())),
                border: iced::Border {
                    color: Color::from_rgba(BORDER().r, BORDER().g, BORDER().b, 0.6),
                    width: 0.5,
                    radius: RADIUS_MD.into(),
                },
                shadow: Shadow {
                    color: SHADOW_ACCENT(),
                    offset: Vector::new(0.0, 3.0),
                    blur_radius: 10.0,
                },
                ..container::Style::default()
            });
            toasts_column = toasts_column.push(card);
        }

        container(row![Space::new().width(Length::Fill), toasts_column])
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding::from([14, 14]))
            .into()
    }

    fn view_notifications_tab(&self) -> Element<'_, Message> {
        let toggle_label = if self.notification_sounds_enabled {
            "Disable Sounds"
        } else {
            "Enable Sounds"
        };

        let mut preset_row = row![].spacing(8);
        for preset in NotificationSoundPreset::all() {
            let is_active = self.notification_sound_preset == preset;
            let bg = if is_active { BG_TERTIARY() } else { BG_SECONDARY() };
            let fg = if is_active { TEXT_ACTIVE() } else { TEXT_PRIMARY() };

            let btn = button(text(preset.label()).size(12).color(fg))
                .on_press(Message::NotificationSoundPresetSelected(preset))
                .padding(Padding::from([4, 9]))
                .style(move |_theme, status| {
                    let hover_bg = match status {
                        button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                        _ => bg,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(hover_bg)),
                        text_color: fg,
                        border: iced::Border {
                            color: BORDER(),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..button::Style::default()
                    }
                });
            preset_row = preset_row.push(btn);
        }

        let add_pattern_row = row![
            text_input(
                "Mute pattern (e.g. work-* or w-default)",
                &self.workspace_mute_pattern_input,
            )
            .on_input(Message::WorkspaceMutePatternInputChanged)
            .on_submit(Message::AddWorkspaceMutePattern)
            .padding(Padding::from([4, 8]))
            .size(12)
            .width(Length::Fill),
            button(text("Add").size(12).color(TEXT_PRIMARY()))
                .on_press(Message::AddWorkspaceMutePattern)
                .padding(Padding::from([4, 9]))
        ]
        .spacing(8)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center);

        let mut mute_pattern_list = column![].spacing(6).width(Length::Fill);
        if self.workspace_mute_patterns.is_empty() {
            mute_pattern_list = mute_pattern_list.push(
                text("No workspace mute patterns configured.")
                    .size(11)
                    .color(TEXT_SECONDARY()),
            );
        } else {
            for pattern in &self.workspace_mute_patterns {
                let remove_pattern = pattern.clone();
                let row = row![
                    text(pattern).size(12).color(TEXT_PRIMARY()),
                    Space::new().width(Length::Fill),
                    button(text("Remove").size(11).color(TEXT_PRIMARY()))
                        .on_press(Message::RemoveWorkspaceMutePattern(remove_pattern))
                        .padding(Padding::from([2, 7]))
                ]
                .spacing(8)
                .width(Length::Fill)
                .align_y(iced::Alignment::Center);
                mute_pattern_list = mute_pattern_list.push(row);
            }
        }

        container(
            column![
                text("Notification Sounds").size(14).color(TEXT_ACTIVE()),
                text("Choose a sound preset for bell events.")
                    .size(12)
                    .color(TEXT_PRIMARY()),
                button(text(toggle_label).size(12).color(TEXT_PRIMARY()))
                    .on_press(Message::NotificationSoundsToggled)
                    .padding(Padding::from([4, 9])),
                preset_row,
                button(text("Play Test Sound").size(12).color(TEXT_PRIMARY()))
                    .on_press(Message::NotificationSoundTest)
                    .padding(Padding::from([4, 9])),
                text("Workspace mute patterns").size(14).color(TEXT_ACTIVE()),
                text("Use * wildcard. Matches workspace id or name.")
                    .size(12)
                    .color(TEXT_PRIMARY()),
                add_pattern_row,
                mute_pattern_list,
            ]
            .spacing(10)
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .into()
    }

    fn view_quick_claude_tab(&self) -> Element<'_, Message> {
        let save_label = if self.quick_claude_edit_index.is_some() {
            "Update Preset"
        } else {
            "Add Preset"
        };

        let mut layout_row = row![].spacing(8).align_y(iced::Alignment::Center);
        for layout in QuickClaudeLayout::all() {
            let is_active = self.quick_claude_layout == layout;
            let bg = if is_active { BG_TERTIARY() } else { BG_SECONDARY() };
            let fg = if is_active { TEXT_ACTIVE() } else { TEXT_PRIMARY() };
            let layout_button = button(text(layout.label()).size(12).color(fg))
                .on_press(Message::QuickClaudeLayoutSelected(layout))
                .padding(Padding::from([4, 9]))
                .style(move |_theme, status| {
                    let hover_bg = match status {
                        button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                        _ => bg,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(hover_bg)),
                        text_color: fg,
                        border: iced::Border {
                            color: BORDER(),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..button::Style::default()
                    }
                });
            layout_row = layout_row.push(layout_button);
        }

        let mut presets_list = column![].spacing(8).width(Length::Fill);
        if self.quick_claude_presets.is_empty() {
            presets_list = presets_list.push(
                text("No Quick Prompt presets yet.")
                    .size(11)
                    .color(TEXT_SECONDARY()),
            );
        } else {
            for (index, preset) in self.quick_claude_presets.iter().enumerate() {
                let is_editing = self.quick_claude_edit_index == Some(index);
                let card_bg = if is_editing {
                    BG_TERTIARY()
                } else {
                    BG_SECONDARY()
                };
                let is_launching = self.quick_claude_launch.is_some();
                let launch_button: Element<'_, Message> = if is_launching {
                    button(text("...").size(11).color(TEXT_SECONDARY()))
                        .padding(Padding::from([2, 7]))
                        .into()
                } else {
                    button(text("Launch").size(11).color(ACCENT()))
                        .on_press(Message::QuickClaudeLaunchPreset(index))
                        .padding(Padding::from([2, 7]))
                        .into()
                };
                let card = container(column![
                    row![
                        text(&preset.name).size(13).color(TEXT_ACTIVE()),
                        Space::new().width(Length::Fill),
                        launch_button,
                        button(text("Edit").size(11).color(TEXT_PRIMARY()))
                            .on_press(Message::QuickClaudeEditPreset(index))
                            .padding(Padding::from([2, 7])),
                        button(text("Delete").size(11).color(TEXT_PRIMARY()))
                            .on_press(Message::QuickClaudeDeletePreset(index))
                            .padding(Padding::from([2, 7])),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                    text(format!("Layout: {}", preset.layout.label()))
                        .size(11)
                        .color(TEXT_SECONDARY()),
                    text(&preset.prompt_template).size(11).color(TEXT_PRIMARY()),
                ])
                .padding(Padding::from([8, 10]))
                .width(Length::Fill)
                .style(move |_theme| container::Style {
                    background: Some(iced::Background::Color(card_bg)),
                    border: iced::Border {
                        color: BORDER(),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..container::Style::default()
                });
                presets_list = presets_list.push(card);
            }
        }

        container(
            column![
                text("Quick Prompt Presets").size(14).color(TEXT_ACTIVE()),
                text("Configure reusable launch presets.")
                    .size(12)
                    .color(TEXT_PRIMARY()),
                text_input("Preset name", &self.quick_claude_name_input)
                    .on_input(Message::QuickClaudeNameInputChanged)
                    .padding(Padding::from([4, 8]))
                    .size(12),
                text_input("Prompt template", &self.quick_claude_prompt_input)
                    .on_input(Message::QuickClaudePromptInputChanged)
                    .padding(Padding::from([4, 8]))
                    .size(12),
                text("Layout").size(12).color(TEXT_SECONDARY()),
                layout_row,
                row![
                    button(text(save_label).size(12).color(TEXT_PRIMARY()))
                        .on_press(Message::QuickClaudeSavePreset)
                        .padding(Padding::from([4, 9])),
                    button(text("Clear").size(12).color(TEXT_PRIMARY()))
                        .on_press(Message::QuickClaudeClearEditor)
                        .padding(Padding::from([4, 9])),
                ]
                .spacing(8),
                text("Saved Presets").size(12).color(TEXT_SECONDARY()),
                {
                    let launch_status: Element<'_, Message> = if let Some(launch) = &self.quick_claude_launch {
                        let step = launch.current_step;
                        let total = launch.total_steps();
                        if let Some(ref err) = launch.error {
                            column![
                                text(format!("Launch failed: {}", err)).size(11).color(iced::Color::from_rgb(0.9, 0.3, 0.3)),
                                button(text("Dismiss").size(11).color(TEXT_PRIMARY()))
                                    .on_press(Message::QuickClaudeLaunchCancel)
                                    .padding(Padding::from([2, 7])),
                            ].spacing(4).into()
                        } else {
                            row![
                                text(format!("Launching {}... step {}/{}", launch.preset_name, step + 1, total))
                                    .size(11).color(ACCENT()),
                                button(text("Cancel").size(11).color(TEXT_PRIMARY()))
                                    .on_press(Message::QuickClaudeLaunchCancel)
                                    .padding(Padding::from([2, 7])),
                            ].spacing(8).align_y(iced::Alignment::Center).into()
                        }
                    } else {
                        Space::new().height(0).into()
                    };
                    launch_status
                },
                presets_list,
            ]
            .spacing(10)
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .into()
    }

    fn view_ai_tools_tab(&self) -> Element<'_, Message> {
        let save_label = if self.ai_tool_edit_index.is_some() {
            "Update Tool"
        } else {
            "Add Tool"
        };

        let mut tools_list = column![].spacing(8).width(Length::Fill);
        if self.ai_tools.is_empty() {
            tools_list = tools_list.push(
                text("No AI tools configured.")
                    .size(11)
                    .color(TEXT_SECONDARY()),
            );
        } else {
            for (index, entry) in self.ai_tools.iter().enumerate() {
                let is_editing = self.ai_tool_edit_index == Some(index);
                let card_bg = if is_editing {
                    BG_TERTIARY()
                } else {
                    BG_SECONDARY()
                };
                let icon_line = entry.icon_tag.as_deref().unwrap_or("-");
                let card = container(column![
                    row![
                        text(&entry.display_name).size(13).color(TEXT_ACTIVE()),
                        Space::new().width(Length::Fill),
                        button(text("Edit").size(11).color(TEXT_PRIMARY()))
                            .on_press(Message::AiToolEdit(index))
                            .padding(Padding::from([2, 7])),
                        button(text("Delete").size(11).color(TEXT_PRIMARY()))
                            .on_press(Message::AiToolDelete(index))
                            .padding(Padding::from([2, 7])),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                    text(format!("Command: {}", entry.command))
                        .size(11)
                        .color(TEXT_PRIMARY()),
                    text(format!("Icon: {}", icon_line))
                        .size(11)
                        .color(TEXT_SECONDARY()),
                ])
                .padding(Padding::from([8, 10]))
                .width(Length::Fill)
                .style(move |_theme| container::Style {
                    background: Some(iced::Background::Color(card_bg)),
                    border: iced::Border {
                        color: BORDER(),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..container::Style::default()
                });
                tools_list = tools_list.push(card);
            }
        }

        container(
            column![
                text("AI Tools").size(14).color(TEXT_ACTIVE()),
                text("Register custom AI tool launch entries.")
                    .size(12)
                    .color(TEXT_PRIMARY()),
                text_input("Display name", &self.ai_tool_name_input)
                    .on_input(Message::AiToolNameInputChanged)
                    .padding(Padding::from([4, 8]))
                    .size(12),
                text_input("Command", &self.ai_tool_command_input)
                    .on_input(Message::AiToolCommandInputChanged)
                    .padding(Padding::from([4, 8]))
                    .size(12),
                text_input("Icon tag (optional)", &self.ai_tool_icon_input)
                    .on_input(Message::AiToolIconInputChanged)
                    .padding(Padding::from([4, 8]))
                    .size(12),
                row![
                    button(text(save_label).size(12).color(TEXT_PRIMARY()))
                        .on_press(Message::AiToolSave)
                        .padding(Padding::from([4, 9])),
                    button(text("Clear").size(12).color(TEXT_PRIMARY()))
                        .on_press(Message::AiToolClearEditor)
                        .padding(Padding::from([4, 9])),
                ]
                .spacing(8),
                text("Registered Tools").size(12).color(TEXT_SECONDARY()),
                tools_list,
            ]
            .spacing(10)
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .into()
    }

    fn view_plugins_tab(&self) -> Element<'_, Message> {
        let save_label = if self.plugin_edit_index.is_some() {
            "Update Plugin"
        } else {
            "Add Plugin"
        };

        let mut plugins_list = column![].spacing(8).width(Length::Fill);
        if self.plugins.is_empty() {
            plugins_list = plugins_list.push(
                text("No plugins configured.")
                    .size(11)
                    .color(TEXT_SECONDARY()),
            );
        } else {
            for (index, entry) in self.plugins.iter().enumerate() {
                let is_editing = self.plugin_edit_index == Some(index);
                let card_bg = if is_editing {
                    BG_TERTIARY()
                } else {
                    BG_SECONDARY()
                };
                let status_label = if entry.enabled { "Enabled" } else { "Disabled" };
                let toggle_label = if entry.enabled { "Disable" } else { "Enable" };
                let card = container(column![
                    row![
                        text(&entry.name).size(13).color(TEXT_ACTIVE()),
                        Space::new().width(Length::Fill),
                        text(status_label).size(11).color(TEXT_SECONDARY()),
                        button(text(toggle_label).size(11).color(TEXT_PRIMARY()))
                            .on_press(Message::PluginToggleEnabled(index))
                            .padding(Padding::from([2, 7])),
                        button(text("Edit").size(11).color(TEXT_PRIMARY()))
                            .on_press(Message::PluginEdit(index))
                            .padding(Padding::from([2, 7])),
                        button(text("Delete").size(11).color(TEXT_PRIMARY()))
                            .on_press(Message::PluginDelete(index))
                            .padding(Padding::from([2, 7])),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                    text(format!("Source: {}", entry.source))
                        .size(11)
                        .color(TEXT_PRIMARY()),
                ])
                .padding(Padding::from([8, 10]))
                .width(Length::Fill)
                .style(move |_theme| container::Style {
                    background: Some(iced::Background::Color(card_bg)),
                    border: iced::Border {
                        color: BORDER(),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..container::Style::default()
                });
                plugins_list = plugins_list.push(card);
            }
        }

        container(
            column![
                text("Plugins").size(14).color(TEXT_ACTIVE()),
                text("Manage plugin entries for native shell runtime.")
                    .size(12)
                    .color(TEXT_PRIMARY()),
                text_input("Plugin name", &self.plugin_name_input)
                    .on_input(Message::PluginNameInputChanged)
                    .padding(Padding::from([4, 8]))
                    .size(12),
                text_input("Source path / URL", &self.plugin_source_input)
                    .on_input(Message::PluginSourceInputChanged)
                    .padding(Padding::from([4, 8]))
                    .size(12),
                row![
                    button(text(save_label).size(12).color(TEXT_PRIMARY()))
                        .on_press(Message::PluginSave)
                        .padding(Padding::from([4, 9])),
                    button(text("Clear").size(12).color(TEXT_PRIMARY()))
                        .on_press(Message::PluginClearEditor)
                        .padding(Padding::from([4, 9])),
                ]
                .spacing(8),
                text("Registered Plugins").size(12).color(TEXT_SECONDARY()),
                plugins_list,
            ]
            .spacing(10)
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .into()
    }

    fn view_flows_tab(&self) -> Element<'_, Message> {
        let save_label = if self.flow_edit_index.is_some() {
            "Update Flow"
        } else {
            "Add Flow"
        };

        let mut flows_list = column![].spacing(8).width(Length::Fill);
        if self.flows.is_empty() {
            flows_list =
                flows_list.push(text("No flows configured.").size(11).color(TEXT_SECONDARY()));
        } else {
            for (index, entry) in self.flows.iter().enumerate() {
                let is_editing = self.flow_edit_index == Some(index);
                let card_bg = if is_editing {
                    BG_TERTIARY()
                } else {
                    BG_SECONDARY()
                };
                let status_label = if entry.enabled { "Enabled" } else { "Disabled" };
                let toggle_label = if entry.enabled { "Disable" } else { "Enable" };
                let steps_label = if entry.steps.is_empty() {
                    "No steps".to_string()
                } else {
                    format!("{} step(s)", entry.steps.len())
                };
                let steps_preview = if entry.steps.is_empty() {
                    "-".to_string()
                } else {
                    entry.steps.join(" -> ")
                };
                let card = container(column![
                    row![
                        text(&entry.name).size(13).color(TEXT_ACTIVE()),
                        Space::new().width(Length::Fill),
                        text(status_label).size(11).color(TEXT_SECONDARY()),
                        button(text(toggle_label).size(11).color(TEXT_PRIMARY()))
                            .on_press(Message::FlowToggleEnabled(index))
                            .padding(Padding::from([2, 7])),
                        button(text("Edit").size(11).color(TEXT_PRIMARY()))
                            .on_press(Message::FlowEdit(index))
                            .padding(Padding::from([2, 7])),
                        button(text("Delete").size(11).color(TEXT_PRIMARY()))
                            .on_press(Message::FlowDelete(index))
                            .padding(Padding::from([2, 7])),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                    text(format!("Trigger: {}", entry.trigger))
                        .size(11)
                        .color(TEXT_PRIMARY()),
                    text(format!("Steps: {} | {}", steps_label, steps_preview))
                        .size(11)
                        .color(TEXT_SECONDARY()),
                ])
                .padding(Padding::from([8, 10]))
                .width(Length::Fill)
                .style(move |_theme| container::Style {
                    background: Some(iced::Background::Color(card_bg)),
                    border: iced::Border {
                        color: BORDER(),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..container::Style::default()
                });
                flows_list = flows_list.push(card);
            }
        }

        container(
            column![
                text("Flows").size(14).color(TEXT_ACTIVE()),
                text("Create simple automation flow profiles with trigger and ordered steps.")
                    .size(12)
                    .color(TEXT_PRIMARY()),
                text_input("Flow name", &self.flow_name_input)
                    .on_input(Message::FlowNameInputChanged)
                    .padding(Padding::from([4, 8]))
                    .size(12),
                text_input(
                    "Trigger (optional, defaults to Manual)",
                    &self.flow_trigger_input
                )
                .on_input(Message::FlowTriggerInputChanged)
                .padding(Padding::from([4, 8]))
                .size(12),
                text_input(
                    "Steps (comma, semicolon, or newline separated)",
                    &self.flow_steps_input
                )
                .on_input(Message::FlowStepsInputChanged)
                .padding(Padding::from([4, 8]))
                .size(12),
                row![
                    button(text(save_label).size(12).color(TEXT_PRIMARY()))
                        .on_press(Message::FlowSave)
                        .padding(Padding::from([4, 9])),
                    button(text("Clear").size(12).color(TEXT_PRIMARY()))
                        .on_press(Message::FlowClearEditor)
                        .padding(Padding::from([4, 9])),
                ]
                .spacing(8),
                text("Registered Flows").size(12).color(TEXT_SECONDARY()),
                flows_list,
            ]
            .spacing(10)
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .into()
    }

    fn view_remote_tab(&self) -> Element<'_, Message> {
        let save_label = if self.remote_edit_index.is_some() {
            "Update Profile"
        } else {
            "Add Profile"
        };

        let auth_placeholder = match self.remote_auth_mode {
            RemoteAuthMode::Password => "Password (optional)",
            RemoteAuthMode::SshKey => "SSH key path (optional)",
        };

        let mut auth_mode_row = row![].spacing(8).align_y(iced::Alignment::Center);
        for mode in RemoteAuthMode::all() {
            let is_active = self.remote_auth_mode == mode;
            let bg = if is_active { BG_TERTIARY() } else { BG_SECONDARY() };
            let fg = if is_active { TEXT_ACTIVE() } else { TEXT_PRIMARY() };
            let mode_button = button(text(mode.label()).size(12).color(fg))
                .on_press(Message::RemoteAuthModeSelected(mode))
                .padding(Padding::from([4, 9]))
                .style(move |_theme, status| {
                    let hover_bg = match status {
                        button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                        _ => bg,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(hover_bg)),
                        text_color: fg,
                        border: iced::Border {
                            color: BORDER(),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..button::Style::default()
                    }
                });
            auth_mode_row = auth_mode_row.push(mode_button);
        }

        let mut remote_list = column![].spacing(8).width(Length::Fill);
        if self.remote_connections.is_empty() {
            remote_list = remote_list.push(
                text("No remote profiles configured.")
                    .size(11)
                    .color(TEXT_SECONDARY()),
            );
        } else {
            for (index, profile) in self.remote_connections.iter().enumerate() {
                let is_editing = self.remote_edit_index == Some(index);
                let card_bg = if is_editing {
                    BG_TERTIARY()
                } else {
                    BG_SECONDARY()
                };
                let status_label = if profile.enabled {
                    "Enabled"
                } else {
                    "Disabled"
                };
                let toggle_label = if profile.enabled { "Disable" } else { "Enable" };
                let auth_display = match (profile.auth_mode, profile.auth_value.as_deref()) {
                    (RemoteAuthMode::Password, Some(_)) => "Password: ******".to_string(),
                    (RemoteAuthMode::Password, None) => "Password: -".to_string(),
                    (RemoteAuthMode::SshKey, Some(value)) => format!("SSH Key: {}", value),
                    (RemoteAuthMode::SshKey, None) => "SSH Key: -".to_string(),
                };
                let card = container(column![
                    row![
                        text(&profile.name).size(13).color(TEXT_ACTIVE()),
                        Space::new().width(Length::Fill),
                        text(status_label).size(11).color(TEXT_SECONDARY()),
                        button(text(toggle_label).size(11).color(TEXT_PRIMARY()))
                            .on_press(Message::RemoteToggleEnabled(index))
                            .padding(Padding::from([2, 7])),
                        button(text("Edit").size(11).color(TEXT_PRIMARY()))
                            .on_press(Message::RemoteEdit(index))
                            .padding(Padding::from([2, 7])),
                        button(text("Delete").size(11).color(TEXT_PRIMARY()))
                            .on_press(Message::RemoteDelete(index))
                            .padding(Padding::from([2, 7])),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                    text(format!(
                        "Connection: {}@{}:{}",
                        profile.username, profile.host, profile.port
                    ))
                    .size(11)
                    .color(TEXT_PRIMARY()),
                    text(auth_display).size(11).color(TEXT_SECONDARY()),
                ])
                .padding(Padding::from([8, 10]))
                .width(Length::Fill)
                .style(move |_theme| container::Style {
                    background: Some(iced::Background::Color(card_bg)),
                    border: iced::Border {
                        color: BORDER(),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..container::Style::default()
                });
                remote_list = remote_list.push(card);
            }
        }

        container(
            column![
                text("Remote (SSH)").size(14).color(TEXT_ACTIVE()),
                text("Configure SSH connection profiles for remote terminal sessions.")
                    .size(12)
                    .color(TEXT_PRIMARY()),
                text_input("Profile name", &self.remote_name_input)
                    .on_input(Message::RemoteNameInputChanged)
                    .padding(Padding::from([4, 8]))
                    .size(12),
                text_input("Host or IP", &self.remote_host_input)
                    .on_input(Message::RemoteHostInputChanged)
                    .padding(Padding::from([4, 8]))
                    .size(12),
                text_input("Port", &self.remote_port_input)
                    .on_input(Message::RemotePortInputChanged)
                    .padding(Padding::from([4, 8]))
                    .size(12),
                text_input("Username", &self.remote_username_input)
                    .on_input(Message::RemoteUsernameInputChanged)
                    .padding(Padding::from([4, 8]))
                    .size(12),
                text("Auth Mode").size(12).color(TEXT_SECONDARY()),
                auth_mode_row,
                text_input(auth_placeholder, &self.remote_auth_value_input)
                    .on_input(Message::RemoteAuthValueInputChanged)
                    .padding(Padding::from([4, 8]))
                    .size(12),
                row![
                    button(text(save_label).size(12).color(TEXT_PRIMARY()))
                        .on_press(Message::RemoteSave)
                        .padding(Padding::from([4, 9])),
                    button(text("Clear").size(12).color(TEXT_PRIMARY()))
                        .on_press(Message::RemoteClearEditor)
                        .padding(Padding::from([4, 9])),
                ]
                .spacing(8),
                text("Connection Profiles").size(12).color(TEXT_SECONDARY()),
                remote_list,
            ]
            .spacing(10)
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            // Keyboard events.
            keyboard::listen().map(Message::KeyboardEvent),
            // Daemon events via channel.
            daemon_events(Arc::clone(&self.event_receiver)).map(Message::DaemonEvent),
            // MCP pipe events via channel.
            crate::subscription::mcp_events(Arc::clone(&self.mcp_event_receiver)).map(Message::McpEvent),
            // Window + mouse events.
            event::listen_with(|ev, status, window_id| match ev {
                event::Event::Window(window::Event::Opened { .. }) => {
                    Some(Message::WindowOpened(window_id))
                }
                event::Event::Window(window::Event::Resized(size)) => {
                    Some(Message::WindowResized {
                        window_id,
                        width: size.width,
                        height: size.height,
                    })
                }
                event::Event::Window(window::Event::Focused) => Some(Message::WindowFocusChanged {
                    window_id,
                    focused: true,
                }),
                event::Event::Window(window::Event::Unfocused) => {
                    Some(Message::WindowFocusChanged {
                        window_id,
                        focused: false,
                    })
                }
                // RedrawRequested fires from WM_TIMER when minimized/unfocused.
                // Route it through update() so the Win32 focus check can run.
                event::Event::Window(window::Event::RedrawRequested(_)) => {
                    Some(Message::Heartbeat)
                }
                event::Event::Window(window::Event::FileDropped(path)) => {
                    Some(Message::FileDropped(path))
                }
                event::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => {
                    if status == event::Status::Captured {
                        return None;
                    }
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
        ];

        // NOTE: Heartbeat is sent via a background thread (spawn_heartbeat_thread)
        // through the daemon event channel, NOT via an Iced subscription. Iced
        // stops polling subscriptions when the window is minimized/invisible.

        if !self.toasts.is_empty() {
            subscriptions.push(
                iced::time::every(Duration::from_millis(TOAST_TICK_INTERVAL_MS))
                    .map(|_| Message::ToastTick),
            );
        }

        if self.sidebar_animation.is_some() {
            subscriptions.push(
                iced::time::every(Duration::from_millis(SIDEBAR_ANIMATION_TICK_MS))
                    .map(|_| Message::SidebarAnimationTick),
            );
        }

        if !self.entering_tabs.is_empty() {
            subscriptions.push(
                iced::time::every(Duration::from_millis(SIDEBAR_ANIMATION_TICK_MS))
                    .map(|_| Message::TabEntryAnimationTick),
            );
        }

        // Whisper recording timer (I4/I5)
        if self.whisper_state.as_ref().is_some_and(|s| s.recording) {
            subscriptions.push(
                iced::time::every(Duration::from_millis(100))
                    .map(|_| Message::WhisperTimerTick),
            );
        }

        Subscription::batch(subscriptions)
    }

    // -----------------------------------------------------------------------
    // App action dispatch
    // -----------------------------------------------------------------------

    fn activate_tab_via_reducer(&mut self, terminal_id: String) -> Task<Message> {
        let in_active_layout = self
            .active_layout()
            .map(|layout| layout.find_leaf(&terminal_id))
            .unwrap_or(false);
        let decision = tab_reducer::reduce_tab_click(tab_reducer::TabClickInput {
            terminal_id: terminal_id.clone(),
            terminal_in_active_layout: in_active_layout,
        });

        self.terminals.set_active(&decision.activate_terminal_id);
        let layout_changed = if let Some(focus_terminal_id) = decision.focus_workspace_terminal_id {
            if let Some(ws) = self.workspaces.active_mut() {
                ws.focused_terminal = focus_terminal_id;
            }
            true
        } else if !in_active_layout {
            // Terminal is not in the layout (e.g., switching tabs in a
            // non-split workspace). Update the layout to show this terminal.
            if let Some(ws) = self.workspaces.active_mut() {
                ws.layout = LayoutNode::Leaf {
                    terminal_id: terminal_id.clone(),
                };
                ws.focused_terminal = terminal_id.clone();
            }
            true
        } else {
            false
        };
        self.notifications
            .mark_read(&decision.mark_terminal_read_id);
        self.tab_context_menu_id = None;
        self.mru_switcher = None;

        if layout_changed {
            // Fetch the grid for the newly focused terminal so the canvas
            // renders up-to-date content immediately.
            self.fetch_grid(&terminal_id)
        } else {
            Task::none()
        }
    }

    fn cycle_tabs_by_mru(&mut self, direction: tab_reducer::TabMruCycleDirection) -> Task<Message> {
        let mru_terminal_ids = self.active_workspace_mru_terminal_ids();
        let current_terminal_id = self
            .active_focused()
            .filter(|terminal_id| mru_terminal_ids.contains(terminal_id));
        let next_terminal_id =
            next_tab_id_from_mru(mru_terminal_ids, current_terminal_id, direction);
        let Some(next_terminal_id) = next_terminal_id else {
            return Task::none();
        };
        self.activate_tab_via_reducer(next_terminal_id)
    }

    fn open_or_cycle_mru_switcher(&mut self, direction: tab_reducer::TabMruCycleDirection) {
        let mru_terminal_ids = self.active_workspace_mru_terminal_ids();
        let next_terminal_id = next_mru_switcher_selection(
            mru_terminal_ids,
            self.active_focused(),
            self.mru_switcher
                .as_ref()
                .map(|state| state.selected_terminal_id.as_str()),
            direction,
        );
        let Some(next_terminal_id) = next_terminal_id else {
            return;
        };

        self.mru_switcher = Some(MruSwitcherState {
            selected_terminal_id: next_terminal_id,
        });
        self.tab_context_menu_id = None;
    }

    fn commit_mru_switcher(&mut self) -> Task<Message> {
        let selected_terminal_id = self
            .mru_switcher
            .take()
            .map(|state| state.selected_terminal_id);
        let Some(selected_terminal_id) = selected_terminal_id else {
            return Task::none();
        };
        if !self
            .active_workspace_mru_terminal_ids()
            .contains(&selected_terminal_id.as_str())
        {
            return Task::none();
        }
        self.activate_tab_via_reducer(selected_terminal_id)
    }

    fn cancel_mru_switcher(&mut self) {
        self.mru_switcher = None;
    }

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
                self.cycle_tabs_by_mru(tab_reducer::TabMruCycleDirection::Forward)
            }
            AppAction::PreviousTab => {
                self.cycle_tabs_by_mru(tab_reducer::TabMruCycleDirection::Backward)
            }
            AppAction::ZoomIn => {
                self.font_metrics = measured_font_metrics(self.font_metrics.font_size + 1.0, &self.font_family);
                self.resize_all_terminals()
            }
            AppAction::ZoomOut => {
                let new_size = (self.font_metrics.font_size - 1.0).max(8.0);
                self.font_metrics = measured_font_metrics(new_size, &self.font_family);
                self.resize_all_terminals()
            }
            AppAction::ZoomReset => {
                self.font_metrics = measured_font_metrics(14.0, &self.font_family);
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
            AppAction::FocusLeft => {
                self.focus_in_direction(FocusDirection::Left);
                Task::none()
            }
            AppAction::FocusRight => {
                self.focus_in_direction(FocusDirection::Right);
                Task::none()
            }
            AppAction::FocusUp => {
                self.focus_in_direction(FocusDirection::Up);
                Task::none()
            }
            AppAction::FocusDown => {
                self.focus_in_direction(FocusDirection::Down);
                Task::none()
            }
            AppAction::SelectAll => {
                self.select_all();
                Task::none()
            }
            AppAction::NextWorkspace => {
                self.workspaces.next();
                let read_target = workspace_reducer::reduce_workspace_switch_read_target(
                    self.workspaces
                        .active()
                        .map(|workspace| workspace.focused_terminal.clone()),
                );
                if let Some(terminal_id) = read_target {
                    self.notifications.mark_read(&terminal_id);
                }
                Task::none()
            }
            AppAction::PrevWorkspace => {
                self.workspaces.previous();
                let read_target = workspace_reducer::reduce_workspace_switch_read_target(
                    self.workspaces
                        .active()
                        .map(|workspace| workspace.focused_terminal.clone()),
                );
                if let Some(terminal_id) = read_target {
                    self.notifications.mark_read(&terminal_id);
                }
                Task::none()
            }
            AppAction::ToggleSidebar => self.toggle_sidebar_visibility(),
            AppAction::OpenSettings => {
                self.settings_open = !self.settings_open;
                Task::none()
            }
            AppAction::RenameTab => {
                if let Some(tab_id) = self.active_focused().map(str::to_string) {
                    if let Some(term) = self.terminals.get(&tab_id) {
                        self.rename_tab_value = term.custom_name.clone().unwrap_or_default();
                        self.rename_tab_id = Some(tab_id);
                        self.tab_context_menu_id = None;
                    }
                }
                Task::none()
            }
            AppAction::TogglePerfOverlay => {
                self.perf_overlay_visible = !self.perf_overlay_visible;
                Task::none()
            }
            AppAction::Find => {
                self.search.open();
                Task::none()
            }
            AppAction::WhisperToggle => {
                return self.update(Message::WhisperToggle);
            }
            AppAction::QuickClaude => {
                return self.update(Message::QuickClaudeDialogOpen);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Whisper voice integration (I4/I5)
    // -----------------------------------------------------------------------

    fn whisper_start(&mut self) -> Task<Message> {
        use godly_app_adapter::whisper::WhisperService;

        self.whisper_state = Some(whisper_ui::WhisperState::new());

        // Shared slot: background task populates, timer/stop reads.
        let slot: Arc<parking_lot::Mutex<Option<WhisperService>>> =
            Arc::new(parking_lot::Mutex::new(None));
        let slot_for_task = Arc::clone(&slot);
        self.whisper_service = Some(slot);

        Task::perform(
            async move {
                WhisperService::spawn().and_then(|mut svc| {
                    svc.start_recording()?;
                    *slot_for_task.lock() = Some(svc);
                    Ok(())
                })
            },
            |result: Result<(), String>| match result {
                Ok(()) => Message::WhisperStarted,
                Err(e) => Message::WhisperStopped(Err(e)),
            },
        )
    }

    fn whisper_stop(&mut self) -> Task<Message> {
        if let Some(ref mut state) = self.whisper_state {
            state.transcribing = true;
            state.recording = false;
        }
        if let Some(ref slot) = self.whisper_service {
            let slot = Arc::clone(slot);
            return Task::perform(
                async move {
                    let mut guard = slot.lock();
                    match guard.as_mut() {
                        Some(svc) => svc.stop_recording().map(|r| (r.text, r.duration_ms)),
                        None => Err("Whisper service not ready".to_string()),
                    }
                },
                Message::WhisperStopped,
            );
        }
        self.whisper_state = None;
        Task::none()
    }

    // -----------------------------------------------------------------------
    // Workspace operations
    // -----------------------------------------------------------------------

    fn handle_sidebar_action(&mut self, action: SidebarAction) -> Task<Message> {
        match action {
            SidebarAction::SelectWorkspace(id) => {
                let focused_terminal_id = self
                    .workspaces
                    .get(&id)
                    .map(|ws| ws.focused_terminal.clone())
                    .or_else(|| {
                        self.workspaces
                            .active()
                            .map(|ws| ws.focused_terminal.clone())
                    });
                let decision = workspace_reducer::reduce_workspace_selection(
                    workspace_reducer::WorkspaceSelectionInput {
                        workspace_id: id,
                        focused_terminal_id,
                    },
                );
                self.workspaces.set_active(&decision.workspace_id);
                if decision.clear_context_menu {
                    self.workspace_context_menu_id = None;
                }
                if let Some(terminal_id) = decision.mark_terminal_read_id {
                    self.notifications.mark_read(&terminal_id);
                }
                self.tab_context_menu_id = None;
                Task::none()
            }
            SidebarAction::WorkspaceDragHover(target_workspace_id) => {
                let Some(dragged_terminal_id) = self.dragging_tab_id.clone() else {
                    return Task::none();
                };
                let Some(source_workspace_id) = self
                    .terminals
                    .get(&dragged_terminal_id)
                    .and_then(|terminal| terminal.workspace_id.clone())
                else {
                    return Task::none();
                };

                let source_snapshot = self.workspaces.get(&source_workspace_id).map(|workspace| {
                    (
                        workspace.layout.clone(),
                        Some(workspace.focused_terminal.clone()),
                    )
                });
                let target_snapshot = self.workspaces.get(&target_workspace_id).map(|workspace| {
                    (
                        workspace.layout.clone(),
                        Some(workspace.focused_terminal.clone()),
                    )
                });

                let (source_layout, source_focused_terminal_id) = match source_snapshot {
                    Some(values) => values,
                    None => return Task::none(),
                };
                let (target_layout, target_focused_terminal_id) = match target_snapshot {
                    Some(values) => values,
                    None => return Task::none(),
                };

                let decision = workspace_reducer::reduce_move_terminal_across_workspaces(
                    workspace_reducer::MoveTerminalAcrossWorkspacesInput {
                        source_workspace_id: source_workspace_id.clone(),
                        target_workspace_id: target_workspace_id.clone(),
                        moved_terminal_id: dragged_terminal_id.clone(),
                        source_layout: Some(source_layout),
                        target_layout: Some(target_layout),
                        source_focused_terminal_id,
                        target_focused_terminal_id,
                    },
                );

                if let workspace_reducer::MoveTerminalAcrossWorkspacesDecision::Move {
                    source_workspace_id,
                    target_workspace_id,
                    moved_terminal_id,
                    next_source_layout,
                    next_target_layout,
                    next_source_focused_terminal_id,
                    next_target_focused_terminal_id,
                } = decision
                {
                    if let Some(workspace) = self.workspaces.get_mut(&source_workspace_id) {
                        workspace.layout = next_source_layout;
                        workspace.focused_terminal = next_source_focused_terminal_id;
                    }
                    if let Some(workspace) = self.workspaces.get_mut(&target_workspace_id) {
                        workspace.layout = next_target_layout;
                        workspace.focused_terminal = next_target_focused_terminal_id;
                    }

                    self.terminals
                        .set_workspace(&moved_terminal_id, Some(target_workspace_id.clone()));
                    self.workspaces.set_active(&target_workspace_id);
                    self.terminals.set_active(&moved_terminal_id);
                    self.notifications.mark_read(&moved_terminal_id);
                    self.workspace_context_menu_id = None;
                    self.dragging_tab_id = None;
                    self.tab_drag_start_pos = None;

                    return self.fetch_grid(&moved_terminal_id);
                }

                Task::none()
            }
            SidebarAction::ToggleWorkspaceContext(id) => {
                self.workspace_context_menu_id = workspace_reducer::reduce_workspace_context_toggle(
                    self.workspace_context_menu_id.as_deref(),
                    &id,
                );
                Task::none()
            }
            SidebarAction::RenameWorkspace(id) => {
                if let Some(ws) = self.workspaces.get(&id) {
                    self.rename_workspace_value = ws.name.clone();
                    self.rename_workspace_id = Some(id);
                    self.workspace_context_menu_id = None;
                }
                Task::none()
            }
            SidebarAction::DeleteWorkspace(id) => self.delete_workspace(&id),
            SidebarAction::OpenWorkspaceInExplorer(id) => {
                self.workspace_context_menu_id = None;
                self.open_workspace_in_explorer(&id);
                Task::none()
            }
            SidebarAction::ToggleWorkspaceWorktreeMode(id) => {
                self.workspace_context_menu_id = None;
                if let Some(workspace) = self.workspaces.get(&id) {
                    let _ = self
                        .workspaces
                        .set_worktree_mode(&id, !workspace.worktree_mode);
                }
                Task::none()
            }
            SidebarAction::MoveWorkspaceUp(id) => {
                self.workspace_context_menu_id = Some(id.clone());
                let _ = self.workspaces.move_up(&id);
                Task::none()
            }
            SidebarAction::MoveWorkspaceDown(id) => {
                self.workspace_context_menu_id = Some(id.clone());
                let _ = self.workspaces.move_down(&id);
                Task::none()
            }
            SidebarAction::NewWorkspace => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Select Workspace Folder")
                            .pick_folder()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    Message::FolderPickerResult,
                );
            }
            SidebarAction::ToggleSettings => {
                self.settings_open = !self.settings_open;
                Task::none()
            }
            SidebarAction::OpenProjectClaudeMd => {
                let folder = self
                    .workspaces
                    .active()
                    .map(|ws| ws.folder_path.clone())
                    .unwrap_or_else(|| ".".to_string());
                let path = std::path::PathBuf::from(folder).join("CLAUDE.md");
                self.update(Message::ClaudeMdOpen { path })
            }
            SidebarAction::OpenUserClaudeMd => {
                let home = std::env::var("USERPROFILE")
                    .unwrap_or_else(|_| std::env::var("HOME").unwrap_or_else(|_| ".".into()));
                let path = std::path::PathBuf::from(home)
                    .join(".claude")
                    .join("CLAUDE.md");
                self.update(Message::ClaudeMdOpen { path })
            }
        }
    }

    pub(crate) fn delete_workspace(&mut self, workspace_id: &str) -> Task<Message> {
        let terminal_ids: Vec<String> = self
            .terminals
            .terminals_for_workspace(workspace_id)
            .into_iter()
            .map(|term| term.id.clone())
            .collect();

        let decision =
            workspace_reducer::reduce_delete_workspace(workspace_reducer::DeleteWorkspaceInput {
                workspace_count: self.workspaces.count(),
                terminal_ids,
            });

        match decision {
            workspace_reducer::DeleteWorkspaceDecision::RejectedLastWorkspace => {
                log::warn!(
                    "Skipping delete for workspace {} (cannot remove last workspace)",
                    workspace_id
                );
                self.workspace_context_menu_id = None;
                return Task::none();
            }
            workspace_reducer::DeleteWorkspaceDecision::Delete {
                terminal_ids,
                clear_context_menu,
            } => {
                for terminal_id in terminal_ids {
                    self.terminals.remove(&terminal_id);
                    self.notifications.clear(&terminal_id);
                    self.last_terminal_sound_ms.remove(&terminal_id);
                    if let Some(client) = &self.client {
                        let _ = commands::close_terminal(client, &terminal_id);
                    }
                }
                self.persist_scrollback_offsets();
                self.workspaces.remove(workspace_id);
                if clear_context_menu {
                    self.workspace_context_menu_id = None;
                }
            }
        }

        let decision = workspace_reducer::reduce_post_workspace_delete(
            self.workspaces
                .active()
                .map(|workspace| workspace.focused_terminal.clone()),
        );
        if let Some(terminal_id) = decision.mark_terminal_read_id {
            self.notifications.mark_read(&terminal_id);
        }
        if let Some(terminal_id) = decision.fetch_grid_terminal_id {
            return self.fetch_grid(&terminal_id);
        }

        Task::none()
    }

    fn open_workspace_in_explorer(&self, workspace_id: &str) {
        let Some(workspace) = self.workspaces.get(workspace_id) else {
            return;
        };

        #[cfg(target_os = "windows")]
        let mut command = {
            let mut cmd = std::process::Command::new("explorer");
            cmd.arg(&workspace.folder_path);
            cmd
        };

        #[cfg(target_os = "macos")]
        let mut command = {
            let mut cmd = std::process::Command::new("open");
            cmd.arg(&workspace.folder_path);
            cmd
        };

        #[cfg(all(unix, not(target_os = "macos")))]
        let mut command = {
            let mut cmd = std::process::Command::new("xdg-open");
            cmd.arg(&workspace.folder_path);
            cmd
        };

        if let Err(err) = command.spawn() {
            log::error!(
                "Failed to open workspace {} in explorer ({}): {}",
                workspace_id,
                workspace.folder_path,
                err
            );
        }
    }

    fn view_workspace_rename_dialog(&self) -> Element<'_, Message> {
        let dialog = container(column![
            text("Rename Workspace").size(16).color(TEXT_ACTIVE()),
            text_input("Workspace name", &self.rename_workspace_value)
                .on_input(Message::WorkspaceRenameInputChanged)
                .on_submit(Message::WorkspaceRenameSubmitted)
                .padding(Padding::from([6, 8]))
                .size(14),
            row![
                Space::new().width(Length::Fill),
                button(text("Cancel").size(12).color(TEXT_PRIMARY()))
                    .on_press(Message::WorkspaceRenameCancelled)
                    .padding(Padding::from([4, 9]))
                    .style(|_theme, status| {
                        let bg = match status {
                            button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                            _ => iced::Color::TRANSPARENT,
                        };
                        button::Style {
                            background: Some(iced::Background::Color(bg)),
                            text_color: TEXT_PRIMARY(),
                            border: iced::Border {
                                color: BORDER(),
                                width: 1.0,
                                radius: 4.0.into(),
                            },
                            ..button::Style::default()
                        }
                    }),
                button(text("Rename").size(12).color(TEXT_ACTIVE()))
                    .on_press(Message::WorkspaceRenameSubmitted)
                    .padding(Padding::from([4, 9]))
                    .style(|_theme, status| {
                        let bg = match status {
                            button::Status::Hovered | button::Status::Pressed => BG_TERTIARY(),
                            _ => BG_SECONDARY(),
                        };
                        button::Style {
                            background: Some(iced::Background::Color(bg)),
                            text_color: TEXT_ACTIVE(),
                            border: iced::Border {
                                color: BORDER(),
                                width: 1.0,
                                radius: 4.0.into(),
                            },
                            ..button::Style::default()
                        }
                    }),
            ]
            .spacing(8),
        ])
        .padding(Padding::from([14, 16]))
        .width(Length::Fixed(340.0))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(BG_SECONDARY())),
            border: iced::Border {
                color: BORDER(),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        });

        container(center(dialog))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(BACKDROP())),
                ..container::Style::default()
            })
            .into()
    }

    /// Create a new workspace with a fresh terminal.
    fn create_new_workspace(&mut self) -> Task<Message> {
        let decision =
            workspace_reducer::reduce_new_workspace(workspace_reducer::NewWorkspaceInput {
                workspace_id: uuid::Uuid::new_v4().to_string(),
                session_id: uuid::Uuid::new_v4().to_string(),
                next_workspace_num: self.next_workspace_num,
            });
        self.next_workspace_num = decision.next_workspace_num;

        let rows = self.calculate_rows();
        let cols = self.calculate_cols();

        self.terminals.add_to_workspace(
            decision.session_id.clone(),
            rows,
            cols,
            decision.workspace_id.clone(),
        );
        self.workspaces.add(
            decision.workspace_id.clone(),
            decision.workspace_name,
            decision.session_id.clone(),
        );
        self.workspaces.set_active(&decision.workspace_id);
        self.terminals.set_active(&decision.session_id);
        self.workspace_context_menu_id = None;

        self.create_terminal_task(decision.session_id)
    }

    /// Create a new workspace with a folder path (from the folder picker).
    fn create_new_workspace_with_folder(
        &mut self,
        name: String,
        folder_path: String,
    ) -> Task<Message> {
        let workspace_id = uuid::Uuid::new_v4().to_string();
        let session_id = uuid::Uuid::new_v4().to_string();

        let rows = self.calculate_rows();
        let cols = self.calculate_cols();

        self.terminals.add_to_workspace(
            session_id.clone(),
            rows,
            cols,
            workspace_id.clone(),
        );
        self.workspaces.add_with_details(
            workspace_id.clone(),
            name,
            folder_path.clone(),
            false,
            LayoutNode::Leaf {
                terminal_id: session_id.clone(),
            },
            session_id.clone(),
        );
        self.workspaces.set_active(&workspace_id);
        self.terminals.set_active(&session_id);
        self.workspace_context_menu_id = None;
        self.next_workspace_num = self.next_workspace_num.saturating_add(1);

        self.create_terminal_task_with_cwd(session_id, Some(folder_path))
    }

    fn apply_init_result(&mut self, result: InitResult) -> Task<Message> {
        self.clear_runtime_state();

        match result {
            InitResult::Fresh { session_id } => {
                let rows = self.calculate_rows();
                let cols = self.calculate_cols();
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
                self.workspaces.set_active("w-default");
                self.terminals.set_active(&session_id);
                self.last_test_workspace_id = Some("w-default".to_string());
                self.last_test_terminal_id = Some(session_id.clone());
                self.fetch_grid(&session_id)
            }
            InitResult::Recovered {
                session_ids,
                restored_scrollback_offsets,
                merged_session_state,
            } => {
                if let Some(merged) = merged_session_state.as_ref() {
                    self.sidebar_visible = merged.sidebar_visible;
                    self.settings_open = merged.settings_open;
                    self.settings_tab = merged.settings_tab.clone();
                    self.font_metrics = measured_font_metrics(merged.font_size, &merged.font_family);
                    self.font_family = merged.font_family.clone();
                    let interned = font_enumerator::intern_font_name(&merged.font_family);
                    self.terminal_font = iced::Font {
                        family: iced::font::Family::Name(interned),
                        weight: iced::font::Weight::Normal,
                        stretch: iced::font::Stretch::Normal,
                        style: iced::font::Style::Normal,
                    };
                    self.next_workspace_num = merged.next_workspace_num;
                }

                let rows = self.calculate_rows();
                let cols = self.calculate_cols();
                let mut assigned_terminal_ids = HashSet::new();

                if let Some(merged) = merged_session_state {
                    // Restore worktree paths for surviving terminals.
                    let worktree_paths = merged.terminal_worktree_paths.clone();

                    // Collect workspaces that lost all their terminals (e.g. after reinstall).
                    // They'll be filled from the missing_live_terminal_ids pool below.
                    let mut empty_workspaces: Vec<session_persistence::MergedWorkspaceState> =
                        Vec::new();

                    for workspace in merged.workspaces {
                        if let (Some(layout), Some(focused)) =
                            (workspace.layout.clone(), workspace.focused_terminal.clone())
                        {
                            for terminal_id in layout.all_leaf_ids() {
                                let terminal_id = terminal_id.to_string();
                                if !assigned_terminal_ids.insert(terminal_id.clone()) {
                                    continue;
                                }
                                self.terminals.add_to_workspace(
                                    terminal_id.clone(),
                                    rows,
                                    cols,
                                    workspace.id.clone(),
                                );
                                if let Some(offset) =
                                    restored_scrollback_offsets.get(&terminal_id)
                                {
                                    if let Some(term) = self.terminals.get_mut(&terminal_id) {
                                        term.scrollback_offset = *offset;
                                    }
                                }
                            }

                            self.workspaces.add_with_details(
                                workspace.id.clone(),
                                workspace.name,
                                workspace.folder_path,
                                workspace.worktree_mode,
                                layout,
                                focused,
                            );
                        } else {
                            // Bug #619: workspace has no live terminals — defer creation.
                            empty_workspaces.push(workspace);
                        }
                    }

                    // Assign missing live terminals: first fill empty workspaces (one each),
                    // then overflow remaining into the active workspace.
                    let mut missing_iter =
                        merged.missing_live_terminal_ids.into_iter().peekable();

                    // Fill empty workspaces with one terminal each.
                    for workspace in empty_workspaces {
                        if let Some(terminal_id) = missing_iter.next() {
                            if assigned_terminal_ids.insert(terminal_id.clone()) {
                                self.terminals.add_to_workspace(
                                    terminal_id.clone(),
                                    rows,
                                    cols,
                                    workspace.id.clone(),
                                );
                                if let Some(offset) =
                                    restored_scrollback_offsets.get(&terminal_id)
                                {
                                    if let Some(term) = self.terminals.get_mut(&terminal_id) {
                                        term.scrollback_offset = *offset;
                                    }
                                }
                                self.workspaces.add_with_details(
                                    workspace.id.clone(),
                                    workspace.name,
                                    workspace.folder_path,
                                    workspace.worktree_mode,
                                    LayoutNode::Leaf {
                                        terminal_id: terminal_id.clone(),
                                    },
                                    terminal_id,
                                );
                            }
                        }
                        // No terminal available — workspace silently dropped.
                    }

                    // Handle remaining unassigned live terminals.
                    if missing_iter.peek().is_some() {
                        if self.workspaces.count() == 0 {
                            if let Some(first_id) = missing_iter.next() {
                                if assigned_terminal_ids.insert(first_id.clone()) {
                                    self.terminals.add_to_workspace(
                                        first_id.clone(),
                                        rows,
                                        cols,
                                        "w-default".to_string(),
                                    );
                                    self.workspaces.add_with_details(
                                        "w-default".to_string(),
                                        "Workspace 1".to_string(),
                                        default_workspace_folder(),
                                        false,
                                        LayoutNode::Leaf {
                                            terminal_id: first_id.clone(),
                                        },
                                        first_id,
                                    );
                                }
                            }
                        }

                        let target_workspace_id = merged
                            .active_workspace_id
                            .clone()
                            .or_else(|| self.workspaces.active_id().map(str::to_string))
                            .unwrap_or_else(|| "w-default".to_string());

                        for terminal_id in missing_iter {
                            if !assigned_terminal_ids.insert(terminal_id.clone()) {
                                continue;
                            }
                            self.terminals.add_to_workspace(
                                terminal_id.clone(),
                                rows,
                                cols,
                                target_workspace_id.clone(),
                            );
                            if let Some(offset) = restored_scrollback_offsets.get(&terminal_id) {
                                if let Some(term) = self.terminals.get_mut(&terminal_id) {
                                    term.scrollback_offset = *offset;
                                }
                            }
                            if let Some(workspace) =
                                self.workspaces.get_mut(&target_workspace_id)
                            {
                                if let Some(anchor) = workspace
                                    .layout
                                    .all_leaf_ids()
                                    .first()
                                    .map(|id| (*id).to_string())
                                {
                                    workspace.layout.split_leaf(
                                        &anchor,
                                        terminal_id.clone(),
                                        SplitDirection::Vertical,
                                    );
                                } else {
                                    workspace.layout = LayoutNode::Leaf {
                                        terminal_id: terminal_id.clone(),
                                    };
                                }
                                workspace.focused_terminal = terminal_id.clone();
                            }
                        }
                    }

                    let first_workspace_id =
                        self.workspaces.iter().next().map(|workspace| workspace.id.clone());

                    if let Some(active_workspace_id) = merged.active_workspace_id {
                        self.workspaces.set_active(&active_workspace_id);
                        self.last_test_workspace_id = Some(active_workspace_id);
                    } else if let Some(first_workspace_id) = first_workspace_id {
                        self.workspaces.set_active(&first_workspace_id);
                        self.last_test_workspace_id = Some(first_workspace_id);
                    }

                    if let Some(active_terminal_id) = merged.active_terminal_id {
                        self.terminals.set_active(&active_terminal_id);
                        self.last_test_terminal_id = Some(active_terminal_id);
                    } else if let Some(active_workspace) = self.workspaces.active() {
                        self.terminals.set_active(&active_workspace.focused_terminal);
                    }

                    // Apply persisted worktree paths to restored terminals.
                    for (terminal_id, path) in worktree_paths {
                        self.terminals.set_worktree_path(&terminal_id, path);
                    }
                }

                if self.workspaces.count() == 0 && !session_ids.is_empty() {
                    let first_id = session_ids[0].clone();
                    for terminal_id in &session_ids {
                        self.terminals.add_to_workspace(
                            terminal_id.clone(),
                            rows,
                            cols,
                            "w-default".to_string(),
                        );
                        if let Some(offset) = restored_scrollback_offsets.get(terminal_id) {
                            if let Some(term) = self.terminals.get_mut(terminal_id) {
                                term.scrollback_offset = *offset;
                            }
                        }
                    }
                    self.workspaces.add(
                        "w-default".to_string(),
                        "Workspace 1".to_string(),
                        first_id.clone(),
                    );
                    if let Some(workspace) = self.workspaces.get_mut("w-default") {
                        for terminal_id in session_ids.iter().skip(1) {
                            workspace.layout.split_leaf(
                                &first_id,
                                terminal_id.clone(),
                                SplitDirection::Vertical,
                            );
                        }
                    }
                    self.workspaces.set_active("w-default");
                    self.terminals.set_active(&first_id);
                    self.last_test_workspace_id = Some("w-default".to_string());
                    self.last_test_terminal_id = Some(first_id);
                }

                let plan = scrollback_restore::build_recovery_fetch_plan(
                    &session_ids,
                    &restored_scrollback_offsets,
                );
                let tasks: Vec<Task<Message>> = plan
                    .into_iter()
                    .map(|action| match action {
                        scrollback_restore::RecoveryFetchAction::FetchGrid { session_id } => {
                            self.fetch_grid(&session_id)
                        }
                        scrollback_restore::RecoveryFetchAction::ScrollFetch {
                            session_id,
                            offset,
                        } => self.scroll_fetch(&session_id, offset),
                    })
                    .collect();
                Task::batch(tasks)
            }
        }
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
        self.persist_scrollback_offsets();

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
        self.persist_scrollback_offsets();

        self.scroll_fetch(&active_id, max)
    }

    fn scroll_to_bottom(&mut self) -> Task<Message> {
        let Some(active_id) = self.active_focused().map(str::to_string) else {
            return Task::none();
        };

        if let Some(term) = self.terminals.get_mut(&active_id) {
            term.scrollback_offset = 0;
        }
        self.persist_scrollback_offsets();

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
        let session_id = uuid::Uuid::new_v4().to_string();
        if let Some(ws) = self.workspaces.active() {
            if ws.worktree_mode {
                return self.create_terminal_task_with_worktree(session_id, ws.folder_path.clone());
            }
        }
        let cwd = self.workspaces.active().map(|ws| ws.folder_path.clone());
        self.create_terminal_task_with_cwd(session_id, cwd)
    }

    fn create_terminal_task(&self, session_id: String) -> Task<Message> {
        if let Some(ws) = self.workspaces.active() {
            if ws.worktree_mode {
                return self.create_terminal_task_with_worktree(session_id, ws.folder_path.clone());
            }
        }
        let cwd = self.workspaces.active().map(|ws| ws.folder_path.clone());
        self.create_terminal_task_with_cwd(session_id, cwd)
    }

    fn create_terminal_task_with_cwd(
        &self,
        session_id: String,
        cwd: Option<String>,
    ) -> Task<Message> {
        let Some(client) = &self.client else {
            return Task::done(Message::TerminalCreated(Err(
                "No daemon connection".to_string()
            )));
        };

        let client = Arc::clone(client);
        let (rows, cols) = self.terminal_grid_size(Some(session_id.as_str()));

        Task::perform(
            async move {
                let (tx, rx) = futures_channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = commands::create_terminal(
                        &client,
                        &session_id,
                        godly_protocol::ShellType::Windows,
                        cwd.as_deref(),
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

    fn create_terminal_task_with_worktree(
        &self,
        session_id: String,
        repo_folder: String,
    ) -> Task<Message> {
        let Some(client) = &self.client else {
            return Task::done(Message::TerminalCreated(Err(
                "No daemon connection".to_string(),
            )));
        };
        let client = Arc::clone(client);
        let (rows, cols) = self.terminal_grid_size(Some(session_id.as_str()));

        Task::perform(
            async move {
                let (tx, rx) =
                    futures_channel::oneshot::channel::<Result<(String, Option<String>), String>>();
                std::thread::spawn(move || {
                    // Check if it's a git repo.
                    if !crate::git_worktree::is_git_repo(&repo_folder) {
                        log::warn!("Worktree mode enabled but not a git repo: {repo_folder}");
                        let result = commands::create_terminal(
                            &client,
                            &session_id,
                            godly_protocol::ShellType::Windows,
                            Some(&repo_folder),
                            rows,
                            cols,
                        )
                        .map(|_| (session_id, None::<String>));
                        let _ = tx.send(result);
                        return;
                    }

                    // Create worktree in detached HEAD.
                    let dir_name = crate::git_worktree::generate_worktree_dir_name();
                    match crate::git_worktree::create_worktree(&repo_folder, &dir_name) {
                        Ok(worktree_path) => {
                            let result = commands::create_terminal(
                                &client,
                                &session_id,
                                godly_protocol::ShellType::Windows,
                                Some(&worktree_path),
                                rows,
                                cols,
                            )
                            .map(|_| (session_id, Some(worktree_path)));
                            let _ = tx.send(result);
                        }
                        Err(e) => {
                            log::warn!("Worktree creation failed, falling back: {e}");
                            let result = commands::create_terminal(
                                &client,
                                &session_id,
                                godly_protocol::ShellType::Windows,
                                Some(&repo_folder),
                                rows,
                                cols,
                            )
                            .map(|_| (session_id, None::<String>));
                            let _ = tx.send(result);
                        }
                    }
                });
                rx.await
                    .unwrap_or_else(|_| Err("Background thread panicked".into()))
            },
            |result| match result {
                Ok((session_id, Some(worktree_path))) => Message::WorktreeCreated {
                    session_id,
                    worktree_path,
                },
                Ok((session_id, None)) => Message::TerminalCreated(Ok(session_id)),
                Err(e) => Message::TerminalCreated(Err(e)),
            },
        )
    }

    pub(crate) fn create_workspace_for_testing(
        &mut self,
        name: String,
        folder_path: String,
    ) -> Result<(String, Task<Message>), String> {
        let client = Arc::clone(
            self.client
                .as_ref()
                .ok_or_else(|| "No daemon connection".to_string())?,
        );
        let workspace_id = uuid::Uuid::new_v4().to_string();
        let session_id = uuid::Uuid::new_v4().to_string();
        let (rows, cols) = self.terminal_grid_size(Some(session_id.as_str()));

        commands::create_terminal(
            &client,
            &session_id,
            godly_protocol::ShellType::Windows,
            Some(folder_path.as_str()),
            rows,
            cols,
        )?;

        self.terminals.add_to_workspace(
            session_id.clone(),
            rows,
            cols,
            workspace_id.clone(),
        );
        self.workspaces.add_with_details(
            workspace_id.clone(),
            name,
            folder_path,
            false,
            LayoutNode::Leaf {
                terminal_id: session_id.clone(),
            },
            session_id.clone(),
        );
        self.workspaces.set_active(&workspace_id);
        self.terminals.set_active(&session_id);
        self.next_workspace_num = self.next_workspace_num.saturating_add(1);
        self.last_test_workspace_id = Some(workspace_id.clone());
        self.last_test_terminal_id = Some(session_id.clone());

        Ok((workspace_id, self.fetch_grid(&session_id)))
    }

    pub(crate) fn create_terminal_for_testing(
        &mut self,
        workspace_id: &str,
        cwd: Option<String>,
        name: Option<String>,
    ) -> Result<(String, Task<Message>), String> {
        let folder_path = self
            .workspaces
            .get(workspace_id)
            .map(|workspace| workspace.folder_path.clone())
            .ok_or_else(|| format!("Workspace {} not found", workspace_id))?;
        let client = Arc::clone(
            self.client
                .as_ref()
                .ok_or_else(|| "No daemon connection".to_string())?,
        );
        let session_id = uuid::Uuid::new_v4().to_string();
        let (rows, cols) = self.terminal_grid_size(Some(session_id.as_str()));
        let effective_cwd = cwd.unwrap_or(folder_path);

        commands::create_terminal(
            &client,
            &session_id,
            godly_protocol::ShellType::Windows,
            Some(effective_cwd.as_str()),
            rows,
            cols,
        )?;

        self.terminals.add_to_workspace(
            session_id.clone(),
            rows,
            cols,
            workspace_id.to_string(),
        );
        if let Some(custom_name) = name {
            self.terminals.rename(&session_id, Some(custom_name));
        }
        if let Some(workspace) = self.workspaces.get_mut(workspace_id) {
            workspace.layout = LayoutNode::Leaf {
                terminal_id: session_id.clone(),
            };
            workspace.focused_terminal = session_id.clone();
        }
        self.workspaces.set_active(workspace_id);
        self.terminals.set_active(&session_id);
        self.last_test_workspace_id = Some(workspace_id.to_string());
        self.last_test_terminal_id = Some(session_id.clone());

        Ok((session_id.clone(), self.fetch_grid(&session_id)))
    }

    pub(crate) fn reset_staging_profile_for_testing(&mut self) -> Result<Task<Message>, String> {
        let client = Arc::clone(
            self.client
                .as_ref()
                .ok_or_else(|| "No daemon connection".to_string())?,
        );
        for session in commands::list_sessions(&client)?
            .into_iter()
            .filter(|session| session.running)
        {
            if let Err(error) = commands::close_terminal(&client, &session.id) {
                log::warn!("Failed to close session {} during reset: {}", session.id, error);
            }
        }

        session_persistence::clear_default_path()?;
        scrollback_restore::clear_offsets()?;

        let result = InitResult::Fresh {
            session_id: create_fresh_session(
                &client,
                self.calculate_rows(),
                self.calculate_cols(),
            )?,
        };
        Ok(self.apply_init_result(result))
    }

    pub(crate) fn perform_test_restart(&mut self) -> Result<Task<Message>, String> {
        let client = Arc::clone(
            self.client
                .as_ref()
                .ok_or_else(|| "No daemon connection".to_string())?,
        );
        self.save_layout_for_testing()?;
        let session_ids: Vec<String> = self.terminals.iter().map(|term| term.id.clone()).collect();
        commands::detach_all_sessions(&client, &session_ids)?;

        let result = collect_init_result_sync(
            &client,
            self.calculate_rows(),
            self.calculate_cols(),
        )?;
        Ok(self.apply_init_result(result))
    }

    pub(crate) fn close_terminal(&mut self, session_id: &str) -> Task<Message> {
        // If this terminal has a worktree, show confirmation dialog.
        if let Some(term) = self.terminals.get(session_id) {
            if term.worktree_path.is_some() {
                self.worktree_close_pending = Some(session_id.to_string());
                return Task::none();
            }
        }
        self.close_terminal_immediate(session_id)
    }

    fn close_terminal_immediate(&mut self, session_id: &str) -> Task<Message> {
        if self.dragging_tab_id.as_deref() == Some(session_id) {
            self.dragging_tab_id = None;
            self.tab_drag_start_pos = None;
        }
        if self.tab_context_menu_id.as_deref() == Some(session_id) {
            self.tab_context_menu_id = None;
        }
        if self.rename_tab_id.as_deref() == Some(session_id) {
            self.rename_tab_id = None;
            self.rename_tab_value.clear();
        }

        // Remove from workspace layout.
        let mut root_leaf_removed = false;
        if let Some(ws) = self.workspaces.active_mut() {
            let decision =
                layout_reducer::reduce_close_terminal(layout_reducer::CloseTerminalInput {
                    layout: ws.layout.clone(),
                    focused_terminal_id: ws.focused_terminal.clone(),
                    closing_terminal_id: session_id.to_string(),
                });
            ws.layout = decision.next_layout;
            root_leaf_removed = decision.root_leaf_removed;
            if let Some(next_focused_terminal_id) = decision.next_focused_terminal_id {
                ws.focused_terminal = next_focused_terminal_id;
            }
        }

        self.terminals.remove(session_id);

        // The closing terminal was the sole root leaf — replace the layout
        // with the next available workspace terminal, or clear focus for the
        // empty state.
        if root_leaf_removed {
            if let Some(ws) = self.workspaces.active_mut() {
                let next_id = self
                    .terminals
                    .terminals_for_workspace(&ws.id)
                    .first()
                    .map(|t| t.id.clone());
                if let Some(next_id) = next_id {
                    ws.layout = LayoutNode::Leaf {
                        terminal_id: next_id.clone(),
                    };
                    ws.focused_terminal = next_id;
                } else {
                    ws.focused_terminal = String::new();
                }
            }
        }
        self.notifications.clear(session_id);
        self.last_terminal_sound_ms.remove(session_id);
        self.persist_scrollback_offsets();

        if let Some(client) = &self.client {
            let _ = commands::close_terminal(client, session_id);
        }

        Task::none()
    }

    fn handle_terminal_created(&mut self, result: Result<String, String>) -> Task<Message> {
        match result {
            Ok(session_id) => {
                let ws_id = self.workspaces.active_id().map(str::to_string);
                let in_layout = self
                    .active_layout()
                    .map(|layout| layout.find_leaf(&session_id))
                    .unwrap_or(false);
                let decision =
                    tab_reducer::reduce_terminal_created(tab_reducer::TerminalCreatedInput {
                        session_id,
                        active_workspace_id: ws_id.clone(),
                        terminal_in_active_layout: in_layout,
                    });
                if let Some(workspace_mutation) = decision.workspace_mutation {
                    if let Some(ws) = self.workspaces.active_mut() {
                        match workspace_mutation {
                            tab_reducer::WorkspaceTerminalMutation::FocusTerminal => {
                                ws.focused_terminal = decision.session_id.clone();
                            }
                            tab_reducer::WorkspaceTerminalMutation::ResetLayoutToSingleTerminal => {
                                ws.layout = LayoutNode::Leaf {
                                    terminal_id: decision.session_id.clone(),
                                };
                                ws.focused_terminal = decision.session_id.clone();
                            }
                        }
                    }
                }
                let (rows, cols) = self.terminal_grid_size(Some(decision.session_id.as_str()));
                if let Some(ws_id) = &decision.assign_workspace_id {
                    self.terminals.add_to_workspace(
                        decision.session_id.clone(),
                        rows,
                        cols,
                        ws_id.clone(),
                    );
                } else {
                    self.terminals.add(decision.session_id.clone(), rows, cols);
                }

                if decision.set_terminal_active {
                    self.terminals.set_active(&decision.session_id);
                }
                // Start tab entry animation.
                self.entering_tabs
                    .insert(decision.session_id.clone(), Self::now_ms());

                // Auto-launch AI tool if workspace has a mode set.
                if let Some(ws_id) = &ws_id {
                    if let Some(client) = &self.client {
                        let cmd = match self.workspace_ai_modes.get(ws_id) {
                            Some(AiToolMode::Claude) => Some("claude\r"),
                            Some(AiToolMode::Codex) => Some("codex\r"),
                            Some(AiToolMode::Both) => Some("claude\r"),
                            Some(AiToolMode::None) | None => None,
                        };
                        if let Some(cmd) = cmd {
                            if let Err(e) = commands::write_to_terminal(
                                client,
                                &decision.session_id,
                                cmd.as_bytes(),
                            ) {
                                log::warn!("Failed to auto-launch AI tool: {e}");
                            }
                        }
                    }
                }

                self.fetch_grid(&decision.fetch_grid_terminal_id)
            }
            Err(e) => {
                log::error!("Failed to create terminal: {}", e);
                Task::none()
            }
        }
    }

    // -----------------------------------------------------------------------
    // Resize
    // -----------------------------------------------------------------------

    pub(crate) fn resize_all_terminals(&mut self) -> Task<Message> {
        let ids: Vec<String> = self.terminals.iter().map(|t| t.id.clone()).collect();

        for id in &ids {
            let (new_rows, new_cols) = self.terminal_grid_size(Some(id.as_str()));
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
        let (_, cols) = self.terminal_grid_size(self.target_terminal_id());
        cols
    }

    fn calculate_rows(&self) -> u16 {
        let (rows, _) = self.terminal_grid_size(self.target_terminal_id());
        rows
    }

    // -----------------------------------------------------------------------
    // Split pane operations (workspace-aware)
    // -----------------------------------------------------------------------

    fn split_focused_pane(&mut self, direction: SplitDirection) -> Task<Message> {
        let decision = layout_reducer::reduce_split_focused(layout_reducer::SplitFocusedInput {
            focused_terminal_id: self.active_focused().map(str::to_string),
            new_terminal_id: uuid::Uuid::new_v4().to_string(),
            direction,
        });
        let Some(decision) = decision else {
            return Task::none();
        };

        // Split the active workspace's layout tree.
        if let Some(ws) = self.workspaces.active_mut() {
            ws.layout.split_leaf(
                &decision.focused_terminal_id,
                decision.new_terminal_id.clone(),
                decision.direction,
            );
        }

        let create_task = self.create_terminal_task(decision.new_terminal_id);
        let resize_task = self.resize_all_terminals();
        Task::batch([create_task, resize_task])
    }

    fn unsplit_focused_pane(&mut self) -> Task<Message> {
        let decision =
            layout_reducer::reduce_unsplit_focused(layout_reducer::UnsplitFocusedInput {
                layout: self.active_layout().cloned(),
                focused_terminal_id: self.active_focused().map(str::to_string),
            });
        let Some(decision) = decision else {
            return Task::none();
        };

        if let Some(ws) = self.workspaces.active_mut() {
            ws.layout = decision.next_layout;
            if let Some(next_focused_terminal_id) = decision.next_focused_terminal_id {
                ws.focused_terminal = next_focused_terminal_id;
            }
        }
        self.terminals.remove(&decision.removed_terminal_id);
        self.notifications.clear(&decision.removed_terminal_id);
        self.last_terminal_sound_ms
            .remove(&decision.removed_terminal_id);
        self.persist_scrollback_offsets();
        if let Some(client) = &self.client {
            let _ = commands::close_terminal(client, &decision.removed_terminal_id);
        }

        self.resize_all_terminals()
    }

    fn cycle_focus(&mut self) {
        let next_id =
            layout_reducer::reduce_cycle_focus(self.active_layout(), self.active_focused());
        if let Some(next_id) = next_id {
            self.notifications.mark_read(&next_id);
            if let Some(ws) = self.workspaces.active_mut() {
                ws.focused_terminal = next_id;
            }
        }
    }

    fn focus_in_direction(&mut self, direction: FocusDirection) {
        let next_id = layout_reducer::reduce_directional_focus(
            self.active_layout(),
            self.active_focused(),
            direction,
        );
        if let Some(next_id) = next_id {
            self.notifications.mark_read(&next_id);
            if let Some(ws) = self.workspaces.active_mut() {
                ws.focused_terminal = next_id;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Quick Claude Launcher
    // -----------------------------------------------------------------------

    fn execute_next_launch_step(&mut self) -> Task<Message> {
        let launch = match &self.quick_claude_launch {
            Some(l) if !l.completed && l.error.is_none() => l,
            _ => return Task::none(),
        };

        let step_index = launch.current_step;
        if step_index >= launch.steps.len() {
            return self.finalize_launch();
        }

        let step = launch.steps[step_index].clone();
        let agent_ids = launch.agent_terminal_ids.clone();

        let Some(client) = &self.client else {
            return Task::none();
        };
        let client = Arc::clone(client);
        let (rows, cols) = self.terminal_grid_size(None);

        Task::perform(
            async move {
                let (tx, rx) = futures_channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = crate::quick_claude::execute_step(
                        client, step, agent_ids, rows, cols,
                    );
                    let _ = tx.send(result);
                });
                rx.await.unwrap_or_else(|_| Err("Launch step thread panicked".into()))
            },
            Message::QuickClaudeLaunchStepComplete,
        )
    }

    fn handle_launch_step_result(
        &mut self,
        result: Result<crate::quick_claude::StepResult, String>,
    ) -> Task<Message> {
        if self.quick_claude_launch.is_none() {
            return Task::none();
        }

        match result {
            Ok(step_result) => {
                let (agent_index_opt, placeholder_opt, ws_id) = {
                    let launch = self.quick_claude_launch.as_ref().unwrap();
                    let si = launch.current_step;
                    let ai = launch.steps.get(si).and_then(|step| {
                        if let crate::quick_claude::LaunchStep::CreateTerminal { agent_index, .. } = step {
                            Some(*agent_index)
                        } else {
                            None
                        }
                    });
                    let ph = if ai == Some(0) {
                        launch.agent_terminal_ids.get(0).and_then(|o| o.clone())
                    } else {
                        None
                    };
                    let wid = launch.workspace_id.clone();
                    (ai, ph, wid)
                };

                if let crate::quick_claude::StepResult::TerminalCreated(ref session_id) = step_result {
                    if let Some(agent_index) = agent_index_opt {
                        if agent_index == 0 {
                            if let Some(placeholder) = placeholder_opt {
                                if let Some(ws) = self.workspaces.get_mut(&ws_id) {
                                    replace_leaf_in_layout(
                                        &mut ws.layout,
                                        &placeholder,
                                        session_id.clone(),
                                    );
                                    ws.focused_terminal = session_id.clone();
                                }
                                self.terminals.remove(&placeholder);
                            }
                        }

                        if let Some(launch) = &mut self.quick_claude_launch {
                            launch.agent_terminal_ids[agent_index] = Some(session_id.clone());
                        }

                        let (rows, cols) = self.terminal_grid_size(Some(session_id.as_str()));
                        self.terminals.add_to_workspace(
                            session_id.clone(),
                            rows,
                            cols,
                            ws_id,
                        );
                        self.terminals.set_active(session_id);
                        self.entering_tabs
                            .insert(session_id.clone(), Self::now_ms());
                    }
                }

                if let Some(launch) = &mut self.quick_claude_launch {
                    launch.current_step += 1;
                }
                self.execute_next_launch_step()
            }
            Err(e) => {
                log::error!("Quick Claude launch step failed: {}", e);
                self.enqueue_toast(
                    "Quick Claude Failed".to_string(),
                    e,
                );
                self.quick_claude_launch = None;
                Task::none()
            }
        }
    }

    fn finalize_launch(&mut self) -> Task<Message> {
        let launch = match &mut self.quick_claude_launch {
            Some(l) => l,
            None => return Task::none(),
        };
        launch.completed = true;

        let terminal_ids: Vec<String> = launch
            .agent_terminal_ids
            .iter()
            .filter_map(|opt| opt.clone())
            .collect();

        if terminal_ids.is_empty() {
            self.quick_claude_launch = None;
            return Task::none();
        }

        let num_agents = terminal_ids.len();
        let ws_id = launch.workspace_id.clone();

        let two_agent_direction = {
            let preset_name = launch.preset_name.clone();
            let matching_preset = self
                .quick_claude_presets
                .iter()
                .find(|p| p.name == preset_name);
            match matching_preset.map(|p| p.layout) {
                Some(QuickClaudeLayout::HSplit) => SplitDirection::Vertical,
                _ => SplitDirection::Horizontal,
            }
        };

        if let Some(ws) = self.workspaces.get_mut(&ws_id) {
            match num_agents {
                1 => {
                    ws.layout = LayoutNode::Leaf {
                        terminal_id: terminal_ids[0].clone(),
                    };
                    ws.focused_terminal = terminal_ids[0].clone();
                }
                2 => {
                    ws.layout = LayoutNode::Leaf {
                        terminal_id: terminal_ids[0].clone(),
                    };
                    ws.layout.split_leaf(
                        &terminal_ids[0],
                        terminal_ids[1].clone(),
                        two_agent_direction,
                    );
                    ws.focused_terminal = terminal_ids[0].clone();
                }
                4 => {
                    ws.layout = LayoutNode::Split {
                        direction: SplitDirection::Horizontal,
                        ratio: 0.5,
                        first: Box::new(LayoutNode::Split {
                            direction: SplitDirection::Vertical,
                            ratio: 0.5,
                            first: Box::new(LayoutNode::Leaf {
                                terminal_id: terminal_ids[0].clone(),
                            }),
                            second: Box::new(LayoutNode::Leaf {
                                terminal_id: terminal_ids[2].clone(),
                            }),
                        }),
                        second: Box::new(LayoutNode::Split {
                            direction: SplitDirection::Vertical,
                            ratio: 0.5,
                            first: Box::new(LayoutNode::Leaf {
                                terminal_id: terminal_ids[1].clone(),
                            }),
                            second: Box::new(LayoutNode::Leaf {
                                terminal_id: terminal_ids[3].clone(),
                            }),
                        }),
                    };
                    ws.focused_terminal = terminal_ids[0].clone();
                }
                _ => {
                    ws.layout = LayoutNode::Leaf {
                        terminal_id: terminal_ids[0].clone(),
                    };
                    ws.focused_terminal = terminal_ids[0].clone();
                }
            }
        }

        self.quick_claude_launch = None;
        self.resize_all_terminals()
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

        if text.len() > crate::confirm_dialog::COPY_PREVIEW_THRESHOLD {
            self.copy_preview_text = Some(text);
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

    // -----------------------------------------------------------------------
    // Pixel renderer helpers
    // -----------------------------------------------------------------------

    /// Pre-render a terminal's grid into an RGBA pixel buffer and cache the
    /// resulting `Handle` on the `TerminalInfo`. Called from `update()` after
    /// grid data changes (GridFetched, selection changes, etc.).
    fn render_terminal_image(&mut self, session_id: &str) {
        let grid_clone = match self.terminals.get(session_id).and_then(|t| t.grid.clone()) {
            Some(g) => g,
            None => return,
        };
        let metrics = self.font_metrics;
        let palette = crate::theme::active_terminal_palette();
        let default_fg = palette.foreground;
        let default_bg = palette.background;

        let (pixels, w, h) = self.pixel_renderer.render(
            &grid_clone,
            &metrics,
            &mut self.glyph_cache,
            &mut self.glyph_rasterizer,
            default_fg,
            default_bg,
            None, // selection handled separately in view for now
        );

        if w > 0 && h > 0 {
            let handle = iced::widget::image::Handle::from_rgba(w, h, pixels.to_vec());
            if let Some(term) = self.terminals.get_mut(session_id) {
                term.cached_image_handle = Some(handle);
            }
        }
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

        let term_palette = crate::theme::active_terminal_palette();
        let tc = TerminalCanvas {
            grid: term.grid.as_ref(),
            metrics: self.font_metrics,
            selection,
            default_fg: term_palette.foreground,
            default_bg: term_palette.background,
            on_redraw: Some(Message::Heartbeat),
            font: self.terminal_font,
        };

        let accent_color = if is_focused {
            PANE_FOCUSED_BORDER()
        } else {
            Color::TRANSPARENT
        };

        let accent_bar = container(Space::new().width(Length::Fill).height(Length::Fixed(
            if is_focused { 1.0 } else { 0.0 },
        )))
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(accent_color)),
            ..container::Style::default()
        });

        // Choose between pixel-rendered Image and canvas-based rendering.
        let inner = if self.use_pixel_renderer {
            if let Some(handle) = &term.cached_image_handle {
                let img = iced::widget::image::Image::new(handle.clone())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .content_fit(iced::ContentFit::None)
                    .filter_method(iced::widget::image::FilterMethod::Nearest);
                column![accent_bar, img]
            } else {
                // No cached handle yet — fall back to canvas
                column![
                    accent_bar,
                    canvas(tc).width(Length::Fill).height(Length::Fill),
                ]
            }
        } else {
            column![
                accent_bar,
                canvas(tc).width(Length::Fill).height(Length::Fill),
            ]
        };

        let pane = container(inner)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding {
                top: TERMINAL_VIEWPORT_INSET_Y,
                right: TERMINAL_VIEWPORT_INSET_X,
                bottom: TERMINAL_VIEWPORT_INSET_Y,
                left: TERMINAL_VIEWPORT_INSET_X,
            })
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(PANE_BG())),
                ..container::Style::default()
            });

        let tid = terminal_id.to_string();
        mouse_area(pane)
            .on_press(Message::PaneClicked(tid))
            .into()
    }

    fn view_terminal_empty_state(&self) -> Element<'_, Message> {
        let card = container(
            column![
                text("No terminals in this workspace")
                    .size(22)
                    .color(TEXT_ACTIVE()),
                text("Create a terminal to start working.")
                    .size(13)
                    .color(TEXT_SECONDARY()),
                text("Press Ctrl+T to create one")
                    .size(11)
                    .color(TEXT_SECONDARY()),
                button(text("Create terminal").size(13).color(BG_SECONDARY()))
                    .on_press(Message::NewTabRequested)
                    .padding(Padding::from([8, 14]))
                    .style(|_theme, status| {
                        let background = match status {
                            button::Status::Hovered | button::Status::Pressed => {
                                iced::Background::Color(PANE_FOCUSED_BORDER())
                            }
                            _ => iced::Background::Color(ACCENT()),
                        };

                        button::Style {
                            background: Some(background),
                            text_color: BG_SECONDARY(),
                            border: iced::Border {
                                color: iced::Color::TRANSPARENT,
                                width: 0.0,
                                radius: 6.0.into(),
                            },
                            ..button::Style::default()
                        }
                    }),
            ]
            .spacing(12)
            .width(Length::Fill),
        )
        .padding(Padding::from([28, 20]))
        .width(Length::Fixed(EMPTY_STATE_CARD_WIDTH))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(EMPTY_STATE_BG())),
            border: iced::Border {
                color: Color::from_rgba(PANE_BORDER().r, PANE_BORDER().g, PANE_BORDER().b, 0.5),
                width: 0.5,
                radius: 10.0.into(),
            },
            ..container::Style::default()
        });

        container(center(card))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn render_terminal_leaf_with_drop_overlay<'a>(
        &'a self,
        terminal_id: &str,
        focused_id: Option<&str>,
    ) -> Element<'a, Message> {
        let base = self.render_terminal_pane(terminal_id, focused_id);

        let Some(dragged_terminal_id) = self.dragging_tab_id.as_deref() else {
            return base;
        };
        // Don't show overlay until drag is committed (mouse moved ≥5 px).
        if self.tab_drag_start_pos.is_some() {
            return base;
        }
        if dragged_terminal_id == terminal_id {
            return base;
        }

        let overlay_zone = |placement: SplitPlacement, alpha: f32| -> Element<'a, Message> {
            let target_terminal_id = terminal_id.to_string();
            mouse_area(
                container(Space::new().width(Length::Fill).height(Length::Fill))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(move |_theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgba(
                            ACCENT().r, ACCENT().g, ACCENT().b, alpha,
                        ))),
                        ..container::Style::default()
                    }),
            )
            .on_enter(Message::TabSplitZoneHover {
                target_terminal_id,
                placement,
            })
            .into()
        };

        let left = container(overlay_zone(SplitPlacement::Left, 0.16))
            .width(Length::FillPortion(22))
            .height(Length::Fill);
        let right = container(overlay_zone(SplitPlacement::Right, 0.16))
            .width(Length::FillPortion(22))
            .height(Length::Fill);
        let top = container(overlay_zone(SplitPlacement::Top, 0.22))
            .width(Length::Fill)
            .height(Length::FillPortion(26));
        let bottom = container(overlay_zone(SplitPlacement::Bottom, 0.22))
            .width(Length::Fill)
            .height(Length::FillPortion(26));
        let middle = container(Space::new().width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill);
        let center = column![top, middle, bottom]
            .width(Length::FillPortion(56))
            .height(Length::Fill);

        let overlay = row![left, center, right]
            .width(Length::Fill)
            .height(Length::Fill);

        stack![base, overlay].into()
    }

    fn has_selection(&self) -> bool {
        let (start, end) = self.selection.normalized();
        start != end
    }
}

fn begin_sidebar_animation(
    current_width: f32,
    target_width: f32,
    started_at_ms: u64,
) -> Option<SidebarAnimation> {
    if (current_width - target_width).abs() < 0.5 {
        None
    } else {
        Some(SidebarAnimation {
            from_width: current_width,
            to_width: target_width,
            started_at_ms,
        })
    }
}

fn sidebar_animation_progress(animation: SidebarAnimation, now_ms: u64) -> f32 {
    let elapsed = now_ms.saturating_sub(animation.started_at_ms) as f32;
    (elapsed / SIDEBAR_ANIMATION_DURATION_MS as f32).clamp(0.0, 1.0)
}

fn ease_in_out_cubic(progress: f32) -> f32 {
    if progress < 0.5 {
        4.0 * progress * progress * progress
    } else {
        1.0 - (-2.0 * progress + 2.0).powi(3) / 2.0
    }
}

fn sidebar_animation_finished(animation: SidebarAnimation, now_ms: u64) -> bool {
    sidebar_animation_progress(animation, now_ms) >= 1.0
}

fn resolved_sidebar_width(
    sidebar_visible: bool,
    sidebar_width: f32,
    sidebar_animation: Option<SidebarAnimation>,
    now_ms: u64,
) -> f32 {
    if let Some(animation) = sidebar_animation {
        let eased = ease_in_out_cubic(sidebar_animation_progress(animation, now_ms));
        return animation.from_width + (animation.to_width - animation.from_width) * eased;
    }

    if sidebar_visible {
        sidebar_width
    } else {
        0.0
    }
}

fn resolve_terminal_empty_state(
    active_layout: Option<&LayoutNode>,
    active_workspace_terminal_count: usize,
) -> Option<TerminalEmptyState> {
    match active_layout {
        Some(layout) if layout.leaf_count() > 0 && active_workspace_terminal_count > 0 => None,
        _ => Some(TerminalEmptyState::NoTerminalsOpen),
    }
}

fn terminal_content_rect(window_width: f32, window_height: f32, sidebar_width: f32) -> PaneRect {
    let top = title_bar::TITLE_BAR_HEIGHT + TAB_BAR_HEIGHT;
    let bottom = status_bar::STATUS_BAR_HEIGHT;
    PaneRect::new(
        sidebar_width.max(0.0),
        top,
        (window_width - sidebar_width).max(1.0),
        (window_height - top - bottom).max(1.0),
    )
}

fn split_ratio_to_portions(ratio: f32) -> (u16, u16) {
    let clamped = ratio.clamp(0.01, 0.99);
    let first = (clamped * 100.0).round() as u16;
    (first, 100 - first)
}

fn pane_rect_for_terminal(
    layout: &LayoutNode,
    terminal_id: &str,
    rect: PaneRect,
) -> Option<PaneRect> {
    match layout {
        LayoutNode::Leaf {
            terminal_id: leaf_id,
        } => (leaf_id == terminal_id).then_some(rect),
        LayoutNode::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let (first_portion, second_portion) = split_ratio_to_portions(*ratio);
            let total_portions = (first_portion + second_portion) as f32;

            let (first_rect, second_rect) = match direction {
                SplitDirection::Horizontal => {
                    let first_width = rect.width * first_portion as f32 / total_portions;
                    let second_width = rect.width - first_width;
                    (
                        PaneRect::new(rect.x, rect.y, first_width, rect.height),
                        PaneRect::new(rect.x + first_width, rect.y, second_width, rect.height),
                    )
                }
                SplitDirection::Vertical => {
                    let first_height = rect.height * first_portion as f32 / total_portions;
                    let second_height = rect.height - first_height;
                    (
                        PaneRect::new(rect.x, rect.y, rect.width, first_height),
                        PaneRect::new(rect.x, rect.y + first_height, rect.width, second_height),
                    )
                }
            };

            pane_rect_for_terminal(first, terminal_id, first_rect)
                .or_else(|| pane_rect_for_terminal(second, terminal_id, second_rect))
        }
    }
}

fn inset_terminal_pane_rect(rect: PaneRect) -> PaneRect {
    rect.inset(TERMINAL_VIEWPORT_INSET_X, TERMINAL_VIEWPORT_INSET_Y)
}

fn grid_dimensions_for_viewport(viewport: PaneRect, font_metrics: FontMetrics) -> (u16, u16) {
    let rows = (viewport.height / font_metrics.cell_height)
        .floor()
        .max(1.0) as u16;
    let cols = (viewport.width / font_metrics.cell_width).floor().max(1.0) as u16;
    (rows, cols)
}

fn pointer_to_grid(point: Point, viewport: PaneRect, font_metrics: FontMetrics) -> GridPos {
    let clamped = viewport.clamp_point(point);
    let local_x = (clamped.x - viewport.x).max(0.0);
    let local_y = (clamped.y - viewport.y).max(0.0);

    GridPos {
        row: (local_y / font_metrics.cell_height) as usize,
        col: (local_x / font_metrics.cell_width) as usize,
    }
}

fn quote_dropped_path(path: &std::path::Path) -> String {
    let raw = path.to_string_lossy();
    let escaped = raw.replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

fn normalize_mute_pattern(pattern: &str) -> Option<String> {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_ascii_lowercase())
    }
}

fn replace_leaf_in_layout(layout: &mut LayoutNode, old_id: &str, new_id: String) {
    match layout {
        LayoutNode::Leaf { terminal_id } if terminal_id == old_id => {
            *terminal_id = new_id;
        }
        LayoutNode::Split { first, second, .. } => {
            replace_leaf_in_layout(first, old_id, new_id.clone());
            replace_leaf_in_layout(second, old_id, new_id);
        }
        _ => {}
    }
}

fn normalize_quick_claude_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_ai_tool_field(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_plugin_field(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_flow_field(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_flow_steps(raw: &str) -> Vec<String> {
    raw.split([',', ';', '\n'])
        .filter_map(normalize_flow_field)
        .collect()
}

fn enqueue_toast_entry(
    toasts: &mut Vec<ToastNotification>,
    next_toast_id: &mut u64,
    title: String,
    message: String,
    now_ms: u64,
) -> u64 {
    let toast_id = *next_toast_id;
    *next_toast_id = next_toast_id.saturating_add(1);

    toasts.push(ToastNotification {
        id: toast_id,
        title,
        message,
        expires_at_ms: now_ms.saturating_add(TOAST_TTL_MS),
    });

    if toasts.len() > MAX_ACTIVE_TOASTS {
        let overflow = toasts.len() - MAX_ACTIVE_TOASTS;
        toasts.drain(0..overflow);
    }

    toast_id
}

fn prune_expired_toasts(toasts: &mut Vec<ToastNotification>, now_ms: u64) -> usize {
    let before = toasts.len();
    toasts.retain(|toast| toast.expires_at_ms > now_ms);
    before.saturating_sub(toasts.len())
}

fn toggle_plugin_enabled(plugins: &mut [PluginEntry], index: usize) -> bool {
    let Some(entry) = plugins.get_mut(index) else {
        return false;
    };
    entry.enabled = !entry.enabled;
    true
}

fn toggle_flow_enabled(flows: &mut [FlowEntry], index: usize) -> bool {
    let Some(entry) = flows.get_mut(index) else {
        return false;
    };
    entry.enabled = !entry.enabled;
    true
}

fn normalize_remote_field(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_remote_port(value: &str) -> Option<u16> {
    let parsed = value.trim().parse::<u16>().ok()?;
    if parsed == 0 {
        None
    } else {
        Some(parsed)
    }
}

fn toggle_remote_enabled(profiles: &mut [RemoteConnectionEntry], index: usize) -> bool {
    let Some(entry) = profiles.get_mut(index) else {
        return false;
    };
    entry.enabled = !entry.enabled;
    true
}

fn wildcard_matches(pattern: &str, candidate: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return candidate == pattern;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let starts_with_wildcard = pattern.starts_with('*');
    let ends_with_wildcard = pattern.ends_with('*');
    let mut cursor = 0usize;

    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        if index == 0 && !starts_with_wildcard {
            if !candidate[cursor..].starts_with(part) {
                return false;
            }
            cursor += part.len();
            continue;
        }

        if index == parts.len() - 1 && !ends_with_wildcard {
            let Some(found) = candidate[cursor..].rfind(part) else {
                return false;
            };
            let absolute = cursor + found;
            return absolute + part.len() == candidate.len();
        }

        let Some(found) = candidate[cursor..].find(part) else {
            return false;
        };
        cursor += found + part.len();
    }

    true
}

fn workspace_matches_mute_patterns(
    patterns: &[String],
    workspace_id: &str,
    workspace_name: &str,
) -> bool {
    let normalized_id = workspace_id.to_ascii_lowercase();
    let normalized_name = workspace_name.to_ascii_lowercase();
    patterns.iter().any(|pattern| {
        let normalized_pattern = pattern.to_ascii_lowercase();
        wildcard_matches(&normalized_pattern, &normalized_id)
            || wildcard_matches(&normalized_pattern, &normalized_name)
    })
}

fn is_escape_key(key: &keyboard::Key) -> bool {
    matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape))
}

// ---------------------------------------------------------------------------
// Pure keyboard-routing logic — extracted for testability
// ---------------------------------------------------------------------------

/// Result of processing a key event through the shortcut capture/resolve pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
enum KeyRoutingResult {
    /// A shortcut action should fire.
    Action(AppAction),
    /// Key was consumed by capture mode (binding recorded or cancelled).
    CapturedBinding,
    /// Key was consumed by an interceptor (MRU switcher, CLAUDE.md editor, etc.).
    Intercepted,
    /// No shortcut matched — forward to PTY.
    ForwardToPty,
}

/// Mutable state that the capture step may modify.
struct CaptureState {
    capturing_index: Option<usize>,
    overrides: HashMap<usize, String>,
    resolver: ShortcutResolver,
}

/// Pure function that determines what a key press should do.
///
/// Mirrors the logic in the `Message::KeyboardEvent(KeyPressed { .. })` arm of
/// `update()` but operates on minimal state so it can be tested without
/// constructing a full `GodlyApp`.
fn resolve_key_event(
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
    physical_key: &keyboard::key::Physical,
    capture: &mut CaptureState,
    mru_switcher_open: bool,
    claude_md_editor_open: bool,
    quick_claude_dialog_open: bool,
) -> KeyRoutingResult {
    // 1. Capture mode
    if let Some(cap_index) = capture.capturing_index {
        if is_escape_key(key) {
            capture.capturing_index = None;
            return KeyRoutingResult::CapturedBinding;
        }
        if matches!(
            key,
            keyboard::Key::Named(
                keyboard::key::Named::Control
                    | keyboard::key::Named::Shift
                    | keyboard::key::Named::Alt
                    | keyboard::key::Named::Super
            )
        ) {
            return KeyRoutingResult::CapturedBinding;
        }
        if modifiers.control() || modifiers.alt() {
            let display = shortcuts::normalize_chord(key, modifiers);
            capture.overrides.insert(cap_index, display);
            capture.resolver = ShortcutResolver::from_overrides(&capture.overrides);
        }
        capture.capturing_index = None;
        return KeyRoutingResult::CapturedBinding;
    }

    // 2. MRU switcher intercepts Ctrl+Tab
    if mru_cycle_direction_from_shortcut_key(key, modifiers).is_some() {
        return KeyRoutingResult::Intercepted;
    }
    if mru_switcher_open {
        return KeyRoutingResult::Intercepted;
    }

    // 3. CLAUDE.md editor intercepts Ctrl+S and Escape
    if claude_md_editor_open {
        return KeyRoutingResult::Intercepted;
    }

    // 3b. Quick Claude dialog intercepts all keys
    if quick_claude_dialog_open {
        return KeyRoutingResult::Intercepted;
    }

    // 4. Shortcut resolution (custom overrides first, then defaults)
    if let Some(action) = capture.resolver.resolve(key, modifiers, Some(physical_key)) {
        return KeyRoutingResult::Action(action);
    }

    KeyRoutingResult::ForwardToPty
}

fn is_control_key(key: &keyboard::Key) -> bool {
    matches!(key, keyboard::Key::Named(keyboard::key::Named::Control))
}

fn mru_cycle_direction_from_shortcut_key(
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<tab_reducer::TabMruCycleDirection> {
    if !matches!(key, keyboard::Key::Named(keyboard::key::Named::Tab))
        || !modifiers.control()
        || modifiers.alt()
    {
        return None;
    }

    if modifiers.shift() {
        Some(tab_reducer::TabMruCycleDirection::Backward)
    } else {
        Some(tab_reducer::TabMruCycleDirection::Forward)
    }
}

fn next_mru_switcher_selection(
    mru_terminal_ids: Vec<&str>,
    active_terminal_id: Option<&str>,
    current_selection_terminal_id: Option<&str>,
    direction: tab_reducer::TabMruCycleDirection,
) -> Option<String> {
    let cursor_terminal_id = current_selection_terminal_id
        .filter(|terminal_id| mru_terminal_ids.contains(terminal_id))
        .or_else(|| {
            active_terminal_id.filter(|terminal_id| mru_terminal_ids.contains(terminal_id))
        });
    next_tab_id_from_mru(mru_terminal_ids, cursor_terminal_id, direction)
}

fn should_commit_mru_switcher_on_key_release(
    popup_open: bool,
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> bool {
    popup_open && (is_control_key(key) || !modifiers.control())
}

fn should_commit_mru_switcher_on_modifiers_changed(
    popup_open: bool,
    modifiers: keyboard::Modifiers,
) -> bool {
    popup_open && !modifiers.control()
}

fn next_tab_id_from_mru(
    mru_terminal_ids: Vec<&str>,
    current_terminal_id: Option<&str>,
    direction: tab_reducer::TabMruCycleDirection,
) -> Option<String> {
    tab_reducer::reduce_tab_mru_cycle(tab_reducer::TabMruCycleInput {
        mru_terminal_ids: mru_terminal_ids.into_iter().map(str::to_string).collect(),
        current_terminal_id: current_terminal_id.map(str::to_string),
        direction,
    })
}

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

fn default_workspace_folder() -> String {
    std::env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| ".".to_string())
}

fn collect_init_result_sync(
    client: &NativeDaemonClient,
    rows: u16,
    cols: u16,
) -> Result<InitResult, String> {
    if let Some(recovered) = collect_recovered_init_result(client)? {
        return Ok(recovered);
    }

    // No live sessions — check persisted state for workspace definitions to restore.
    // This handles the rebuild/reinstall case: daemon has no sessions but the JSON
    // file remembers which workspaces the user had open (names, folder paths, etc.).
    if let Some(persisted) = session_persistence::load_from_default_path() {
        if !persisted.workspaces.is_empty() {
            log::info!(
                "Restoring {} workspace(s) from persisted state (no live sessions)",
                persisted.workspaces.len()
            );
            let mut new_session_ids = Vec::new();
            for workspace in &persisted.workspaces {
                let session_id = uuid::Uuid::new_v4().to_string();
                let cwd = if workspace.folder_path.is_empty() {
                    None
                } else {
                    Some(workspace.folder_path.as_str())
                };
                commands::create_terminal(
                    client,
                    &session_id,
                    godly_protocol::ShellType::Windows,
                    cwd,
                    rows,
                    cols,
                )?;
                new_session_ids.push(session_id);
            }

            // merge_with_live_sessions will produce empty layouts for each workspace
            // (since persisted terminal IDs won't match new UUIDs), and the new session
            // IDs will appear in missing_live_terminal_ids — apply_init_result fills
            // the empty workspaces from that pool.
            let merged =
                session_persistence::merge_with_live_sessions(&persisted, &new_session_ids);

            return Ok(InitResult::Recovered {
                session_ids: new_session_ids,
                restored_scrollback_offsets: HashMap::new(),
                merged_session_state: Some(merged),
            });
        }
    }

    let session_id = create_fresh_session(client, rows, cols)?;
    Ok(InitResult::Fresh { session_id })
}

fn collect_recovered_init_result(
    client: &NativeDaemonClient,
) -> Result<Option<InitResult>, String> {
    let sessions = commands::list_sessions(client)?;
    let live_session_ids: Vec<String> = sessions
        .into_iter()
        .filter(|session| session.running)
        .map(|session| session.id)
        .collect();

    if live_session_ids.is_empty() {
        return Ok(None);
    }

    let mut recovered_ids = Vec::new();
    for session_id in &live_session_ids {
        match commands::attach_session(client, session_id) {
            Ok(()) => {
                log::info!("Recovered session: {}", session_id);
                recovered_ids.push(session_id.clone());
            }
            Err(error) => {
                log::warn!("Failed to recover session {}: {}", session_id, error);
            }
        }
    }

    if recovered_ids.is_empty() {
        return Ok(None);
    }

    let persisted_offsets = scrollback_restore::load_offsets();
    let pruned_offsets =
        scrollback_restore::prune_offsets_for_live_sessions(&persisted_offsets, &live_session_ids);
    if pruned_offsets != persisted_offsets {
        if let Err(error) = scrollback_restore::save_offsets(&pruned_offsets) {
            log::warn!("Failed to persist pruned scrollback offsets: {}", error);
        }
    }

    let restored_scrollback_offsets =
        scrollback_restore::restored_offsets_for_recovered_sessions(&pruned_offsets, &recovered_ids);
    let merged_session_state = session_persistence::load_from_default_path().map(|persisted| {
        session_persistence::merge_with_live_sessions(&persisted, &recovered_ids)
    });

    Ok(Some(InitResult::Recovered {
        session_ids: recovered_ids,
        restored_scrollback_offsets,
        merged_session_state,
    }))
}

fn create_fresh_session(
    client: &NativeDaemonClient,
    rows: u16,
    cols: u16,
) -> Result<String, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    commands::create_terminal(
        client,
        &session_id,
        godly_protocol::ShellType::Windows,
        None,
        rows,
        cols,
    )?;
    Ok(session_id)
}

/// Initialize the app: connect to daemon, set up bridge, recover or create sessions.
pub fn initialize(app: &mut GodlyApp) -> Task<Message> {
    // Build the shortcut resolver from persisted overrides loaded in Default.
    app.shortcut_resolver = ShortcutResolver::from_overrides(&app.shortcut_overrides);

    let client = match NativeDaemonClient::connect_or_launch() {
        Ok(c) => Arc::new(c),
        Err(e) => {
            return Task::done(Message::Initialized(Err(e)));
        }
    };

    let (tx, rx) = mpsc::unbounded();
    *app.event_receiver.lock() = Some(rx);

    // Spawn heartbeat thread that keeps the event loop alive via the same
    // channel. Iced subscriptions stop being polled when the window is
    // minimized, so we need an external thread to prevent dormancy.
    crate::subscription::spawn_heartbeat_thread(tx.clone());

    let sink = Arc::new(ChannelEventSink::new(tx));
    if let Err(e) = client.setup_bridge(sink) {
        return Task::done(Message::Initialized(Err(e)));
    }

    app.client = Some(Arc::clone(&client));

    // Start the MCP named pipe server for godly-mcp integration (J1-J9).
    {
        let (mcp_tx, mcp_rx) = mpsc::unbounded();
        *app.mcp_event_receiver.lock() = Some(mcp_rx);
        godly_app_adapter::mcp_pipe::start_mcp_server(mcp_tx);
    }

    let rows = app.calculate_rows();
    let cols = app.calculate_cols();

    Task::perform(
        async move {
            let (tx, rx) = futures_channel::oneshot::channel();
            std::thread::spawn(move || {
                let result = collect_init_result_sync(&client, rows, cols);
                let _ = tx.send(result);
            });
            rx.await
                .unwrap_or_else(|_| Err("Background thread panicked".into()))
        },
        Message::Initialized,
    )
}

#[cfg(test)]
mod helper_tests {
    use super::tab_reducer::TabMruCycleDirection;
    use super::{
        begin_sidebar_animation, enqueue_toast_entry, grid_dimensions_for_viewport,
        inset_terminal_pane_rect, mru_cycle_direction_from_shortcut_key,
        next_mru_switcher_selection, next_tab_id_from_mru, normalize_ai_tool_field,
        normalize_flow_field, normalize_mute_pattern, normalize_plugin_field,
        normalize_quick_claude_text, normalize_remote_field, normalize_remote_port,
        pane_rect_for_terminal, parse_flow_steps, pointer_to_grid, prune_expired_toasts,
        quote_dropped_path, resolve_terminal_empty_state, resolved_sidebar_width,
        should_commit_mru_switcher_on_key_release, should_commit_mru_switcher_on_modifiers_changed,
        sidebar_animation_finished, terminal_content_rect, toggle_flow_enabled,
        toggle_plugin_enabled, toggle_remote_enabled, wildcard_matches,
        workspace_matches_mute_patterns, FlowEntry, PaneRect, PluginEntry, RemoteAuthMode,
        RemoteConnectionEntry, TerminalEmptyState, ToastNotification, MAX_ACTIVE_TOASTS,
        SIDEBAR_ANIMATION_DURATION_MS, TOAST_TTL_MS,
    };
    use super::{GridPos, LayoutNode, SplitDirection, TAB_BAR_HEIGHT};
    use crate::terminal_state::TerminalCollection;
    use crate::status_bar;
    use crate::title_bar;
    use godly_terminal_surface::FontMetrics;
    use iced::keyboard::{key::Named, Key, Modifiers};
    use iced::Point;
    use std::path::Path;

    #[test]
    fn quote_dropped_path_wraps_spaces() {
        let quoted = quote_dropped_path(Path::new(r"C:\Program Files\Godly Terminal\file.txt"));
        assert_eq!(quoted, r#""C:\Program Files\Godly Terminal\file.txt""#);
    }

    #[test]
    fn quote_dropped_path_escapes_double_quotes() {
        let quoted = quote_dropped_path(Path::new(r#"C:\tmp\a"b.txt"#));
        assert_eq!(quoted, r#""C:\tmp\a\"b.txt""#);
    }

    #[test]
    fn quote_dropped_path_keeps_unicode() {
        let quoted = quote_dropped_path(Path::new(r"C:\tmp\áéí\file.txt"));
        assert_eq!(quoted, "\"C:\\tmp\\áéí\\file.txt\"");
    }

    #[test]
    fn normalize_mute_pattern_trims_and_lowercases() {
        assert_eq!(
            normalize_mute_pattern("  Work-*  "),
            Some("work-*".to_string())
        );
        assert_eq!(normalize_mute_pattern("   "), None);
    }

    #[test]
    fn normalize_quick_claude_text_trims_and_rejects_empty() {
        assert_eq!(
            normalize_quick_claude_text("  launch preset  "),
            Some("launch preset".to_string())
        );
        assert_eq!(normalize_quick_claude_text(""), None);
        assert_eq!(normalize_quick_claude_text("   "), None);
    }

    #[test]
    fn normalize_ai_tool_field_trims_and_rejects_empty() {
        assert_eq!(
            normalize_ai_tool_field("  my-tool  "),
            Some("my-tool".to_string())
        );
        assert_eq!(normalize_ai_tool_field(""), None);
        assert_eq!(normalize_ai_tool_field("   "), None);
    }

    #[test]
    fn normalize_plugin_field_trims_and_rejects_empty() {
        assert_eq!(
            normalize_plugin_field("  plugin-a  "),
            Some("plugin-a".to_string())
        );
        assert_eq!(normalize_plugin_field(""), None);
        assert_eq!(normalize_plugin_field("   "), None);
    }

    #[test]
    fn toggle_plugin_enabled_flips_state_and_handles_missing_index() {
        let mut plugins = vec![PluginEntry {
            name: "plugin-a".to_string(),
            source: "local".to_string(),
            enabled: true,
        }];
        assert!(toggle_plugin_enabled(plugins.as_mut_slice(), 0));
        assert!(!plugins[0].enabled);
        assert!(!toggle_plugin_enabled(plugins.as_mut_slice(), 10));
    }

    #[test]
    fn normalize_flow_field_and_parse_steps_work() {
        assert_eq!(
            normalize_flow_field("  build release  "),
            Some("build release".to_string())
        );
        assert_eq!(normalize_flow_field("  "), None);
        assert_eq!(
            parse_flow_steps("build,test ; deploy\n notify"),
            vec![
                "build".to_string(),
                "test".to_string(),
                "deploy".to_string(),
                "notify".to_string()
            ]
        );
    }

    #[test]
    fn toggle_flow_enabled_flips_state_and_handles_missing_index() {
        let mut flows = vec![FlowEntry {
            name: "nightly".to_string(),
            trigger: "Manual".to_string(),
            steps: vec!["build".to_string(), "deploy".to_string()],
            enabled: true,
        }];
        assert!(toggle_flow_enabled(flows.as_mut_slice(), 0));
        assert!(!flows[0].enabled);
        assert!(!toggle_flow_enabled(flows.as_mut_slice(), 10));
    }

    #[test]
    fn normalize_remote_fields_and_port_validation_work() {
        assert_eq!(
            normalize_remote_field("  prod-cluster  "),
            Some("prod-cluster".to_string())
        );
        assert_eq!(normalize_remote_field("  "), None);
        assert_eq!(normalize_remote_port("22"), Some(22));
        assert_eq!(normalize_remote_port(" 65535 "), Some(65535));
        assert_eq!(normalize_remote_port("0"), None);
        assert_eq!(normalize_remote_port("70000"), None);
        assert_eq!(normalize_remote_port("abc"), None);
    }

    #[test]
    fn toggle_remote_enabled_flips_state_and_handles_missing_index() {
        let mut profiles = vec![RemoteConnectionEntry {
            name: "Prod SSH".to_string(),
            host: "10.0.0.20".to_string(),
            port: 22,
            username: "deploy".to_string(),
            auth_mode: RemoteAuthMode::Password,
            auth_value: Some("secret".to_string()),
            enabled: true,
        }];
        assert!(toggle_remote_enabled(profiles.as_mut_slice(), 0));
        assert!(!profiles[0].enabled);
        assert!(!toggle_remote_enabled(profiles.as_mut_slice(), 10));
    }

    #[test]
    fn enqueue_toast_entry_assigns_ids_and_limits_queue() {
        let mut toasts = Vec::new();
        let mut next_id = 1u64;
        for index in 0..(MAX_ACTIVE_TOASTS + 2) {
            enqueue_toast_entry(
                &mut toasts,
                &mut next_id,
                format!("title-{index}"),
                format!("message-{index}"),
                10_000,
            );
        }

        assert_eq!(toasts.len(), MAX_ACTIVE_TOASTS);
        assert_eq!(toasts.first().map(|toast| toast.id), Some(3));
        assert_eq!(
            toasts.last().map(|toast| toast.id),
            Some((MAX_ACTIVE_TOASTS + 2) as u64)
        );
        assert_eq!(next_id, (MAX_ACTIVE_TOASTS + 3) as u64);
    }

    #[test]
    fn prune_expired_toasts_removes_elapsed_entries() {
        let mut toasts = vec![
            ToastNotification {
                id: 1,
                title: "A".to_string(),
                message: "first".to_string(),
                expires_at_ms: 1_000 + TOAST_TTL_MS,
            },
            ToastNotification {
                id: 2,
                title: "B".to_string(),
                message: "second".to_string(),
                expires_at_ms: 5_000 + TOAST_TTL_MS,
            },
        ];

        let removed = prune_expired_toasts(&mut toasts, 1_000 + TOAST_TTL_MS);
        assert_eq!(removed, 1);
        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0].id, 2);
    }

    #[test]
    fn wildcard_matches_supports_infix_and_suffix_globs() {
        assert!(wildcard_matches("work-*", "work-alpha"));
        assert!(wildcard_matches("*-prod", "cluster-prod"));
        assert!(wildcard_matches("alpha*beta", "alpha-123-beta"));
        assert!(!wildcard_matches("work-*", "prod-work"));
    }

    #[test]
    fn workspace_mute_patterns_match_name_or_id() {
        let patterns = vec!["ops-*".to_string(), "w-default".to_string()];
        assert!(workspace_matches_mute_patterns(
            &patterns,
            "w-default",
            "Workspace 1"
        ));
        assert!(workspace_matches_mute_patterns(
            &patterns, "w-prod", "Ops-Live"
        ));
        assert!(!workspace_matches_mute_patterns(
            &patterns,
            "w-alpha",
            "Workspace Alpha"
        ));
    }

    #[test]
    fn mru_cycle_direction_from_shortcut_key_detects_forward_and_backward() {
        assert_eq!(
            mru_cycle_direction_from_shortcut_key(&Key::Named(Named::Tab), Modifiers::CTRL),
            Some(TabMruCycleDirection::Forward)
        );
        assert_eq!(
            mru_cycle_direction_from_shortcut_key(
                &Key::Named(Named::Tab),
                Modifiers::CTRL.union(Modifiers::SHIFT),
            ),
            Some(TabMruCycleDirection::Backward)
        );
        assert_eq!(
            mru_cycle_direction_from_shortcut_key(
                &Key::Named(Named::Tab),
                Modifiers::CTRL.union(Modifiers::ALT),
            ),
            None
        );
        assert_eq!(
            mru_cycle_direction_from_shortcut_key(&Key::Named(Named::Escape), Modifiers::CTRL),
            None
        );
    }

    #[test]
    fn next_mru_switcher_selection_uses_active_then_selected_cursor() {
        let from_active = next_mru_switcher_selection(
            vec!["t-3", "t-2", "t-1"],
            Some("t-1"),
            None,
            TabMruCycleDirection::Forward,
        );
        assert_eq!(from_active, Some("t-3".to_string()));

        let from_selection = next_mru_switcher_selection(
            vec!["t-3", "t-2", "t-1"],
            Some("t-1"),
            Some("t-3"),
            TabMruCycleDirection::Forward,
        );
        assert_eq!(from_selection, Some("t-2".to_string()));
    }

    #[test]
    fn next_mru_switcher_selection_ignores_cursor_outside_workspace_scope() {
        let next = next_mru_switcher_selection(
            vec!["w1-b", "w1-a"],
            Some("w1-a"),
            Some("w2-a"),
            TabMruCycleDirection::Forward,
        );
        assert_eq!(next, Some("w1-b".to_string()));
    }

    #[test]
    fn workspace_scoped_mru_cycle_uses_active_workspace_only() {
        let mut terminals = TerminalCollection::new();
        terminals.add_to_workspace("w1-a".into(), 24, 80, "w1".into());
        terminals.add_to_workspace("w2-a".into(), 24, 80, "w2".into());
        terminals.add_to_workspace("w1-b".into(), 24, 80, "w1".into());

        terminals.set_active("w1-b");
        terminals.set_active("w2-a");
        terminals.set_active("w1-a");

        let next = next_tab_id_from_mru(
            terminals.mru_terminal_ids_for_workspace(Some("w1")),
            Some("w1-a"),
            TabMruCycleDirection::Forward,
        );
        assert_eq!(next, Some("w1-b".to_string()));
    }

    #[test]
    fn mru_switcher_commit_guards_match_release_semantics() {
        assert!(!should_commit_mru_switcher_on_key_release(
            false,
            &Key::Named(Named::Control),
            Modifiers::CTRL,
        ));
        assert!(should_commit_mru_switcher_on_key_release(
            true,
            &Key::Named(Named::Control),
            Modifiers::CTRL,
        ));
        assert!(should_commit_mru_switcher_on_key_release(
            true,
            &Key::Named(Named::Tab),
            Modifiers::empty(),
        ));
        assert!(!should_commit_mru_switcher_on_key_release(
            true,
            &Key::Named(Named::Tab),
            Modifiers::CTRL,
        ));

        assert!(should_commit_mru_switcher_on_modifiers_changed(
            true,
            Modifiers::empty(),
        ));
        assert!(!should_commit_mru_switcher_on_modifiers_changed(
            true,
            Modifiers::CTRL,
        ));
    }

    #[test]
    fn next_tab_id_from_mru_cycles_forward_and_wraps() {
        let next = next_tab_id_from_mru(
            vec!["t-3", "t-2", "t-1"],
            Some("t-1"),
            TabMruCycleDirection::Forward,
        );
        assert_eq!(next, Some("t-3".to_string()));
    }

    #[test]
    fn next_tab_id_from_mru_cycles_backward_and_wraps() {
        let next = next_tab_id_from_mru(
            vec!["t-3", "t-2", "t-1"],
            Some("t-3"),
            TabMruCycleDirection::Backward,
        );
        assert_eq!(next, Some("t-1".to_string()));
    }

    #[test]
    fn next_tab_id_from_mru_handles_missing_current_id() {
        let next = next_tab_id_from_mru(
            vec!["t-3", "t-2", "t-1"],
            Some("missing"),
            TabMruCycleDirection::Forward,
        );
        assert_eq!(next, Some("t-2".to_string()));
    }

    #[test]
    fn sidebar_animation_interpolates_and_finishes_on_target_width() {
        let animation = begin_sidebar_animation(220.0, 0.0, 1_000).expect("animation expected");

        let mid_width = resolved_sidebar_width(true, 220.0, Some(animation), 1_100);
        assert!(mid_width > 0.0);
        assert!(mid_width < 220.0);
        assert!(!sidebar_animation_finished(animation, 1_100));

        let end_width = resolved_sidebar_width(
            true,
            220.0,
            Some(animation),
            1_000 + SIDEBAR_ANIMATION_DURATION_MS,
        );
        assert_eq!(end_width, 0.0);
        assert!(sidebar_animation_finished(
            animation,
            1_000 + SIDEBAR_ANIMATION_DURATION_MS
        ));
    }

    #[test]
    fn terminal_empty_state_requires_layout_and_live_terminal() {
        assert_eq!(
            resolve_terminal_empty_state(None, 0),
            Some(TerminalEmptyState::NoTerminalsOpen)
        );

        let layout = LayoutNode::Leaf {
            terminal_id: "t1".to_string(),
        };
        assert_eq!(resolve_terminal_empty_state(Some(&layout), 1), None);
        assert_eq!(
            resolve_terminal_empty_state(Some(&layout), 0),
            Some(TerminalEmptyState::NoTerminalsOpen)
        );
    }

    #[test]
    fn terminal_content_geometry_tracks_sidebar_and_split_ratios() {
        let top = title_bar::TITLE_BAR_HEIGHT + TAB_BAR_HEIGHT;
        let bottom = status_bar::STATUS_BAR_HEIGHT;
        let content_rect = terminal_content_rect(1_200.0, 800.0, 220.0);
        let expected_h = 800.0 - top - bottom;
        assert_eq!(
            content_rect,
            PaneRect::new(220.0, top, 980.0, expected_h)
        );

        let layout = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "left".to_string(),
            }),
            second: Box::new(LayoutNode::Split {
                direction: SplitDirection::Vertical,
                ratio: 0.25,
                first: Box::new(LayoutNode::Leaf {
                    terminal_id: "top-right".to_string(),
                }),
                second: Box::new(LayoutNode::Leaf {
                    terminal_id: "bottom-right".to_string(),
                }),
            }),
        };

        let quarter_h = expected_h * 0.25;
        let three_quarter_h = expected_h - quarter_h;

        let top_right = pane_rect_for_terminal(&layout, "top-right", content_rect)
            .expect("top-right pane should resolve");
        assert_eq!(
            top_right,
            PaneRect::new(710.0, top, 490.0, quarter_h)
        );

        let bottom_right = pane_rect_for_terminal(&layout, "bottom-right", content_rect)
            .expect("bottom-right pane should resolve");
        assert_eq!(
            bottom_right,
            PaneRect::new(710.0, top + quarter_h, 490.0, three_quarter_h)
        );
    }

    #[test]
    fn pointer_mapping_uses_inset_terminal_viewport_and_clamps_to_edges() {
        let font_metrics = FontMetrics::default();
        let viewport = inset_terminal_pane_rect(PaneRect::new(220.0, TAB_BAR_HEIGHT, 490.0, 200.0));

        let pos = pointer_to_grid(
            Point::new(
                viewport.x + font_metrics.cell_width * 3.4,
                viewport.y + font_metrics.cell_height * 2.2,
            ),
            viewport,
            font_metrics,
        );
        assert_eq!(pos, GridPos { row: 2, col: 3 });

        let edge = pointer_to_grid(
            Point::new(viewport.x - 50.0, viewport.y - 50.0),
            viewport,
            font_metrics,
        );
        assert_eq!(edge, GridPos { row: 0, col: 0 });

        let (rows, cols) = grid_dimensions_for_viewport(viewport, font_metrics);
        assert!(rows >= 1);
        assert!(cols >= 1);
    }
}

// ---------------------------------------------------------------------------
// Keyboard routing integration tests — exercises the same resolve_key_event()
// function that the real update() loop uses.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod keyboard_routing_tests {
    use super::{
        resolve_key_event, CaptureState, KeyRoutingResult, ShortcutResolver,
    };
    use godly_app_adapter::shortcuts::AppAction;
    use iced::keyboard::{key, key::Named, Key, Modifiers};
    use std::collections::HashMap;

    fn char_key(s: &str) -> Key {
        Key::Character(s.into())
    }

    fn ctrl_shift() -> Modifiers {
        Modifiers::CTRL.union(Modifiers::SHIFT)
    }

    /// Default physical key for tests that don't care about layout.
    const PHYS_UNIDENT: key::Physical = key::Physical::Unidentified(key::NativeCode::Unidentified);

    fn make_capture() -> CaptureState {
        CaptureState {
            capturing_index: None,
            overrides: HashMap::new(),
            resolver: ShortcutResolver::empty(),
        }
    }

    // --- The exact bug scenario: rebind split-right to Ctrl+Shift+/, then press it ---

    #[test]
    fn rebind_split_right_and_press_triggers_split() {
        let mut cap = make_capture();

        // Step 1: enter capture mode for SplitRight (flat index 5).
        cap.capturing_index = Some(5);

        // Step 2: user presses Ctrl+Shift+/ while in capture mode.
        let result = resolve_key_event(
            &char_key("/"),
            ctrl_shift(),
            &PHYS_UNIDENT,
            &mut cap,
            false,
            false,
            false,
        );
        assert_eq!(result, KeyRoutingResult::CapturedBinding);
        // Override was recorded.
        assert_eq!(cap.overrides.get(&5), Some(&"Ctrl+Shift+/".to_string()));
        // Capture mode exited.
        assert!(cap.capturing_index.is_none());

        // Step 3: user presses Ctrl+Shift+/ again (no longer in capture mode).
        let result = resolve_key_event(
            &char_key("/"),
            ctrl_shift(),
            &PHYS_UNIDENT,
            &mut cap,
            false,
            false,
            false,
        );
        // The custom binding should fire SplitRight.
        assert_eq!(result, KeyRoutingResult::Action(AppAction::SplitRight));
    }

    #[test]
    fn rebind_split_right_disables_original_default() {
        let mut cap = make_capture();

        // Rebind SplitRight from Ctrl+\ to Ctrl+Shift+/.
        cap.capturing_index = Some(5);
        resolve_key_event(&char_key("/"), ctrl_shift(), &PHYS_UNIDENT, &mut cap, false, false, false);

        // Original Ctrl+\ should no longer trigger SplitRight.
        let result = resolve_key_event(
            &char_key("\\"),
            Modifiers::CTRL,
            &PHYS_UNIDENT,
            &mut cap,
            false,
            false,
            false,
        );
        assert_eq!(result, KeyRoutingResult::ForwardToPty);
    }

    #[test]
    fn default_split_right_works_without_override() {
        let mut cap = make_capture();

        // Without any overrides, Ctrl+\ should trigger SplitRight.
        let result = resolve_key_event(
            &char_key("\\"),
            Modifiers::CTRL,
            &PHYS_UNIDENT,
            &mut cap,
            false,
            false,
            false,
        );
        assert_eq!(result, KeyRoutingResult::Action(AppAction::SplitRight));
    }

    #[test]
    fn capture_mode_escape_cancels_without_recording() {
        let mut cap = make_capture();
        cap.capturing_index = Some(5);

        let result = resolve_key_event(
            &Key::Named(Named::Escape),
            Modifiers::empty(),
            &PHYS_UNIDENT,
            &mut cap,
            false,
            false,
            false,
        );
        assert_eq!(result, KeyRoutingResult::CapturedBinding);
        // No override recorded.
        assert!(cap.overrides.is_empty());
        assert!(cap.capturing_index.is_none());
    }

    #[test]
    fn capture_mode_ignores_modifier_only_presses() {
        let mut cap = make_capture();
        cap.capturing_index = Some(5);

        let result = resolve_key_event(
            &Key::Named(Named::Control),
            Modifiers::CTRL,
            &PHYS_UNIDENT,
            &mut cap,
            false,
            false,
            false,
        );
        assert_eq!(result, KeyRoutingResult::CapturedBinding);
        // Still in capture mode — modifier-only press didn't exit.
        assert_eq!(cap.capturing_index, Some(5));
        assert!(cap.overrides.is_empty());
    }

    #[test]
    fn multiple_actions_can_be_rebound() {
        let mut cap = make_capture();

        // Rebind SplitRight (5) to Ctrl+Shift+/
        cap.capturing_index = Some(5);
        resolve_key_event(&char_key("/"), ctrl_shift(), &PHYS_UNIDENT, &mut cap, false, false, false);

        // Rebind SplitDown (6) to Ctrl+Shift+.
        cap.capturing_index = Some(6);
        resolve_key_event(&char_key("."), ctrl_shift(), &PHYS_UNIDENT, &mut cap, false, false, false);

        // Both custom bindings fire.
        assert_eq!(
            resolve_key_event(&char_key("/"), ctrl_shift(), &PHYS_UNIDENT, &mut cap, false, false, false),
            KeyRoutingResult::Action(AppAction::SplitRight)
        );
        assert_eq!(
            resolve_key_event(&char_key("."), ctrl_shift(), &PHYS_UNIDENT, &mut cap, false, false, false),
            KeyRoutingResult::Action(AppAction::SplitDown)
        );

        // Both original defaults are disabled.
        assert_eq!(
            resolve_key_event(&char_key("\\"), Modifiers::CTRL, &PHYS_UNIDENT, &mut cap, false, false, false),
            KeyRoutingResult::ForwardToPty
        );
        assert_eq!(
            resolve_key_event(
                &char_key("\\"),
                Modifiers::CTRL.union(Modifiers::ALT),
                &PHYS_UNIDENT,
                &mut cap,
                false,
                false,
                false,
            ),
            KeyRoutingResult::ForwardToPty
        );

        // Unrelated defaults still work.
        assert_eq!(
            resolve_key_event(&char_key("t"), Modifiers::CTRL, &PHYS_UNIDENT, &mut cap, false, false, false),
            KeyRoutingResult::Action(AppAction::NewTab)
        );
    }

    // -----------------------------------------------------------------------
    // Bug #639 — Windows Ctrl+\ sends 0x1C control char
    // -----------------------------------------------------------------------

    #[test]
    fn default_ctrl_backslash_control_char_triggers_split_right() {
        let mut cap = make_capture();
        // On Windows, Ctrl+\ produces Key::Character("\x1c") (File Separator).
        let result = resolve_key_event(
            &char_key("\x1c"),
            Modifiers::CTRL,
            &PHYS_UNIDENT,
            &mut cap,
            false,
            false,
            false,
        );
        assert_eq!(result, KeyRoutingResult::Action(AppAction::SplitRight));
    }

    #[test]
    fn capture_ctrl_backslash_control_char_then_resolve() {
        let mut cap = make_capture();
        // Capture mode: bind SplitRight (5) by pressing Ctrl+\ (as 0x1C).
        cap.capturing_index = Some(5);
        let result = resolve_key_event(
            &char_key("\x1c"),
            Modifiers::CTRL,
            &PHYS_UNIDENT,
            &mut cap,
            false,
            false,
            false,
        );
        assert_eq!(result, KeyRoutingResult::CapturedBinding);
        // Should normalize 0x1C to "\" in the stored chord.
        assert_eq!(cap.overrides.get(&5), Some(&"Ctrl+\\".to_string()));

        // Now press Ctrl+\ again — should resolve to SplitRight via custom binding.
        let result = resolve_key_event(
            &char_key("\x1c"),
            Modifiers::CTRL,
            &PHYS_UNIDENT,
            &mut cap,
            false,
            false,
            false,
        );
        assert_eq!(result, KeyRoutingResult::Action(AppAction::SplitRight));
    }

    #[test]
    fn non_modifier_key_in_capture_exits_without_recording() {
        let mut cap = make_capture();
        cap.capturing_index = Some(5);

        // Press a plain letter (no Ctrl/Alt) — should exit capture without recording.
        let result = resolve_key_event(
            &char_key("a"),
            Modifiers::empty(),
            &PHYS_UNIDENT,
            &mut cap,
            false,
            false,
            false,
        );
        assert_eq!(result, KeyRoutingResult::CapturedBinding);
        assert!(cap.overrides.is_empty());
        assert!(cap.capturing_index.is_none());
    }
}
