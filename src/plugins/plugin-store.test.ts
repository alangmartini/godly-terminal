import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock localStorage
const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

// Reset module between tests so the singleton reinitializes
let pluginStore: typeof import('./plugin-store').pluginStore;

describe('PluginStore', () => {
  beforeEach(async () => {
    storage.clear();
    vi.resetModules();
    const mod = await import('./plugin-store');
    pluginStore = mod.pluginStore;
  });

  it('returns false for unknown plugin IDs', () => {
    expect(pluginStore.isEnabled('nonexistent')).toBe(false);
  });

  it('stores and retrieves enabled state', () => {
    pluginStore.setEnabled('test-plugin', true);
    expect(pluginStore.isEnabled('test-plugin')).toBe(true);

    pluginStore.setEnabled('test-plugin', false);
    expect(pluginStore.isEnabled('test-plugin')).toBe(false);
  });

  it('stores and retrieves plugin settings with defaults', () => {
    expect(pluginStore.getSetting('p1', 'volume', 0.5)).toBe(0.5);

    pluginStore.setSetting('p1', 'volume', 0.8);
    expect(pluginStore.getSetting('p1', 'volume', 0.5)).toBe(0.8);
  });

  it('notifies subscribers on state changes', () => {
    const fn = vi.fn();
    pluginStore.subscribe(fn);

    pluginStore.setEnabled('p1', true);
    expect(fn).toHaveBeenCalledTimes(1);

    pluginStore.setSetting('p1', 'key', 'value');
    expect(fn).toHaveBeenCalledTimes(2);
  });

  it('unsubscribes correctly', () => {
    const fn = vi.fn();
    const unsub = pluginStore.subscribe(fn);

    unsub();
    pluginStore.setEnabled('p1', true);

    expect(fn).not.toHaveBeenCalled();
  });

  it('persists to localStorage', () => {
    pluginStore.setEnabled('p1', true);
    pluginStore.setSetting('p1', 'color', 'red');

    const raw = storage.get('godly-plugin-settings');
    expect(raw).toBeTruthy();
    const data = JSON.parse(raw!);
    expect(data.enabledPlugins.p1).toBe(true);
    expect(data.pluginSettings.p1.color).toBe('red');
  });

  it('loads from localStorage on construction', async () => {
    storage.set('godly-plugin-settings', JSON.stringify({
      enabledPlugins: { 'saved-plugin': true },
      pluginSettings: { 'saved-plugin': { vol: 0.3 } },
    }));

    vi.resetModules();
    const mod = await import('./plugin-store');
    const freshStore = mod.pluginStore;

    expect(freshStore.isEnabled('saved-plugin')).toBe(true);
    expect(freshStore.getSetting('saved-plugin', 'vol', 1.0)).toBe(0.3);
  });
});
