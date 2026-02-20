import { PluginEventBus } from './event-bus';
import { PluginRegistry } from './plugin-registry';
import { PeonPingPlugin } from './peon-ping/index';

let registry: PluginRegistry | null = null;

export function initPlugins(): PluginRegistry {
  const bus = new PluginEventBus();
  registry = new PluginRegistry(bus);

  // Register built-in plugins
  const peonPing = new PeonPingPlugin();
  peonPing.setBus(bus);
  registry.register(peonPing);

  // Init all registered plugins (async, non-blocking)
  registry.initAll().catch(e => {
    console.warn('[Plugins] Failed to initialize plugins:', e);
  });

  return registry;
}

export function getPluginRegistry(): PluginRegistry | null {
  return registry;
}

export { PluginEventBus } from './event-bus';
export { PluginRegistry } from './plugin-registry';
export type { GodlyPlugin, PluginContext, PluginEvent, PluginEventType, SoundPackManifest } from './types';
