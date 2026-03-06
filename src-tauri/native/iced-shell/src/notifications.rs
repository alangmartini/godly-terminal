/// Per-terminal sound debounce window in milliseconds.
pub const TERMINAL_SOUND_DEBOUNCE_MS: u64 = 2_000;

/// Global cross-terminal sound debounce window in milliseconds.
pub const GLOBAL_SOUND_DEBOUNCE_MS: u64 = 500;

/// Global window-attention debounce window in milliseconds.
pub const WINDOW_ATTENTION_DEBOUNCE_MS: u64 = 2_000;

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
}
