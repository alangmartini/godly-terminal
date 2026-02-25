use crate::engine::LlmEngine;
use crate::prompt::build_chat_prompt;

const BRANCH_NAME_SYSTEM_PROMPT: &str = "\
You are a git branch name generator. Given a description of a task, output ONLY a short, \
kebab-case branch name. Rules:\n\
- Use lowercase letters, numbers, and hyphens only\n\
- Start with a conventional prefix: feat/, fix/, refactor/, docs/, chore/, test/\n\
- Keep it under 50 characters total\n\
- No explanations, just the branch name\n\
\n\
Examples:\n\
Input: \"Add user authentication with OAuth\"\n\
Output: feat/add-oauth-auth\n\
Input: \"Fix crash when opening empty file\"\n\
Output: fix/empty-file-crash\n\
Input: \"Refactor database connection pooling\"\n\
Output: refactor/db-connection-pool";

/// Generate a git branch name from a description using the LLM.
pub fn generate_branch_name(engine: &mut LlmEngine, description: &str) -> anyhow::Result<String> {
    let prompt = build_chat_prompt(BRANCH_NAME_SYSTEM_PROMPT, description);
    let raw = engine.generate(&prompt, 30, 0.3)?;
    Ok(sanitize_branch_name(&raw))
}

/// Sanitize a raw LLM output into a valid git branch name.
pub fn sanitize_branch_name(raw: &str) -> String {
    let trimmed = raw.trim();

    // Take only the first line
    let first_line = trimmed.lines().next().unwrap_or(trimmed);

    // Remove any quoting or backticks
    let cleaned = first_line.trim_matches(|c: char| c == '`' || c == '"' || c == '\'');

    // Convert to lowercase
    let lower = cleaned.to_lowercase();

    // Replace invalid characters with hyphens, keep letters/digits/hyphens/slashes
    let replaced: String = lower
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' | '-' | '/' => c,
            ' ' | '_' => '-',
            _ => '-',
        })
        .collect();

    // Collapse multiple consecutive hyphens
    let mut result = String::new();
    let mut prev_hyphen = false;
    for ch in replaced.chars() {
        if ch == '-' {
            if !prev_hyphen {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(ch);
            prev_hyphen = false;
        }
    }

    // Trim leading/trailing hyphens
    let result = result.trim_matches('-').to_string();

    // Enforce max length (50 chars), truncate at last hyphen boundary
    if result.len() <= 50 {
        result
    } else {
        let truncated = &result[..50];
        match truncated.rfind('-') {
            Some(pos) if pos > 10 => truncated[..pos].to_string(),
            _ => truncated.to_string(),
        }
    }
}

/// Check if a sanitized branch name is high enough quality to use.
///
/// Rejects names that are mostly single-letter gibberish (common with small LLMs).
/// Returns `true` if the name has enough meaningful content to be useful.
pub fn is_quality_branch_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    // Split on hyphens and slashes to get segments
    let segments: Vec<&str> = name.split(|c| c == '-' || c == '/').collect();

    // Need at least 2 segments (e.g. "fix/something" or "fix-something")
    if segments.len() < 2 {
        return false;
    }

    // Count "meaningful" segments (3+ alphabetic chars)
    let meaningful = segments
        .iter()
        .filter(|s| s.len() >= 3 && s.chars().all(|c| c.is_ascii_alphabetic()))
        .count();

    // Need at least 2 meaningful segments
    if meaningful < 2 {
        return false;
    }

    // Reject if more than half the segments are single-char
    let single_char = segments.iter().filter(|s| s.len() <= 1).count();
    if single_char > segments.len() / 2 {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_basic() {
        assert_eq!(sanitize_branch_name("feat/add-login"), "feat/add-login");
    }

    #[test]
    fn test_sanitize_uppercase() {
        assert_eq!(sanitize_branch_name("FEAT/Add-Login"), "feat/add-login");
    }

    #[test]
    fn test_sanitize_spaces() {
        assert_eq!(
            sanitize_branch_name("feat/add login page"),
            "feat/add-login-page"
        );
    }

    #[test]
    fn test_sanitize_underscores() {
        assert_eq!(
            sanitize_branch_name("feat/add_login_page"),
            "feat/add-login-page"
        );
    }

    #[test]
    fn test_sanitize_special_chars() {
        assert_eq!(
            sanitize_branch_name("feat/add:login@page!"),
            "feat/add-login-page"
        );
    }

    #[test]
    fn test_sanitize_multiple_hyphens() {
        assert_eq!(sanitize_branch_name("feat/add---login"), "feat/add-login");
    }

    #[test]
    fn test_sanitize_backtick_wrapping() {
        assert_eq!(sanitize_branch_name("`feat/add-login`"), "feat/add-login");
    }

    #[test]
    fn test_sanitize_multiline() {
        assert_eq!(
            sanitize_branch_name("feat/add-login\nsome explanation"),
            "feat/add-login"
        );
    }

    #[test]
    fn test_sanitize_length_limit() {
        let long =
            "feat/this-is-a-very-long-branch-name-that-exceeds-the-fifty-character-limit-by-a-lot";
        let result = sanitize_branch_name(long);
        assert!(result.len() <= 50);
    }

    #[test]
    fn test_sanitize_empty() {
        assert_eq!(sanitize_branch_name(""), "");
    }

    #[test]
    fn test_sanitize_trim_hyphens() {
        assert_eq!(sanitize_branch_name("-feat/add-login-"), "feat/add-login");
    }

    #[test]
    fn test_sanitize_quoted() {
        assert_eq!(
            sanitize_branch_name("\"feat/add-login\""),
            "feat/add-login"
        );
    }

    #[test]
    fn test_sanitize_preserves_prefix_slash() {
        assert_eq!(
            sanitize_branch_name("fix/crash-on-startup"),
            "fix/crash-on-startup"
        );
    }

    // --- Quality gate tests ---

    #[test]
    fn test_quality_rejects_single_letter_gibberish() {
        // The actual bad branch name from production
        assert!(!is_quality_branch_name("s-s-ss-s-s-s-guide-guide"));
    }

    #[test]
    fn test_quality_rejects_all_single_chars() {
        assert!(!is_quality_branch_name("a-b-c-d-e"));
    }

    #[test]
    fn test_quality_rejects_empty() {
        assert!(!is_quality_branch_name(""));
    }

    #[test]
    fn test_quality_rejects_single_word() {
        assert!(!is_quality_branch_name("fix"));
    }

    #[test]
    fn test_quality_accepts_good_hyphen_name() {
        assert!(is_quality_branch_name("feat-add-login"));
    }

    #[test]
    fn test_quality_accepts_good_slash_name() {
        assert!(is_quality_branch_name("fix/crash-on-startup"));
    }

    #[test]
    fn test_quality_accepts_prefix_with_short_words() {
        // "on" is short but there are enough meaningful segments
        assert!(is_quality_branch_name("fix/crash-on-resize"));
    }

    #[test]
    fn test_quality_accepts_two_meaningful_segments() {
        assert!(is_quality_branch_name("fix/scrollback"));
    }

    #[test]
    fn test_quality_rejects_repeated_gibberish() {
        assert!(!is_quality_branch_name("x-y-z-x-y-z"));
    }

    #[test]
    fn test_quality_rejects_mostly_numbers() {
        assert!(!is_quality_branch_name("1-2-3-fix"));
    }

    #[test]
    fn test_quality_accepts_real_llm_output() {
        assert!(is_quality_branch_name("feat-add-oauth-auth"));
        assert!(is_quality_branch_name("fix-empty-file-crash"));
        assert!(is_quality_branch_name("refactor-db-connection-pool"));
        assert!(is_quality_branch_name("docs-update-readme"));
        assert!(is_quality_branch_name("chore-bump-deps"));
    }
}
