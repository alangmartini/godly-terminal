import { isBuiltinPreset, isCustomPreset, type SoundPreset } from '../services/notification-sound';

const STORAGE_KEY = 'godly-notification-settings';
const DEBOUNCE_MS = 2000;

export interface NotificationSettings {
  globalEnabled: boolean;
  volume: number;       // 0–1
  soundPreset: SoundPreset;
}

type Subscriber = () => void;

class NotificationStore {
  private settings: NotificationSettings = {
    globalEnabled: true,
    volume: 0.5,
    soundPreset: 'chime',
  };

  /** Terminal IDs that have unread notification badges */
  private badgedTerminals: Set<string> = new Set();

  /** Debounce: terminal_id → last notify timestamp */
  private lastNotify: Map<string, number> = new Map();

  private subscribers: Subscriber[] = [];

  constructor() {
    this.loadFromStorage();
  }

  // ── Queries ──────────────────────────────────────────────────────

  getSettings(): NotificationSettings {
    return { ...this.settings };
  }

  hasBadge(terminalId: string): boolean {
    return this.badgedTerminals.has(terminalId);
  }

  getBadgedTerminals(): ReadonlySet<string> {
    return this.badgedTerminals;
  }

  /** Check if any terminal in a workspace has a badge */
  workspaceHasBadge(workspaceId: string, getTerminalsForWorkspace: (wsId: string) => { id: string }[]): boolean {
    const terminals = getTerminalsForWorkspace(workspaceId);
    return terminals.some(t => this.badgedTerminals.has(t.id));
  }

  /** Returns true if the notify should be suppressed (debounced) */
  isDebounced(terminalId: string): boolean {
    const last = this.lastNotify.get(terminalId);
    if (!last) return false;
    return Date.now() - last < DEBOUNCE_MS;
  }

  // ── Mutations ────────────────────────────────────────────────────

  /** Record a notification event and add badge. Returns false if debounced. */
  recordNotify(terminalId: string): boolean {
    if (this.isDebounced(terminalId)) return false;
    this.lastNotify.set(terminalId, Date.now());
    this.badgedTerminals.add(terminalId);
    this.notify();
    return true;
  }

  clearBadge(terminalId: string): void {
    if (this.badgedTerminals.delete(terminalId)) {
      this.notify();
    }
  }

  clearAllBadges(): void {
    if (this.badgedTerminals.size > 0) {
      this.badgedTerminals.clear();
      this.notify();
    }
  }

  setGlobalEnabled(enabled: boolean): void {
    this.settings.globalEnabled = enabled;
    this.saveToStorage();
    this.notify();
  }

  setVolume(volume: number): void {
    this.settings.volume = Math.max(0, Math.min(1, volume));
    this.saveToStorage();
    this.notify();
  }

  setSoundPreset(preset: SoundPreset): void {
    this.settings.soundPreset = preset;
    this.saveToStorage();
    this.notify();
  }

  // ── Subscriptions ────────────────────────────────────────────────

  subscribe(fn: Subscriber): () => void {
    this.subscribers.push(fn);
    return () => {
      this.subscribers = this.subscribers.filter(s => s !== fn);
    };
  }

  private notify(): void {
    for (const fn of this.subscribers) fn();
  }

  // ── Persistence ──────────────────────────────────────────────────

  private loadFromStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return;
      const data = JSON.parse(raw) as Partial<NotificationSettings>;
      if (typeof data.globalEnabled === 'boolean') this.settings.globalEnabled = data.globalEnabled;
      if (typeof data.volume === 'number') this.settings.volume = data.volume;
      if (data.soundPreset && (isBuiltinPreset(data.soundPreset) || isCustomPreset(data.soundPreset))) {
        this.settings.soundPreset = data.soundPreset;
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

export const notificationStore = new NotificationStore();
