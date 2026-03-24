/// Per-terminal sound debounce window in milliseconds.
pub const TERMINAL_SOUND_DEBOUNCE_MS: u64 = 2_000;

/// Global cross-terminal sound debounce window in milliseconds.
pub const GLOBAL_SOUND_DEBOUNCE_MS: u64 = 500;

/// Global window-attention debounce window in milliseconds.
pub const WINDOW_ATTENTION_DEBOUNCE_MS: u64 = 2_000;

/// Burst detection window: if the last played sound was within this window,
/// subsequent bells are suppressed (burst mode).
pub const BURST_WINDOW_MS: u64 = 30_000;

/// Quiet detection: after this many ms with no new bells, a burst is
/// considered "settled" and one final notification fires.
pub const BURST_QUIET_MS: u64 = 10_000;

/// Detailed result for sound debounce evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundDecision {
    /// True when sound playback is allowed for this event.
    pub should_play_sound: bool,
    /// True when the event is blocked by per-terminal debounce.
    pub terminal_debounced: bool,
    /// True when the event is blocked by global debounce.
    pub global_debounced: bool,
}

/// Detailed result for window-attention debounce evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowAttentionDecision {
    /// True when a native attention request should be sent.
    pub should_request_attention: bool,
    /// True when the event is blocked by global attention debounce.
    pub debounced: bool,
}

/// Pure helper that decides whether a sound should play for an event.
///
/// Inputs are explicit timestamps to keep the logic deterministic and testable.
pub fn should_play_sound(
    now_ms: u64,
    last_terminal_event_ms: Option<u64>,
    last_global_sound_ms: Option<u64>,
) -> bool {
    decide_sound_playback(now_ms, last_terminal_event_ms, last_global_sound_ms).should_play_sound
}

/// Pure helper returning full debounce decision details for a sound event.
pub fn decide_sound_playback(
    now_ms: u64,
    last_terminal_event_ms: Option<u64>,
    last_global_sound_ms: Option<u64>,
) -> SoundDecision {
    let terminal_debounced =
        is_within_debounce_window(last_terminal_event_ms, now_ms, TERMINAL_SOUND_DEBOUNCE_MS);
    let global_debounced =
        is_within_debounce_window(last_global_sound_ms, now_ms, GLOBAL_SOUND_DEBOUNCE_MS);

    SoundDecision {
        should_play_sound: !terminal_debounced && !global_debounced,
        terminal_debounced,
        global_debounced,
    }
}

/// Pure helper that decides whether native window attention should be requested.
///
/// Inputs are explicit to keep logic deterministic and testable.
pub fn should_request_window_attention(
    now_ms: u64,
    app_window_focused: bool,
    last_attention_request_ms: Option<u64>,
) -> bool {
    decide_window_attention_request(now_ms, app_window_focused, last_attention_request_ms)
        .should_request_attention
}

/// Pure helper returning full debounce decision details for a window-attention event.
pub fn decide_window_attention_request(
    now_ms: u64,
    app_window_focused: bool,
    last_attention_request_ms: Option<u64>,
) -> WindowAttentionDecision {
    let debounced = is_within_debounce_window(
        last_attention_request_ms,
        now_ms,
        WINDOW_ATTENTION_DEBOUNCE_MS,
    );

    WindowAttentionDecision {
        should_request_attention: !app_window_focused && !debounced,
        debounced,
    }
}

/// Pure helper deciding whether bell attention should be "critical".
///
/// On Windows, critical attention triggers taskbar flashing.
pub fn bell_attention_is_critical(is_windows: bool) -> bool {
    is_windows
}

/// Pure helper: returns true when a terminal is in bell-burst mode.
///
/// A burst is active when the last sound that actually played for this
/// terminal was within [`BURST_WINDOW_MS`].
pub fn is_burst_active(now_ms: u64, last_sound_played_ms: Option<u64>) -> bool {
    is_within_debounce_window(last_sound_played_ms, now_ms, BURST_WINDOW_MS)
}

/// Pure helper: returns true when a bell burst has gone quiet.
///
/// A burst is "quiet" when at least one bell was suppressed and no new
/// bell has arrived for [`BURST_QUIET_MS`].
pub fn is_burst_quiet(now_ms: u64, last_bell_ms: Option<u64>, suppressed_count: u32) -> bool {
    if suppressed_count == 0 {
        return false;
    }
    match last_bell_ms {
        Some(last) => now_ms.saturating_sub(last) >= BURST_QUIET_MS,
        None => false,
    }
}

fn is_within_debounce_window(last_ms: Option<u64>, now_ms: u64, debounce_window_ms: u64) -> bool {
    match last_ms {
        Some(last_ms) => now_ms.saturating_sub(last_ms) < debounce_window_ms,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_play_sound_when_no_prior_timestamps() {
        assert!(should_play_sound(1_000, None, None));
    }

    #[test]
    fn test_terminal_debounce_blocks_sound_within_window() {
        let decision = decide_sound_playback(1_999, Some(0), None);

        assert!(!decision.should_play_sound);
        assert!(decision.terminal_debounced);
        assert!(!decision.global_debounced);
    }

    #[test]
    fn test_terminal_debounce_allows_sound_at_boundary() {
        let decision = decide_sound_playback(TERMINAL_SOUND_DEBOUNCE_MS, Some(0), None);

        assert!(decision.should_play_sound);
        assert!(!decision.terminal_debounced);
        assert!(!decision.global_debounced);
    }

    #[test]
    fn test_global_debounce_blocks_sound_within_window() {
        let decision = decide_sound_playback(499, None, Some(0));

        assert!(!decision.should_play_sound);
        assert!(!decision.terminal_debounced);
        assert!(decision.global_debounced);
    }

    #[test]
    fn test_global_debounce_allows_sound_at_boundary() {
        let decision = decide_sound_playback(GLOBAL_SOUND_DEBOUNCE_MS, None, Some(0));

        assert!(decision.should_play_sound);
        assert!(!decision.terminal_debounced);
        assert!(!decision.global_debounced);
    }

    #[test]
    fn test_both_debounces_reported_when_both_windows_match() {
        let decision = decide_sound_playback(100, Some(0), Some(0));

        assert!(!decision.should_play_sound);
        assert!(decision.terminal_debounced);
        assert!(decision.global_debounced);
    }

    #[test]
    fn test_clock_rollback_is_treated_as_debounced() {
        let decision = decide_sound_playback(900, Some(1_000), Some(1_000));

        assert!(!decision.should_play_sound);
        assert!(decision.terminal_debounced);
        assert!(decision.global_debounced);
    }

    #[test]
    fn test_window_attention_allowed_when_unfocused_and_no_prior_timestamp() {
        assert!(should_request_window_attention(1_000, false, None));
    }

    #[test]
    fn test_window_attention_blocked_when_focused() {
        let decision = decide_window_attention_request(1_000, true, None);

        assert!(!decision.should_request_attention);
        assert!(!decision.debounced);
    }

    #[test]
    fn test_window_attention_debounce_blocks_within_window() {
        let decision = decide_window_attention_request(1_999, false, Some(0));

        assert!(!decision.should_request_attention);
        assert!(decision.debounced);
    }

    #[test]
    fn test_window_attention_debounce_allows_at_boundary() {
        let decision =
            decide_window_attention_request(WINDOW_ATTENTION_DEBOUNCE_MS, false, Some(0));

        assert!(decision.should_request_attention);
        assert!(!decision.debounced);
    }

    #[test]
    fn test_window_attention_clock_rollback_is_treated_as_debounced() {
        let decision = decide_window_attention_request(900, false, Some(1_000));

        assert!(!decision.should_request_attention);
        assert!(decision.debounced);
    }

    #[test]
    fn test_bell_attention_is_critical_on_windows() {
        assert!(bell_attention_is_critical(true));
    }

    #[test]
    fn test_bell_attention_is_not_critical_elsewhere() {
        assert!(!bell_attention_is_critical(false));
    }

    // --- Burst detection tests ---

    #[test]
    fn test_burst_not_active_when_no_prior_sound() {
        assert!(!is_burst_active(5_000, None));
    }

    #[test]
    fn test_burst_active_when_sound_within_window() {
        assert!(is_burst_active(20_000, Some(5_000))); // 15s ago, within 30s
    }

    #[test]
    fn test_burst_not_active_when_sound_outside_window() {
        assert!(!is_burst_active(35_000, Some(0))); // 35s ago, outside 30s
    }

    #[test]
    fn test_burst_active_at_boundary() {
        // At exactly 29_999ms after last sound, still in burst
        assert!(is_burst_active(BURST_WINDOW_MS - 1, Some(0)));
    }

    #[test]
    fn test_burst_not_active_at_boundary() {
        // At exactly BURST_WINDOW_MS, no longer in burst
        assert!(!is_burst_active(BURST_WINDOW_MS, Some(0)));
    }

    #[test]
    fn test_burst_active_clock_rollback() {
        // now < last_sound → saturating_sub=0 < BURST_WINDOW_MS → burst active (safe: suppresses)
        assert!(is_burst_active(500, Some(1_000)));
    }

    // --- Burst quiet tests ---

    #[test]
    fn test_burst_quiet_when_no_bells_suppressed() {
        // suppressed_count=0 → never quiet (nothing to settle)
        assert!(!is_burst_quiet(20_000, Some(5_000), 0));
    }

    #[test]
    fn test_burst_quiet_when_bells_suppressed_and_enough_silence() {
        // Last bell was 15s ago, 3 suppressed → quiet
        assert!(is_burst_quiet(20_000, Some(5_000), 3));
    }

    #[test]
    fn test_burst_not_quiet_when_recent_bell() {
        // Last bell was 5s ago, 3 suppressed → not quiet yet
        assert!(!is_burst_quiet(10_000, Some(5_000), 3));
    }

    #[test]
    fn test_burst_quiet_at_boundary() {
        // Exactly BURST_QUIET_MS after last bell → quiet
        assert!(is_burst_quiet(BURST_QUIET_MS, Some(0), 1));
    }

    #[test]
    fn test_burst_not_quiet_just_before_boundary() {
        assert!(!is_burst_quiet(BURST_QUIET_MS - 1, Some(0), 1));
    }

    #[test]
    fn test_burst_quiet_no_last_bell() {
        // No last_bell_ms recorded → not quiet
        assert!(!is_burst_quiet(20_000, None, 5));
    }

    #[test]
    fn test_burst_quiet_clock_rollback() {
        // now < last_bell → saturating_sub=0 < BURST_QUIET_MS → not quiet (safe: no premature settle)
        assert!(!is_burst_quiet(500, Some(1_000), 5));
    }
}
