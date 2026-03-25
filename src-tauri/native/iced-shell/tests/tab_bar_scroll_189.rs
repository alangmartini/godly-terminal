//! Bug #189 regression: Tab bar must allow scrolling to reach overflow tabs.
//!
//! When many terminals are open, tabs overflow the tab bar width. The scrollbar
//! must be visible (not hidden) and/or wheel events must be handled so users can
//! scroll to reach off-screen tabs. Without this, the last terminal tabs are
//! completely inaccessible via mouse interaction.

/// Bug #189: The tab bar scrollable must not use Scrollbar::hidden().
///
/// `Scrollbar::hidden()` removes any visible scroll affordance, making it
/// impossible for users to drag-scroll through overflow tabs. The fix should
/// use a visible scrollbar, scroll arrows, or equivalent navigation.
#[test]
fn tab_bar_scrollbar_not_hidden() {
    let source = include_str!("../src/tab_bar.rs");

    assert!(
        !source.contains("Scrollbar::hidden()"),
        "Bug #189: tab_bar.rs uses Scrollbar::hidden() which prevents users \
         from scrolling to reach overflow tabs. Use a visible scrollbar or \
         provide alternative scroll navigation (arrows, wheel handler)."
    );
}

/// Bug #189: The tab bar must handle vertical mouse wheel events for horizontal
/// scrolling, OR the app must route wheel events over the tab bar to horizontal
/// scroll — not to terminal scrollback.
///
/// Currently, app.rs MouseWheel handler (line ~1992) only scrolls the terminal
/// viewport. Vertical wheel events on the tab bar are either ignored or
/// incorrectly routed to terminal scrollback.
#[test]
fn tab_bar_has_wheel_scroll_support() {
    let app_source = include_str!("../src/app.rs");
    let tab_bar_source = include_str!("../src/tab_bar.rs");

    // The fix should add one of:
    // 1. A dedicated TabBarWheel / TabBarScroll message in app.rs
    // 2. An on_scroll / wheel callback accepted by view_tab_bar
    // 3. A mouse_listener / wheel handler inside tab_bar.rs
    let has_tab_wheel_in_app = app_source.contains("TabBarWheel")
        || app_source.contains("TabBarScroll")
        || app_source.contains("tab_bar_scroll");

    let has_wheel_in_tab_bar = tab_bar_source.contains("on_scroll")
        || tab_bar_source.contains("on_wheel")
        || tab_bar_source.contains("mouse_listener");

    assert!(
        has_tab_wheel_in_app || has_wheel_in_tab_bar,
        "Bug #189: No wheel/scroll handler found for the tab bar. Vertical \
         mouse wheel events over the tab bar should scroll tabs horizontally, \
         not the terminal viewport. Add a TabBarWheel message or on_scroll \
         callback to view_tab_bar."
    );
}

/// Bug #189: view_tab_bar must accept a scroll/wheel callback so the app can
/// translate wheel events into horizontal tab bar scrolling.
///
/// The current function signature only accepts: on_tab_click, on_close,
/// on_drag_start, on_drag_hover, on_context_toggle, on_drag_end, on_new.
/// It has no parameter for scroll/wheel events.
#[test]
fn view_tab_bar_accepts_scroll_callback() {
    let source = include_str!("../src/tab_bar.rs");

    // Find the view_tab_bar function signature and check for scroll parameter
    let fn_start = source
        .find("pub fn view_tab_bar")
        .expect("view_tab_bar function not found in tab_bar.rs");
    let fn_sig_end = source[fn_start..]
        .find('{')
        .expect("view_tab_bar function body not found");
    let signature = &source[fn_start..fn_start + fn_sig_end];

    let has_scroll_param = signature.contains("on_scroll")
        || signature.contains("on_wheel")
        || signature.contains("scroll_offset");

    assert!(
        has_scroll_param,
        "Bug #189: view_tab_bar() has no scroll/wheel callback parameter. \
         Signature:\n{}\n\n\
         Add an on_scroll or on_wheel parameter so the app can translate \
         vertical wheel events into horizontal tab bar scrolling.",
        signature.trim()
    );
}
