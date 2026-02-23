// @vitest-environment jsdom

// Regression tests for Bug #199: SmolLM2 download Retry button and error quality.
// Retry button must call llmDownloadModel() (not just llmGetStatus()).
// Error messages must include the root cause from the full anyhow chain.

import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const mockListen = vi.fn().mockResolvedValue(() => {});
vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => mockListen(...args),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn().mockResolvedValue(null),
}));

// Mock localStorage
const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

import { SmolLM2Plugin } from './index';
import type { PluginContext, PluginEventType } from '../types';

function createMockContext(overrides: Partial<PluginContext> = {}): PluginContext {
  return {
    on: vi.fn((_type: PluginEventType, _handler: (e: any) => void) => {
      return () => {};
    }),
    readSoundFile: vi.fn().mockResolvedValue(''),
    listSoundPackFiles: vi.fn().mockResolvedValue([]),
    listSoundPacks: vi.fn().mockResolvedValue([]),
    getAudioContext: vi.fn().mockReturnValue({
      decodeAudioData: vi.fn().mockResolvedValue({ duration: 1 }),
    }),
    getSetting: vi.fn().mockImplementation((_key: string, defaultValue: any) => defaultValue),
    setSetting: vi.fn(),
    playSound: vi.fn(),
    ...overrides,
  };
}

describe('Bug #199 regression: SmolLM2 download retry and error quality', () => {
  let plugin: SmolLM2Plugin;

  beforeEach(() => {
    storage.clear();
    mockInvoke.mockReset();
    mockListen.mockReset();
    mockListen.mockResolvedValue(() => {});
    plugin = new SmolLM2Plugin();
  });

  describe('Download button after error state', () => {
    it('calls llm_download_model when clicking Download after error', async () => {
      // Bug #199: Download/retry must call llm_download_model, not just llm_get_status
      // In the new preset UI, error state shows Download button (files don't exist yet)
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'llm_get_status') {
          return Promise.resolve({ status: 'Error', detail: 'Download failed: Failed to download tokenizer: connection refused' });
        }
        if (cmd === 'llm_check_model_files') return Promise.resolve(false);
        if (cmd === 'llm_download_model') return Promise.resolve(undefined);
        return Promise.resolve(undefined);
      });

      const ctx = createMockContext();
      await plugin.init(ctx);

      const el = plugin.renderSettings!();
      // Wait for async preset buttons to render
      await new Promise(r => setTimeout(r, 50));

      const downloadBtn = Array.from(el.querySelectorAll('button'))
        .find(b => b.textContent?.includes('Download'));

      expect(downloadBtn).toBeDefined();

      mockInvoke.mockClear();
      // Re-set mocks for the click
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'llm_get_status') return Promise.resolve({ status: 'Downloaded' });
        if (cmd === 'llm_download_model') return Promise.resolve(undefined);
        if (cmd === 'llm_check_model_files') return Promise.resolve(true);
        return Promise.resolve(undefined);
      });

      await downloadBtn!.click();
      await new Promise(r => setTimeout(r, 50));

      const downloadCalls = mockInvoke.mock.calls.filter(
        (call: unknown[]) => call[0] === 'llm_download_model'
      );
      expect(downloadCalls.length).toBeGreaterThan(0);
    });

    it('updates status after successful download retry', async () => {
      let callCount = 0;
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'llm_get_status') {
          callCount++;
          if (callCount <= 1) {
            return Promise.resolve({ status: 'Error', detail: 'Download failed: Failed to download tokenizer: connection refused' });
          }
          return Promise.resolve({ status: 'Downloaded' });
        }
        if (cmd === 'llm_check_model_files') return Promise.resolve(false);
        if (cmd === 'llm_download_model') return Promise.resolve(undefined);
        return Promise.resolve(undefined);
      });

      const ctx = createMockContext();
      await plugin.init(ctx);

      const el = plugin.renderSettings!();
      await new Promise(r => setTimeout(r, 50));

      const statusValue = el.querySelector('.shortcut-keys') as HTMLElement;
      expect(statusValue?.textContent).toContain('Error');

      const downloadBtn = Array.from(el.querySelectorAll('button'))
        .find(b => b.textContent?.includes('Download'));

      // After click, check_model_files should return true
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'llm_get_status') return Promise.resolve({ status: 'Downloaded' });
        if (cmd === 'llm_check_model_files') return Promise.resolve(true);
        if (cmd === 'llm_download_model') return Promise.resolve(undefined);
        return Promise.resolve(undefined);
      });

      await downloadBtn!.click();
      await new Promise(r => setTimeout(r, 50));

      expect(statusValue?.textContent).not.toContain('Error');
    });
  });

  describe('Error message quality', () => {
    it('displays full error chain including root cause', async () => {
      // Bug #199 regression: backend now sends full chain via {:#} format
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'llm_get_status') {
          return Promise.resolve({
            status: 'Error',
            detail: 'Download failed: Failed to download tokenizer: connection refused: huggingface.co:443',
          });
        }
        return Promise.resolve(undefined);
      });

      const ctx = createMockContext();
      await plugin.init(ctx);

      const el = plugin.renderSettings!();
      const statusValue = el.querySelector('.shortcut-keys') as HTMLElement;
      const statusText = statusValue?.textContent || '';

      expect(statusText).toContain('connection refused');
    });
  });

  describe('Download button visible in error state', () => {
    it('shows a Download button in error state (files not yet on disk)', async () => {
      // Bug #199 regression: error state must have a button to retry download
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'llm_get_status') {
          return Promise.resolve({
            status: 'Error',
            detail: 'Download failed: Failed to download tokenizer: connection refused',
          });
        }
        if (cmd === 'llm_check_model_files') return Promise.resolve(false);
        return Promise.resolve(undefined);
      });

      const ctx = createMockContext();
      await plugin.init(ctx);

      const el = plugin.renderSettings!();
      await new Promise(r => setTimeout(r, 50));

      const buttons = Array.from(el.querySelectorAll('button'));
      const buttonTexts = buttons.map(b => b.textContent);

      const hasDownloadAction = buttonTexts.some(
        t => t?.includes('Download')
      );
      expect(hasDownloadAction).toBe(true);
    });
  });
});
