import { invoke } from '@tauri-apps/api/core';

export type BranchAiProvider = 'gemini' | 'openai-compatible';

export async function llmHasApiKey(): Promise<boolean> {
  return invoke<boolean>('llm_has_api_key');
}

export async function llmSetApiKey(key: string | null): Promise<void> {
  return invoke<void>('llm_set_api_key', { key });
}

export async function llmSetProvider(provider: BranchAiProvider): Promise<void> {
  return invoke<void>('llm_set_provider', { provider });
}

export async function llmGetProvider(): Promise<BranchAiProvider> {
  return invoke<BranchAiProvider>('llm_get_provider');
}

export async function llmSetModel(model: string): Promise<void> {
  return invoke<void>('llm_set_model', { model });
}

export async function llmGetModel(): Promise<string> {
  return invoke<string>('llm_get_model');
}

export async function llmSetApiBaseUrl(apiBaseUrl: string | null): Promise<void> {
  return invoke<void>('llm_set_api_base_url', { apiBaseUrl });
}

export async function llmGetApiBaseUrl(): Promise<string | null> {
  return invoke<string | null>('llm_get_api_base_url');
}

export async function llmGenerateBranchName(description: string): Promise<string> {
  return invoke<string>('llm_generate_branch_name', { description });
}
