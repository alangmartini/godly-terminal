//! Bug #818: Quick Claude concurrent launches into the same workspace collide
//! on workspace_id lookup.
//!
//! When multiple Quick Claude launches target the same workspace, they share
//! the same `workspace_id`. The step routing functions use
//! `iter().find(|l| l.workspace_id == workspace_id)` which always returns the
//! first match — so later launches never get their steps executed, and
//! `finalize_launch` removes ALL launches for that workspace when any one finishes.
//!
//! Symptoms:
//! 1. Only the last triggered Quick Claude shows progress
//! 2. Earlier launches never finish loading
//!
//! The fix should give each LaunchState a unique launch_id and route step
//! completions by launch_id (not workspace_id).
//!
//! Run with:
//!   cd src-tauri && cargo nextest run -p godly-iced-shell --test quick_claude_same_workspace_collision_818

/// Bug #818: QuickClaudeLaunchStepComplete must carry a launch-specific ID,
/// not just workspace_id.
///
/// The message `QuickClaudeLaunchStepComplete(String, Result<...>)` uses
/// workspace_id as the routing key. When multiple launches target the same
/// workspace, completion messages cannot distinguish between them.
///
/// The fix should either:
/// (a) Add a unique launch_id to each LaunchState and use it in the message, or
/// (b) Use a composite key (workspace_id + launch_id) for routing.
#[test]
fn launch_step_complete_carries_unique_launch_id() {
    let source = include_str!("../src/app.rs");

    // The QuickClaudeLaunchStepComplete message definition should reference
    // a launch_id, not just workspace_id. Currently:
    //   QuickClaudeLaunchStepComplete(String, Result<...>)
    // where the String is workspace_id.
    //
    // After fix, the message should carry a unique launch identifier.
    // We check that the step execution function (`execute_next_launch_step`)
    // uses something other than bare workspace_id for the callback message.

    // Find the line where ws_id_for_msg is constructed for the step complete callback.
    // Current buggy code:
    //   let ws_id_for_msg = workspace_id.to_string();
    //   ...
    //   move |result| Message::QuickClaudeLaunchStepComplete(ws_id_for_msg, result)
    //
    // This means the routing key is workspace_id, which is NOT unique per launch.

    let has_ws_id_routing = source.contains("ws_id_for_msg = workspace_id.to_string()")
        && source.contains("QuickClaudeLaunchStepComplete(ws_id_for_msg");

    assert!(
        !has_ws_id_routing,
        "\n\n\
         Bug #818: QuickClaudeLaunchStepComplete routes by workspace_id, not \
         by a unique launch identifier.\n\
         \n\
         Found: ws_id_for_msg = workspace_id.to_string() used in \
         QuickClaudeLaunchStepComplete callback.\n\
         \n\
         When multiple Quick Claude launches target the same workspace, \
         their step completions are indistinguishable — handle_launch_step_result \
         always processes the first launch found via iter().find().\n\
         \n\
         Fix: Add a unique launch_id to LaunchState and use it as the \
         routing key in QuickClaudeLaunchStepComplete instead of workspace_id.\n"
    );
}

/// Bug #818: execute_next_launch_step must not use workspace_id as the sole
/// lookup key.
///
/// `execute_next_launch_step` uses:
///   `self.quick_claude_launches.iter().find(|l| l.workspace_id == workspace_id)`
///
/// When multiple launches share a workspace_id, find() always returns the
/// first match. Later launches never get their steps executed.
#[test]
fn execute_next_launch_step_uses_unique_key() {
    let source = include_str!("../src/app.rs");

    // Find the execute_next_launch_step function and check its lookup pattern.
    // The buggy pattern is: find(|l| l.workspace_id == workspace_id)
    // within the execute_next_launch_step function.
    let fn_start = source
        .find("fn execute_next_launch_step")
        .expect("execute_next_launch_step not found");
    // Extract ~500 chars of the function body to check the lookup
    let fn_body = &source[fn_start..(fn_start + 500).min(source.len())];

    let uses_workspace_id_find = fn_body.contains(".find(|l| l.workspace_id == ");

    assert!(
        !uses_workspace_id_find,
        "\n\n\
         Bug #818: execute_next_launch_step uses workspace_id as the sole \
         lookup key for finding the target launch.\n\
         \n\
         Found: iter().find(|l| l.workspace_id == ...) in execute_next_launch_step\n\
         \n\
         When multiple Quick Claude launches target the same workspace, \
         find() always returns the first match. Later launches' \
         execute_next_launch_step calls get routed to the first launch, \
         so later launches never make progress.\n\
         \n\
         Fix: Use a unique launch_id instead of workspace_id for the lookup.\n"
    );
}

/// Bug #818: handle_launch_step_result must not use workspace_id as the sole
/// lookup key.
///
/// Same issue as execute_next_launch_step: iter().find() with workspace_id
/// routes all step completions to the first launch with that workspace_id.
#[test]
fn handle_launch_step_result_uses_unique_key() {
    let source = include_str!("../src/app.rs");

    let fn_start = source
        .find("fn handle_launch_step_result")
        .expect("handle_launch_step_result not found");
    // The function is large; check first 2000 chars
    let fn_body = &source[fn_start..(fn_start + 2000).min(source.len())];

    // Count how many find(|l| l.workspace_id == ...) calls exist in this function.
    // Current buggy code has multiple such lookups.
    let ws_find_count = fn_body.matches(".find(|l| l.workspace_id == ").count();

    assert!(
        ws_find_count == 0,
        "\n\n\
         Bug #818: handle_launch_step_result uses workspace_id-based find() \
         {} time(s) to locate the target launch.\n\
         \n\
         When multiple launches share a workspace_id, all step completions \
         get routed to the first launch. Later launches' steps are never \
         processed, causing them to hang in loading state forever.\n\
         \n\
         Fix: Use a unique launch_id for all launch lookups.\n",
        ws_find_count
    );
}

/// Bug #818: finalize_launch must not remove ALL launches for a workspace.
///
/// `finalize_launch` uses:
///   `self.quick_claude_launches.retain(|l| l.workspace_id != workspace_id)`
///
/// This removes ALL launches targeting that workspace when ANY one finishes,
/// killing sibling launches that are still in progress.
#[test]
fn finalize_launch_does_not_remove_sibling_launches() {
    let source = include_str!("../src/app.rs");

    let fn_start = source
        .find("fn finalize_launch")
        .expect("finalize_launch not found");
    let fn_body = &source[fn_start..(fn_start + 1500).min(source.len())];

    // The buggy pattern: retain(|l| l.workspace_id != workspace_id)
    // This removes ALL launches with the matching workspace_id.
    let removes_all_by_ws_id = fn_body.contains(".retain(|l| l.workspace_id != ");

    assert!(
        !removes_all_by_ws_id,
        "\n\n\
         Bug #818: finalize_launch uses retain(|l| l.workspace_id != ...) \
         which removes ALL launches for the workspace when any one completes.\n\
         \n\
         When multiple Quick Claude launches target the same workspace, \
         completing one launch kills all sibling launches that are still \
         in progress — their loading overlays disappear without the \
         sessions ever being set up.\n\
         \n\
         Fix: Use a unique launch_id to remove only the specific launch \
         that completed.\n"
    );
}

/// Bug #818: LaunchState must have a unique identifier per launch instance.
///
/// Currently LaunchState is identified only by workspace_id, which is not
/// unique when multiple launches target the same workspace.
#[test]
fn launch_state_has_unique_launch_id() {
    let source = include_str!("../src/quick_claude.rs");

    // Check that LaunchState has a launch_id (or id) field.
    // Find the struct definition.
    let struct_start = source
        .find("pub struct LaunchState")
        .expect("LaunchState struct not found");
    // Read ~600 chars to cover all fields
    let struct_body = &source[struct_start..(struct_start + 600).min(source.len())];

    let has_launch_id = struct_body.contains("launch_id:") || struct_body.contains("pub id:");

    assert!(
        has_launch_id,
        "\n\n\
         Bug #818: LaunchState has no unique per-launch identifier.\n\
         \n\
         LaunchState is only identified by workspace_id, which is shared \
         when multiple launches target the same workspace. All routing \
         functions (execute_next_launch_step, handle_launch_step_result, \
         finalize_launch) use workspace_id for lookups, causing collisions.\n\
         \n\
         Found struct fields:\n{}\n\
         \n\
         Fix: Add a `launch_id: String` field (e.g., UUID) to LaunchState \
         and use it as the routing key everywhere instead of workspace_id.\n",
        struct_body.lines().take(20).collect::<Vec<_>>().join("\n")
    );
}
