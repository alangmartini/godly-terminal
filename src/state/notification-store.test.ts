import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock localStorage
const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

// Mock notification-sound before import
vi.mock('../services/notification-sound', () => ({
  isBuiltinPreset: (s: string) => ['chime', 'bell'].includes(s),
  isCustomPreset: (s: string) => s.startsWith('custom:'),
}));

// We need a fresh store per test, so we re-import after resetting modules
let notificationStore: typeof import('./notification-store')['notificationStore'];

describe('NotificationStore — workspace muting', () => {
  beforeEach(async () => {
    storage.clear();
    vi.resetModules();

    // Re-stub localStorage after resetModules
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
      removeItem: (key: string) => storage.delete(key),
      clear: () => storage.clear(),
    });

    const mod = await import('./notification-store');
    notificationStore = mod.notificationStore;
  });

  describe('getMutedPatterns / addMutedPattern / removeMutedPattern', () => {
    it('starts with empty patterns', () => {
      expect(notificationStore.getMutedPatterns()).toEqual([]);
    });

    it('adds and retrieves patterns', () => {
      notificationStore.addMutedPattern('Agent *');
      notificationStore.addMutedPattern('*-orchestrator');
      expect(notificationStore.getMutedPatterns()).toEqual(['Agent *', '*-orchestrator']);
    });

    it('ignores duplicate patterns', () => {
      notificationStore.addMutedPattern('Agent *');
      notificationStore.addMutedPattern('Agent *');
      expect(notificationStore.getMutedPatterns()).toEqual(['Agent *']);
    });

    it('ignores empty/whitespace patterns', () => {
      notificationStore.addMutedPattern('');
      notificationStore.addMutedPattern('   ');
      expect(notificationStore.getMutedPatterns()).toEqual([]);
    });

    it('trims whitespace from patterns', () => {
      notificationStore.addMutedPattern('  Agent *  ');
      expect(notificationStore.getMutedPatterns()).toEqual(['Agent *']);
    });

    it('removes patterns', () => {
      notificationStore.addMutedPattern('Agent *');
      notificationStore.addMutedPattern('*-orchestrator');
      notificationStore.removeMutedPattern('Agent *');
      expect(notificationStore.getMutedPatterns()).toEqual(['*-orchestrator']);
    });

    it('removing non-existent pattern is a no-op', () => {
      notificationStore.addMutedPattern('Agent *');
      notificationStore.removeMutedPattern('nonexistent');
      expect(notificationStore.getMutedPatterns()).toEqual(['Agent *']);
    });
  });

  describe('workspace overrides', () => {
    it('returns undefined for unknown workspace', () => {
      expect(notificationStore.getWorkspaceOverride('ws-1')).toBeUndefined();
    });

    it('sets and retrieves overrides', () => {
      notificationStore.setWorkspaceOverride('ws-1', false);
      expect(notificationStore.getWorkspaceOverride('ws-1')).toBe(false);

      notificationStore.setWorkspaceOverride('ws-1', true);
      expect(notificationStore.getWorkspaceOverride('ws-1')).toBe(true);
    });

    it('clears overrides', () => {
      notificationStore.setWorkspaceOverride('ws-1', false);
      notificationStore.clearWorkspaceOverride('ws-1');
      expect(notificationStore.getWorkspaceOverride('ws-1')).toBeUndefined();
    });

    it('cleanup removes override', () => {
      notificationStore.setWorkspaceOverride('ws-1', false);
      notificationStore.cleanupWorkspaceOverride('ws-1');
      expect(notificationStore.getWorkspaceOverride('ws-1')).toBeUndefined();
    });
  });

  describe('isWorkspaceNotificationEnabled', () => {
    it('returns true by default (no patterns, no overrides)', () => {
      expect(notificationStore.isWorkspaceNotificationEnabled('ws-1', 'Default')).toBe(true);
    });

    it('returns false when name matches a muted pattern', () => {
      notificationStore.addMutedPattern('Agent *');
      expect(notificationStore.isWorkspaceNotificationEnabled('ws-1', 'Agent Foo')).toBe(false);
    });

    it('returns true when name does not match any pattern', () => {
      notificationStore.addMutedPattern('Agent *');
      expect(notificationStore.isWorkspaceNotificationEnabled('ws-1', 'Default')).toBe(true);
    });

    it('manual override takes priority over glob pattern', () => {
      notificationStore.addMutedPattern('Agent *');
      notificationStore.setWorkspaceOverride('ws-1', true);
      expect(notificationStore.isWorkspaceNotificationEnabled('ws-1', 'Agent Foo')).toBe(true);
    });

    it('manual override can mute a workspace not matching any pattern', () => {
      notificationStore.setWorkspaceOverride('ws-1', false);
      expect(notificationStore.isWorkspaceNotificationEnabled('ws-1', 'Default')).toBe(false);
    });

    it('clearing override falls back to pattern check', () => {
      notificationStore.addMutedPattern('Agent *');
      notificationStore.setWorkspaceOverride('ws-1', true);
      notificationStore.clearWorkspaceOverride('ws-1');
      expect(notificationStore.isWorkspaceNotificationEnabled('ws-1', 'Agent Foo')).toBe(false);
    });
  });

  describe('persistence', () => {
    it('persists workspace mute settings to localStorage', async () => {
      notificationStore.addMutedPattern('Agent *');
      notificationStore.setWorkspaceOverride('ws-1', false);

      // Re-create store from localStorage
      vi.resetModules();
      vi.stubGlobal('localStorage', {
        getItem: (key: string) => storage.get(key) ?? null,
        setItem: (key: string, value: string) => storage.set(key, value),
        removeItem: (key: string) => storage.delete(key),
        clear: () => storage.clear(),
      });

      const mod2 = await import('./notification-store');
      const store2 = mod2.notificationStore;

      expect(store2.getMutedPatterns()).toEqual(['Agent *']);
      expect(store2.getWorkspaceOverride('ws-1')).toBe(false);
    });
  });

  describe('subscriber notification', () => {
    it('notifies subscribers on pattern add', () => {
      const fn = vi.fn();
      notificationStore.subscribe(fn);
      notificationStore.addMutedPattern('Agent *');
      expect(fn).toHaveBeenCalledTimes(1);
    });

    it('notifies subscribers on workspace override change', () => {
      const fn = vi.fn();
      notificationStore.subscribe(fn);
      notificationStore.setWorkspaceOverride('ws-1', false);
      expect(fn).toHaveBeenCalledTimes(1);
    });
  });
});
