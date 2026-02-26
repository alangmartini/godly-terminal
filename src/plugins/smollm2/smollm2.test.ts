// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
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

describe('SmolLM2Plugin', () => {
  let plugin: SmolLM2Plugin;

  beforeEach(() => {
    storage.clear();
    mockInvoke.mockReset();
    plugin = new SmolLM2Plugin();
  });

  it('has correct metadata', () => {
    expect(plugin.id).toBe('smollm2');
    expect(plugin.name).toBe('Branch Name AI');
    expect(plugin.version).toBe('2.0.0');
  });

  it('init checks for API key', async () => {
    mockInvoke.mockResolvedValue(false);
    const ctx = createMockContext();
    await plugin.init(ctx);
    expect(mockInvoke).toHaveBeenCalledWith('llm_has_api_key');
  });

  it('init handles API key check failure gracefully', async () => {
    mockInvoke.mockRejectedValue(new Error('not available'));
    const ctx = createMockContext();
    await plugin.init(ctx);
    // Should not throw
  });

  it('renderSettings returns a DOM element', async () => {
    mockInvoke.mockResolvedValue(false);
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    expect(el).toBeInstanceOf(HTMLElement);
    expect(el.className).toBe('smollm2-settings');
  });

  it('renderSettings has API key input', async () => {
    mockInvoke.mockResolvedValue(false);
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    const passwordInput = el.querySelector('input[type="password"]');
    expect(passwordInput).toBeDefined();
  });

  it('renderSettings has save button', async () => {
    mockInvoke.mockResolvedValue(false);
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    const saveBtn = Array.from(el.querySelectorAll('button'))
      .find(b => b.textContent === 'Save');
    expect(saveBtn).toBeDefined();
  });

  it('renderSettings has test generation button', async () => {
    mockInvoke.mockResolvedValue(true);
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    const testBtn = Array.from(el.querySelectorAll('button'))
      .find(b => b.textContent === 'Generate Branch Name');
    expect(testBtn).toBeDefined();
  });

  it('renderSettings shows Active status when API key is set', async () => {
    mockInvoke.mockResolvedValue(true);
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    expect(el.textContent).toContain('Active');
  });

  it('renderSettings shows Not set status when no API key', async () => {
    mockInvoke.mockResolvedValue(false);
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    expect(el.textContent).toContain('Not set');
  });
});
