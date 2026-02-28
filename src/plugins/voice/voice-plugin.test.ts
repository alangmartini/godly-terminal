// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock all whisper-service functions
vi.mock('./whisper-service', () => ({
  whisperGetStatus: vi.fn().mockResolvedValue({
    state: 'idle',
    modelLoaded: false,
    modelName: null,
    gpuAvailable: false,
    gpuInUse: false,
    sidecarRunning: false,
  }),
  whisperGetConfig: vi.fn().mockResolvedValue({
    modelName: 'ggml-base.bin',
    language: '',
    useGpu: true,
    gpuDevice: 0,
    microphoneDeviceId: null,
  }),
  whisperSetConfig: vi.fn().mockResolvedValue(undefined),
  whisperLoadModel: vi.fn().mockResolvedValue(undefined),
  whisperListModels: vi.fn().mockResolvedValue([]),
  whisperDownloadModel: vi.fn().mockResolvedValue(undefined),
  whisperStartSidecar: vi.fn().mockResolvedValue('started'),
  whisperRestartSidecar: vi.fn().mockResolvedValue('restarted'),
  whisperStartRecording: vi.fn().mockResolvedValue(undefined),
  whisperStopRecording: vi.fn().mockResolvedValue('test text'),
}));

// Mock Tauri event listener
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import { VoiceToTextPlugin } from './index';

describe('VoiceToTextPlugin', () => {
  let plugin: VoiceToTextPlugin;

  beforeEach(() => {
    plugin = new VoiceToTextPlugin();
  });

  it('has correct metadata', () => {
    expect(plugin.id).toBe('voice-to-text');
    expect(plugin.name).toBe('Voice to Text');
    expect(plugin.version).toBe('1.0.0');
  });

  it('renderSettings creates container with sections', () => {
    const el = plugin.renderSettings();
    expect(el).toBeInstanceOf(HTMLElement);
    expect(el.className).toBe('voice-plugin-settings');
    // Should have status, model, GPU, language, test sections
    const sections = el.querySelectorAll('.settings-section');
    expect(sections.length).toBeGreaterThanOrEqual(3);
  });

  it('renderSettings creates model dropdown', () => {
    const el = plugin.renderSettings();
    const select = el.querySelector('select');
    expect(select).not.toBeNull();
    // Should have model presets as options
    expect(select!.options.length).toBeGreaterThan(0);
  });

  it('renderSettings creates GPU checkbox', () => {
    const el = plugin.renderSettings();
    const checkbox = el.querySelector('.voice-gpu-checkbox') as HTMLInputElement;
    expect(checkbox).not.toBeNull();
    expect(checkbox.type).toBe('checkbox');
    expect(checkbox.checked).toBe(true); // default
  });

  it('disable stops recording if active', async () => {
    const { whisperStopRecording } = await import('./whisper-service');
    // Simulate recording state
    await plugin.init({} as any);
    (plugin as any).status = { state: 'recording', modelLoaded: true, sidecarRunning: true };
    await plugin.disable();
    expect(whisperStopRecording).toHaveBeenCalled();
  });

  it('destroy cleans up progress listener', () => {
    const mockUnlisten = vi.fn();
    (plugin as any).progressUnlisten = mockUnlisten;
    plugin.destroy();
    expect(mockUnlisten).toHaveBeenCalled();
    expect((plugin as any).progressUnlisten).toBeNull();
  });
});
