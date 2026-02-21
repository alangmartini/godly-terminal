import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock localStorage
const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

// Mock @tauri-apps/api/core
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import { pluginInstaller } from './plugin-installer';
import { pluginStore } from './plugin-store';

describe('PluginInstaller', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    storage.clear();
  });

  describe('installPlugin', () => {
    it('invokes install_plugin and stores metadata', async () => {
      const manifest = {
        id: 'cool-plugin',
        name: 'Cool Plugin',
        description: 'Does cool things',
        author: 'tester',
        version: '2.0.0',
      };
      mockInvoke.mockResolvedValueOnce(manifest);

      const result = await pluginInstaller.installPlugin('owner', 'repo');

      expect(mockInvoke).toHaveBeenCalledWith('install_plugin', { owner: 'owner', repo: 'repo' });
      expect(result).toEqual(manifest);

      const installed = pluginStore.getInstalledPlugin('cool-plugin');
      expect(installed).toBeDefined();
      expect(installed!.version).toBe('2.0.0');
      expect(installed!.repo).toBe('owner/repo');
    });

    it('propagates errors from invoke', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('Network error'));

      await expect(pluginInstaller.installPlugin('owner', 'repo')).rejects.toThrow('Network error');
    });
  });

  describe('uninstallPlugin', () => {
    it('invokes uninstall_plugin and removes from store', async () => {
      // Pre-install
      pluginStore.setInstalled('cool-plugin', {
        id: 'cool-plugin',
        version: '1.0.0',
        repo: 'owner/repo',
        installedAt: Date.now(),
      });
      pluginStore.setEnabled('cool-plugin', true);
      mockInvoke.mockResolvedValueOnce(undefined);

      await pluginInstaller.uninstallPlugin('cool-plugin');

      expect(mockInvoke).toHaveBeenCalledWith('uninstall_plugin', { pluginId: 'cool-plugin' });
      expect(pluginStore.getInstalledPlugin('cool-plugin')).toBeUndefined();
      expect(pluginStore.isEnabled('cool-plugin')).toBe(false);
    });
  });

  describe('checkForUpdate', () => {
    it('returns new version when available', async () => {
      mockInvoke.mockResolvedValueOnce('2.0.0');

      const result = await pluginInstaller.checkForUpdate('owner', 'repo', '1.0.0');

      expect(mockInvoke).toHaveBeenCalledWith('check_plugin_update', {
        owner: 'owner',
        repo: 'repo',
        installedVersion: '1.0.0',
      });
      expect(result).toBe('2.0.0');
    });

    it('returns null when up to date', async () => {
      mockInvoke.mockResolvedValueOnce(null);

      const result = await pluginInstaller.checkForUpdate('owner', 'repo', '1.0.0');

      expect(result).toBeNull();
    });
  });
});
