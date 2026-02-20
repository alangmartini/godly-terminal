// Plugin event types that map to semantic actions
export type PluginEventType =
  | 'notification'
  | 'terminal:output'
  | 'terminal:closed'
  | 'process:changed'
  | 'agent:task-complete'
  | 'agent:error'
  | 'agent:permission'
  | 'agent:ready'
  | 'app:focus'
  | 'app:blur';

export interface PluginEvent {
  type: PluginEventType;
  terminalId?: string;
  message?: string;
  processName?: string;
  timestamp: number;
}

export interface SoundPackManifest {
  id: string;
  name: string;
  description: string;
  author: string;
  version: string;
  sounds: {
    ready?: string[];
    complete?: string[];
    error?: string[];
    permission?: string[];
    notification?: string[];
  };
}

export interface PluginContext {
  /** Subscribe to plugin events */
  on(type: PluginEventType, handler: (event: PluginEvent) => void): () => void;
  /** Read an audio file from a sound pack (returns base64) */
  readSoundFile(packId: string, filename: string): Promise<string>;
  /** List audio files in a sound pack */
  listSoundPackFiles(packId: string): Promise<string[]>;
  /** List all installed sound packs */
  listSoundPacks(): Promise<SoundPackManifest[]>;
  /** Get the shared AudioContext */
  getAudioContext(): AudioContext;
  /** Get a plugin-scoped setting */
  getSetting<T>(key: string, defaultValue: T): T;
  /** Set a plugin-scoped setting */
  setSetting<T>(key: string, value: T): void;
  /** Play an AudioBuffer at the given volume */
  playSound(buffer: AudioBuffer, volume: number): void;
}

export interface GodlyPlugin {
  id: string;
  name: string;
  description: string;
  version: string;
  init(ctx: PluginContext): void | Promise<void>;
  enable?(): void;
  disable?(): void;
  destroy?(): void;
  renderSettings?(): HTMLElement;
}
