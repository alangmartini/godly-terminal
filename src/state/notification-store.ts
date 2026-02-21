import { isBuiltinPreset, isCustomPreset, type SoundPreset } from '../services/notification-sound';
import { globMatch } from '../utils/glob-match';

const STORAGE_KEY = 'godly-notification-settings';
const WORKSPACE_MUTE_KEY = 'godly-workspace-mute-settings';
const DEBOUNCE_MS = 2000;

export interface NotificationSettings {
  globalEnabled: boolean;
  volume: number;       // 0–1
  soundPreset: SoundPreset;
  idleNotifyEnabled: boolean;
}

interface WorkspaceMuteSettings {
  mutedWorkspacePatterns: string[];
  workspaceOverrides: Record<string, boolean>; // workspace_id → enabled
}

type Subscriber = () => void;

class NotificationStore {
  private settings: NotificationSettings = {
    globalEnabled: true,
    volume: 0.5,
    soundPreset: 'chime',
    idleNotifyEnabled: true,
  };

  private workspaceMute: WorkspaceMuteSettings = {
    mutedWorkspacePatterns: [],
    workspaceOverrides: {},
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

  // ── Workspace mute queries ─────────────────────────────────────

  getMutedPatterns(): string[] {
    return [...this.workspaceMute.mutedWorkspacePatterns];
  }

  getWorkspaceOverride(workspaceId: string): boolean | undefined {
    const val = this.workspaceMute.workspaceOverrides[workspaceId];
    return val === undefined ? undefined : val;
  }

  /**
   * Check if notifications are enabled for a workspace.
   * Priority: manual override > glob pattern match > global default (true).
   */
  isWorkspaceNotificationEnabled(workspaceId: string, workspaceName: string): boolean {
    // Manual override takes priority
    const override = this.workspaceMute.workspaceOverrides[workspaceId];
    if (override !== undefined) return override;

    // Check glob patterns — if any pattern matches, muted
    for (const pattern of this.workspaceMute.mutedWorkspacePatterns) {
      if (globMatch(pattern, workspaceName)) return false;
    }

    // Default: enabled
    return true;
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

  setIdleNotifyEnabled(enabled: boolean): void {
    this.settings.idleNotifyEnabled = enabled;
    this.saveToStorage();
    this.notify();
  }

  // ── Workspace mute mutations ───────────────────────────────────

  addMutedPattern(pattern: string): void {
    const trimmed = pattern.trim();
    if (!trimmed) return;
    if (this.workspaceMute.mutedWorkspacePatterns.includes(trimmed)) return;
    this.workspaceMute.mutedWorkspacePatterns.push(trimmed);
    this.saveWorkspaceMuteToStorage();
    this.notify();
  }

  removeMutedPattern(pattern: string): void {
    const idx = this.workspaceMute.mutedWorkspacePatterns.indexOf(pattern);
    if (idx === -1) return;
    this.workspaceMute.mutedWorkspacePatterns.splice(idx, 1);
    this.saveWorkspaceMuteToStorage();
    this.notify();
  }

  setWorkspaceOverride(workspaceId: string, enabled: boolean): void {
    this.workspaceMute.workspaceOverrides[workspaceId] = enabled;
    this.saveWorkspaceMuteToStorage();
    this.notify();
  }

  clearWorkspaceOverride(workspaceId: string): void {
    delete this.workspaceMute.workspaceOverrides[workspaceId];
    this.saveWorkspaceMuteToStorage();
    this.notify();
  }

  /** Remove stale workspace override when workspace is deleted */
  cleanupWorkspaceOverride(workspaceId: string): void {
    if (workspaceId in this.workspaceMute.workspaceOverrides) {
      delete this.workspaceMute.workspaceOverrides[workspaceId];
      this.saveWorkspaceMuteToStorage();
    }
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
      if (raw) {
        const data = JSON.parse(raw) as Partial<NotificationSettings>;
        if (typeof data.globalEnabled === 'boolean') this.settings.globalEnabled = data.globalEnabled;
        if (typeof data.volume === 'number') this.settings.volume = data.volume;
        if (data.soundPreset && (isBuiltinPreset(data.soundPreset) || isCustomPreset(data.soundPreset))) {
          this.settings.soundPreset = data.soundPreset;
        }
        if (typeof data.idleNotifyEnabled === 'boolean') this.settings.idleNotifyEnabled = data.idleNotifyEnabled;
      }
    } catch {
      // Corrupt data — use defaults
    }

    try {
      if (typeof localStorage === 'undefined') return;
      const raw = localStorage.getItem(WORKSPACE_MUTE_KEY);
      if (raw) {
        const data = JSON.parse(raw) as Partial<WorkspaceMuteSettings>;
        if (Array.isArray(data.mutedWorkspacePatterns)) {
          this.workspaceMute.mutedWorkspacePatterns = data.mutedWorkspacePatterns.filter(
            (p): p is string => typeof p === 'string'
          );
        }
        if (data.workspaceOverrides && typeof data.workspaceOverrides === 'object') {
          for (const [k, v] of Object.entries(data.workspaceOverrides)) {
            if (typeof v === 'boolean') {
              this.workspaceMute.workspaceOverrides[k] = v;
            }
          }
        }
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

  private saveWorkspaceMuteToStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      localStorage.setItem(WORKSPACE_MUTE_KEY, JSON.stringify(this.workspaceMute));
    } catch {
      // No localStorage available
    }
  }
}

export const notificationStore = new NotificationStore();
