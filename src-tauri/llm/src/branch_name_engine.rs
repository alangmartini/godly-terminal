use std::path::Path;

use anyhow::{Context, Result};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_llama::ModelWeights;
use tokenizers::Tokenizer;

use crate::branch_name::sanitize_branch_name;

/// System prompt baked into the tiny model's training.
const SYSTEM_PROMPT: &str = "\
You generate concise git branch names from descriptions. \
Output only the branch name slug (e.g. fix-crash-on-resize). \
Use prefixes: feat-, fix-, refactor-, docs-, chore-, test-, style-. \
Use only lowercase, numbers, hyphens. Max 50 chars.";

/// A lightweight inference engine for the tiny branch name generator model.
/// Separate from LlmEngine to avoid mutex contention with the main model.
pub struct BranchNameEngine {
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
}

impl BranchNameEngine {
    /// Load the tiny branch name GGUF model and tokenizer from disk.
    pub fn load(gguf_path: &Path, tokenizer_path: &Path) -> Result<Self> {
        let file_size = std::fs::metadata(gguf_path)
            .map(|m| m.len())
            .unwrap_or(0);
        eprintln!(
            "[branch-name-engine] Loading GGUF from {:?} ({:.1} MB)",
            gguf_path,
            file_size as f64 / 1e6
        );

        let mut file = std::fs::File::open(gguf_path)
            .with_context(|| format!("Failed to open GGUF: {:?}", gguf_path))?;

        let device = Device::Cpu;
        let content =
            gguf_file::Content::read(&mut file).context("Failed to parse GGUF")?;

        let model = ModelWeights::from_gguf(content, &mut file, &device)
            .context("Failed to load branch name model weights")?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        eprintln!("[branch-name-engine] Model loaded");
        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    /// Generate a branch name slug from a description.
    ///
    /// Uses hardcoded parameters optimized for branch name generation:
    /// - max_tokens: 20 (branch names are short)
    /// - temperature: 0.1 (near-deterministic output)
    pub fn generate(&mut self, description: &str) -> Result<String> {
        let prompt = format!(
            "<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n\
             <|im_start|>user\n{description}<|im_end|>\n\
             <|im_start|>assistant\n"
        );

        let encoding = self
            .tokenizer
            .encode(prompt.as_str(), true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
        let prompt_tokens = encoding.get_ids().to_vec();
        let prompt_len = prompt_tokens.len();

        let eos_token = self.tokenizer.token_to_id("<|endoftext|>");
        let im_end_token = self.tokenizer.token_to_id("<|im_end|>");

        let mut logits_processor = LogitsProcessor::new(42, Some(0.1), None);
        let mut generated_tokens: Vec<u32> = Vec::new();

        // Forward pass on all prompt tokens
        let input =
            Tensor::new(prompt_tokens.as_slice(), &self.device)?.unsqueeze(0)?;
        let logits = self.model.forward(&input, 0)?;
        let mut next_token = Self::sample_last(&logits, &mut logits_processor)?;
        let mut pos = prompt_len;

        // Autoregressive generation (max 20 tokens)
        for _ in 0..20 {
            if Self::is_stop_token(next_token, eos_token, im_end_token) {
                break;
            }
            generated_tokens.push(next_token);

            let input =
                Tensor::new(&[next_token], &self.device)?.unsqueeze(0)?;
            let logits = self.model.forward(&input, pos)?;
            next_token = Self::sample_last(&logits, &mut logits_processor)?;
            pos += 1;
        }

        let raw = self
            .tokenizer
            .decode(&generated_tokens, true)
            .map_err(|e| anyhow::anyhow!("Decoding failed: {}", e))?;

        Ok(sanitize_branch_name(&raw))
    }

    fn sample_last(logits: &Tensor, processor: &mut LogitsProcessor) -> Result<u32> {
        let logits = logits.squeeze(0)?;
        let last = match logits.dims().len() {
            1 => logits,
            2 => logits.get(logits.dim(0)? - 1)?,
            n => anyhow::bail!("Unexpected logits dimensions: {}", n),
        };
        Ok(processor.sample(&last)?)
    }

    fn is_stop_token(token: u32, eos: Option<u32>, im_end: Option<u32>) -> bool {
        if let Some(eos) = eos {
            if token == eos {
                return true;
            }
        }
        if let Some(im_end) = im_end {
            if token == im_end {
                return true;
            }
        }
        false
    }
}

/// Try to generate a branch name using the tiny engine.
/// Returns None if the engine is unavailable or generation fails.
pub fn try_generate_branch_name(
    engine: &mut BranchNameEngine,
    description: &str,
) -> Option<String> {
    match engine.generate(description) {
        Ok(name) if !name.is_empty() && name.len() >= 3 => Some(name),
        Ok(_) => {
            eprintln!("[branch-name-engine] Generated name too short, falling back");
            None
        }
        Err(e) => {
            eprintln!("[branch-name-engine] Generation failed: {}, falling back", e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_not_empty() {
        assert!(!SYSTEM_PROMPT.is_empty());
    }

    #[test]
    fn test_prompt_format() {
        // Verify the prompt format matches ChatML
        let description = "Fix crash on resize";
        let prompt = format!(
            "<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n\
             <|im_start|>user\n{description}<|im_end|>\n\
             <|im_start|>assistant\n"
        );
        assert!(prompt.starts_with("<|im_start|>system\n"));
        assert!(prompt.contains("Fix crash on resize"));
        assert!(prompt.ends_with("<|im_start|>assistant\n"));
    }
}
