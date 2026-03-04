use parking_lot::RwLock;

/// Default provider for branch name generation.
const DEFAULT_PROVIDER: &str = godly_llm::PROVIDER_GEMINI;
/// Default model for branch name generation.
const DEFAULT_MODEL: &str = godly_llm::DEFAULT_GEMINI_MODEL;

pub struct LlmState {
    /// LLM provider API key for branch name generation.
    pub api_key: RwLock<Option<String>>,
    /// Selected provider ID.
    pub provider: RwLock<String>,
    /// Selected model ID.
    pub model: RwLock<String>,
    /// Optional custom API base URL (used by compatible providers).
    pub api_base_url: RwLock<Option<String>>,
}

impl LlmState {
    pub fn new() -> Self {
        Self {
            api_key: RwLock::new(None),
            provider: RwLock::new(DEFAULT_PROVIDER.to_string()),
            model: RwLock::new(DEFAULT_MODEL.to_string()),
            api_base_url: RwLock::new(None),
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

    pub fn set_provider(&self, provider: String) {
        *self.provider.write() = provider;
    }

    pub fn get_provider(&self) -> String {
        self.provider.read().clone()
    }

    pub fn set_model(&self, model: String) {
        *self.model.write() = model;
    }

    pub fn get_model(&self) -> String {
        self.model.read().clone()
    }

    pub fn set_api_base_url(&self, api_base_url: Option<String>) {
        *self.api_base_url.write() = api_base_url;
    }

    pub fn get_api_base_url(&self) -> Option<String> {
        self.api_base_url.read().clone()
    }
}
