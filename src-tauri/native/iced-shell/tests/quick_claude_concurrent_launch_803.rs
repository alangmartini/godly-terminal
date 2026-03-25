//! Bug #803: Quick Claude launch blocked until previous launch fully completes.
//!
//! Quick Claude uses `quick_claude_launch: Option<LaunchState>` as a singleton
//! lock — when any launch is in progress, all three launch handlers
//! (QuickClaudeLaunchPreset, QuickClaudeDialogLaunch, QuickClaudeDialogResume)
//! guard on `is_some()` and silently return `Task::none()`.
//!
//! The launch sequence includes blocking steps (WaitIdle: 2000ms,
//! HandleTrustPromptIfNeeded: up to 8000ms), so the lock can be held for
//! 10+ seconds. During this window, any attempt to launch a new Quick Claude
//! session is silently dropped — no error, no queuing, no feedback.
//!
//! Expected: Users should be able to launch multiple Quick Claude sessions
//! concurrently. Each launch creates its own workspace and terminal — there
//! is no fundamental reason they must be serialized.
//!
//! Run with:
//!   cd src-tauri && cargo nextest run -p godly-iced-shell --test quick_claude_concurrent_launch_803

/// Bug #803: quick_claude_launch must NOT be a singleton Option that blocks
/// concurrent launches.
///
/// The fix should change `quick_claude_launch` from `Option<LaunchState>` to
/// a collection type (e.g., `Vec<LaunchState>` or `HashMap<String, LaunchState>`)
/// so multiple launches can proceed concurrently.
#[test]
fn quick_claude_launch_supports_concurrent_launches() {
    let source = include_str!("../src/app.rs");

    // Find the field declaration for quick_claude_launch in the App struct.
    // Current buggy code: `quick_claude_launch: Option<...::LaunchState>`
    // The fix should use a collection type instead of Option.
    let field_line = source
        .lines()
        .find(|line| line.contains("quick_claude_launch:") && !line.trim().starts_with("//"))
        .expect("quick_claude_launch field not found in app.rs");

    let is_singleton = field_line.contains("Option<");

    assert!(
        !is_singleton,
        "\n\n\
         Bug #803: quick_claude_launch is declared as Option<LaunchState>, \
         which acts as a singleton lock that blocks concurrent launches.\n\
         \n\
         Found: {}\n\
         \n\
         While a launch is in progress (up to 10+ seconds), all three launch \
         handlers silently ignore new launch requests:\n\
         - QuickClaudeLaunchPreset: if self.quick_claude_launch.is_some() {{ return Task::none(); }}\n\
         - QuickClaudeDialogLaunch: if self.quick_claude_launch.is_some() {{ return Task::none(); }}\n\
         - QuickClaudeDialogResume: if self.quick_claude_launch.is_some() {{ return Task::none(); }}\n\
         \n\
         Fix: Change quick_claude_launch to a Vec<LaunchState> or similar \
         collection so multiple launches can proceed concurrently. Each launch \
         creates its own workspace and terminal — the daemon supports this.\n",
        field_line.trim()
    );
}

/// Bug #803: Launch handlers must not guard on a singleton launch state.
///
/// The three handlers that initiate Quick Claude launches all check
/// `self.quick_claude_launch.is_some()` and silently bail. This guard must
/// be removed or changed to allow concurrent launches.
#[test]
fn launch_handlers_do_not_block_on_singleton_guard() {
    let source = include_str!("../src/app.rs");

    // Count how many times the pattern `quick_claude_launch.is_some()` appears
    // as a guard that returns Task::none(). The buggy code has 3 such guards.
    let blocking_guard_count = source
        .match_indices("quick_claude_launch.is_some()")
        .filter(|(idx, _)| {
            // Look ahead ~100 chars for "return Task::none()" to confirm it's a blocking guard
            let after = &source[*idx..(*idx + 200).min(source.len())];
            after.contains("return Task::none()")
        })
        .count();

    assert!(
        blocking_guard_count == 0,
        "\n\n\
         Bug #803: Found {} launch handler(s) that block on \
         quick_claude_launch.is_some() and silently return Task::none().\n\
         \n\
         These guards prevent concurrent Quick Claude launches. When a launch \
         is in progress (which can take 10+ seconds due to WaitIdle and \
         HandleTrustPromptIfNeeded steps), clicking 'Launch' silently does \
         nothing — no error message, no queuing, no feedback.\n\
         \n\
         Fix: Remove these singleton guards or replace them with logic that \
         supports concurrent launches (e.g., adding to a Vec<LaunchState>).\n",
        blocking_guard_count
    );
}

/// Bug #803: The UI launch button must not be globally disabled during launches.
///
/// The current code checks `self.quick_claude_launch.is_some()` to show "..."
/// instead of the launch button, making it impossible to start a new launch
/// while any launch is in progress.
#[test]
fn launch_button_not_globally_disabled_during_launch() {
    let source = include_str!("../src/app.rs");

    // The buggy pattern: using is_some() to disable the launch button for ALL
    // launches, not just the one being displayed.
    // Look for the specific UI pattern: `let is_launching = self.quick_claude_launch.is_some()`
    let has_global_disable = source.contains("is_launching = self.quick_claude_launch.is_some()");

    assert!(
        !has_global_disable,
        "\n\n\
         Bug #803: The Quick Claude launch button uses a global \
         `is_launching = self.quick_claude_launch.is_some()` check that \
         disables the button for ALL presets while ANY launch is in progress.\n\
         \n\
         Users should be able to launch a new Quick Claude session even while \
         a previous one is still initializing. Each launch is independent \
         (creates its own workspace and terminal).\n\
         \n\
         Fix: Either remove the global disable, or make it per-preset (e.g., \
         check if this specific preset is already launching).\n"
    );
}
