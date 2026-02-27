use parking_lot::RwLock;

/// Default Gemini model for branch name generation.
const DEFAULT_MODEL: &str = "gemini-2.0-flash-lite";

pub struct LlmState {
    /// Google Gemini API key for branch name generation.
    pub api_key: RwLock<Option<String>>,
    /// Selected Gemini model ID.
    pub model: RwLock<String>,
}

impl LlmState {
    pub fn new() -> Self {
        Self {
            api_key: RwLock::new(None),
            model: RwLock::new(DEFAULT_MODEL.to_string()),
        }
    }

    pub fn set_api_key(&self, key: Option<String>) {
        *self.api_key.write() = key;
    }

    pub fn get_api_key(&self) -> Option<String> {
        self.api_key.read().clone()
    }

    pub fn has_api_key(&self) -> bool {
        self.api_key.read().is_some()
    }

    pub fn set_model(&self, model: String) {
        *self.model.write() = model;
    }

    pub fn get_model(&self) -> String {
        self.model.read().clone()
    }
}
