use std::path::PathBuf;

use godly_llm::{BranchNameEngine, BranchNameModelPaths, LlmEngine, LlmStatus, ModelPaths};
use parking_lot::RwLock;

pub struct LlmState {
    pub engine: RwLock<Option<LlmEngine>>,
    pub status: RwLock<LlmStatus>,
    pub model_dir: RwLock<Option<PathBuf>>,
    /// Separate engine for the tiny branch name generator model.
    /// Loaded independently from the main LLM to avoid mutex contention.
    pub branch_engine: RwLock<Option<BranchNameEngine>>,
}

impl LlmState {
    pub fn new() -> Self {
        Self {
            engine: RwLock::new(None),
            status: RwLock::new(LlmStatus::NotDownloaded),
            model_dir: RwLock::new(None),
            branch_engine: RwLock::new(None),
        }
    }

    /// Initialize with app data dir and check if model is already downloaded.
    pub fn init(&self, app_data_dir: PathBuf) {
        let paths = ModelPaths::new(&app_data_dir);
        if paths.is_downloaded() {
            *self.status.write() = LlmStatus::Downloaded;
        }

        // Auto-load branch name engine if the tiny model exists
        let branch_paths = BranchNameModelPaths::new(&app_data_dir);
        if branch_paths.is_downloaded() {
            match BranchNameEngine::load(&branch_paths.gguf_path, &branch_paths.tokenizer_path) {
                Ok(engine) => {
                    eprintln!("[llm] Branch name engine loaded from {:?}", branch_paths.model_dir);
                    *self.branch_engine.write() = Some(engine);
                }
                Err(e) => {
                    eprintln!("[llm] Failed to load branch name engine: {}", e);
                }
            }
        }

        *self.model_dir.write() = Some(app_data_dir);
    }

    pub fn get_app_data_dir(&self) -> Option<PathBuf> {
        self.model_dir.read().clone()
    }

    /// Try to generate a branch name using the tiny engine.
    /// Returns None if the engine is unavailable or busy.
    pub fn try_generate_branch_name(&self, description: &str) -> Option<String> {
        // Use try_write to avoid blocking if someone else is generating
        match self.branch_engine.try_write() {
            Some(mut guard) => {
                guard.as_mut().and_then(|engine| {
                    godly_llm::try_generate_branch_name(engine, description)
                })
            }
            None => {
                eprintln!("[llm] Branch name engine busy, falling back to UUID");
                None
            }
        }
    }
}
