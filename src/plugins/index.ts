import { PluginEventBus } from './event-bus';
import { PluginRegistry } from './plugin-registry';
import { PeonPingPlugin } from './peon-ping/index';
import { SmolLM2Plugin } from './smollm2/index';
import { pluginStore } from './plugin-store';
import { loadExternalPlugin } from './external-plugin-loader';
import { invoke } from '@tauri-apps/api/core';
import type { ExternalPluginManifest } from './types';

let registry: PluginRegistry | null = null;

export async function initPlugins(): Promise<PluginRegistry> {
  const bus = new PluginEventBus();
  registry = new PluginRegistry(bus);

  // Phase 1: Register built-in plugins
  const peonPing = new PeonPingPlugin();
  peonPing.setBus(bus);
  registry.register(peonPing, { builtin: true });

  registry.register(new SmolLM2Plugin(), { builtin: true });

  // Phase 2: Load installed external plugins
  const installed = pluginStore.getInstalledPlugins();
  for (const [pluginId] of Object.entries(installed)) {
    try {
      const plugin = await loadExternalPlugin(pluginId);
      // Try to read manifest for metadata
      let manifest: ExternalPluginManifest | undefined;
      try {
        const manifests = await invoke<ExternalPluginManifest[]>('list_installed_plugins');
        manifest = manifests.find(m => m.id === pluginId);
      } catch {
        // Manifest read is optional
      }
      registry.register(plugin, { builtin: false, manifest });
    } catch (e) {
      console.warn(`[Plugins] Failed to load external plugin "${pluginId}":`, e);
      // Don't remove from installed — might be a temporary issue
    }
  }

  // Init all registered plugins (async, non-blocking)
  await registry.initAll();

  return registry;
}

export function getPluginRegistry(): PluginRegistry | null {
  return registry;
}

export { PluginEventBus } from './event-bus';
export { PluginRegistry } from './plugin-registry';
export type { GodlyPlugin, PluginContext, PluginEvent, PluginEventType, SoundPackManifest } from './types';
export type { RegisteredPlugin } from './plugin-registry';
