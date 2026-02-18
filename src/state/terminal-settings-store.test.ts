import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock localStorage
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

describe('TerminalSettingsStore', () => {
  beforeEach(() => {
    localStorageMock.clear();
    vi.resetModules();
  });

  async function createStore() {
    const mod = await import('./terminal-settings-store');
    return mod.terminalSettingsStore;
  }

  it('defaults to windows shell', async () => {
    const store = await createStore();
    expect(store.getDefaultShell()).toEqual({ type: 'windows' });
  });

  it('persists shell selection to localStorage', async () => {
    const store = await createStore();
    store.setDefaultShell({ type: 'cmd' });

    expect(localStorageMock.setItem).toHaveBeenCalledWith(
      'godly-terminal-settings',
      expect.stringContaining('"cmd"'),
    );
  });

  it('loads persisted shell from localStorage', async () => {
    localStorageMock.setItem(
      'godly-terminal-settings',
      JSON.stringify({ defaultShell: { type: 'pwsh' } }),
    );

    const store = await createStore();
    expect(store.getDefaultShell()).toEqual({ type: 'pwsh' });
  });

  it('loads persisted WSL shell with distribution', async () => {
    localStorageMock.setItem(
      'godly-terminal-settings',
      JSON.stringify({ defaultShell: { type: 'wsl', distribution: 'Ubuntu' } }),
    );

    const store = await createStore();
    expect(store.getDefaultShell()).toEqual({ type: 'wsl', distribution: 'Ubuntu' });
  });

  it('falls back to windows on corrupt data', async () => {
    localStorageMock.setItem('godly-terminal-settings', '{invalid json');

    const store = await createStore();
    expect(store.getDefaultShell()).toEqual({ type: 'windows' });
  });

  it('notifies subscribers on change', async () => {
    const store = await createStore();
    const listener = vi.fn();
    store.subscribe(listener);

    store.setDefaultShell({ type: 'cmd' });
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it('unsubscribe stops notifications', async () => {
    const store = await createStore();
    const listener = vi.fn();
    const unsub = store.subscribe(listener);

    unsub();
    store.setDefaultShell({ type: 'cmd' });
    expect(listener).not.toHaveBeenCalled();
  });

  it('defaults autoScrollOnOutput to false', async () => {
    const store = await createStore();
    expect(store.getAutoScrollOnOutput()).toBe(false);
  });

  it('persists autoScrollOnOutput setting', async () => {
    const store = await createStore();
    store.setAutoScrollOnOutput(true);

    expect(localStorageMock.setItem).toHaveBeenCalledWith(
      'godly-terminal-settings',
      expect.stringContaining('"autoScrollOnOutput":true'),
    );
  });

  it('loads persisted autoScrollOnOutput from localStorage', async () => {
    localStorageMock.setItem(
      'godly-terminal-settings',
      JSON.stringify({ defaultShell: { type: 'windows' }, autoScrollOnOutput: true }),
    );

    const store = await createStore();
    expect(store.getAutoScrollOnOutput()).toBe(true);
  });

  it('notifies subscribers when autoScrollOnOutput changes', async () => {
    const store = await createStore();
    const listener = vi.fn();
    store.subscribe(listener);

    store.setAutoScrollOnOutput(true);
    expect(listener).toHaveBeenCalledTimes(1);
  });
});
