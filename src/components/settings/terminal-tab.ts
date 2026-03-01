import { invoke } from '@tauri-apps/api/core';
import { terminalSettingsStore } from '../../state/terminal-settings-store';
import { workspaceService } from '../../services/workspace-service';
import type { ShellType } from '../../state/store';
import { showFileEditorDialog } from '../FileEditorDialog';
import type { SettingsTabProvider, SettingsDialogContext } from './types';

interface ShellOption {
  id: string;
  label: string;
  tooltip: string;
  shellType: ShellType;
  checkAvailability?: () => Promise<boolean>;
}

export class TerminalTab implements SettingsTabProvider {
  id = 'terminal';
  label = 'Terminal';

  buildContent(_dialog: SettingsDialogContext): HTMLDivElement {
    const content = document.createElement('div');
    content.className = 'settings-tab-content';

    // ── Default Shell section ─────────────────────────────────
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
      {
        id: 'custom',
        label: 'Custom Shell',
        tooltip: 'Run any shell executable (e.g. nu.exe, fish, bash.exe).',
        shellType: { type: 'custom', program: '' },
      },
    ];

    const currentDefault = terminalSettingsStore.getDefaultShell();
    let selectedShellId: string = currentDefault.type;

    const radioGroup = document.createElement('div');
    radioGroup.className = 'shell-radio-group';

    // WSL distribution dropdown (hidden unless WSL is selected)
    const wslDistroRow = document.createElement('div');
    wslDistroRow.className = 'shell-wsl-distro-row';
    wslDistroRow.style.display = 'none';

    // Custom shell inputs (hidden unless Custom is selected)
    const customShellRow = document.createElement('div');
    customShellRow.className = 'shell-wsl-distro-row';
    customShellRow.style.display = 'none';

    const customProgramLabel = document.createElement('span');
    customProgramLabel.className = 'shortcut-label';
    customProgramLabel.textContent = 'Program';
    customShellRow.appendChild(customProgramLabel);

    const customProgramInput = document.createElement('input');
    customProgramInput.type = 'text';
    customProgramInput.className = 'notification-preset';
    customProgramInput.placeholder = 'e.g. nu.exe, fish, bash.exe';
    customProgramInput.style.flex = '1';
    customShellRow.appendChild(customProgramInput);

    const customArgsLabel = document.createElement('span');
    customArgsLabel.className = 'shortcut-label';
    customArgsLabel.textContent = 'Args';
    customArgsLabel.style.marginLeft = '8px';
    customShellRow.appendChild(customArgsLabel);

    const customArgsInput = document.createElement('input');
    customArgsInput.type = 'text';
    customArgsInput.className = 'notification-preset';
    customArgsInput.placeholder = 'e.g. -l --config';
    customArgsInput.style.flex = '1';
    customShellRow.appendChild(customArgsInput);

    // Pre-populate if current default is custom
    if (currentDefault.type === 'custom') {
      const custom = currentDefault as { type: 'custom'; program: string; args?: string[] };
      customProgramInput.value = custom.program;
      customArgsInput.value = (custom.args ?? []).join(' ');
      customShellRow.style.display = 'flex';
    }

    // Custom shell input change handlers
    const updateCustomShell = () => {
      if (selectedShellId === 'custom' && customProgramInput.value.trim()) {
        const args = customArgsInput.value.trim()
          ? customArgsInput.value.trim().split(/\s+/)
          : undefined;
        terminalSettingsStore.setDefaultShell({
          type: 'custom',
          program: customProgramInput.value.trim(),
          args,
        });
      }
    };
    customProgramInput.onchange = updateCustomShell;
    customArgsInput.onchange = updateCustomShell;

    const wslDistroLabel = document.createElement('span');
    wslDistroLabel.className = 'shortcut-label';
    wslDistroLabel.textContent = 'Distribution';
    wslDistroRow.appendChild(wslDistroLabel);

    const wslDistroSelect = document.createElement('select');
    wslDistroSelect.className = 'notification-preset';
    wslDistroRow.appendChild(wslDistroSelect);

    // Track availability per option
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

      const infoIcon = document.createElement('span');
      infoIcon.className = 'shell-info-icon';
      infoIcon.textContent = '\u24D8';
      infoIcon.title = opt.tooltip;
      label.appendChild(infoIcon);

      const unavailableLabel = document.createElement('span');
      unavailableLabel.className = 'shell-unavailable-label';
      unavailableLabel.style.display = 'none';
      unavailableLabel.textContent = '(not installed)';
      label.appendChild(unavailableLabel);

      row.appendChild(label);

      radio.onchange = () => {
        if (!radio.checked) return;
        selectedShellId = opt.id;

        wslDistroRow.style.display = opt.id === 'wsl' ? 'flex' : 'none';
        customShellRow.style.display = opt.id === 'custom' ? 'flex' : 'none';

        if (opt.id === 'wsl') {
          const distro = wslDistroSelect.value || undefined;
          terminalSettingsStore.setDefaultShell({ type: 'wsl', distribution: distro });
        } else if (opt.id === 'custom') {
          if (customProgramInput.value.trim()) {
            const args = customArgsInput.value.trim()
              ? customArgsInput.value.trim().split(/\s+/)
              : undefined;
            terminalSettingsStore.setDefaultShell({
              type: 'custom',
              program: customProgramInput.value.trim(),
              args,
            });
          }
        } else {
          terminalSettingsStore.setDefaultShell(opt.shellType);
        }
      };

      radioGroup.appendChild(row);
      optionElements[opt.id] = row;
    }

    termSection.appendChild(radioGroup);
    termSection.appendChild(wslDistroRow);
    termSection.appendChild(customShellRow);

    // WSL distro select change handler
    wslDistroSelect.onchange = () => {
      if (selectedShellId === 'wsl') {
        const distro = wslDistroSelect.value || undefined;
        terminalSettingsStore.setDefaultShell({ type: 'wsl', distribution: distro });
      }
    };

    // Check availability asynchronously and disable unavailable options
    for (const opt of shellOptions) {
      if (!opt.checkAvailability) continue;
      opt.checkAvailability().then(available => {
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

    content.appendChild(termSection);

    // ── Scrollback section ─────────────────────────────────────
    const scrollSection = document.createElement('div');
    scrollSection.className = 'settings-section';

    const scrollTitle = document.createElement('div');
    scrollTitle.className = 'settings-section-title';
    scrollTitle.textContent = 'Scrollback';
    scrollSection.appendChild(scrollTitle);

    const autoScrollRow = document.createElement('div');
    autoScrollRow.className = 'shortcut-row';
    const autoScrollLabel = document.createElement('span');
    autoScrollLabel.className = 'shortcut-label';
    autoScrollLabel.textContent = 'Auto-scroll to bottom on new output';
    autoScrollRow.appendChild(autoScrollLabel);
    const autoScrollCheckbox = document.createElement('input');
    autoScrollCheckbox.type = 'checkbox';
    autoScrollCheckbox.className = 'notification-checkbox';
    autoScrollCheckbox.checked = terminalSettingsStore.getAutoScrollOnOutput();
    autoScrollCheckbox.onchange = () => {
      terminalSettingsStore.setAutoScrollOnOutput(autoScrollCheckbox.checked);
    };
    autoScrollRow.appendChild(autoScrollCheckbox);
    scrollSection.appendChild(autoScrollRow);

    const autoScrollDesc = document.createElement('div');
    autoScrollDesc.className = 'settings-description';
    autoScrollDesc.textContent = 'When enabled, new terminal output will snap the view to the bottom even while you are scrolled up. When disabled (default), your scroll position is preserved.';
    scrollSection.appendChild(autoScrollDesc);

    content.appendChild(scrollSection);

    // ── Split Tabs section ─────────────────────────────────────
    const splitSection = document.createElement('div');
    splitSection.className = 'settings-section';

    const splitTitle = document.createElement('div');
    splitTitle.className = 'settings-section-title';
    splitTitle.textContent = 'Split Tabs';
    splitSection.appendChild(splitTitle);

    const splitDesc = document.createElement('div');
    splitDesc.className = 'settings-description';
    splitDesc.textContent = 'Choose how terminals in a split view appear in the tab bar.';
    splitSection.appendChild(splitDesc);

    const splitRadioGroup = document.createElement('div');
    splitRadioGroup.className = 'shell-radio-group';

    const currentSplitMode = terminalSettingsStore.getSplitTabMode();

    const splitModeOptions: { id: string; label: string; desc: string }[] = [
      { id: 'individual', label: 'Individual tabs', desc: 'Each terminal in a split gets its own tab.' },
      { id: 'unified', label: 'Unified tab', desc: 'Terminals in a split share a single tab.' },
    ];

    for (const opt of splitModeOptions) {
      const row = document.createElement('div');
      row.className = 'shell-option-row';

      const radio = document.createElement('input');
      radio.type = 'radio';
      radio.name = 'split-tab-mode';
      radio.id = `split-tab-${opt.id}`;
      radio.value = opt.id;
      radio.checked = currentSplitMode === opt.id;
      row.appendChild(radio);

      const label = document.createElement('label');
      label.htmlFor = `split-tab-${opt.id}`;
      label.className = 'shell-option-label';

      const nameSpan = document.createElement('span');
      nameSpan.className = 'shell-option-name';
      nameSpan.textContent = opt.label;
      label.appendChild(nameSpan);

      const infoIcon = document.createElement('span');
      infoIcon.className = 'shell-info-icon';
      infoIcon.textContent = '\u24D8';
      infoIcon.title = opt.desc;
      label.appendChild(infoIcon);

      row.appendChild(label);

      radio.onchange = () => {
        if (radio.checked) {
          terminalSettingsStore.setSplitTabMode(opt.id as 'individual' | 'unified');
        }
      };

      splitRadioGroup.appendChild(row);
    }

    splitSection.appendChild(splitRadioGroup);
    content.appendChild(splitSection);

    // ── CMD Aliases section ────────────────────────────────────
    const aliasSection = document.createElement('div');
    aliasSection.className = 'settings-section';

    const aliasTitle = document.createElement('div');
    aliasTitle.className = 'settings-section-title';
    aliasTitle.textContent = 'CMD Aliases';
    aliasSection.appendChild(aliasTitle);

    const aliasDesc = document.createElement('div');
    aliasDesc.className = 'settings-description';
    aliasDesc.textContent = 'Define command aliases for Command Prompt sessions. Aliases load automatically via the Windows AutoRun registry key.';
    aliasSection.appendChild(aliasDesc);

    const aliasRow = document.createElement('div');
    aliasRow.className = 'shortcut-row';

    const aliasLabel = document.createElement('span');
    aliasLabel.className = 'shortcut-label';
    aliasLabel.textContent = 'Aliases file';
    aliasRow.appendChild(aliasLabel);

    const aliasStatus = document.createElement('span');
    aliasStatus.className = 'shell-info-icon';
    aliasStatus.style.marginLeft = '8px';
    aliasStatus.style.fontSize = '11px';
    aliasStatus.style.opacity = '0.7';

    const editAliasBtn = document.createElement('button');
    editAliasBtn.className = 'dialog-btn dialog-btn-secondary';
    editAliasBtn.textContent = 'Edit Aliases';
    editAliasBtn.onclick = async () => {
      try {
        const aliasPath = await invoke<string>('get_cmd_aliases_path');
        const defaultTemplate = '@echo off\ndoskey dclaude=claude --dangerously-skip-permissions $*\n';
        await showFileEditorDialog('CMD Aliases', aliasPath, defaultTemplate);
        const status = await invoke<string>('ensure_cmd_autorun');
        if (status === 'configured' || status === 'appended') {
          aliasStatus.textContent = 'AutoRun configured';
        } else {
          aliasStatus.textContent = 'Already configured';
        }
      } catch (err) {
        console.warn('CMD aliases setup failed:', err);
        aliasStatus.textContent = 'Setup failed';
      }
    };
    aliasRow.appendChild(editAliasBtn);
    aliasRow.appendChild(aliasStatus);
    aliasSection.appendChild(aliasRow);

    content.appendChild(aliasSection);

    return content;
  }
}
