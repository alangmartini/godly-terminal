//! tmux format string expansion.
//!
//! tmux uses `#{variable}` syntax in format strings (e.g. `-F '#{pane_id}'`).
//! We support a subset of the variables that Claude Code actually uses.

use crate::state::TmuxState;

/// Context for format string expansion.
pub struct FormatContext<'a> {
    pub state: &'a TmuxState,
    pub pane_id: &'a str,
}

/// Expand tmux format variables in a string.
///
/// Supported variables:
/// - `#{pane_id}` — tmux pane ID (e.g. `%0`)
/// - `#{session_name}` — session name
/// - `#{window_index}` — always `0` (we use a flat model)
/// - `#{pane_index}` — pane index within session
/// - `#{pane_width}` — defaults to `80` (would need ReadGrid for real value)
/// - `#{pane_height}` — defaults to `24` (would need ReadGrid for real value)
pub fn expand(template: &str, ctx: &FormatContext) -> String {
    let mut result = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '#' && chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut var_name = String::new();
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                var_name.push(c);
            }
            result.push_str(&resolve_variable(&var_name, ctx));
        } else {
            result.push(ch);
        }
    }

    result
}

fn resolve_variable(name: &str, ctx: &FormatContext) -> String {
    match name {
        "pane_id" => ctx.pane_id.to_string(),
        "session_name" => ctx
            .state
            .pane_session(ctx.pane_id)
            .unwrap_or("default")
            .to_string(),
        "window_index" => "0".to_string(),
        "pane_index" => ctx.state.pane_index(ctx.pane_id).to_string(),
        "pane_width" => "80".to_string(),
        "pane_height" => "24".to_string(),
        _ => format!("#{{{}}}", name), // pass through unknown variables
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{PaneState, SessionState, TmuxState};
    use std::collections::HashMap;

    fn test_ctx() -> (TmuxState, String) {
        let mut state = TmuxState::default();
        state.sessions.insert(
            "dev".to_string(),
            SessionState {
                workspace_id: "ws-1".to_string(),
            },
        );
        state.panes.insert(
            "%0".to_string(),
            PaneState {
                terminal_id: "t-1".to_string(),
                session: "dev".to_string(),
            },
        );
        state.panes.insert(
            "%1".to_string(),
            PaneState {
                terminal_id: "t-2".to_string(),
                session: "dev".to_string(),
            },
        );
        (state, "%1".to_string())
    }

    #[test]
    fn expand_pane_id() {
        let (state, pane_id) = test_ctx();
        let ctx = FormatContext {
            state: &state,
            pane_id: &pane_id,
        };
        assert_eq!(expand("#{pane_id}", &ctx), "%1");
    }

    #[test]
    fn expand_session_name() {
        let (state, pane_id) = test_ctx();
        let ctx = FormatContext {
            state: &state,
            pane_id: &pane_id,
        };
        assert_eq!(expand("#{session_name}", &ctx), "dev");
    }

    #[test]
    fn expand_window_index_always_zero() {
        let (state, pane_id) = test_ctx();
        let ctx = FormatContext {
            state: &state,
            pane_id: &pane_id,
        };
        assert_eq!(expand("#{window_index}", &ctx), "0");
    }

    #[test]
    fn expand_pane_index() {
        let (state, pane_id) = test_ctx();
        let ctx = FormatContext {
            state: &state,
            pane_id: &pane_id,
        };
        assert_eq!(expand("#{pane_index}", &ctx), "1");
    }

    #[test]
    fn expand_pane_dimensions_defaults() {
        let (state, pane_id) = test_ctx();
        let ctx = FormatContext {
            state: &state,
            pane_id: &pane_id,
        };
        assert_eq!(expand("#{pane_width}", &ctx), "80");
        assert_eq!(expand("#{pane_height}", &ctx), "24");
    }

    #[test]
    fn expand_multiple_variables() {
        let (state, pane_id) = test_ctx();
        let ctx = FormatContext {
            state: &state,
            pane_id: &pane_id,
        };
        let result = expand("#{session_name}:#{window_index}.#{pane_index}", &ctx);
        assert_eq!(result, "dev:0.1");
    }

    #[test]
    fn expand_literal_text_preserved() {
        let (state, pane_id) = test_ctx();
        let ctx = FormatContext {
            state: &state,
            pane_id: &pane_id,
        };
        assert_eq!(expand("hello world", &ctx), "hello world");
    }

    #[test]
    fn expand_unknown_variable_passed_through() {
        let (state, pane_id) = test_ctx();
        let ctx = FormatContext {
            state: &state,
            pane_id: &pane_id,
        };
        assert_eq!(expand("#{unknown_var}", &ctx), "#{unknown_var}");
    }

    #[test]
    fn expand_mixed_text_and_variables() {
        let (state, pane_id) = test_ctx();
        let ctx = FormatContext {
            state: &state,
            pane_id: &pane_id,
        };
        assert_eq!(
            expand("pane=#{pane_id} session=#{session_name}", &ctx),
            "pane=%1 session=dev"
        );
    }

    #[test]
    fn expand_empty_state_uses_defaults() {
        let state = TmuxState::default();
        let ctx = FormatContext {
            state: &state,
            pane_id: "%5",
        };
        assert_eq!(expand("#{pane_id}", &ctx), "%5");
        assert_eq!(expand("#{session_name}", &ctx), "default");
        assert_eq!(expand("#{pane_index}", &ctx), "0");
    }
}
