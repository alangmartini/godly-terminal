// ── Types ──────────────────────────────────────────────────────────────

export interface KeyChord {
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  key: string; // normalised lowercase letter or key name (e.g. "tab")
}

export type ActionId =
  | 'terminal.interrupt'
  | 'terminal.suspend'
  | 'terminal.literalNext'
  | 'clipboard.copy'
  | 'clipboard.copyClean'
  | 'clipboard.paste'
  | 'tabs.newTerminal'
  | 'tabs.closeTerminal'
  | 'tabs.nextTab'
  | 'tabs.previousTab'
  | 'split.focusOtherPane'
  | 'split.unsplit'
  | 'workspace.toggleWorktreeMode'
  | 'workspace.toggleClaudeCodeMode'
  | 'scroll.pageUp'
  | 'scroll.pageDown'
  | 'scroll.toTop'
  | 'scroll.toBottom'
  | 'tabs.renameTerminal'
  | 'tabs.quickClaude'
  | 'debug.togglePerfOverlay';

export type ShortcutCategory = 'Terminal' | 'Clipboard' | 'Tabs' | 'Split' | 'Workspace' | 'Scroll' | 'Debug';

/** Whether the shortcut is an app-level action or a terminal control key. */
export type ShortcutType = 'app' | 'terminal-control';

export interface ShortcutDefinition {
  id: ActionId;
  label: string;
  category: ShortcutCategory;
  type: ShortcutType;
  defaultChord: KeyChord;
}

// ── Defaults (single source of truth) ──────────────────────────────────

export const DEFAULT_SHORTCUTS: ShortcutDefinition[] = [
  {
    id: 'terminal.interrupt',
    label: 'Interrupt (SIGINT)',
    category: 'Terminal',
    type: 'terminal-control',
    defaultChord: { ctrlKey: true, shiftKey: false, altKey: false, key: 'c' },
  },
  {
    id: 'terminal.suspend',
    label: 'Suspend (SIGTSTP)',
    category: 'Terminal',
    type: 'terminal-control',
    defaultChord: { ctrlKey: true, shiftKey: false, altKey: false, key: 'z' },
  },
  {
    id: 'terminal.literalNext',
    label: 'Literal Next',
    category: 'Terminal',
    type: 'terminal-control',
    defaultChord: { ctrlKey: true, shiftKey: false, altKey: false, key: 'v' },
  },
  {
    id: 'clipboard.copy',
    label: 'Copy',
    category: 'Clipboard',
    type: 'app',
    defaultChord: { ctrlKey: true, shiftKey: true, altKey: false, key: 'c' },
  },
  {
    id: 'clipboard.copyClean',
    label: 'Copy (Clean)',
    category: 'Clipboard',
    type: 'app',
    defaultChord: { ctrlKey: true, shiftKey: true, altKey: true, key: 'c' },
  },
  {
    id: 'clipboard.paste',
    label: 'Paste',
    category: 'Clipboard',
    type: 'app',
    defaultChord: { ctrlKey: true, shiftKey: true, altKey: false, key: 'v' },
  },
  {
    id: 'tabs.newTerminal',
    label: 'New Terminal',
    category: 'Tabs',
    type: 'app',
    defaultChord: { ctrlKey: true, shiftKey: false, altKey: false, key: 't' },
  },
  {
    id: 'tabs.closeTerminal',
    label: 'Close Terminal',
    category: 'Tabs',
    type: 'app',
    defaultChord: { ctrlKey: true, shiftKey: false, altKey: false, key: 'w' },
  },
  {
    id: 'tabs.nextTab',
    label: 'Next Tab',
    category: 'Tabs',
    type: 'app',
    defaultChord: { ctrlKey: true, shiftKey: false, altKey: false, key: 'tab' },
  },
  {
    id: 'tabs.previousTab',
    label: 'Previous Tab',
    category: 'Tabs',
    type: 'app',
    defaultChord: { ctrlKey: true, shiftKey: true, altKey: false, key: 'tab' },
  },
  {
    id: 'split.focusOtherPane',
    label: 'Focus Other Pane',
    category: 'Split',
    type: 'app',
    defaultChord: { ctrlKey: false, shiftKey: false, altKey: true, key: '\\' },
  },
  {
    id: 'split.unsplit',
    label: 'Unsplit',
    category: 'Split',
    type: 'app',
    defaultChord: { ctrlKey: true, shiftKey: true, altKey: false, key: '\\' },
  },
  {
    id: 'workspace.toggleWorktreeMode',
    label: 'Toggle Worktree Mode',
    category: 'Workspace',
    type: 'app',
    defaultChord: { ctrlKey: true, shiftKey: true, altKey: false, key: 'w' },
  },
  {
    id: 'workspace.toggleClaudeCodeMode',
    label: 'Toggle Claude Code Mode',
    category: 'Workspace',
    type: 'app',
    defaultChord: { ctrlKey: true, shiftKey: true, altKey: false, key: 'e' },
  },
  {
    id: 'scroll.pageUp',
    label: 'Scroll Page Up',
    category: 'Scroll',
    type: 'app',
    defaultChord: { ctrlKey: false, shiftKey: false, altKey: false, key: 'pageup' },
  },
  {
    id: 'scroll.pageDown',
    label: 'Scroll Page Down',
    category: 'Scroll',
    type: 'app',
    defaultChord: { ctrlKey: false, shiftKey: false, altKey: false, key: 'pagedown' },
  },
  {
    id: 'scroll.toTop',
    label: 'Scroll to Top',
    category: 'Scroll',
    type: 'app',
    defaultChord: { ctrlKey: true, shiftKey: true, altKey: false, key: 'home' },
  },
  {
    id: 'scroll.toBottom',
    label: 'Scroll to Bottom',
    category: 'Scroll',
    type: 'app',
    defaultChord: { ctrlKey: true, shiftKey: true, altKey: false, key: 'end' },
  },
  {
    id: 'tabs.renameTerminal',
    label: 'Rename Terminal',
    category: 'Tabs',
    type: 'app',
    defaultChord: { ctrlKey: false, shiftKey: false, altKey: false, key: 'f2' },
  },
  {
    id: 'tabs.quickClaude',
    label: 'Quick Claude',
    category: 'Tabs',
    type: 'app',
    defaultChord: { ctrlKey: true, shiftKey: true, altKey: false, key: 'q' },
  },
  {
    id: 'debug.togglePerfOverlay',
    label: 'Toggle Perf Overlay',
    category: 'Debug',
    type: 'app',
    defaultChord: { ctrlKey: true, shiftKey: true, altKey: false, key: 'p' },
  },
];

// ── Helpers ─────────────────────────────────────────────────────────────

/** Normalise a key name to lowercase for comparison purposes. */
function normaliseKey(key: string): string {
  return key.toLowerCase();
}

/** Produce a stable string for a chord, used as a cache key. */
export function chordToString(chord: KeyChord): string {
  const parts: string[] = [];
  if (chord.ctrlKey) parts.push('Ctrl');
  if (chord.shiftKey) parts.push('Shift');
  if (chord.altKey) parts.push('Alt');
  parts.push(normaliseKey(chord.key));
  return parts.join('+');
}

/** Convert a DOM KeyboardEvent into a KeyChord. */
export function eventToChord(event: {
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  key: string;
}): KeyChord {
  return {
    ctrlKey: event.ctrlKey,
    shiftKey: event.shiftKey,
    altKey: event.altKey,
    key: normaliseKey(event.key),
  };
}

/** Human-readable display string for a chord (e.g. "Ctrl+Shift+C"). */
export function formatChord(chord: KeyChord): string {
  const parts: string[] = [];
  if (chord.ctrlKey) parts.push('Ctrl');
  if (chord.shiftKey) parts.push('Shift');
  if (chord.altKey) parts.push('Alt');
  // Capitalise the key for display
  const keyDisplayMap: Record<string, string> = {
    tab: 'Tab',
    pageup: 'PageUp',
    pagedown: 'PageDown',
    home: 'Home',
    end: 'End',
    f2: 'F2',
  };
  const displayKey = keyDisplayMap[chord.key] ?? chord.key.toUpperCase();
  parts.push(displayKey);
  return parts.join('+');
}

// ── Persistence key ────────────────────────────────────────────────────

const STORAGE_KEY = 'godly-custom-keybindings';

// ── Store ──────────────────────────────────────────────────────────────

type Subscriber = () => void;

export class KeybindingStore {
  /** Maps actionId → current chord. Initialised from defaults + localStorage. */
  private bindings: Map<ActionId, KeyChord> = new Map();

  /** Reverse index: chordString → actionId for O(1) event matching. */
  private chordIndex: Map<string, ActionId> = new Map();

  /** Type classification per action (never changes). */
  private typeMap: Map<ActionId, ShortcutType> = new Map();

  private subscribers: Subscriber[] = [];

  constructor() {
    // Seed type map
    for (const def of DEFAULT_SHORTCUTS) {
      this.typeMap.set(def.id, def.type);
    }
    // Load defaults then apply overrides from storage
    this.loadDefaults();
    this.loadFromStorage();
    this.rebuildIndex();
  }

  // ── Queries ────────────────────────────────────────────────────────

  /** Get the current chord for an action. */
  getBinding(actionId: ActionId): KeyChord {
    return this.bindings.get(actionId)!;
  }

  /** Check if the current binding differs from the default. */
  isCustom(actionId: ActionId): boolean {
    const def = DEFAULT_SHORTCUTS.find((d) => d.id === actionId);
    if (!def) return false;
    return chordToString(this.getBinding(actionId)) !== chordToString(def.defaultChord);
  }

  /** Match a keyboard event to an action. Returns the action ID or null. */
  matchAction(event: {
    ctrlKey: boolean;
    shiftKey: boolean;
    altKey: boolean;
    key: string;
    type: string;
  }): ActionId | null {
    if (event.type !== 'keydown') return null;
    const chord = eventToChord(event);
    return this.chordIndex.get(chordToString(chord)) ?? null;
  }

  /**
   * Returns true if the event matches any shortcut whose *type* is `app`.
   * Used by the canvas key handler to let events bubble.
   */
  isAppShortcut(event: {
    ctrlKey: boolean;
    shiftKey: boolean;
    altKey: boolean;
    key: string;
    type: string;
  }): boolean {
    if (event.type !== 'keydown') return false;
    const chord = eventToChord(event);
    const str = chordToString(chord);
    const actionId = this.chordIndex.get(str);
    if (!actionId) return false;
    return this.typeMap.get(actionId) === 'app';
  }

  /**
   * Returns true if the event matches any shortcut whose *type* is
   * `terminal-control`. These need `preventDefault()` so the browser
   * doesn't intercept them.
   */
  isTerminalControlKey(event: {
    ctrlKey: boolean;
    shiftKey: boolean;
    altKey: boolean;
    key: string;
    type: string;
  }): boolean {
    if (event.type !== 'keydown') return false;
    const chord = eventToChord(event);
    const str = chordToString(chord);
    const actionId = this.chordIndex.get(str);
    if (!actionId) return false;
    return this.typeMap.get(actionId) === 'terminal-control';
  }

  /**
   * Find an action that conflicts with a given chord (excluding a
   * specific action, e.g. the one being edited).
   */
  findConflict(chord: KeyChord, excludeAction?: ActionId): ActionId | null {
    const str = chordToString(chord);
    const existing = this.chordIndex.get(str);
    if (existing && existing !== excludeAction) return existing;
    return null;
  }

  // ── Mutations ──────────────────────────────────────────────────────

  /** Rebind an action to a new chord. */
  setBinding(actionId: ActionId, chord: KeyChord): void {
    this.bindings.set(actionId, chord);
    this.rebuildIndex();
    this.saveToStorage();
    this.notify();
  }

  /** Reset a single action to its default. */
  resetBinding(actionId: ActionId): void {
    const def = DEFAULT_SHORTCUTS.find((d) => d.id === actionId);
    if (def) {
      this.bindings.set(actionId, { ...def.defaultChord });
      this.rebuildIndex();
      this.saveToStorage();
      this.notify();
    }
  }

  /** Reset all bindings to defaults. */
  resetAll(): void {
    this.loadDefaults();
    this.rebuildIndex();
    this.saveToStorage();
    this.notify();
  }

  // ── Subscriptions ──────────────────────────────────────────────────

  subscribe(fn: Subscriber): () => void {
    this.subscribers.push(fn);
    return () => {
      this.subscribers = this.subscribers.filter((s) => s !== fn);
    };
  }

  private notify(): void {
    for (const fn of this.subscribers) fn();
  }

  // ── Internal ───────────────────────────────────────────────────────

  private loadDefaults(): void {
    for (const def of DEFAULT_SHORTCUTS) {
      this.bindings.set(def.id, { ...def.defaultChord });
    }
  }

  private rebuildIndex(): void {
    this.chordIndex.clear();
    for (const [actionId, chord] of this.bindings) {
      this.chordIndex.set(chordToString(chord), actionId);
    }
  }

  private loadFromStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return;
      const overrides: Record<string, KeyChord> = JSON.parse(raw);
      for (const [id, chord] of Object.entries(overrides)) {
        if (this.bindings.has(id as ActionId)) {
          this.bindings.set(id as ActionId, chord);
        }
      }
    } catch {
      // Corrupt data or no localStorage — ignore and use defaults
    }
  }

  private saveToStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      const overrides: Record<string, KeyChord> = {};
      for (const def of DEFAULT_SHORTCUTS) {
        const current = this.bindings.get(def.id)!;
        if (chordToString(current) !== chordToString(def.defaultChord)) {
          overrides[def.id] = current;
        }
      }
      if (Object.keys(overrides).length === 0) {
        localStorage.removeItem(STORAGE_KEY);
      } else {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(overrides));
      }
    } catch {
      // No localStorage available — silently skip
    }
  }
}

// ── Singleton ──────────────────────────────────────────────────────────

export const keybindingStore = new KeybindingStore();
