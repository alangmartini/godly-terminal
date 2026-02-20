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
}
