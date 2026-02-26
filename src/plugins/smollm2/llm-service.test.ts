// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import {
  llmHasApiKey,
  llmSetApiKey,
  llmGenerateBranchName,
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

  it('llmGenerateBranchName passes description', async () => {
    mockInvoke.mockResolvedValue('feat/add-login');
    const result = await llmGenerateBranchName('Add login page');
    expect(mockInvoke).toHaveBeenCalledWith('llm_generate_branch_name', {
      description: 'Add login page',
    });
    expect(result).toBe('feat/add-login');
  });
});
