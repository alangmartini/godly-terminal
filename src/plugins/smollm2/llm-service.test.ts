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
  llmCheckModelFiles,
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

  it('llmDownloadModel invokes correct command with default params', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await llmDownloadModel();
    expect(mockInvoke).toHaveBeenCalledWith('llm_download_model', {
      hfRepo: null,
      hfFilename: null,
      tokenizerRepo: null,
      subdir: null,
    });
  });

  it('llmDownloadModel passes custom params', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await llmDownloadModel('repo/name', 'model.gguf', 'tok/repo', 'mymodel');
    expect(mockInvoke).toHaveBeenCalledWith('llm_download_model', {
      hfRepo: 'repo/name',
      hfFilename: 'model.gguf',
      tokenizerRepo: 'tok/repo',
      subdir: 'mymodel',
    });
  });

  it('llmLoadModel invokes correct command with default params', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await llmLoadModel();
    expect(mockInvoke).toHaveBeenCalledWith('llm_load_model', {
      ggufPath: null,
      tokenizerPath: null,
      subdir: null,
      ggufFilename: null,
    });
  });

  it('llmLoadModel passes custom paths', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await llmLoadModel({ ggufPath: '/path/model.gguf', tokenizerPath: '/path/tok.json' });
    expect(mockInvoke).toHaveBeenCalledWith('llm_load_model', {
      ggufPath: '/path/model.gguf',
      tokenizerPath: '/path/tok.json',
      subdir: null,
      ggufFilename: null,
    });
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

  it('llmGenerateBranchName passes description with default useTiny', async () => {
    mockInvoke.mockResolvedValue('feat/add-login');
    const result = await llmGenerateBranchName('Add login page');
    expect(mockInvoke).toHaveBeenCalledWith('llm_generate_branch_name', {
      description: 'Add login page',
      useTiny: null,
    });
    expect(result).toBe('feat/add-login');
  });

  it('llmGenerateBranchName passes useTiny flag', async () => {
    mockInvoke.mockResolvedValue('feat/add-login');
    await llmGenerateBranchName('Add login page', true);
    expect(mockInvoke).toHaveBeenCalledWith('llm_generate_branch_name', {
      description: 'Add login page',
      useTiny: true,
    });
  });

  it('llmCheckModelFiles invokes with subdir and filename', async () => {
    mockInvoke.mockResolvedValue(true);
    const result = await llmCheckModelFiles({ subdir: 'smollm2-135m', ggufFilename: 'model.gguf' });
    expect(mockInvoke).toHaveBeenCalledWith('llm_check_model_files', {
      subdir: 'smollm2-135m',
      ggufFilename: 'model.gguf',
      ggufPath: null,
      tokenizerPath: null,
    });
    expect(result).toBe(true);
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
