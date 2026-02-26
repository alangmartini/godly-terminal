// @vitest-environment jsdom

// Tests for SmolLM2Plugin (Branch Name AI) settings UI.

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
  const settings = new Map<string, unknown>();
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
    getSetting: vi.fn().mockImplementation((key: string, defaultValue: any) => {
      return settings.has(key) ? settings.get(key) : defaultValue;
    }),
    setSetting: vi.fn().mockImplementation((key: string, value: any) => {
      settings.set(key, value);
    }),
    playSound: vi.fn(),
    ...overrides,
  };
}

describe('SmolLM2Plugin settings', () => {
  let plugin: SmolLM2Plugin;

  beforeEach(() => {
    storage.clear();
    mockInvoke.mockReset();
    plugin = new SmolLM2Plugin();
  });

  it('renders API key input and save button', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'llm_has_api_key') return Promise.resolve(false);
      return Promise.resolve(undefined);
    });

    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();

    const inputs = el.querySelectorAll('input[type="password"]');
    expect(inputs.length).toBe(1);

    const saveBtn = Array.from(el.querySelectorAll('button'))
      .find(b => b.textContent === 'Save');
    expect(saveBtn).toBeDefined();
  });

  it('saves API key via llm_set_api_key', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'llm_has_api_key') return Promise.resolve(false);
      if (cmd === 'llm_set_api_key') return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });

    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    const keyInput = el.querySelector('input[type="password"]') as HTMLInputElement;
    const saveBtn = Array.from(el.querySelectorAll('button'))
      .find(b => b.textContent === 'Save')!;

    keyInput.value = 'AIza-test-key-123';
    await saveBtn.click();
    await new Promise(r => setTimeout(r, 50));

    const setCalls = mockInvoke.mock.calls.filter(
      (call: unknown[]) => call[0] === 'llm_set_api_key',
    );
    expect(setCalls.length).toBe(1);
    expect(setCalls[0][1]).toEqual({ key: 'AIza-test-key-123' });
    expect(ctx.setSetting).toHaveBeenCalledWith('geminiApiKey', 'AIza-test-key-123');
  });

  it('shows test generation section', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'llm_has_api_key') return Promise.resolve(true);
      return Promise.resolve(undefined);
    });

    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    const testBtn = Array.from(el.querySelectorAll('button'))
      .find(b => b.textContent === 'Generate Branch Name');
    expect(testBtn).toBeDefined();
  });

  it('restores API key on enable', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'llm_has_api_key') return Promise.resolve(false);
      if (cmd === 'llm_set_api_key') return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });

    const ctx = createMockContext();
    (ctx.getSetting as ReturnType<typeof vi.fn>).mockImplementation(
      (key: string, def: any) => key === 'geminiApiKey' ? 'saved-key' : def,
    );

    await plugin.init(ctx);
    await plugin.enable();

    const setCalls = mockInvoke.mock.calls.filter(
      (call: unknown[]) => call[0] === 'llm_set_api_key',
    );
    expect(setCalls.length).toBe(1);
    expect(setCalls[0][1]).toEqual({ key: 'saved-key' });
  });
});
