import { invoke } from '@tauri-apps/api/core';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { notificationStore } from '../../state/notification-store';
import { playNotificationSound, type SoundPreset } from '../../services/notification-sound';
import type { SettingsTabProvider, SettingsDialogContext } from './types';

function formatCustomSoundName(filename: string): string {
  const name = filename.replace(/\.[^.]+$/, '');
  return name
    .replace(/[_-]/g, ' ')
    .replace(/\b\w/g, c => c.toUpperCase());
}

export class NotificationsTab implements SettingsTabProvider {
  id = 'notifications';
  label = 'Notifications';

  buildContent(_dialog: SettingsDialogContext): HTMLDivElement {
    const content = document.createElement('div');
    content.className = 'settings-tab-content';

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
    }).catch(() => {});

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
    const customSoundsInfo = document.createElement('span');
    customSoundsInfo.className = 'shell-info-icon';
    customSoundsInfo.textContent = '\u24D8';
    customSoundsInfo.title =
      'Drop .wav, .mp3, or .ogg files into this folder to add them as notification sound options.';
    folderLabel.appendChild(customSoundsInfo);
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

    // Idle activity notifications toggle
    const idleRow = document.createElement('div');
    idleRow.className = 'shortcut-row';
    const idleLabel = document.createElement('span');
    idleLabel.className = 'shortcut-label';
    idleLabel.textContent = 'Idle activity notifications';
    const idleInfo = document.createElement('span');
    idleInfo.className = 'shell-info-icon';
    idleInfo.textContent = '\u24D8';
    idleInfo.title =
      'Plays a sound when a background terminal stops producing output (e.g. a long build finishes or a CLI tool is waiting for input). Only triggers for terminals you are not currently looking at.';
    idleLabel.appendChild(idleInfo);
    idleRow.appendChild(idleLabel);
    const idleCheckbox = document.createElement('input');
    idleCheckbox.type = 'checkbox';
    idleCheckbox.className = 'notification-checkbox';
    idleCheckbox.checked = notificationStore.getSettings().idleNotifyEnabled;
    idleCheckbox.onchange = () => {
      notificationStore.setIdleNotifyEnabled(idleCheckbox.checked);
    };
    idleRow.appendChild(idleCheckbox);
    notifSection.appendChild(idleRow);

    content.appendChild(notifSection);

    // ── Auto-Mute Workspace Patterns section ──────────────────────
    const muteSection = document.createElement('div');
    muteSection.className = 'settings-section';

    const muteTitle = document.createElement('div');
    muteTitle.className = 'settings-section-title';
    muteTitle.textContent = 'Auto-Mute Workspace Patterns';
    muteSection.appendChild(muteTitle);

    const muteDesc = document.createElement('div');
    muteDesc.className = 'settings-description';
    muteDesc.textContent = 'Workspaces matching these patterns will have notifications automatically disabled. Use * as a wildcard.';
    muteSection.appendChild(muteDesc);

    const patternListContainer = document.createElement('div');
    patternListContainer.className = 'mute-pattern-list';
    muteSection.appendChild(patternListContainer);

    function renderPatternList() {
      patternListContainer.textContent = '';
      const patterns = notificationStore.getMutedPatterns();
      for (const pattern of patterns) {
        const row = document.createElement('div');
        row.className = 'shortcut-row';

        const label = document.createElement('span');
        label.className = 'shortcut-label';
        label.style.fontFamily = "'Cascadia Code', Consolas, monospace";
        label.textContent = pattern;
        row.appendChild(label);

        const removeBtn = document.createElement('button');
        removeBtn.className = 'dialog-btn dialog-btn-secondary';
        removeBtn.textContent = 'Remove';
        removeBtn.style.fontSize = '11px';
        removeBtn.style.padding = '2px 10px';
        removeBtn.onclick = () => {
          notificationStore.removeMutedPattern(pattern);
          renderPatternList();
        };
        row.appendChild(removeBtn);

        patternListContainer.appendChild(row);
      }
    }

    renderPatternList();

    // Add pattern row
    const addPatternRow = document.createElement('div');
    addPatternRow.className = 'shortcut-row';

    const patternInput = document.createElement('input');
    patternInput.type = 'text';
    patternInput.className = 'notification-preset';
    patternInput.placeholder = 'e.g. Agent *';
    patternInput.style.flex = '1';
    addPatternRow.appendChild(patternInput);

    const addPatternBtn = document.createElement('button');
    addPatternBtn.className = 'dialog-btn dialog-btn-primary';
    addPatternBtn.textContent = 'Add';
    addPatternBtn.style.fontSize = '11px';
    addPatternBtn.style.padding = '2px 14px';
    addPatternBtn.onclick = () => {
      const val = patternInput.value.trim();
      if (val) {
        notificationStore.addMutedPattern(val);
        patternInput.value = '';
        renderPatternList();
      }
    };
    addPatternRow.appendChild(addPatternBtn);

    patternInput.onkeydown = (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        addPatternBtn.click();
      }
    };

    muteSection.appendChild(addPatternRow);
    content.appendChild(muteSection);

    return content;
  }
}
