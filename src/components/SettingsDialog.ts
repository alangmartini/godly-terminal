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
import { terminalSettingsStore } from '../state/terminal-settings-store';
import { workspaceService } from '../services/workspace-service';
import { playNotificationSound, type SoundPreset } from '../services/notification-sound';
import { getRendererBackend } from './TerminalRenderer';
import { themeStore } from '../state/theme-store';
import type { ThemeDefinition } from '../themes/types';
import { createThemePreview } from './ThemePreview';
import type { ShellType } from '../state/store';

function formatCustomSoundName(filename: string): string {
  // Strip extension
  const name = filename.replace(/\.[^.]+$/, '');
  // Replace _ and - with spaces, then title-case
  return name
    .replace(/[_-]/g, ' ')
    .replace(/\b\w/g, c => c.toUpperCase());
}

/**
 * Show the settings dialog for customising themes, notifications, and
 * keyboard shortcuts. Returns a promise that resolves when the dialog
 * is closed.
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

    // ── Tab bar ──────────────────────────────────────────────────
    let activeTab = 'themes';

    const tabBar = document.createElement('div');
    tabBar.className = 'settings-tabs';

    const tabs: { id: string; label: string }[] = [
      { id: 'themes', label: 'Themes' },
      { id: 'terminal', label: 'Terminal' },
      { id: 'notifications', label: 'Notifications' },
      { id: 'shortcuts', label: 'Shortcuts' },
    ];

    const tabButtons: Record<string, HTMLButtonElement> = {};
    const tabContents: Record<string, HTMLDivElement> = {};

    for (const tab of tabs) {
      const btn = document.createElement('button');
      btn.className = 'settings-tab' + (tab.id === activeTab ? ' active' : '');
      btn.textContent = tab.label;
      btn.onclick = () => switchTab(tab.id);
      tabBar.appendChild(btn);
      tabButtons[tab.id] = btn;
    }

    dialog.appendChild(tabBar);

    function switchTab(tabId: string) {
      activeTab = tabId;
      for (const id of Object.keys(tabButtons)) {
        tabButtons[id].className = 'settings-tab' + (id === tabId ? ' active' : '');
        tabContents[id].className = 'settings-tab-content' + (id === tabId ? ' active' : '');
      }
    }

    // ── Themes tab content ──────────────────────────────────────
    const themesContent = document.createElement('div');
    themesContent.className = 'settings-tab-content active';
    tabContents['themes'] = themesContent;

    const themeGrid = document.createElement('div');
    themeGrid.className = 'theme-grid';
    themesContent.appendChild(themeGrid);

    function renderThemeGrid() {
      themeGrid.textContent = '';
      const allThemes = themeStore.getAllThemes();
      const activeTheme = themeStore.getActiveTheme();

      for (const theme of allThemes) {
        const card = document.createElement('div');
        card.className = 'theme-card' + (theme.id === activeTheme.id ? ' active' : '');

        // Canvas preview
        const preview = createThemePreview(theme, 280, 140);
        card.appendChild(preview);

        // Info area
        const info = document.createElement('div');
        info.className = 'theme-card-info';

        const nameEl = document.createElement('div');
        nameEl.className = 'theme-card-name';
        nameEl.textContent = theme.name;
        info.appendChild(nameEl);

        const descEl = document.createElement('div');
        descEl.className = 'theme-card-description';
        descEl.textContent = theme.description;
        info.appendChild(descEl);

        const authorEl = document.createElement('div');
        authorEl.className = 'theme-card-author';
        authorEl.textContent = theme.author;
        info.appendChild(authorEl);

        card.appendChild(info);

        // Remove button for non-builtin themes
        if (!theme.builtin) {
          const actions = document.createElement('div');
          actions.className = 'theme-card-actions';

          const removeBtn = document.createElement('button');
          removeBtn.className = 'dialog-btn dialog-btn-secondary';
          removeBtn.textContent = 'Remove';
          removeBtn.style.fontSize = '11px';
          removeBtn.style.padding = '2px 10px';
          removeBtn.onclick = (e) => {
            e.stopPropagation();
            themeStore.removeCustomTheme(theme.id);
            renderThemeGrid();
          };
          actions.appendChild(removeBtn);
          card.appendChild(actions);
        }

        card.onclick = () => {
          themeStore.setActiveTheme(theme.id);
          renderThemeGrid();
        };

        themeGrid.appendChild(card);
      }
    }

    renderThemeGrid();

    // Import button
    const importBtn = document.createElement('button');
    importBtn.className = 'dialog-btn dialog-btn-secondary theme-import-btn';
    importBtn.textContent = 'Import Theme (JSON)';
    importBtn.onclick = () => {
      const fileInput = document.createElement('input');
      fileInput.type = 'file';
      fileInput.accept = '.json';
      fileInput.style.display = 'none';
      fileInput.onchange = async () => {
        const file = fileInput.files?.[0];
        if (!file) return;
        try {
          const text = await file.text();
          const parsed = JSON.parse(text) as ThemeDefinition;
          // Validate required fields
          if (
            !parsed.id ||
            !parsed.name ||
            !parsed.terminal ||
            !parsed.ui
          ) {
            alert('Invalid theme file: missing required fields (id, name, terminal, ui).');
            return;
          }
          parsed.builtin = false;
          themeStore.addCustomTheme(parsed);
          renderThemeGrid();
        } catch (err) {
          alert('Failed to import theme: ' + (err instanceof Error ? err.message : String(err)));
        }
        fileInput.remove();
      };
      document.body.appendChild(fileInput);
      fileInput.click();
    };
    themesContent.appendChild(importBtn);

    dialog.appendChild(themesContent);

    // ── Terminal tab content ─────────────────────────────────────
    const terminalContent = document.createElement('div');
    terminalContent.className = 'settings-tab-content';
    tabContents['terminal'] = terminalContent;

    const termSection = document.createElement('div');
    termSection.className = 'settings-section';

    const termTitle = document.createElement('div');
    termTitle.className = 'settings-section-title';
    termTitle.textContent = 'Default Shell';
    termSection.appendChild(termTitle);

    const termDesc = document.createElement('div');
    termDesc.className = 'settings-description';
    termDesc.textContent = 'Choose which shell to use when creating new terminals and workspaces.';
    termSection.appendChild(termDesc);

    interface ShellOption {
      id: string;
      label: string;
      tooltip: string;
      shellType: ShellType;
      checkAvailability?: () => Promise<boolean>;
    }

    const shellOptions: ShellOption[] = [
      {
        id: 'windows',
        label: 'PowerShell',
        tooltip: 'Built-in on all Windows. Rich scripting, but slower startup (~1-2s).',
        shellType: { type: 'windows' },
      },
      {
        id: 'pwsh',
        label: 'PowerShell 7',
        tooltip: 'Modern, faster startup, cross-platform. Must be installed separately.',
        shellType: { type: 'pwsh' },
        checkAvailability: () => invoke<boolean>('is_pwsh_available'),
      },
      {
        id: 'cmd',
        label: 'Command Prompt',
        tooltip: 'Fastest startup. Lightweight but limited scripting capabilities.',
        shellType: { type: 'cmd' },
      },
      {
        id: 'wsl',
        label: 'WSL',
        tooltip: 'Full Linux environment. Requires WSL to be installed.',
        shellType: { type: 'wsl' },
        checkAvailability: () => workspaceService.isWslAvailable(),
      },
    ];

    const currentDefault = terminalSettingsStore.getDefaultShell();
    let selectedShellId = currentDefault.type === 'windows' ? 'windows' : currentDefault.type;

    const radioGroup = document.createElement('div');
    radioGroup.className = 'shell-radio-group';

    // WSL distribution dropdown (hidden unless WSL is selected)
    const wslDistroRow = document.createElement('div');
    wslDistroRow.className = 'shell-wsl-distro-row';
    wslDistroRow.style.display = 'none';

    const wslDistroLabel = document.createElement('span');
    wslDistroLabel.className = 'shortcut-label';
    wslDistroLabel.textContent = 'Distribution';
    wslDistroRow.appendChild(wslDistroLabel);

    const wslDistroSelect = document.createElement('select');
    wslDistroSelect.className = 'notification-preset';
    wslDistroRow.appendChild(wslDistroSelect);

    // Track availability per option
    const optionAvailability: Record<string, boolean> = {};
    const optionElements: Record<string, HTMLDivElement> = {};

    for (const opt of shellOptions) {
      const row = document.createElement('div');
      row.className = 'shell-option-row';

      const radio = document.createElement('input');
      radio.type = 'radio';
      radio.name = 'default-shell';
      radio.id = `shell-${opt.id}`;
      radio.value = opt.id;
      radio.checked = selectedShellId === opt.id;
      row.appendChild(radio);

      const label = document.createElement('label');
      label.htmlFor = `shell-${opt.id}`;
      label.className = 'shell-option-label';

      const nameSpan = document.createElement('span');
      nameSpan.className = 'shell-option-name';
      nameSpan.textContent = opt.label;
      label.appendChild(nameSpan);

      // Info tooltip icon
      const infoIcon = document.createElement('span');
      infoIcon.className = 'shell-info-icon';
      infoIcon.textContent = '\u24D8'; // circled i
      infoIcon.title = opt.tooltip;
      label.appendChild(infoIcon);

      // "(not installed)" label placeholder
      const unavailableLabel = document.createElement('span');
      unavailableLabel.className = 'shell-unavailable-label';
      unavailableLabel.style.display = 'none';
      unavailableLabel.textContent = '(not installed)';
      label.appendChild(unavailableLabel);

      row.appendChild(label);

      radio.onchange = () => {
        if (!radio.checked) return;
        selectedShellId = opt.id;

        if (opt.id === 'wsl') {
          wslDistroRow.style.display = 'flex';
          const distro = wslDistroSelect.value || undefined;
          terminalSettingsStore.setDefaultShell({ type: 'wsl', distribution: distro });
        } else {
          wslDistroRow.style.display = 'none';
          terminalSettingsStore.setDefaultShell(opt.shellType);
        }
      };

      radioGroup.appendChild(row);
      optionElements[opt.id] = row;
    }

    termSection.appendChild(radioGroup);
    termSection.appendChild(wslDistroRow);

    // WSL distro select change handler
    wslDistroSelect.onchange = () => {
      if (selectedShellId === 'wsl') {
        const distro = wslDistroSelect.value || undefined;
        terminalSettingsStore.setDefaultShell({ type: 'wsl', distribution: distro });
      }
    };

    // Check availability asynchronously and disable unavailable options
    for (const opt of shellOptions) {
      if (!opt.checkAvailability) {
        optionAvailability[opt.id] = true;
        continue;
      }
      opt.checkAvailability().then(available => {
        optionAvailability[opt.id] = available;
        if (!available) {
          const row = optionElements[opt.id];
          const radio = row.querySelector('input') as HTMLInputElement;
          radio.disabled = true;
          row.classList.add('disabled');
          const unavailableSpan = row.querySelector('.shell-unavailable-label') as HTMLElement;
          if (unavailableSpan) unavailableSpan.style.display = 'inline';
        }
      }).catch(() => {
        // Treat check failure as available
        optionAvailability[opt.id] = true;
      });
    }

    // If WSL is currently selected, show the distro dropdown and load distributions
    if (currentDefault.type === 'wsl') {
      wslDistroRow.style.display = 'flex';
      workspaceService.getWslDistributions().then(distros => {
        wslDistroSelect.textContent = '';
        const defaultOpt = document.createElement('option');
        defaultOpt.value = '';
        defaultOpt.textContent = 'Default';
        wslDistroSelect.appendChild(defaultOpt);

        for (const d of distros) {
          const dOpt = document.createElement('option');
          dOpt.value = d;
          dOpt.textContent = d;
          if ((currentDefault as { type: 'wsl'; distribution?: string }).distribution === d) {
            dOpt.selected = true;
          }
          wslDistroSelect.appendChild(dOpt);
        }
      }).catch(() => {});
    } else {
      // Pre-load distros so they're ready if user switches to WSL
      workspaceService.getWslDistributions().then(distros => {
        wslDistroSelect.textContent = '';
        const defaultOpt = document.createElement('option');
        defaultOpt.value = '';
        defaultOpt.textContent = 'Default';
        wslDistroSelect.appendChild(defaultOpt);

        for (const d of distros) {
          const dOpt = document.createElement('option');
          dOpt.value = d;
          dOpt.textContent = d;
          wslDistroSelect.appendChild(dOpt);
        }
      }).catch(() => {});
    }

    terminalContent.appendChild(termSection);
    dialog.appendChild(terminalContent);

    // ── Notifications tab content ───────────────────────────────
    const notifContent = document.createElement('div');
    notifContent.className = 'settings-tab-content';
    tabContents['notifications'] = notifContent;

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

    notifContent.appendChild(notifSection);
    dialog.appendChild(notifContent);

    // ── Shortcuts tab content ───────────────────────────────────
    const shortcutsContent = document.createElement('div');
    shortcutsContent.className = 'settings-tab-content';
    tabContents['shortcuts'] = shortcutsContent;

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
      renderShortcuts();
    };
    kbHeader.appendChild(resetAllBtn);

    shortcutsContent.appendChild(kbHeader);

    // Shortcuts container
    const shortcutsContainer = document.createElement('div');
    shortcutsContainer.className = 'settings-shortcuts';
    shortcutsContent.appendChild(shortcutsContainer);

    dialog.appendChild(shortcutsContent);

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

      const categories: ShortcutCategory[] = ['Terminal', 'Clipboard', 'Tabs', 'Split', 'Workspace', 'Scroll'];

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

    // ── Info footer ──────────────────────────────────────────────
    const footer = document.createElement('div');
    footer.className = 'settings-footer';
    footer.textContent = `Renderer: ${getRendererBackend()}`;
    dialog.appendChild(footer);

    const versionLine = document.createElement('div');
    versionLine.className = 'settings-footer settings-version';
    versionLine.textContent = `Version: ${__APP_VERSION__}`;
    dialog.appendChild(versionLine);

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
