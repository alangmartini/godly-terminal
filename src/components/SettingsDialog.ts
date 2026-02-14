import { invoke } from '@tauri-apps/api/core';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import {
  keybindingStore,
  DEFAULT_SHORTCUTS,
  formatChord,
  eventToChord,
  type ActionId,
  type ShortcutCategory,
} from '../state/keybinding-store';
import { notificationStore } from '../state/notification-store';
import { playNotificationSound, type SoundPreset } from '../services/notification-sound';

function formatCustomSoundName(filename: string): string {
  // Strip extension
  const name = filename.replace(/\.[^.]+$/, '');
  // Replace _ and - with spaces, then title-case
  return name
    .replace(/[_-]/g, ' ')
    .replace(/\b\w/g, c => c.toUpperCase());
}

/**
 * Show the settings dialog for customising keyboard shortcuts.
 * Returns a promise that resolves when the dialog is closed.
 */
export function showSettingsDialog(): Promise<void> {
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const dialog = document.createElement('div');
    dialog.className = 'dialog settings-dialog';

    // ── Header ──────────────────────────────────────────────────
    const header = document.createElement('div');
    header.className = 'settings-header';

    const title = document.createElement('div');
    title.className = 'dialog-title';
    title.textContent = 'Settings';
    header.appendChild(title);

    dialog.appendChild(header);

    // ── Notifications section ───────────────────────────────────
    const notifSection = document.createElement('div');
    notifSection.className = 'settings-section';

    const notifTitle = document.createElement('div');
    notifTitle.className = 'settings-section-title';
    notifTitle.textContent = 'Notifications';
    notifSection.appendChild(notifTitle);

    // Enable/disable row
    const enableRow = document.createElement('div');
    enableRow.className = 'shortcut-row';
    const enableLabel = document.createElement('span');
    enableLabel.className = 'shortcut-label';
    enableLabel.textContent = 'Sound notifications';
    enableRow.appendChild(enableLabel);
    const enableCheckbox = document.createElement('input');
    enableCheckbox.type = 'checkbox';
    enableCheckbox.className = 'notification-checkbox';
    enableCheckbox.checked = notificationStore.getSettings().globalEnabled;
    enableCheckbox.onchange = () => {
      notificationStore.setGlobalEnabled(enableCheckbox.checked);
    };
    enableRow.appendChild(enableCheckbox);
    notifSection.appendChild(enableRow);

    // Volume row
    const volumeRow = document.createElement('div');
    volumeRow.className = 'shortcut-row';
    const volumeLabel = document.createElement('span');
    volumeLabel.className = 'shortcut-label';
    volumeLabel.textContent = 'Volume';
    volumeRow.appendChild(volumeLabel);
    const volumeSlider = document.createElement('input');
    volumeSlider.type = 'range';
    volumeSlider.className = 'notification-volume';
    volumeSlider.min = '0';
    volumeSlider.max = '100';
    volumeSlider.value = String(Math.round(notificationStore.getSettings().volume * 100));
    volumeSlider.oninput = () => {
      notificationStore.setVolume(parseInt(volumeSlider.value) / 100);
    };
    volumeRow.appendChild(volumeSlider);
    notifSection.appendChild(volumeRow);

    // Sound preset row
    const presetRow = document.createElement('div');
    presetRow.className = 'shortcut-row';
    const presetLabel = document.createElement('span');
    presetLabel.className = 'shortcut-label';
    presetLabel.textContent = 'Sound';
    presetRow.appendChild(presetLabel);
    const presetSelect = document.createElement('select');
    presetSelect.className = 'notification-preset';

    // Built-in presets
    const builtinGroup = document.createElement('optgroup');
    builtinGroup.label = 'Built-in Sounds';
    const presets: { value: SoundPreset; label: string }[] = [
      { value: 'chime', label: 'Chime' },
      { value: 'bell', label: 'Bell' },
      { value: 'ping', label: 'Ping' },
      { value: 'soft-rise', label: 'Soft Rise' },
      { value: 'crystal', label: 'Crystal' },
      { value: 'bubble', label: 'Bubble' },
      { value: 'harp', label: 'Harp' },
      { value: 'marimba', label: 'Marimba' },
      { value: 'cosmic', label: 'Cosmic' },
      { value: 'droplet', label: 'Droplet' },
    ];
    presets.forEach(p => {
      const opt = document.createElement('option');
      opt.value = p.value;
      opt.textContent = p.label;
      if (p.value === notificationStore.getSettings().soundPreset) opt.selected = true;
      builtinGroup.appendChild(opt);
    });
    presetSelect.appendChild(builtinGroup);

    // Load custom sounds and populate dropdown
    const customGroup = document.createElement('optgroup');
    customGroup.label = 'Custom Sounds';
    invoke<string[]>('list_custom_sounds').then(files => {
      if (files.length === 0) return;
      const currentPreset = notificationStore.getSettings().soundPreset;
      files.forEach(filename => {
        const opt = document.createElement('option');
        opt.value = `custom:${filename}`;
        opt.textContent = formatCustomSoundName(filename);
        if (`custom:${filename}` === currentPreset) opt.selected = true;
        customGroup.appendChild(opt);
      });
      presetSelect.appendChild(customGroup);
    }).catch(() => {
      // Custom sounds unavailable — silently omit the group
    });

    presetSelect.onchange = () => {
      const selected = presetSelect.value as SoundPreset;
      notificationStore.setSoundPreset(selected);
      playNotificationSound(selected, notificationStore.getSettings().volume);
    };
    presetRow.appendChild(presetSelect);

    const testBtn = document.createElement('button');
    testBtn.className = 'dialog-btn dialog-btn-secondary';
    testBtn.textContent = 'Test';
    testBtn.style.marginLeft = '8px';
    testBtn.onclick = () => {
      const s = notificationStore.getSettings();
      playNotificationSound(s.soundPreset, s.volume);
    };
    presetRow.appendChild(testBtn);
    notifSection.appendChild(presetRow);

    // Custom sounds folder row
    const folderRow = document.createElement('div');
    folderRow.className = 'shortcut-row';
    const folderLabel = document.createElement('span');
    folderLabel.className = 'shortcut-label';
    folderLabel.textContent = 'Custom sounds';
    folderRow.appendChild(folderLabel);
    const openFolderBtn = document.createElement('button');
    openFolderBtn.className = 'dialog-btn dialog-btn-secondary';
    openFolderBtn.textContent = 'Open Sounds Folder';
    openFolderBtn.onclick = async () => {
      try {
        const dir: string = await invoke('get_sounds_dir');
        await revealItemInDir(dir);
      } catch (e) {
        console.warn('Failed to open sounds folder:', e);
      }
    };
    folderRow.appendChild(openFolderBtn);
    notifSection.appendChild(folderRow);

    dialog.appendChild(notifSection);

    // ── Keyboard Shortcuts header ────────────────────────────────
    const kbHeader = document.createElement('div');
    kbHeader.className = 'settings-header';
    kbHeader.style.marginTop = '16px';

    const kbTitle = document.createElement('div');
    kbTitle.className = 'settings-section-title';
    kbTitle.textContent = 'Keyboard Shortcuts';
    kbHeader.appendChild(kbTitle);

    const resetAllBtn = document.createElement('button');
    resetAllBtn.className = 'dialog-btn dialog-btn-secondary';
    resetAllBtn.textContent = 'Reset All';
    resetAllBtn.onclick = () => {
      keybindingStore.resetAll();
      renderShortcuts();
    };
    kbHeader.appendChild(resetAllBtn);

    dialog.appendChild(kbHeader);

    // ── Shortcuts container ─────────────────────────────────────
    const shortcutsContainer = document.createElement('div');
    shortcutsContainer.className = 'settings-shortcuts';
    dialog.appendChild(shortcutsContainer);

    // Currently capturing element (if any)
    let capturingBadge: HTMLElement | null = null;
    let capturingAction: ActionId | null = null;
    let captureHandler: ((e: KeyboardEvent) => void) | null = null;

    function stopCapture() {
      if (capturingBadge) {
        capturingBadge.classList.remove('capturing');
        capturingBadge.textContent = formatChord(
          keybindingStore.getBinding(capturingAction!)
        );
      }
      if (captureHandler) {
        document.removeEventListener('keydown', captureHandler, true);
        captureHandler = null;
      }
      capturingBadge = null;
      capturingAction = null;
    }

    function startCapture(badge: HTMLElement, actionId: ActionId) {
      // Stop any existing capture first
      stopCapture();

      capturingBadge = badge;
      capturingAction = actionId;
      badge.classList.add('capturing');
      badge.textContent = 'Press a key...';

      captureHandler = (e: KeyboardEvent) => {
        e.preventDefault();
        e.stopImmediatePropagation();

        // Escape cancels capture
        if (e.key === 'Escape') {
          stopCapture();
          return;
        }

        // Ignore bare modifier keys
        if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) {
          return;
        }

        const chord = eventToChord(e);

        // Must have at least Ctrl or Alt modifier
        if (!chord.ctrlKey && !chord.altKey) {
          return;
        }

        // Check for conflicts
        const conflict = keybindingStore.findConflict(chord, actionId);
        if (conflict) {
          const conflictDef = DEFAULT_SHORTCUTS.find((d) => d.id === conflict);
          const conflictLabel = conflictDef?.label ?? conflict;
          const proceed = confirm(
            `"${formatChord(chord)}" is already bound to "${conflictLabel}".\n\nOverwrite? The conflicting shortcut will be reset to its default.`
          );
          if (!proceed) {
            stopCapture();
            return;
          }
          // Reset the conflicting binding to its default
          keybindingStore.resetBinding(conflict);
        }

        keybindingStore.setBinding(actionId, chord);
        stopCapture();
        renderShortcuts();
      };

      document.addEventListener('keydown', captureHandler, true);
    }

    function renderShortcuts() {
      shortcutsContainer.textContent = '';

      const categories: ShortcutCategory[] = ['Terminal', 'Clipboard', 'Tabs', 'Split', 'Workspace'];

      for (const category of categories) {
        const defs = DEFAULT_SHORTCUTS.filter((d) => d.category === category);
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
          badge.onclick = () => startCapture(badge, def.id);
          row.appendChild(badge);

          const resetBtn = document.createElement('button');
          resetBtn.className = 'shortcut-reset';
          resetBtn.textContent = 'Reset';
          resetBtn.title = `Reset to ${formatChord(def.defaultChord)}`;
          resetBtn.onclick = () => {
            keybindingStore.resetBinding(def.id);
            renderShortcuts();
          };
          row.appendChild(resetBtn);

          section.appendChild(row);
        }

        shortcutsContainer.appendChild(section);
      }
    }

    renderShortcuts();

    // ── Close handling ──────────────────────────────────────────
    const close = () => {
      stopCapture();
      overlay.remove();
      resolve();
    };

    overlay.onclick = (e) => {
      if (e.target === overlay) close();
    };

    const escHandler = (e: KeyboardEvent) => {
      // Only close on Escape when not capturing
      if (e.key === 'Escape' && !capturingBadge) {
        close();
        document.removeEventListener('keydown', escHandler);
      }
    };
    document.addEventListener('keydown', escHandler);

    overlay.appendChild(dialog);
    document.body.appendChild(overlay);
  });
}
