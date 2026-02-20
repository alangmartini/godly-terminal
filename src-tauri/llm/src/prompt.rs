/// Build a ChatML-formatted prompt for SmolLM2-Instruct.
pub fn build_chat_prompt(system: &str, user: &str) -> String {
    format!(
        "<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chatml_format() {
        let result = build_chat_prompt("You are helpful.", "Hello!");
        assert_eq!(
            result,
            "<|im_start|>system\nYou are helpful.<|im_end|>\n<|im_start|>user\nHello!<|im_end|>\n<|im_start|>assistant\n"
        );
    }

    #[test]
    fn test_empty_system() {
        let result = build_chat_prompt("", "Hello!");
        assert!(result.contains("<|im_start|>system\n<|im_end|>"));
    }

    #[test]
    fn test_multiline_user() {
        let result = build_chat_prompt("sys", "line 1\nline 2");
        assert!(result.contains("line 1\nline 2"));
    }

    #[test]
    fn test_prompt_ends_with_assistant_tag() {
        let result = build_chat_prompt("sys", "msg");
        assert!(result.ends_with("<|im_start|>assistant\n"));
    }
}
