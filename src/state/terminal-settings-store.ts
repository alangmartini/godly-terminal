import type { ShellType } from './store';

const STORAGE_KEY = 'godly-terminal-settings';

/** Which rendering backend to use for terminal grid display. */
export type RendererMode = 'gpu';

/** How split-panel terminals appear in the tab bar. */
export type SplitTabMode = 'individual' | 'unified';

export interface TerminalSettings {
  defaultShell: ShellType;
  /** When true, new output snaps the view to bottom even when scrolled up. */
  autoScrollOnOutput: boolean;
  /** Terminal font size in CSS pixels (clamped to 8–32). */
  fontSize: number;
  /** Rendering backend: 'gpu' (Rust-side wgpu renderer). */
  rendererMode: RendererMode;
  /** How split-panel terminals appear in the tab bar. */
  splitTabMode: SplitTabMode;
  /** When true, show a confirmation dialog before quitting with active sessions. */
  confirmQuit: boolean;
  /** When true, show timestamps on Claude Code message boundaries. */
  messageTimestamps: boolean;
}

type Subscriber = () => void;

class TerminalSettingsStore {
  private static readonly DEFAULT_FONT_SIZE = 13;
  private static readonly MIN_FONT_SIZE = 8;
  private static readonly MAX_FONT_SIZE = 32;

  private settings: TerminalSettings = {
    defaultShell: { type: 'windows' },
    autoScrollOnOutput: false,
    fontSize: TerminalSettingsStore.DEFAULT_FONT_SIZE,
    rendererMode: 'gpu',
    splitTabMode: 'unified',
    confirmQuit: true,
    messageTimestamps: false,
  };

  private subscribers: Subscriber[] = [];

  constructor() {
    this.loadFromStorage();
  }

  getDefaultShell(): ShellType {
    return this.settings.defaultShell;
  }

  setDefaultShell(shell: ShellType): void {
    this.settings.defaultShell = shell;
    this.saveToStorage();
    this.notify();
  }

  getAutoScrollOnOutput(): boolean {
    return this.settings.autoScrollOnOutput;
  }

  setAutoScrollOnOutput(enabled: boolean): void {
    this.settings.autoScrollOnOutput = enabled;
    this.saveToStorage();
    this.notify();
  }

  getFontSize(): number {
    return this.settings.fontSize;
  }

  setFontSize(size: number): void {
    const clamped = Math.max(
      TerminalSettingsStore.MIN_FONT_SIZE,
      Math.min(TerminalSettingsStore.MAX_FONT_SIZE, Math.round(size))
    );
    if (clamped === this.settings.fontSize) return;
    this.settings.fontSize = clamped;
    this.saveToStorage();
    this.notify();
  }

  getRendererMode(): RendererMode {
    return this.settings.rendererMode;
  }

  setRendererMode(mode: RendererMode): void {
    if (mode === this.settings.rendererMode) return;
    this.settings.rendererMode = mode;
    this.saveToStorage();
    this.notify();
  }

  getSplitTabMode(): SplitTabMode {
    return this.settings.splitTabMode;
  }

  setSplitTabMode(mode: SplitTabMode): void {
    if (mode === this.settings.splitTabMode) return;
    this.settings.splitTabMode = mode;
    this.saveToStorage();
    this.notify();
  }

  getConfirmQuit(): boolean {
    return this.settings.confirmQuit;
  }

  setConfirmQuit(enabled: boolean): void {
    if (enabled === this.settings.confirmQuit) return;
    this.settings.confirmQuit = enabled;
    this.saveToStorage();
    this.notify();
  }

  getMessageTimestamps(): boolean {
    return this.settings.messageTimestamps;
  }

  setMessageTimestamps(enabled: boolean): void {
    if (enabled === this.settings.messageTimestamps) return;
    this.settings.messageTimestamps = enabled;
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
      const data = JSON.parse(raw) as Partial<TerminalSettings>;
      if (data.defaultShell && typeof data.defaultShell === 'object' && 'type' in data.defaultShell) {
        // Reject custom shell with empty program
        if (data.defaultShell.type === 'custom' && !(data.defaultShell as { type: 'custom'; program?: string }).program) {
          return;
        }
        this.settings.defaultShell = data.defaultShell;
      }
      if (typeof data.autoScrollOnOutput === 'boolean') {
        this.settings.autoScrollOnOutput = data.autoScrollOnOutput;
      }
      if (typeof data.fontSize === 'number' && data.fontSize >= TerminalSettingsStore.MIN_FONT_SIZE && data.fontSize <= TerminalSettingsStore.MAX_FONT_SIZE) {
        this.settings.fontSize = data.fontSize;
      }
      // GPU is now the only renderer mode; ignore stored legacy values (canvas2d, webgl)
      if (data.rendererMode === 'gpu') {
        this.settings.rendererMode = data.rendererMode;
      }
      if (data.splitTabMode === 'individual' || data.splitTabMode === 'unified') {
        this.settings.splitTabMode = data.splitTabMode;
      }
      if (typeof data.confirmQuit === 'boolean') {
        this.settings.confirmQuit = data.confirmQuit;
      }
      if (typeof data.messageTimestamps === 'boolean') {
        this.settings.messageTimestamps = data.messageTimestamps;
      }
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
}

export const terminalSettingsStore = new TerminalSettingsStore();
