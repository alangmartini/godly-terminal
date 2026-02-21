import { invoke } from '@tauri-apps/api/core';
import type { GodlyPlugin } from './types';

/**
 * Load an external plugin by reading its JS from the backend,
 * creating a Blob URL, and dynamically importing it.
 */
export async function loadExternalPlugin(pluginId: string): Promise<GodlyPlugin> {
  const js = await invoke<string>('read_plugin_js', { pluginId });

  const blob = new Blob([js], { type: 'application/javascript' });
  const url = URL.createObjectURL(blob);

  try {
    const mod = await import(/* @vite-ignore */ url);
    const PluginClass = mod.default || mod[Object.keys(mod)[0]];

    if (!PluginClass) {
      throw new Error(`Plugin "${pluginId}" has no default export`);
    }

    const plugin: GodlyPlugin = typeof PluginClass === 'function'
      ? new PluginClass()
      : PluginClass;

    // Validate GodlyPlugin shape
    if (!plugin.id || typeof plugin.id !== 'string') {
      throw new Error(`Plugin "${pluginId}" missing required "id" field`);
    }
    if (!plugin.name || typeof plugin.name !== 'string') {
      throw new Error(`Plugin "${pluginId}" missing required "name" field`);
    }
    if (typeof plugin.init !== 'function') {
      throw new Error(`Plugin "${pluginId}" missing required "init" method`);
    }

    return plugin;
  } finally {
    URL.revokeObjectURL(url);
  }
}

/**
 * Load plugin icon as a data URL, or return null if no icon.
 */
export async function loadPluginIconDataUrl(pluginId: string): Promise<string | null> {
  try {
    const base64 = await invoke<string | null>('read_plugin_icon', { pluginId });
    if (!base64) return null;
    return `data:image/png;base64,${base64}`;
  } catch {
    return null;
  }
}
