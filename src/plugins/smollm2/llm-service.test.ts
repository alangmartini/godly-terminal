// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import {
  llmGetApiBaseUrl,
  llmGetProvider,
  llmHasApiKey,
  llmGenerateBranchName,
  llmSetApiBaseUrl,
  llmSetApiKey,
  llmSetProvider,
} from './llm-service';

describe('llm-service', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('llmHasApiKey invokes correct command', async () => {
    mockInvoke.mockResolvedValue(true);
    const result = await llmHasApiKey();
    expect(mockInvoke).toHaveBeenCalledWith('llm_has_api_key');
    expect(result).toBe(true);
  });

  it('llmSetApiKey invokes with key', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await llmSetApiKey('AIza-test-key');
    expect(mockInvoke).toHaveBeenCalledWith('llm_set_api_key', { key: 'AIza-test-key' });
  });

  it('llmSetApiKey invokes with null to clear', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await llmSetApiKey(null);
    expect(mockInvoke).toHaveBeenCalledWith('llm_set_api_key', { key: null });
  });

  it('llmSetProvider invokes with provider', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await llmSetProvider('openai-compatible');
    expect(mockInvoke).toHaveBeenCalledWith('llm_set_provider', { provider: 'openai-compatible' });
  });

  it('llmGetProvider invokes correct command', async () => {
    mockInvoke.mockResolvedValue('gemini');
    const provider = await llmGetProvider();
    expect(mockInvoke).toHaveBeenCalledWith('llm_get_provider');
    expect(provider).toBe('gemini');
  });

  it('llmGenerateBranchName passes description', async () => {
    mockInvoke.mockResolvedValue('feat/add-login');
    const result = await llmGenerateBranchName('Add login page');
    expect(mockInvoke).toHaveBeenCalledWith('llm_generate_branch_name', {
      description: 'Add login page',
    });
    expect(result).toBe('feat/add-login');
  });

  it('llmSetApiBaseUrl invokes with URL', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await llmSetApiBaseUrl('https://example.com/v1/chat/completions');
    expect(mockInvoke).toHaveBeenCalledWith('llm_set_api_base_url', {
      apiBaseUrl: 'https://example.com/v1/chat/completions',
    });
  });

  it('llmGetApiBaseUrl invokes correct command', async () => {
    mockInvoke.mockResolvedValue('https://example.com/v1/chat/completions');
    const result = await llmGetApiBaseUrl();
    expect(mockInvoke).toHaveBeenCalledWith('llm_get_api_base_url');
    expect(result).toBe('https://example.com/v1/chat/completions');
  });
});
