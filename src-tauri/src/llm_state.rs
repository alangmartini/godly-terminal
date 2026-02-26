use parking_lot::RwLock;

pub struct LlmState {
    /// Google Gemini API key for branch name generation.
    pub api_key: RwLock<Option<String>>,
}

impl LlmState {
    pub fn new() -> Self {
        Self {
            api_key: RwLock::new(None),
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
}
