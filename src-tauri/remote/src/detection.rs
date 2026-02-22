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
];

/// A detected prompt with metadata.
#[derive(Debug, Clone, Serialize)]
pub struct DetectedPrompt {
    pub matched_pattern: String,
    pub prompt_type: String,
    pub context_text: String,
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
                return Some(DetectedPrompt {
                    matched_pattern: source.clone(),
                    prompt_type: type_label.clone(),
                    context_text: text.to_string(),
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
                results.push(DetectedPrompt {
                    matched_pattern: source.clone(),
                    prompt_type: type_label.clone(),
                    context_text: text.to_string(),
                });
            }
        }
        results
    }
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
}
