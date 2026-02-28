import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock Tauri invoke
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import {
  whisperGetStatus,
  whisperStartRecording,
  whisperStopRecording,
  whisperLoadModel,
  whisperListModels,
  whisperStartSidecar,
  whisperRestartSidecar,
  whisperDownloadModel,
  whisperGetConfig,
  whisperSetConfig,
} from './whisper-service';

describe('whisper-service', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('whisperGetStatus invokes correct command', async () => {
    mockInvoke.mockResolvedValue({ state: 'idle', modelLoaded: false });
    const result = await whisperGetStatus();
    expect(mockInvoke).toHaveBeenCalledWith('whisper_get_status');
    expect(result.state).toBe('idle');
  });

  it('whisperStartRecording invokes correct command', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await whisperStartRecording();
    expect(mockInvoke).toHaveBeenCalledWith('whisper_start_recording');
  });

  it('whisperStopRecording invokes correct command and returns TranscriptionResult', async () => {
    mockInvoke.mockResolvedValue({ text: 'hello world', durationMs: 1500 });
    const result = await whisperStopRecording();
    expect(mockInvoke).toHaveBeenCalledWith('whisper_stop_recording');
    expect(result).toEqual({ text: 'hello world', durationMs: 1500 });
    expect(result.text).toBe('hello world');
    expect(result.durationMs).toBe(1500);
  });

  it('whisperLoadModel invokes with correct args', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await whisperLoadModel('ggml-base.bin', true, 0, 'en');
    expect(mockInvoke).toHaveBeenCalledWith('whisper_load_model', {
      modelName: 'ggml-base.bin',
      useGpu: true,
      gpuDevice: 0,
      language: 'en',
    });
  });

  it('whisperListModels invokes correct command', async () => {
    mockInvoke.mockResolvedValue(['ggml-base.bin', 'ggml-small.bin']);
    const result = await whisperListModels();
    expect(mockInvoke).toHaveBeenCalledWith('whisper_list_models');
    expect(result).toEqual(['ggml-base.bin', 'ggml-small.bin']);
  });

  it('whisperStartSidecar invokes correct command', async () => {
    mockInvoke.mockResolvedValue('Sidecar started (PID 1234)');
    const result = await whisperStartSidecar();
    expect(mockInvoke).toHaveBeenCalledWith('whisper_start_sidecar');
    expect(result).toContain('PID');
  });

  it('whisperRestartSidecar invokes correct command', async () => {
    mockInvoke.mockResolvedValue('Sidecar started (PID 5678)');
    await whisperRestartSidecar();
    expect(mockInvoke).toHaveBeenCalledWith('whisper_restart_sidecar');
  });

  it('whisperDownloadModel invokes with model name', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await whisperDownloadModel('ggml-large.bin');
    expect(mockInvoke).toHaveBeenCalledWith('whisper_download_model', { modelName: 'ggml-large.bin' });
  });

  it('whisperGetConfig invokes correct command', async () => {
    mockInvoke.mockResolvedValue({ modelName: 'ggml-base.bin', language: '', useGpu: true, gpuDevice: 0 });
    const result = await whisperGetConfig();
    expect(mockInvoke).toHaveBeenCalledWith('whisper_get_config');
    expect(result.modelName).toBe('ggml-base.bin');
  });

  it('whisperSetConfig invokes with config object', async () => {
    mockInvoke.mockResolvedValue(undefined);
    const config = { modelName: 'ggml-base.bin', language: 'en', useGpu: false, gpuDevice: 0, microphoneDeviceId: null };
    await whisperSetConfig(config);
    expect(mockInvoke).toHaveBeenCalledWith('whisper_set_config', { config });
  });
});
