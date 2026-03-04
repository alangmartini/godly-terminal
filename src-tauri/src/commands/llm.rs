use std::sync::Arc;

use tauri::State;

use crate::llm_state::LlmState;
use godly_llm::{generate_branch_name, is_quality_branch_name, normalize_provider};

#[tauri::command]
pub async fn llm_has_api_key(llm: State<'_, Arc<LlmState>>) -> Result<bool, String> {
    Ok(llm.has_api_key())
}

#[tauri::command]
pub async fn llm_set_api_key(
    llm: State<'_, Arc<LlmState>>,
    key: Option<String>,
) -> Result<(), String> {
    let key = key
        .filter(|k| !k.trim().is_empty())
        .map(|k| k.trim().to_string());
    llm.set_api_key(key);
    Ok(())
}

#[tauri::command]
pub async fn llm_set_provider(
    llm: State<'_, Arc<LlmState>>,
    provider: String,
) -> Result<(), String> {
    let provider = normalize_provider(&provider).ok_or_else(|| {
        format!(
            "Unsupported provider '{}'. Supported providers: 'gemini', 'openai-compatible'",
            provider
        )
    })?;

    let previous_provider = llm.get_provider();
    let previous_default_model = godly_llm::default_model_for_provider(&previous_provider);
    let current_model = llm.get_model();

    llm.set_provider(provider.to_string());

    // Keep model sensible when switching providers.
    if current_model.trim().is_empty() || current_model == previous_default_model {
        llm.set_model(godly_llm::default_model_for_provider(provider).to_string());
    }

    Ok(())
}

#[tauri::command]
pub async fn llm_get_provider(llm: State<'_, Arc<LlmState>>) -> Result<String, String> {
    Ok(llm.get_provider())
}

#[tauri::command]
pub async fn llm_set_model(
    llm: State<'_, Arc<LlmState>>,
    model: String,
) -> Result<(), String> {
    let model = model.trim();
    if model.is_empty() {
        return Err("Model cannot be empty".to_string());
    }
    llm.set_model(model.to_string());
    Ok(())
}

#[tauri::command]
pub async fn llm_get_model(llm: State<'_, Arc<LlmState>>) -> Result<String, String> {
    Ok(llm.get_model())
}

#[tauri::command]
pub async fn llm_set_api_base_url(
    llm: State<'_, Arc<LlmState>>,
    api_base_url: Option<String>,
) -> Result<(), String> {
    let api_base_url = api_base_url
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty());
    llm.set_api_base_url(api_base_url);
    Ok(())
}

#[tauri::command]
pub async fn llm_get_api_base_url(
    llm: State<'_, Arc<LlmState>>,
) -> Result<Option<String>, String> {
    Ok(llm.get_api_base_url())
}

#[tauri::command]
pub async fn llm_generate_branch_name(
    llm: State<'_, Arc<LlmState>>,
    description: String,
) -> Result<String, String> {
    let api_key = llm.get_api_key().ok_or_else(|| {
        "No API key configured. Add your provider API key in Settings > Branch Name AI."
            .to_string()
    })?;
    let provider = llm.get_provider();
    let model = llm.get_model();
    let api_base_url = llm.get_api_base_url();

    let name = generate_branch_name(
        &provider,
        &api_key,
        &description,
        &model,
        api_base_url.as_deref(),
    )
        .await
        .map_err(|e| format!("Branch name generation failed: {}", e))?;

    if !is_quality_branch_name(&name) {
        return Err(format!("Generated name '{}' failed quality check", name));
    }

    Ok(name)
}
