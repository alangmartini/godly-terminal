use std::path::Path;
use std::time::Instant;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct Transcriber {
    ctx: Option<WhisperContext>,
    model_name: Option<String>,
    language: String,
    gpu_in_use: bool,
}

impl Transcriber {
    pub fn new() -> Self {
        Self {
            ctx: None,
            model_name: None,
            language: String::new(),
            gpu_in_use: false,
        }
    }

    /// Load a whisper model from the given path.
    pub fn load_model(
        &mut self,
        model_path: &str,
        use_gpu: bool,
        _gpu_device: i32,
        language: String,
    ) -> Result<(String, bool), String> {
        let path = Path::new(model_path);
        if !path.exists() {
            return Err(format!("Model file not found: {}", model_path));
        }

        let model_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| model_path.to_string());

        let cuda_compiled = cfg!(feature = "cuda");
        let effective_gpu = use_gpu && cuda_compiled;

        if use_gpu && !cuda_compiled {
            eprintln!("[whisper] WARNING: GPU requested but binary was built without CUDA support. Running on CPU.");
        }

        let mut params = WhisperContextParameters::default();
        params.use_gpu(effective_gpu);

        let ctx = WhisperContext::new_with_params(model_path, params)
            .map_err(|e| format!("Failed to load whisper model: {}", e))?;

        let gpu_in_use = effective_gpu;

        self.ctx = Some(ctx);
        self.model_name = Some(model_name.clone());
        self.language = language;
        self.gpu_in_use = gpu_in_use;

        Ok((model_name, gpu_in_use))
    }

    /// Transcribe PCM audio samples (16kHz mono f32). Returns (text, duration_ms).
    pub fn transcribe(&self, samples: &[f32]) -> Result<(String, u64), String> {
        let ctx = self.ctx.as_ref()
            .ok_or("No model loaded")?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(if self.language.is_empty() {
            None
        } else {
            Some(&self.language)
        });
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        // Single-segment mode for voice-to-text (no timestamps needed)
        params.set_single_segment(true);

        let start = Instant::now();

        let mut state = ctx.create_state()
            .map_err(|e| format!("Failed to create whisper state: {}", e))?;

        state.full(params, samples)
            .map_err(|e| format!("Transcription failed: {}", e))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        let num_segments = state.full_n_segments()
            .map_err(|e| format!("Failed to get segment count: {}", e))?;

        let mut text = String::new();
        for i in 0..num_segments {
            if let Ok(segment) = state.full_get_segment_text(i) {
                text.push_str(&segment);
            }
        }

        Ok((text.trim().to_string(), duration_ms))
    }

    pub fn is_loaded(&self) -> bool {
        self.ctx.is_some()
    }

    pub fn model_name(&self) -> Option<&str> {
        self.model_name.as_deref()
    }

    pub fn gpu_in_use(&self) -> bool {
        self.gpu_in_use
    }

    pub fn cuda_available() -> bool {
        cfg!(feature = "cuda")
    }
}
