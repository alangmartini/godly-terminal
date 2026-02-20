use anyhow::{Context, Result};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_llama::ModelWeights;
use serde::{Deserialize, Serialize};
use tokenizers::Tokenizer;

use crate::download::ModelPaths;

/// Status of the LLM engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "detail")]
pub enum LlmStatus {
    NotDownloaded,
    Downloading { progress: f32 },
    Downloaded,
    Loading,
    Ready,
    Generating,
    Error(String),
}

/// The LLM inference engine backed by candle.
pub struct LlmEngine {
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
}

impl LlmEngine {
    /// Load the model and tokenizer from the given paths.
    pub fn load(paths: &ModelPaths) -> Result<Self> {
        eprintln!("[llm] Loading GGUF model from {:?}", paths.gguf_path);

        let mut file = std::fs::File::open(&paths.gguf_path)
            .with_context(|| format!("Failed to open GGUF file: {:?}", paths.gguf_path))?;

        let device = Device::Cpu;
        let content =
            gguf_file::Content::read(&mut file).context("Failed to parse GGUF file")?;

        let model = ModelWeights::from_gguf(content, &mut file, &device)
            .context("Failed to load model weights from GGUF")?;

        eprintln!("[llm] Loading tokenizer from {:?}", paths.tokenizer_path);
        let tokenizer = Tokenizer::from_file(&paths.tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        eprintln!("[llm] Model loaded successfully");
        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    /// Generate text from a prompt.
    /// Returns only the generated text (not including the prompt).
    pub fn generate(
        &mut self,
        prompt: &str,
        max_tokens: usize,
        temperature: f64,
    ) -> Result<String> {
        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;
        let prompt_tokens = encoding.get_ids().to_vec();
        let prompt_len = prompt_tokens.len();

        let eos_token = self.tokenizer.token_to_id("<|endoftext|>");
        let im_end_token = self.tokenizer.token_to_id("<|im_end|>");

        let mut logits_processor = LogitsProcessor::new(42, Some(temperature), None);
        let mut generated_tokens: Vec<u32> = Vec::new();

        // Forward pass on all prompt tokens
        let input =
            Tensor::new(prompt_tokens.as_slice(), &self.device)?.unsqueeze(0)?;
        let logits = self.model.forward(&input, 0)?;
        let mut next_token = Self::sample_last(&logits, &mut logits_processor)?;
        let mut pos = prompt_len;

        // Autoregressive generation loop
        for _ in 0..max_tokens {
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

        let output = self
            .tokenizer
            .decode(&generated_tokens, true)
            .map_err(|e| anyhow::anyhow!("Decoding failed: {}", e))?;

        Ok(output)
    }

    /// Extract and sample from the last token's logits.
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
