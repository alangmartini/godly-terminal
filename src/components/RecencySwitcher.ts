import { store, type Terminal } from '../state/store';
import { keybindingStore, chordToString, eventToChord } from '../state/keybinding-store';

export interface RecencySwitcherEntry {
  terminalId: string;
  name: string;
  processName: string;
  workspaceName: string;
}

/**
 * Modal overlay that shows tabs ordered by most-recently-used.
 * Opens on the tab-switching shortcut, cycles with repeated presses while
 * the primary modifier is held, and commits the selection when the modifier
 * is released. Dynamically reads keybindings so custom shortcuts work.
 */
export class RecencySwitcher {
  private overlay: HTMLElement;
  private list: HTMLElement;
  private entries: RecencySwitcherEntry[] = [];
  private selectedIndex = 0;
  private keydownHandler: ((e: KeyboardEvent) => void) | null = null;
  private keyupHandler: ((e: KeyboardEvent) => void) | null = null;
  private visible = false;
  /** The modifier key whose release commits the selection. */
  private commitModifier: string = 'Control';

  constructor() {
    this.overlay = document.createElement('div');
    this.overlay.className = 'recency-switcher-overlay';
    this.overlay.style.display = 'none';

    const container = document.createElement('div');
    container.className = 'recency-switcher';

    const title = document.createElement('div');
    title.className = 'recency-switcher-title';
    title.textContent = 'Switch Tab';
    container.appendChild(title);

    this.list = document.createElement('div');
    this.list.className = 'recency-switcher-list';
    container.appendChild(this.list);

    this.overlay.appendChild(container);
  }

  mount(parent: HTMLElement): void {
    parent.appendChild(this.overlay);
  }

  destroy(): void {
    this.hide();
    this.overlay.remove();
  }

  isVisible(): boolean {
    return this.visible;
  }

  /**
   * Show the switcher with MRU-ordered entries.
   * @param reverse If true, start selection at the end (Shift+Tab direction).
   */
  show(reverse = false): void {
    const state = store.getState();
    const wsId = state.activeWorkspaceId;
    if (!wsId) return;

    const accessHistory = store.getAccessHistory(wsId);
    const wsTerminals = store.getWorkspaceTerminals(wsId);
    const workspace = state.workspaces.find(w => w.id === wsId);

    if (wsTerminals.length < 2) return;

    // Determine the commit modifier from the actual keybinding
    const chord = reverse
      ? keybindingStore.getBinding('tabs.previousTab')
      : keybindingStore.getBinding('tabs.nextTab');
    this.commitModifier = chord.ctrlKey ? 'Control'
      : chord.altKey ? 'Alt'
      : chord.shiftKey ? 'Shift'
      : 'Control'; // fallback

    // Build entries in MRU order, falling back to tab order for terminals not in history
    const ordered = this.buildMruList(accessHistory, wsTerminals);

    this.entries = ordered.map(t => ({
      terminalId: t.id,
      name: t.userRenamed ? t.name : (t.oscTitle || t.name),
      processName: t.processName,
      workspaceName: workspace?.name ?? '',
    }));

    // Start at index 1 (second most recent = the one you were on before current)
    // for forward, or at the last entry for reverse
    if (reverse) {
      this.selectedIndex = this.entries.length - 1;
    } else {
      this.selectedIndex = Math.min(1, this.entries.length - 1);
    }

    this.render();
    this.overlay.style.display = '';
    this.visible = true;
    this.attachListeners();
  }

  hide(): void {
    this.overlay.style.display = 'none';
    this.visible = false;
    this.detachListeners();
  }

  /** Commit the current selection and hide. */
  commit(): void {
    if (this.entries.length > 0 && this.selectedIndex >= 0) {
      const entry = this.entries[this.selectedIndex];
      store.setActiveTerminal(entry.terminalId);
    }
    this.hide();
  }

  /** Cycle to the next entry. */
  cycleNext(): void {
    if (this.entries.length === 0) return;
    this.selectedIndex = (this.selectedIndex + 1) % this.entries.length;
    this.render();
  }

  /** Cycle to the previous entry. */
  cyclePrev(): void {
    if (this.entries.length === 0) return;
    this.selectedIndex = (this.selectedIndex - 1 + this.entries.length) % this.entries.length;
    this.render();
  }

  private buildMruList(accessHistory: string[], wsTerminals: Terminal[]): Terminal[] {
    const termMap = new Map(wsTerminals.map(t => [t.id, t]));
    const ordered: Terminal[] = [];
    const seen = new Set<string>();

    // First: terminals from access history (MRU order)
    for (const id of accessHistory) {
      const t = termMap.get(id);
      if (t && !seen.has(id)) {
        ordered.push(t);
        seen.add(id);
      }
    }

    // Then: any remaining terminals in tab order
    for (const t of wsTerminals) {
      if (!seen.has(t.id)) {
        ordered.push(t);
        seen.add(t.id);
      }
    }

    return ordered;
  }

  private render(): void {
    this.list.textContent = '';

    for (let i = 0; i < this.entries.length; i++) {
      const entry = this.entries[i];
      const row = document.createElement('div');
      row.className = 'recency-switcher-item' + (i === this.selectedIndex ? ' selected' : '');

      const name = document.createElement('span');
      name.className = 'recency-switcher-item-name';
      name.textContent = entry.name;
      row.appendChild(name);

      const process = document.createElement('span');
      process.className = 'recency-switcher-item-process';
      process.textContent = entry.processName;
      row.appendChild(process);

      this.list.appendChild(row);
    }
  }

  private attachListeners(): void {
    // Read the actual chords for tab cycling
    const nextChord = keybindingStore.getBinding('tabs.nextTab');
    const prevChord = keybindingStore.getBinding('tabs.previousTab');
    const nextStr = chordToString(nextChord);
    const prevStr = chordToString(prevChord);

    // Capture keydown to intercept cycle keys
    this.keydownHandler = (e: KeyboardEvent) => {
      if (!this.visible) return;

      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopImmediatePropagation();
        this.hide();
        return;
      }

      // Match against the actual keybindings for cycling
      const chord = eventToChord(e);
      const str = chordToString(chord);
      if (str === nextStr) {
        e.preventDefault();
        e.stopImmediatePropagation();
        this.cycleNext();
      } else if (str === prevStr) {
        e.preventDefault();
        e.stopImmediatePropagation();
        this.cyclePrev();
      }
    };

    // Commit selection when the primary modifier key is released
    this.keyupHandler = (e: KeyboardEvent) => {
      if (!this.visible) return;
      if (e.key === this.commitModifier) {
        e.preventDefault();
        this.commit();
      }
    };

    document.addEventListener('keydown', this.keydownHandler, true);
    document.addEventListener('keyup', this.keyupHandler, true);
  }

  private detachListeners(): void {
    if (this.keydownHandler) {
      document.removeEventListener('keydown', this.keydownHandler, true);
      this.keydownHandler = null;
    }
    if (this.keyupHandler) {
      document.removeEventListener('keyup', this.keyupHandler, true);
      this.keyupHandler = null;
    }
  }
}
