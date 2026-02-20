use std::path::PathBuf;

use godly_llm::{LlmEngine, LlmStatus, ModelPaths};
use parking_lot::RwLock;

pub struct LlmState {
    pub engine: RwLock<Option<LlmEngine>>,
    pub status: RwLock<LlmStatus>,
    pub model_dir: RwLock<Option<PathBuf>>,
}

impl LlmState {
    pub fn new() -> Self {
        Self {
            engine: RwLock::new(None),
            status: RwLock::new(LlmStatus::NotDownloaded),
            model_dir: RwLock::new(None),
        }
    }

    /// Initialize with app data dir and check if model is already downloaded.
    pub fn init(&self, app_data_dir: PathBuf) {
        let paths = ModelPaths::new(&app_data_dir);
        if paths.is_downloaded() {
            *self.status.write() = LlmStatus::Downloaded;
        }
        *self.model_dir.write() = Some(app_data_dir);
    }

    pub fn get_app_data_dir(&self) -> Option<PathBuf> {
        self.model_dir.read().clone()
    }
}
