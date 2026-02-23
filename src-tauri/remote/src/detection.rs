use regex::Regex;
use serde::Serialize;

/// Default patterns that match common Claude Code permission prompts.
const DEFAULT_PATTERNS: &[(&str, &str)] = &[
    (r"Do you want to proceed\?", "yes_no_prompt"),
    (r"Allow this action\?", "tool_approval"),
    (r"\(Y\)es.*\(N\)o", "yes_no_prompt"),
    (r"Do you want to allow", "tool_approval"),
    (r"Press Enter to continue", "continue_prompt"),
    (r"Allow .+ to run", "tool_approval"),
    (r"Approve\?", "tool_approval"),
    (r"\[Y/n\]", "yes_no_prompt"),
    (r"Enter to select.*to navigate", "select_menu"),
];

/// A single option in a select menu.
#[derive(Debug, Clone, Serialize)]
pub struct SelectMenuOption {
    /// 0-based index of this option in the list.
    pub index: usize,
    /// Display label (e.g. "Full snapshot (Recommended)").
    pub label: String,
    /// Whether this option is currently highlighted (has the `>` cursor).
    pub selected: bool,
}

/// A detected prompt with metadata.
#[derive(Debug, Clone, Serialize)]
pub struct DetectedPrompt {
    pub matched_pattern: String,
    pub prompt_type: String,
    pub context_text: String,
    /// For `select_menu` prompts, the parsed options with their selection state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_options: Option<Vec<SelectMenuOption>>,
}

/// Compiled set of patterns for detecting prompts in terminal output.
pub struct PromptDetector {
    patterns: Vec<(Regex, String, String)>, // (regex, source, type_label)
}

impl PromptDetector {
    /// Create a detector with the default pattern set.
    pub fn new() -> Self {
        let patterns = DEFAULT_PATTERNS
            .iter()
            .filter_map(|(pattern, label)| {
                Regex::new(pattern)
                    .ok()
                    .map(|r| (r, pattern.to_string(), label.to_string()))
            })
            .collect();
        Self { patterns }
    }

    /// Check text for any matching prompt patterns.
    /// Returns the first match found, or None.
    pub fn detect(&self, text: &str) -> Option<DetectedPrompt> {
        for (regex, source, type_label) in &self.patterns {
            if regex.is_match(text) {
                let menu_options = if type_label == "select_menu" {
                    Some(parse_select_menu(text))
                } else {
                    None
                };
                return Some(DetectedPrompt {
                    matched_pattern: source.clone(),
                    prompt_type: type_label.clone(),
                    context_text: text.to_string(),
                    menu_options,
                });
            }
        }
        None
    }

    /// Check text and return all matches (not just the first).
    pub fn detect_all(&self, text: &str) -> Vec<DetectedPrompt> {
        let mut results = Vec::new();
        for (regex, source, type_label) in &self.patterns {
            if regex.is_match(text) {
                let menu_options = if type_label == "select_menu" {
                    Some(parse_select_menu(text))
                } else {
                    None
                };
                results.push(DetectedPrompt {
                    matched_pattern: source.clone(),
                    prompt_type: type_label.clone(),
                    context_text: text.to_string(),
                    menu_options,
                });
            }
        }
        results
    }
}

/// Parse a Claude Code select menu from terminal output.
///
/// Looks for lines matching `> N. Label` (selected) or `  N. Label` (unselected).
/// Returns the list of options with their selection state.
fn parse_select_menu(text: &str) -> Vec<SelectMenuOption> {
    // Match lines like:  "> 1. Full snapshot (Recommended)"  or  "  2. Daemon-level only"
    // The `>` prefix indicates the currently selected option.
    // Also handle cases where the `>` may have varying whitespace.
    let option_re = Regex::new(r"(?m)^[>\s]{0,4}\s*(\d+)\.\s+(.+?)$").unwrap();
    let selected_re = Regex::new(r"(?m)^>\s*(\d+)\.").unwrap();

    // Find which option number is selected (has `>` prefix)
    let selected_num: Option<u32> = selected_re.captures(text).and_then(|c| {
        c.get(1).and_then(|m| m.as_str().parse().ok())
    });

    let mut options = Vec::new();
    for (idx, cap) in option_re.captures_iter(text).enumerate() {
        let num: u32 = cap[1].parse().unwrap_or(0);
        let label = cap[2].trim().to_string();

        // Skip the "Enter to select" footer line if it accidentally matches
        if label.contains("to navigate") || label.contains("to cancel") {
            continue;
        }

        options.push(SelectMenuOption {
            index: idx,
            label,
            selected: selected_num == Some(num),
        });
    }

    options
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_patterns_compile() {
        let detector = PromptDetector::new();
        assert_eq!(detector.patterns.len(), DEFAULT_PATTERNS.len());
    }

    #[test]
    fn detects_yes_no_prompt() {
        let detector = PromptDetector::new();
        let result = detector.detect("Some output\n  (Y)es  (N)o\n");
        assert!(result.is_some());
        assert_eq!(result.unwrap().prompt_type, "yes_no_prompt");
    }

    #[test]
    fn detects_tool_approval() {
        let detector = PromptDetector::new();
        let result = detector.detect("Allow this action?");
        assert!(result.is_some());
        assert_eq!(result.unwrap().prompt_type, "tool_approval");
    }

    #[test]
    fn detects_do_you_want_to_allow() {
        let detector = PromptDetector::new();
        let result = detector.detect("Do you want to allow this tool call?");
        assert!(result.is_some());
        assert_eq!(result.unwrap().prompt_type, "tool_approval");
    }

    #[test]
    fn detects_press_enter() {
        let detector = PromptDetector::new();
        let result = detector.detect("Press Enter to continue");
        assert!(result.is_some());
        assert_eq!(result.unwrap().prompt_type, "continue_prompt");
    }

    #[test]
    fn detects_allow_tool_to_run() {
        let detector = PromptDetector::new();
        let result = detector.detect("Allow Bash to run `ls -la`?");
        assert!(result.is_some());
        assert_eq!(result.unwrap().prompt_type, "tool_approval");
    }

    #[test]
    fn detects_y_n_bracket() {
        let detector = PromptDetector::new();
        let result = detector.detect("Continue? [Y/n]");
        assert!(result.is_some());
        assert_eq!(result.unwrap().prompt_type, "yes_no_prompt");
    }

    #[test]
    fn no_match_on_normal_output() {
        let detector = PromptDetector::new();
        let result = detector.detect("$ ls -la\ntotal 42\ndrwxr-xr-x  5 user user 4096 Feb 19 12:00 .");
        assert!(result.is_none());
    }

    #[test]
    fn no_match_on_empty() {
        let detector = PromptDetector::new();
        assert!(detector.detect("").is_none());
    }

    #[test]
    fn detect_all_returns_multiple() {
        let detector = PromptDetector::new();
        // Text that matches both yes_no and tool_approval
        let result = detector.detect_all("Do you want to allow this? (Y)es (N)o");
        assert!(result.len() >= 2);
    }

    #[test]
    fn detects_select_menu() {
        let detector = PromptDetector::new();
        let text = r#"What level of detail?

> 1. Full snapshot (Recommended)
  2. Daemon-level only
  3. Per-session only
  4. Type something.

Enter to select · ↑/↓ to navigate · Esc to cancel"#;

        let result = detector.detect(text);
        assert!(result.is_some());
        let det = result.unwrap();
        assert_eq!(det.prompt_type, "select_menu");
        assert!(det.menu_options.is_some());

        let opts = det.menu_options.unwrap();
        assert_eq!(opts.len(), 4);
        assert_eq!(opts[0].label, "Full snapshot (Recommended)");
        assert!(opts[0].selected);
        assert_eq!(opts[1].label, "Daemon-level only");
        assert!(!opts[1].selected);
        assert_eq!(opts[2].label, "Per-session only");
        assert!(!opts[2].selected);
        assert_eq!(opts[3].label, "Type something.");
        assert!(!opts[3].selected);
    }

    #[test]
    fn select_menu_tracks_selected_option() {
        let text = r#"  1. Option A
> 2. Option B
  3. Option C

Enter to select · ↑/↓ to navigate · Esc to cancel"#;

        let opts = parse_select_menu(text);
        assert_eq!(opts.len(), 3);
        assert!(!opts[0].selected);
        assert!(opts[1].selected);
        assert!(!opts[2].selected);
    }

    #[test]
    fn select_menu_empty_on_no_options() {
        let text = "Enter to select · ↑/↓ to navigate · Esc to cancel";
        let opts = parse_select_menu(text);
        assert!(opts.is_empty());
    }
}
