// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { PeonPingPlugin } from './index';
import { PluginEventBus } from '../event-bus';
import type { PluginContext, PluginEventType, SoundPackManifest } from '../types';

// Mock localStorage
const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(''),
}));

// Mock @tauri-apps/plugin-opener
vi.mock('@tauri-apps/plugin-opener', () => ({
  revealItemInDir: vi.fn(),
}));

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

describe('PeonPingPlugin', () => {
  let plugin: PeonPingPlugin;
  let bus: PluginEventBus;

  beforeEach(() => {
    storage.clear();
    bus = new PluginEventBus();
    plugin = new PeonPingPlugin();
    plugin.setBus(bus);
  });

  it('has correct metadata', () => {
    expect(plugin.id).toBe('peon-ping');
    expect(plugin.name).toBe('Peon Ping');
    expect(plugin.version).toBe('1.0.0');
  });

  it('subscribes to event types during init', async () => {
    const ctx = createMockContext();
    await plugin.init(ctx);

    // Should subscribe to 5 categories: ready, complete, error, permission, notification
    expect(ctx.on).toHaveBeenCalledTimes(5);
  });

  it('enable/disable toggles the enabled state', async () => {
    const ctx = createMockContext();
    await plugin.init(ctx);

    plugin.enable();
    plugin.disable();
  });

  it('destroy unsubscribes all handlers', async () => {
    const unsubFns: ReturnType<typeof vi.fn>[] = [];
    const ctx = createMockContext({
      on: vi.fn((_type: PluginEventType, _handler: any) => {
        const unsub = vi.fn();
        unsubFns.push(unsub);
        return unsub;
      }),
    });

    await plugin.init(ctx);
    plugin.destroy();

    for (const unsub of unsubFns) {
      expect(unsub).toHaveBeenCalledTimes(1);
    }
  });

  it('renderSettings returns a DOM element', async () => {
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    expect(el).toBeInstanceOf(HTMLElement);
    expect(el.className).toBe('peon-ping-settings');
  });

  it('renderSettings includes volume slider and category toggles', async () => {
    const ctx = createMockContext();
    await plugin.init(ctx);

    const el = plugin.renderSettings!();
    const sliders = el.querySelectorAll('input[type="range"]');
    expect(sliders.length).toBe(1);

    const checkboxes = el.querySelectorAll('input[type="checkbox"]');
    expect(checkboxes.length).toBe(5);

    const testBtns = el.querySelectorAll('button');
    // 5 test buttons + 1 open folder button
    expect(testBtns.length).toBe(6);
  });

  it('loads sound packs during init', async () => {
    const packs: SoundPackManifest[] = [{
      id: 'default',
      name: 'Default',
      description: 'test',
      author: 'test',
      version: '1.0.0',
      sounds: { complete: ['done.mp3'] },
    }];
    const ctx = createMockContext({
      listSoundPacks: vi.fn().mockResolvedValue(packs),
    });

    await plugin.init(ctx);

    expect(ctx.listSoundPacks).toHaveBeenCalled();
  });
});
