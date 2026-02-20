import { describe, it, expect, vi, beforeEach } from 'vitest';
import { PluginEventBus } from './event-bus';
import type { GodlyPlugin, PluginContext } from './types';

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
  invoke: vi.fn().mockResolvedValue([]),
}));

// Mock notification-sound exports
vi.mock('../services/notification-sound', () => ({
  getSharedAudioContext: vi.fn().mockReturnValue({}),
  playBuffer: vi.fn(),
}));

import { PluginRegistry } from './plugin-registry';

function createTestPlugin(overrides: Partial<GodlyPlugin> = {}): GodlyPlugin {
  return {
    id: 'test-plugin',
    name: 'Test Plugin',
    description: 'A test plugin',
    version: '1.0.0',
    init: vi.fn(),
    enable: vi.fn(),
    disable: vi.fn(),
    destroy: vi.fn(),
    ...overrides,
  };
}

describe('PluginRegistry', () => {
  let bus: PluginEventBus;
  let registry: PluginRegistry;

  beforeEach(() => {
    storage.clear();
    bus = new PluginEventBus();
    registry = new PluginRegistry(bus);
  });

  it('registers and retrieves plugins', () => {
    const plugin = createTestPlugin();
    registry.register(plugin);

    expect(registry.getAll()).toHaveLength(1);
    expect(registry.getPlugin('test-plugin')).toBe(plugin);
  });

  it('initializes all plugins with a context', async () => {
    const plugin = createTestPlugin();
    registry.register(plugin);

    await registry.initAll();

    expect(plugin.init).toHaveBeenCalledTimes(1);
    const ctx = (plugin.init as ReturnType<typeof vi.fn>).mock.calls[0][0] as PluginContext;
    expect(ctx).toBeDefined();
    expect(typeof ctx.on).toBe('function');
    expect(typeof ctx.getSetting).toBe('function');
    expect(typeof ctx.setSetting).toBe('function');
    expect(typeof ctx.playSound).toBe('function');
  });

  it('calls enable() on init when plugin is stored as enabled', async () => {
    storage.set('godly-plugin-settings', JSON.stringify({
      enabledPlugins: { 'test-plugin': true },
      pluginSettings: {},
    }));

    // Re-import to pick up localStorage state
    vi.resetModules();
    const { PluginEventBus: FreshBus } = await import('./event-bus');
    const { PluginRegistry: FreshRegistry } = await import('./plugin-registry');

    const freshBus = new FreshBus();
    const freshRegistry = new FreshRegistry(freshBus);
    const plugin = createTestPlugin();
    freshRegistry.register(plugin);

    await freshRegistry.initAll();

    expect(plugin.enable).toHaveBeenCalledTimes(1);
  });

  it('does not call enable() on init when plugin is disabled', async () => {
    const plugin = createTestPlugin();
    registry.register(plugin);

    await registry.initAll();

    expect(plugin.enable).not.toHaveBeenCalled();
  });

  it('setEnabled calls enable/disable on the plugin', () => {
    const plugin = createTestPlugin();
    registry.register(plugin);

    registry.setEnabled('test-plugin', true);
    expect(plugin.enable).toHaveBeenCalledTimes(1);

    registry.setEnabled('test-plugin', false);
    expect(plugin.disable).toHaveBeenCalledTimes(1);
  });

  it('destroyAll calls destroy on all plugins', () => {
    const p1 = createTestPlugin({ id: 'p1' });
    const p2 = createTestPlugin({ id: 'p2' });
    registry.register(p1);
    registry.register(p2);

    registry.destroyAll();

    expect(p1.destroy).toHaveBeenCalledTimes(1);
    expect(p2.destroy).toHaveBeenCalledTimes(1);
    expect(registry.getAll()).toHaveLength(0);
  });

  it('context.on subscribes to the event bus', async () => {
    const plugin = createTestPlugin({
      init: vi.fn((ctx: PluginContext) => {
        ctx.on('notification', () => {});
      }),
    });
    registry.register(plugin);
    await registry.initAll();

    expect(plugin.init).toHaveBeenCalledTimes(1);
  });

  it('handles init errors gracefully', async () => {
    const badPlugin = createTestPlugin({
      id: 'bad',
      init: vi.fn(() => { throw new Error('init failed'); }),
    });
    const goodPlugin = createTestPlugin({ id: 'good' });

    registry.register(badPlugin);
    registry.register(goodPlugin);

    await registry.initAll();

    expect(goodPlugin.init).toHaveBeenCalledTimes(1);
  });
});
