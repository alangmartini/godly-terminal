import { invoke } from '@tauri-apps/api/core';

export interface LlmStatus {
  status: 'NotDownloaded' | 'Downloading' | 'Downloaded' | 'Loading' | 'Ready' | 'Generating' | 'Error';
  detail?: { progress?: number } | string;
}

export async function llmGetStatus(): Promise<LlmStatus> {
  return invoke<LlmStatus>('llm_get_status');
}

export async function llmDownloadModel(
  hfRepo?: string,
  hfFilename?: string,
  tokenizerRepo?: string,
  subdir?: string,
): Promise<void> {
  return invoke<void>('llm_download_model', {
    hfRepo: hfRepo ?? null,
    hfFilename: hfFilename ?? null,
    tokenizerRepo: tokenizerRepo ?? null,
    subdir: subdir ?? null,
  });
}

export async function llmLoadModel(
  opts?: {
    ggufPath?: string;
    tokenizerPath?: string;
    subdir?: string;
    ggufFilename?: string;
  },
): Promise<void> {
  return invoke<void>('llm_load_model', {
    ggufPath: opts?.ggufPath ?? null,
    tokenizerPath: opts?.tokenizerPath ?? null,
    subdir: opts?.subdir ?? null,
    ggufFilename: opts?.ggufFilename ?? null,
  });
}

export async function llmUnloadModel(): Promise<void> {
  return invoke<void>('llm_unload_model');
}

export async function llmGenerate(
  prompt: string,
  maxTokens?: number,
  temperature?: number,
): Promise<string> {
  return invoke<string>('llm_generate', {
    prompt,
    maxTokens: maxTokens ?? undefined,
    temperature: temperature ?? undefined,
  });
}

export async function llmGenerateBranchName(
  description: string,
  useTiny?: boolean,
): Promise<string> {
  return invoke<string>('llm_generate_branch_name', {
    description,
    useTiny: useTiny ?? null,
  });
}

export async function llmCheckModelFiles(
  opts?: {
    subdir?: string;
    ggufFilename?: string;
    ggufPath?: string;
    tokenizerPath?: string;
  },
): Promise<boolean> {
  return invoke<boolean>('llm_check_model_files', {
    subdir: opts?.subdir ?? null,
    ggufFilename: opts?.ggufFilename ?? null,
    ggufPath: opts?.ggufPath ?? null,
    tokenizerPath: opts?.tokenizerPath ?? null,
  });
}

export function isModelReady(status: LlmStatus): boolean {
  return status.status === 'Ready';
}

export function isModelDownloaded(status: LlmStatus): boolean {
  return status.status === 'Downloaded' || status.status === 'Ready' || status.status === 'Loading';
}

export function getStatusLabel(status: LlmStatus): string {
  switch (status.status) {
    case 'NotDownloaded': return 'Not Downloaded';
    case 'Downloading': return 'Downloading...';
    case 'Downloaded': return 'Downloaded (not loaded)';
    case 'Loading': return 'Loading...';
    case 'Ready': return 'Ready';
    case 'Generating': return 'Generating...';
    case 'Error': return `Error: ${typeof status.detail === 'string' ? status.detail : 'Unknown'}`;
  }
}
