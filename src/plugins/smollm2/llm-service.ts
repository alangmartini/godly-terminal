import { invoke } from '@tauri-apps/api/core';

export async function llmHasApiKey(): Promise<boolean> {
  return invoke<boolean>('llm_has_api_key');
}

export async function llmSetApiKey(key: string | null): Promise<void> {
  return invoke<void>('llm_set_api_key', { key });
}

export async function llmSetModel(model: string): Promise<void> {
  return invoke<void>('llm_set_model', { model });
}

export async function llmGetModel(): Promise<string> {
  return invoke<string>('llm_get_model');
}

export async function llmGenerateBranchName(description: string): Promise<string> {
  return invoke<string>('llm_generate_branch_name', { description });
}
