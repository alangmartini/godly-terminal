import { describe, it, expect, beforeEach, vi } from 'vitest';

const localStorageMock = (() => {
  let store: Record<string, string> = {};
  return {
    getItem: vi.fn((key: string) => store[key] ?? null),
    setItem: vi.fn((key: string, value: string) => { store[key] = value; }),
    removeItem: vi.fn((key: string) => { delete store[key]; }),
    clear: vi.fn(() => { store = {}; }),
  };
})();
Object.defineProperty(globalThis, 'localStorage', { value: localStorageMock });

describe('SettingsTabStore', () => {
  beforeEach(() => {
    localStorageMock.clear();
    vi.resetModules();
  });

  async function createStore() {
    const mod = await import('./settings-tab-store');
    return mod.settingsTabStore;
  }

  // Bug: plugins tab was missing from DEFAULT_ORDER, so it never appeared
  it('includes plugins tab in default order', async () => {
    const store = await createStore();
    expect(store.getTabOrder()).toContain('plugins');
  });

  it('default order has all expected tabs', async () => {
    const store = await createStore();
    const order = store.getTabOrder();
    expect(order).toContain('themes');
    expect(order).toContain('terminal');
    expect(order).toContain('notifications');
    expect(order).toContain('plugins');
    expect(order).toContain('shortcuts');
  });

  // Bug regression: users who saved tab order before plugins existed
  // should get plugins reconciled into their order
  it('reconciles plugins into a stale saved order missing it', async () => {
    localStorageMock.setItem(
      'godly-settings-tab-order',
      JSON.stringify(['themes', 'terminal', 'notifications', 'shortcuts']),
    );
    const store = await createStore();
    expect(store.getTabOrder()).toContain('plugins');
  });

  it('preserves user custom tab order during reconciliation', async () => {
    localStorageMock.setItem(
      'godly-settings-tab-order',
      JSON.stringify(['shortcuts', 'themes', 'notifications', 'terminal']),
    );
    const store = await createStore();
    const order = store.getTabOrder();
    // User's original tabs should keep their relative order
    expect(order.indexOf('shortcuts')).toBeLessThan(order.indexOf('themes'));
    expect(order.indexOf('themes')).toBeLessThan(order.indexOf('notifications'));
    // plugins should be appended since it was missing
    expect(order).toContain('plugins');
  });

  it('setTabOrder persists and returns updated order', async () => {
    const store = await createStore();
    store.setTabOrder(['plugins', 'shortcuts', 'themes', 'terminal', 'notifications']);
    const order = store.getTabOrder();
    expect(order[0]).toBe('plugins');
    expect(order[1]).toBe('shortcuts');
  });

  it('strips unknown tab IDs from saved order', async () => {
    localStorageMock.setItem(
      'godly-settings-tab-order',
      JSON.stringify(['themes', 'bogus', 'terminal', 'notifications', 'plugins', 'shortcuts']),
    );
    const store = await createStore();
    expect(store.getTabOrder()).not.toContain('bogus');
  });
});
