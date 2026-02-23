import { invoke } from '@tauri-apps/api/core';

const STORAGE_KEY = 'godly-remote-settings';

export interface RemoteSettings {
  password: string;
  port: number;
  autoStart: boolean;
  apiKey: string;
}

type Subscriber = () => void;

function generateRandomString(length: number, charset: string): string {
  const array = new Uint8Array(length);
  crypto.getRandomValues(array);
  let result = '';
  for (let i = 0; i < length; i++) {
    result += charset[array[i] % charset.length];
  }
  return result;
}

export function generatePassword(length: number = 100): string {
  const charset = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()-_=+[]{}|;:,.<>?';
  return generateRandomString(length, charset);
}

export function generateApiKey(): string {
  const charset = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  return generateRandomString(24, charset);
}

class RemoteSettingsStore {
  private settings: RemoteSettings = {
    password: '',
    port: 3377,
    autoStart: false,
    apiKey: '',
  };

  private subscribers: Subscriber[] = [];

  constructor() {
    this.loadFromStorage();
  }

  getSettings(): RemoteSettings {
    return { ...this.settings };
  }

  getPassword(): string {
    return this.settings.password;
  }

  setPassword(password: string): void {
    this.settings.password = password;
    this.saveToStorage();
    this.writeSidecarConfig();
    this.notify();
  }

  getPort(): number {
    return this.settings.port;
  }

  setPort(port: number): void {
    this.settings.port = Math.max(1024, Math.min(65535, port));
    this.saveToStorage();
    this.writeSidecarConfig();
    this.notify();
  }

  getAutoStart(): boolean {
    return this.settings.autoStart;
  }

  setAutoStart(autoStart: boolean): void {
    this.settings.autoStart = autoStart;
    this.saveToStorage();
    this.writeSidecarConfig();
    this.notify();
  }

  getApiKey(): string {
    return this.settings.apiKey;
  }

  setApiKey(apiKey: string): void {
    this.settings.apiKey = apiKey;
    this.saveToStorage();
    this.writeSidecarConfig();
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
      const data = JSON.parse(raw) as Partial<RemoteSettings>;
      if (typeof data.password === 'string') this.settings.password = data.password;
      if (typeof data.port === 'number' && data.port >= 1024 && data.port <= 65535) {
        this.settings.port = data.port;
      }
      if (typeof data.autoStart === 'boolean') this.settings.autoStart = data.autoStart;
      if (typeof data.apiKey === 'string') this.settings.apiKey = data.apiKey;
    } catch {
      // Corrupt data — use defaults
    }
  }

  private saveToStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.settings));
    } catch {
      // No localStorage available
    }
  }

  /** Write a JSON sidecar file that setup-phone.ps1 can read */
  private writeSidecarConfig(): void {
    invoke('write_remote_config', {
      config: {
        password: this.settings.password,
        port: this.settings.port,
        auto_start: this.settings.autoStart,
        api_key: this.settings.apiKey,
      },
    }).catch(() => {
      // Tauri command may not be available yet
    });
  }
}

export const remoteSettingsStore = new RemoteSettingsStore();
