import { invoke } from '@tauri-apps/api/core';
import type { ExternalPluginManifest } from './types';
import { pluginStore } from './plugin-store';

export class PluginInstaller {
  async installPlugin(owner: string, repo: string): Promise<ExternalPluginManifest> {
    const manifest = await invoke<ExternalPluginManifest>('install_plugin', { owner, repo });

    pluginStore.setInstalled(manifest.id, {
      id: manifest.id,
      version: manifest.version,
      repo: `${owner}/${repo}`,
      installedAt: Date.now(),
    });

    return manifest;
  }

  async uninstallPlugin(pluginId: string): Promise<void> {
    await invoke<void>('uninstall_plugin', { pluginId });
    pluginStore.removeInstalled(pluginId);
    pluginStore.setEnabled(pluginId, false);
  }

  async checkForUpdate(owner: string, repo: string, currentVersion: string): Promise<string | null> {
    return invoke<string | null>('check_plugin_update', { owner, repo, installedVersion: currentVersion });
  }
}

export const pluginInstaller = new PluginInstaller();
