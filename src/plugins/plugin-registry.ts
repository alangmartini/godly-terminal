import { invoke } from '@tauri-apps/api/core';
import type { GodlyPlugin, PluginContext, PluginEventType, SoundPackManifest } from './types';
import { pluginStore } from './plugin-store';
import { PluginEventBus } from './event-bus';
import { getSharedAudioContext, playBuffer } from '../services/notification-sound';

export class PluginRegistry {
  private plugins = new Map<string, GodlyPlugin>();
  private bus: PluginEventBus;

  constructor(bus: PluginEventBus) {
    this.bus = bus;
  }

  register(plugin: GodlyPlugin): void {
    this.plugins.set(plugin.id, plugin);
  }

  async initAll(): Promise<void> {
    for (const [id, plugin] of this.plugins) {
      const ctx = this.createContext(id);
      try {
        await plugin.init(ctx);
        if (pluginStore.isEnabled(id)) {
          plugin.enable?.();
        }
      } catch (e) {
        console.warn(`[PluginRegistry] Failed to init plugin "${id}":`, e);
      }
    }
  }

  setEnabled(pluginId: string, enabled: boolean): void {
    pluginStore.setEnabled(pluginId, enabled);
    const plugin = this.plugins.get(pluginId);
    if (!plugin) return;
    if (enabled) {
      plugin.enable?.();
    } else {
      plugin.disable?.();
    }
  }

  isEnabled(pluginId: string): boolean {
    return pluginStore.isEnabled(pluginId);
  }

  getAll(): GodlyPlugin[] {
    return Array.from(this.plugins.values());
  }

  getPlugin(id: string): GodlyPlugin | undefined {
    return this.plugins.get(id);
  }

  destroyAll(): void {
    for (const plugin of this.plugins.values()) {
      try {
        plugin.destroy?.();
      } catch (e) {
        console.warn(`[PluginRegistry] Failed to destroy plugin "${plugin.id}":`, e);
      }
    }
    this.plugins.clear();
    this.bus.removeAllHandlers();
  }

  getBus(): PluginEventBus {
    return this.bus;
  }

  private createContext(pluginId: string): PluginContext {
    const bus = this.bus;
    return {
      on(type: PluginEventType, handler: (event: import('./types').PluginEvent) => void): () => void {
        return bus.on(type, handler);
      },

      async readSoundFile(packId: string, filename: string): Promise<string> {
        return invoke<string>('read_sound_pack_file', { packId, filename });
      },

      async listSoundPackFiles(packId: string): Promise<string[]> {
        return invoke<string[]>('list_sound_pack_files', { packId });
      },

      async listSoundPacks(): Promise<SoundPackManifest[]> {
        return invoke<SoundPackManifest[]>('list_sound_packs');
      },

      getAudioContext(): AudioContext {
        return getSharedAudioContext();
      },

      getSetting<T>(key: string, defaultValue: T): T {
        return pluginStore.getSetting(pluginId, key, defaultValue);
      },

      setSetting<T>(key: string, value: T): void {
        pluginStore.setSetting(pluginId, key, value);
      },

      playSound(buffer: AudioBuffer, volume: number): void {
        playBuffer(buffer, volume);
      },
    };
  }
}
