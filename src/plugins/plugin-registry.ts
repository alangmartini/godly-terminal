import { invoke } from '@tauri-apps/api/core';
import type { GodlyPlugin, PluginContext, PluginEventType, SoundPackManifest, ExternalPluginManifest } from './types';
import { pluginStore } from './plugin-store';
import { PluginEventBus } from './event-bus';
import { getSharedAudioContext, playBuffer } from '../services/notification-sound';

export interface RegisteredPlugin {
  plugin: GodlyPlugin;
  builtin: boolean;
  manifest?: ExternalPluginManifest;
}

// Whitelisted Tauri commands that external plugins may invoke
const EXTERNAL_PLUGIN_ALLOWED_COMMANDS = new Set([
  'list_sound_packs',
  'list_sound_pack_files',
  'read_sound_pack_file',
  'get_sound_packs_dir',
]);

export class PluginRegistry {
  private plugins = new Map<string, RegisteredPlugin>();
  private bus: PluginEventBus;

  constructor(bus: PluginEventBus) {
    this.bus = bus;
  }

  register(plugin: GodlyPlugin, options?: { builtin?: boolean; manifest?: ExternalPluginManifest }): void {
    this.plugins.set(plugin.id, {
      plugin,
      builtin: options?.builtin ?? true,
      manifest: options?.manifest,
    });
  }

  async initAll(): Promise<void> {
    for (const [id, entry] of this.plugins) {
      const ctx = this.createContext(id, entry.builtin);
      try {
        await entry.plugin.init(ctx);
        if (pluginStore.isEnabled(id)) {
          entry.plugin.enable?.();
        }
      } catch (e) {
        console.warn(`[PluginRegistry] Failed to init plugin "${id}":`, e);
      }
    }
  }

  setEnabled(pluginId: string, enabled: boolean): void {
    pluginStore.setEnabled(pluginId, enabled);
    const entry = this.plugins.get(pluginId);
    if (!entry) return;
    if (enabled) {
      entry.plugin.enable?.();
    } else {
      entry.plugin.disable?.();
    }
  }

  isEnabled(pluginId: string): boolean {
    return pluginStore.isEnabled(pluginId);
  }

  isBuiltin(pluginId: string): boolean {
    return this.plugins.get(pluginId)?.builtin ?? false;
  }

  getAll(): GodlyPlugin[] {
    return Array.from(this.plugins.values()).map(e => e.plugin);
  }

  getAllWithMeta(): RegisteredPlugin[] {
    return Array.from(this.plugins.values());
  }

  getPlugin(id: string): GodlyPlugin | undefined {
    return this.plugins.get(id)?.plugin;
  }

  getRegisteredPlugin(id: string): RegisteredPlugin | undefined {
    return this.plugins.get(id);
  }

  destroyAll(): void {
    for (const entry of this.plugins.values()) {
      try {
        entry.plugin.destroy?.();
      } catch (e) {
        console.warn(`[PluginRegistry] Failed to destroy plugin "${entry.plugin.id}":`, e);
      }
    }
    this.plugins.clear();
    this.bus.removeAllHandlers();
  }

  getBus(): PluginEventBus {
    return this.bus;
  }

  private createContext(pluginId: string, isBuiltin: boolean): PluginContext {
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

      async invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
        if (!isBuiltin && !EXTERNAL_PLUGIN_ALLOWED_COMMANDS.has(command)) {
          throw new Error(`Plugin "${pluginId}" is not allowed to invoke "${command}"`);
        }
        return invoke<T>(command, args);
      },

      showToast(message: string, type?: 'info' | 'error' | 'success'): void {
        // Dispatch a custom event that the UI can listen for
        window.dispatchEvent(new CustomEvent('plugin-toast', {
          detail: { pluginId, message, type: type || 'info' },
        }));
      },
    };
  }
}
