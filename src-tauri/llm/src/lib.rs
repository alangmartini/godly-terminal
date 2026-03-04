mod branch_name;
mod gemini;
mod openai_compatible;

pub use branch_name::{is_quality_branch_name, sanitize_branch_name};
pub use gemini::generate_branch_name_gemini;
pub use openai_compatible::generate_branch_name_openai_compatible;

use anyhow::Result;

pub const PROVIDER_GEMINI: &str = "gemini";
pub const PROVIDER_OPENAI_COMPATIBLE: &str = "openai-compatible";

pub const DEFAULT_GEMINI_MODEL: &str = "gemini-2.0-flash-lite";
pub const DEFAULT_OPENAI_COMPAT_MODEL: &str = "gpt-4o-mini";

/// Normalize provider aliases into canonical provider IDs.
pub fn normalize_provider(provider: &str) -> Option<&'static str> {
    let normalized = provider.trim().to_ascii_lowercase();
    match normalized.as_str() {
        PROVIDER_GEMINI => Some(PROVIDER_GEMINI),
        PROVIDER_OPENAI_COMPATIBLE | "openai_compatible" | "openai" => {
            Some(PROVIDER_OPENAI_COMPATIBLE)
        }
        _ => None,
    }
}

/// Return a sensible default model for a given provider.
pub fn default_model_for_provider(provider: &str) -> &'static str {
    match normalize_provider(provider) {
        Some(PROVIDER_OPENAI_COMPATIBLE) => DEFAULT_OPENAI_COMPAT_MODEL,
        _ => DEFAULT_GEMINI_MODEL,
    }
}

/// Generate a branch name using the configured provider backend.
pub async fn generate_branch_name(
    provider: &str,
    api_key: &str,
    description: &str,
    model: &str,
    api_base_url: Option<&str>,
) -> Result<String> {
    match normalize_provider(provider) {
        Some(PROVIDER_GEMINI) => generate_branch_name_gemini(api_key, description, model).await,
        Some(PROVIDER_OPENAI_COMPATIBLE) => {
            generate_branch_name_openai_compatible(
                api_key,
                description,
                model,
                api_base_url,
            )
            .await
        }
        _ => anyhow::bail!(
            "Unsupported LLM provider '{}'. Supported providers: '{}', '{}'",
            provider,
            PROVIDER_GEMINI,
            PROVIDER_OPENAI_COMPATIBLE
        ),
    }
}
