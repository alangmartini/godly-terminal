// @vitest-environment jsdom

// Bug #199: SmolLM2 download Retry button doesn't actually retry the download.
// It only calls llmGetStatus() which re-reads the Error state, never llmDownloadModel().

import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const mockListen = vi.fn().mockResolvedValue(() => {});
vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => mockListen(...args),
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

describe('Bug #199: SmolLM2 download retry and error quality', () => {
  let plugin: SmolLM2Plugin;

  beforeEach(() => {
    storage.clear();
    mockInvoke.mockReset();
    mockListen.mockReset();
    mockListen.mockResolvedValue(() => {});
    plugin = new SmolLM2Plugin();
  });

  describe('Retry button behavior', () => {
    it('should attempt to re-download when clicking Retry after error', async () => {
      // Setup: plugin is in Error state after a failed download
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'llm_get_status') {
          return Promise.resolve({ status: 'Error', detail: 'Download failed: Failed to download tokenizer' });
        }
        if (cmd === 'llm_download_model') {
          return Promise.resolve(undefined);
        }
        return Promise.resolve(undefined);
      });

      const ctx = createMockContext();
      await plugin.init(ctx);

      const el = plugin.renderSettings!();
      const retryBtn = Array.from(el.querySelectorAll('button'))
        .find(b => b.textContent === 'Retry');

      expect(retryBtn).toBeDefined();

      // Clear mock history before clicking
      mockInvoke.mockClear();

      // Simulate clicking Retry
      await retryBtn!.click();

      // Wait for any async operations
      await new Promise(r => setTimeout(r, 50));

      // Bug #199: Retry should call llm_download_model to actually retry the download
      // Currently it only calls llm_get_status which just re-reads the error
      const downloadCalls = mockInvoke.mock.calls.filter(
        (call: unknown[]) => call[0] === 'llm_download_model'
      );
      expect(downloadCalls.length).toBeGreaterThan(0);
    });

    it('should not stay in Error state after clicking Retry', async () => {
      // Setup: Error state
      let callCount = 0;
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'llm_get_status') {
          callCount++;
          // First call during init returns Error
          // After a successful retry, status should change
          if (callCount <= 1) {
            return Promise.resolve({ status: 'Error', detail: 'Download failed: Failed to download tokenizer' });
          }
          return Promise.resolve({ status: 'Downloaded' });
        }
        if (cmd === 'llm_download_model') {
          return Promise.resolve(undefined);
        }
        return Promise.resolve(undefined);
      });

      const ctx = createMockContext();
      await plugin.init(ctx);

      const el = plugin.renderSettings!();
      const statusValue = el.querySelector('.shortcut-keys') as HTMLElement;
      const initialStatus = statusValue?.textContent;

      // Status should show error initially
      expect(initialStatus).toContain('Error');

      const retryBtn = Array.from(el.querySelectorAll('button'))
        .find(b => b.textContent === 'Retry');

      // Click retry
      await retryBtn!.click();
      await new Promise(r => setTimeout(r, 50));

      // Bug #199: After retry succeeds, status should no longer show error
      // Currently Retry just re-reads the Error status, so it stays as Error
      const currentStatus = statusValue?.textContent;
      expect(currentStatus).not.toContain('Error');
    });
  });

  describe('Error message quality', () => {
    it('should show actionable error details, not just "Failed to download tokenizer"', async () => {
      // Setup: download fails with a specific error chain from Rust
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'llm_get_status') {
          return Promise.resolve({
            status: 'Error',
            // Bug #199: This is what Rust sends — it only includes the context, not root cause
            detail: 'Download failed: Failed to download tokenizer',
          });
        }
        return Promise.resolve(undefined);
      });

      const ctx = createMockContext();
      await plugin.init(ctx);

      const el = plugin.renderSettings!();
      const statusValue = el.querySelector('.shortcut-keys') as HTMLElement;
      const statusText = statusValue?.textContent || '';

      // The error message should contain actionable information beyond boilerplate
      // "Error: Download failed: Failed to download tokenizer" tells the user nothing useful
      // It should include the WHY: HTTP status, network error, timeout, etc.
      const boilerplateStripped = statusText
        .replace('Error:', '')
        .replace('Download failed:', '')
        .replace('Failed to download tokenizer', '')
        .trim();

      // Bug #199: After stripping boilerplate, there should be actual error info
      expect(boilerplateStripped.length).toBeGreaterThan(0);
    });

    it('should display the full error chain from backend', async () => {
      // If the backend properly sends the full chain, the frontend should display it
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'llm_get_status') {
          return Promise.resolve({
            status: 'Error',
            // This is what a properly formatted error chain would look like
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

      // The UI should show the root cause
      expect(statusText).toContain('connection refused');
    });
  });

  describe('Download button after error', () => {
    it('should show Download button (not just Retry) to allow re-download', async () => {
      // Bug #199: After an error, there should be a clear way to start a fresh download
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'llm_get_status') {
          return Promise.resolve({
            status: 'Error',
            detail: 'Download failed: Failed to download tokenizer',
          });
        }
        return Promise.resolve(undefined);
      });

      const ctx = createMockContext();
      await plugin.init(ctx);

      const el = plugin.renderSettings!();
      const buttons = Array.from(el.querySelectorAll('button'));
      const buttonTexts = buttons.map(b => b.textContent);

      // There should be a button that clearly indicates it will re-attempt the download
      // Either "Download" or "Retry Download" — not just "Retry" that only checks status
      const hasDownloadAction = buttonTexts.some(
        t => t?.includes('Download') || t?.includes('Retry Download')
      );

      expect(hasDownloadAction).toBe(true);
    });
  });
});
