import {
  keybindingStore,
  DEFAULT_SHORTCUTS,
  formatChord,
  eventToChord,
  type ActionId,
} from '../../state/keybinding-store';
import type { SettingsTabProvider, SettingsDialogContext } from './types';

export class ShortcutsTab implements SettingsTabProvider {
  id = 'shortcuts';
  label = 'Shortcuts';

  private capturingBadge: HTMLElement | null = null;
  private capturingAction: ActionId | null = null;
  private captureHandler: ((e: KeyboardEvent) => void) | null = null;
  private shortcutsContainer: HTMLDivElement | null = null;
  private shortcutSearchQuery = '';

  buildContent(_dialog: SettingsDialogContext): HTMLDivElement {
    const content = document.createElement('div');
    content.className = 'settings-tab-content';

    // Keyboard Shortcuts header with Reset All button
    const kbHeader = document.createElement('div');
    kbHeader.className = 'settings-header';

    const kbTitle = document.createElement('div');
    kbTitle.className = 'settings-section-title';
    kbTitle.textContent = 'Keyboard Shortcuts';
    kbHeader.appendChild(kbTitle);

    const resetAllBtn = document.createElement('button');
    resetAllBtn.className = 'dialog-btn dialog-btn-secondary';
    resetAllBtn.textContent = 'Reset All';
    resetAllBtn.onclick = () => {
      keybindingStore.resetAll();
      this.renderShortcuts();
    };
    kbHeader.appendChild(resetAllBtn);

    content.appendChild(kbHeader);

    // Search input for filtering shortcuts
    const shortcutSearchInput = document.createElement('input');
    shortcutSearchInput.type = 'text';
    shortcutSearchInput.className = 'notification-preset shortcut-search';
    shortcutSearchInput.placeholder = 'Filter shortcuts...';
    shortcutSearchInput.oninput = () => {
      this.shortcutSearchQuery = shortcutSearchInput.value.toLowerCase();
      this.renderShortcuts();
    };
    content.appendChild(shortcutSearchInput);

    // Shortcuts container
    this.shortcutsContainer = document.createElement('div');
    this.shortcutsContainer.className = 'settings-shortcuts';
    content.appendChild(this.shortcutsContainer);

    this.renderShortcuts();

    return content;
  }

  onDialogClose(): void {
    this.stopCapture();
  }

  isCapturing(): boolean {
    return this.capturingBadge !== null;
  }

  private stopCapture(): void {
    if (this.capturingBadge) {
      this.capturingBadge.classList.remove('capturing');
      this.capturingBadge.textContent = formatChord(
        keybindingStore.getBinding(this.capturingAction!)
      );
    }
    if (this.captureHandler) {
      document.removeEventListener('keydown', this.captureHandler, true);
      this.captureHandler = null;
    }
    this.capturingBadge = null;
    this.capturingAction = null;
  }

  private startCapture(badge: HTMLElement, actionId: ActionId): void {
    this.stopCapture();

    this.capturingBadge = badge;
    this.capturingAction = actionId;
    badge.classList.add('capturing');
    badge.textContent = 'Press a key...';

    this.captureHandler = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopImmediatePropagation();

      if (e.key === 'Escape') {
        this.stopCapture();
        return;
      }

      if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) {
        return;
      }

      const chord = eventToChord(e);

      if (!chord.ctrlKey && !chord.altKey) {
        return;
      }

      const conflict = keybindingStore.findConflict(chord, actionId);
      if (conflict) {
        const conflictDef = DEFAULT_SHORTCUTS.find((d) => d.id === conflict);
        const conflictLabel = conflictDef?.label ?? conflict;
        const proceed = confirm(
          `"${formatChord(chord)}" is already bound to "${conflictLabel}".\n\nOverwrite? The conflicting shortcut will be reset to its default.`
        );
        if (!proceed) {
          this.stopCapture();
          return;
        }
        keybindingStore.resetBinding(conflict);
      }

      keybindingStore.setBinding(actionId, chord);
      this.stopCapture();
      this.renderShortcuts();
    };

    document.addEventListener('keydown', this.captureHandler, true);
  }

  private renderShortcuts(): void {
    const container = this.shortcutsContainer;
    if (!container) return;
    container.textContent = '';

    const categories = [...new Set(DEFAULT_SHORTCUTS.map(d => d.category))];

    for (const category of categories) {
      const defs = DEFAULT_SHORTCUTS.filter((d) => {
        if (d.category !== category) return false;
        if (!this.shortcutSearchQuery) return true;
        const chord = formatChord(keybindingStore.getBinding(d.id));
        return d.label.toLowerCase().includes(this.shortcutSearchQuery)
          || chord.toLowerCase().includes(this.shortcutSearchQuery);
      });
      if (defs.length === 0) continue;

      const section = document.createElement('div');
      section.className = 'settings-section';

      const sectionTitle = document.createElement('div');
      sectionTitle.className = 'settings-section-title';
      sectionTitle.textContent = category;
      section.appendChild(sectionTitle);

      for (const def of defs) {
        const row = document.createElement('div');
        row.className = 'shortcut-row';

        const label = document.createElement('span');
        label.className = 'shortcut-label';
        label.textContent = def.label;
        row.appendChild(label);

        const badge = document.createElement('span');
        badge.className = 'shortcut-binding';
        if (keybindingStore.isCustom(def.id)) {
          badge.classList.add('custom');
        }
        badge.textContent = formatChord(keybindingStore.getBinding(def.id));
        badge.onclick = () => this.startCapture(badge, def.id);
        row.appendChild(badge);

        const resetBtn = document.createElement('button');
        resetBtn.className = 'shortcut-reset';
        resetBtn.textContent = 'Reset';
        resetBtn.title = `Reset to ${formatChord(def.defaultChord)}`;
        resetBtn.onclick = () => {
          keybindingStore.resetBinding(def.id);
          this.renderShortcuts();
        };
        row.appendChild(resetBtn);

        section.appendChild(row);
      }

      container.appendChild(section);
    }
  }
}
