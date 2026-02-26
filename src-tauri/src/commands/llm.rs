use std::sync::Arc;

use tauri::State;

use crate::llm_state::LlmState;
use godly_llm::{generate_branch_name_gemini, is_quality_branch_name};

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
pub async fn llm_generate_branch_name(
    llm: State<'_, Arc<LlmState>>,
    description: String,
) -> Result<String, String> {
    let api_key = llm.get_api_key().ok_or_else(|| {
        "No API key configured. Add your Google Gemini API key in Settings > Branch Name AI."
            .to_string()
    })?;

    let name = generate_branch_name_gemini(&api_key, &description)
        .await
        .map_err(|e| format!("Branch name generation failed: {}", e))?;

    if !is_quality_branch_name(&name) {
        return Err(format!("Generated name '{}' failed quality check", name));
    }

    Ok(name)
}
