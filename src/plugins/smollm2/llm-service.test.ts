// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import {
  llmGetStatus,
  llmDownloadModel,
  llmLoadModel,
  llmUnloadModel,
  llmGenerate,
  llmGenerateBranchName,
  isModelReady,
  isModelDownloaded,
  getStatusLabel,
  type LlmStatus,
} from './llm-service';

describe('llm-service', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('llmGetStatus invokes correct command', async () => {
    mockInvoke.mockResolvedValue({ status: 'Ready' });
    const result = await llmGetStatus();
    expect(mockInvoke).toHaveBeenCalledWith('llm_get_status');
    expect(result.status).toBe('Ready');
  });

  it('llmDownloadModel invokes correct command', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await llmDownloadModel();
    expect(mockInvoke).toHaveBeenCalledWith('llm_download_model');
  });

  it('llmLoadModel invokes correct command', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await llmLoadModel();
    expect(mockInvoke).toHaveBeenCalledWith('llm_load_model');
  });

  it('llmUnloadModel invokes correct command', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await llmUnloadModel();
    expect(mockInvoke).toHaveBeenCalledWith('llm_unload_model');
  });

  it('llmGenerate passes prompt and optional params', async () => {
    mockInvoke.mockResolvedValue('feat/add-login');
    const result = await llmGenerate('test prompt', 30, 0.3);
    expect(mockInvoke).toHaveBeenCalledWith('llm_generate', {
      prompt: 'test prompt',
      maxTokens: 30,
      temperature: 0.3,
    });
    expect(result).toBe('feat/add-login');
  });

  it('llmGenerate uses undefined for missing optional params', async () => {
    mockInvoke.mockResolvedValue('output');
    await llmGenerate('prompt');
    expect(mockInvoke).toHaveBeenCalledWith('llm_generate', {
      prompt: 'prompt',
      maxTokens: undefined,
      temperature: undefined,
    });
  });

  it('llmGenerateBranchName passes description', async () => {
    mockInvoke.mockResolvedValue('feat/add-login');
    const result = await llmGenerateBranchName('Add login page');
    expect(mockInvoke).toHaveBeenCalledWith('llm_generate_branch_name', {
      description: 'Add login page',
    });
    expect(result).toBe('feat/add-login');
  });

  describe('isModelReady', () => {
    it('returns true for Ready status', () => {
      expect(isModelReady({ status: 'Ready' })).toBe(true);
    });

    it('returns false for other statuses', () => {
      expect(isModelReady({ status: 'NotDownloaded' })).toBe(false);
      expect(isModelReady({ status: 'Downloaded' })).toBe(false);
      expect(isModelReady({ status: 'Loading' })).toBe(false);
    });
  });

  describe('isModelDownloaded', () => {
    it('returns true for Downloaded, Ready, Loading', () => {
      expect(isModelDownloaded({ status: 'Downloaded' })).toBe(true);
      expect(isModelDownloaded({ status: 'Ready' })).toBe(true);
      expect(isModelDownloaded({ status: 'Loading' })).toBe(true);
    });

    it('returns false for NotDownloaded', () => {
      expect(isModelDownloaded({ status: 'NotDownloaded' })).toBe(false);
    });
  });

  describe('getStatusLabel', () => {
    it('returns correct labels', () => {
      expect(getStatusLabel({ status: 'NotDownloaded' })).toBe('Not Downloaded');
      expect(getStatusLabel({ status: 'Ready' })).toBe('Ready');
      expect(getStatusLabel({ status: 'Error', detail: 'something broke' })).toContain('something broke');
    });
  });
});
