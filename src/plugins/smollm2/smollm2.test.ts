// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
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

/** Wait for pending microtasks (async button rendering). */
function flushMicrotasks(): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, 0));
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
    expect(plugin.name).toBe('SmolLM2 Local LLM');
    expect(plugin.version).toBe('1.0.0');
  });

  it('init fetches model status', async () => {
    mockInvoke.mockResolvedValue({ status: 'NotDownloaded' });
    const ctx = createMockContext();
    await plugin.init(ctx);
    expect(mockInvoke).toHaveBeenCalledWith('llm_get_status');
  });

  it('init handles status fetch failure gracefully', async () => {
    mockInvoke.mockRejectedValue(new Error('not available'));
    const ctx = createMockContext();
    await plugin.init(ctx);
    // Should not throw
  });

  it('renderSettings returns a DOM element', async () => {
    mockInvoke.mockResolvedValue({ status: 'NotDownloaded' });
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    expect(el).toBeInstanceOf(HTMLElement);
    expect(el.className).toBe('smollm2-settings');
  });

  it('renderSettings includes status, engine, model source, auto-load, and test sections', async () => {
    mockInvoke.mockResolvedValue({ status: 'Ready' });
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    const rows = el.querySelectorAll('.shortcut-row');
    // Status + Engine + Source + Preset model + hint + preset action + Auto-load + Test row
    expect(rows.length).toBeGreaterThanOrEqual(4);
  });

  it('renderSettings has auto-load checkbox defaulting to true', async () => {
    mockInvoke.mockResolvedValue({ status: 'Downloaded' });
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    const checkboxes = el.querySelectorAll('input[type="checkbox"]');
    expect(checkboxes.length).toBe(1);
    expect((checkboxes[0] as HTMLInputElement).checked).toBe(true);
  });

  it('renderSettings shows download button when preset not downloaded', async () => {
    // llm_get_status returns NotDownloaded
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'llm_get_status') return Promise.resolve({ status: 'NotDownloaded' });
      if (cmd === 'llm_check_model_files') return Promise.resolve(false);
      return Promise.resolve(null);
    });
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    // Wait for async updatePresetButtons to populate
    await flushMicrotasks();

    const buttons = el.querySelectorAll('button');
    const downloadBtn = Array.from(buttons).find(b => b.textContent?.includes('Download'));
    expect(downloadBtn).toBeDefined();
  });

  it('renderSettings shows load button when preset downloaded but not loaded', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'llm_get_status') return Promise.resolve({ status: 'Downloaded' });
      if (cmd === 'llm_check_model_files') return Promise.resolve(true);
      return Promise.resolve(null);
    });
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    await flushMicrotasks();

    const buttons = el.querySelectorAll('button');
    const loadBtn = Array.from(buttons).find(b => b.textContent?.includes('Load'));
    expect(loadBtn).toBeDefined();
  });

  it('renderSettings shows unload button when ready', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'llm_get_status') return Promise.resolve({ status: 'Ready' });
      if (cmd === 'llm_check_model_files') return Promise.resolve(true);
      return Promise.resolve(null);
    });
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    await flushMicrotasks();

    const buttons = el.querySelectorAll('button');
    const unloadBtn = Array.from(buttons).find(b => b.textContent?.includes('Unload'));
    expect(unloadBtn).toBeDefined();
  });

  it('renderSettings has engine toggle defaulting to tiny', async () => {
    mockInvoke.mockResolvedValue({ status: 'NotDownloaded' });
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    const selects = el.querySelectorAll('select');
    const engineSelect = Array.from(selects).find(s =>
      Array.from(s.options).some(o => o.value === 'tiny'),
    );
    expect(engineSelect).toBeDefined();
    expect((engineSelect as HTMLSelectElement).value).toBe('tiny');
  });

  it('renderSettings has model source toggle defaulting to preset', async () => {
    mockInvoke.mockResolvedValue({ status: 'NotDownloaded' });
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    const selects = el.querySelectorAll('select');
    const sourceSelect = Array.from(selects).find(s =>
      Array.from(s.options).some(o => o.value === 'preset'),
    );
    expect(sourceSelect).toBeDefined();
    expect((sourceSelect as HTMLSelectElement).value).toBe('preset');
  });
});
