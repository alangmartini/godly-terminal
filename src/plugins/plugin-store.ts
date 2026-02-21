import type { InstalledPluginMeta } from './types';

const STORAGE_KEY = 'godly-plugin-settings';

type Subscriber = () => void;

interface PluginStoreData {
  enabledPlugins: Record<string, boolean>;
  pluginSettings: Record<string, Record<string, unknown>>;
  installedPlugins: Record<string, InstalledPluginMeta>;
}

class PluginStore {
  private data: PluginStoreData = {
    enabledPlugins: {},
    pluginSettings: {},
    installedPlugins: {},
  };

  private subscribers: Subscriber[] = [];

  constructor() {
    this.loadFromStorage();
  }

  isEnabled(pluginId: string): boolean {
    return this.data.enabledPlugins[pluginId] ?? false;
  }

  setEnabled(pluginId: string, enabled: boolean): void {
    this.data.enabledPlugins[pluginId] = enabled;
    this.saveToStorage();
    this.notify();
  }

  getSetting<T>(pluginId: string, key: string, defaultValue: T): T {
    const settings = this.data.pluginSettings[pluginId];
    if (!settings || !(key in settings)) return defaultValue;
    return settings[key] as T;
  }

  setSetting<T>(pluginId: string, key: string, value: T): void {
    if (!this.data.pluginSettings[pluginId]) {
      this.data.pluginSettings[pluginId] = {};
    }
    this.data.pluginSettings[pluginId][key] = value;
    this.saveToStorage();
    this.notify();
  }

  getInstalledPlugins(): Record<string, InstalledPluginMeta> {
    return { ...this.data.installedPlugins };
  }

  getInstalledPlugin(pluginId: string): InstalledPluginMeta | undefined {
    return this.data.installedPlugins[pluginId];
  }

  setInstalled(pluginId: string, meta: InstalledPluginMeta): void {
    this.data.installedPlugins[pluginId] = meta;
    this.saveToStorage();
    this.notify();
  }

  removeInstalled(pluginId: string): void {
    delete this.data.installedPlugins[pluginId];
    this.saveToStorage();
    this.notify();
  }

  subscribe(fn: Subscriber): () => void {
    this.subscribers.push(fn);
    return () => {
      this.subscribers = this.subscribers.filter(s => s !== fn);
    };
  }

  private notify(): void {
    for (const fn of this.subscribers) fn();
  }

  private loadFromStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return;
      const parsed = JSON.parse(raw) as Partial<PluginStoreData>;
      if (parsed.enabledPlugins && typeof parsed.enabledPlugins === 'object') {
        this.data.enabledPlugins = parsed.enabledPlugins;
      }
      if (parsed.pluginSettings && typeof parsed.pluginSettings === 'object') {
        this.data.pluginSettings = parsed.pluginSettings;
      }
      if (parsed.installedPlugins && typeof parsed.installedPlugins === 'object') {
        this.data.installedPlugins = parsed.installedPlugins;
      }
    } catch {
      // Corrupt data — use defaults
    }
  }

  private saveToStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.data));
    } catch {
      // No localStorage available
    }
  }
}

export const pluginStore = new PluginStore();
