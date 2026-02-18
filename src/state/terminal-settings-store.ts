import type { ShellType } from './store';

const STORAGE_KEY = 'godly-terminal-settings';

export interface TerminalSettings {
  defaultShell: ShellType;
  /** When true, new output snaps the view to bottom even when scrolled up. */
  autoScrollOnOutput: boolean;
}

type Subscriber = () => void;

class TerminalSettingsStore {
  private settings: TerminalSettings = {
    defaultShell: { type: 'windows' },
    autoScrollOnOutput: false,
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
