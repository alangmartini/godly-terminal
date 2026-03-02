import { aiToolsSettingsStore, type CustomAiTool } from '../../state/ai-tools-settings-store';
import type { SettingsTabProvider, SettingsDialogContext } from './types';

export class AiToolsTab implements SettingsTabProvider {
  id = 'ai-tools';
  label = 'AI Tools';

  buildContent(_dialog: SettingsDialogContext): HTMLDivElement {
    const content = document.createElement('div');
    content.className = 'settings-tab-content';

    // ── Custom AI Tools section ────────────────────────────────
    content.appendChild(this.buildCustomToolsSection());

    // ── Branch Suffixes section ────────────────────────────────
    content.appendChild(this.buildBranchSuffixesSection());

    // ── Simultaneous Launches section ──────────────────────────
    content.appendChild(this.buildSimultaneousSection());

    return content;
  }

  private buildCustomToolsSection(): HTMLDivElement {
    const section = document.createElement('div');
    section.className = 'settings-section';

    const title = document.createElement('div');
    title.className = 'settings-section-title';
    title.textContent = 'Custom AI Tools';
    section.appendChild(title);

    const desc = document.createElement('div');
    desc.className = 'settings-description';
    desc.textContent = 'Add custom AI tool binaries that appear in the Quick Claude dialog alongside Claude Code and Codex.';
    section.appendChild(desc);

    const toolList = document.createElement('div');
    toolList.className = 'ai-tools-list';

    const renderToolList = () => {
      toolList.innerHTML = '';
      const tools = aiToolsSettingsStore.getCustomTools();

      if (tools.length === 0) {
        const empty = document.createElement('div');
        empty.className = 'settings-description';
        empty.style.fontStyle = 'italic';
        empty.textContent = 'No custom tools configured. Click "Add Tool" to get started.';
        toolList.appendChild(empty);
      }

      for (const tool of tools) {
        toolList.appendChild(this.buildToolRow(tool, renderToolList));
      }
    };

    renderToolList();
    section.appendChild(toolList);

    const addBtn = document.createElement('button');
    addBtn.className = 'dialog-btn dialog-btn-secondary';
    addBtn.textContent = '+ Add Tool';
    addBtn.style.marginTop = '8px';
    addBtn.onclick = () => {
      const id = `custom-${Date.now()}`;
      aiToolsSettingsStore.addCustomTool({
        id,
        name: '',
        binaryPath: '',
        launchCommand: '{binary} --prompt "{prompt}"',
        branchSuffix: '',
      });
      renderToolList();
    };
    section.appendChild(addBtn);

    return section;
  }

  private buildToolRow(tool: CustomAiTool, onUpdate: () => void): HTMLDivElement {
    const row = document.createElement('div');
    row.className = 'ai-tool-row';

    // Name
    const nameRow = document.createElement('div');
    nameRow.className = 'shortcut-row';
    const nameLabel = document.createElement('span');
    nameLabel.className = 'shortcut-label';
    nameLabel.textContent = 'Name';
    nameRow.appendChild(nameLabel);
    const nameInput = document.createElement('input');
    nameInput.type = 'text';
    nameInput.className = 'notification-preset';
    nameInput.placeholder = 'e.g. Aider, Cursor Agent';
    nameInput.value = tool.name;
    nameInput.style.flex = '1';
    nameInput.onchange = () => {
      aiToolsSettingsStore.updateCustomTool(tool.id, { name: nameInput.value.trim() });
    };
    nameRow.appendChild(nameInput);
    row.appendChild(nameRow);

    // Binary path
    const binaryRow = document.createElement('div');
    binaryRow.className = 'shortcut-row';
    const binaryLabel = document.createElement('span');
    binaryLabel.className = 'shortcut-label';
    binaryLabel.textContent = 'Binary';
    binaryRow.appendChild(binaryLabel);
    const binaryInput = document.createElement('input');
    binaryInput.type = 'text';
    binaryInput.className = 'notification-preset';
    binaryInput.placeholder = 'e.g. aider.exe, /usr/bin/aider';
    binaryInput.value = tool.binaryPath;
    binaryInput.style.flex = '1';
    binaryInput.onchange = () => {
      aiToolsSettingsStore.updateCustomTool(tool.id, { binaryPath: binaryInput.value.trim() });
    };
    binaryRow.appendChild(binaryInput);
    row.appendChild(binaryRow);

    // Launch command template
    const cmdRow = document.createElement('div');
    cmdRow.className = 'shortcut-row';
    const cmdLabel = document.createElement('span');
    cmdLabel.className = 'shortcut-label';
    cmdLabel.textContent = 'Command';
    cmdRow.appendChild(cmdLabel);
    const cmdInput = document.createElement('input');
    cmdInput.type = 'text';
    cmdInput.className = 'notification-preset';
    cmdInput.placeholder = '{binary} --prompt "{prompt}"';
    cmdInput.value = tool.launchCommand;
    cmdInput.style.flex = '1';
    cmdInput.onchange = () => {
      aiToolsSettingsStore.updateCustomTool(tool.id, { launchCommand: cmdInput.value.trim() });
    };
    cmdRow.appendChild(cmdInput);
    row.appendChild(cmdRow);

    // Branch suffix
    const suffixRow = document.createElement('div');
    suffixRow.className = 'shortcut-row';
    const suffixLabel = document.createElement('span');
    suffixLabel.className = 'shortcut-label';
    suffixLabel.textContent = 'Branch suffix';
    suffixRow.appendChild(suffixLabel);
    const suffixInput = document.createElement('input');
    suffixInput.type = 'text';
    suffixInput.className = 'notification-preset';
    suffixInput.placeholder = 'e.g. -ai';
    suffixInput.value = tool.branchSuffix;
    suffixInput.style.width = '100px';
    suffixInput.onchange = () => {
      const suffix = suffixInput.value.trim();
      aiToolsSettingsStore.updateCustomTool(tool.id, { branchSuffix: suffix });
      aiToolsSettingsStore.setBranchSuffix(tool.id, suffix);
    };
    suffixRow.appendChild(suffixInput);

    // Remove button
    const removeBtn = document.createElement('button');
    removeBtn.className = 'dialog-btn dialog-btn-secondary';
    removeBtn.textContent = 'Remove';
    removeBtn.style.cssText = 'font-size: 11px; padding: 2px 8px; margin-left: auto;';
    removeBtn.onclick = () => {
      aiToolsSettingsStore.removeCustomTool(tool.id);
      onUpdate();
    };
    suffixRow.appendChild(removeBtn);
    row.appendChild(suffixRow);

    return row;
  }

  private buildBranchSuffixesSection(): HTMLDivElement {
    const section = document.createElement('div');
    section.className = 'settings-section';

    const title = document.createElement('div');
    title.className = 'settings-section-title';
    title.textContent = 'Branch Name Suffixes';
    section.appendChild(title);

    const desc = document.createElement('div');
    desc.className = 'settings-description';
    desc.textContent = 'Configure the suffix appended to branch names when launching AI tools via Quick Claude. Used to distinguish branches when running multiple tools in parallel.';
    section.appendChild(desc);

    const builtinTools = [
      { id: 'claude', label: 'Claude Code' },
      { id: 'codex', label: 'Codex' },
    ];

    for (const tool of builtinTools) {
      const row = document.createElement('div');
      row.className = 'shortcut-row';

      const label = document.createElement('span');
      label.className = 'shortcut-label';
      label.textContent = tool.label;
      row.appendChild(label);

      const input = document.createElement('input');
      input.type = 'text';
      input.className = 'notification-preset';
      input.value = aiToolsSettingsStore.getBranchSuffix(tool.id);
      input.style.width = '100px';
      input.placeholder = 'e.g. -cc';
      input.onchange = () => {
        aiToolsSettingsStore.setBranchSuffix(tool.id, input.value.trim());
      };
      row.appendChild(input);

      section.appendChild(row);
    }

    return section;
  }

  private buildSimultaneousSection(): HTMLDivElement {
    const section = document.createElement('div');
    section.className = 'settings-section';

    const title = document.createElement('div');
    title.className = 'settings-section-title';
    title.textContent = 'Simultaneous Launches';
    section.appendChild(title);

    const desc = document.createElement('div');
    desc.className = 'settings-description';
    desc.textContent = 'Maximum number of AI tools to launch in parallel from Quick Claude. The "Both" option launches 2; increase this to run up to 4 tools simultaneously.';
    section.appendChild(desc);

    const row = document.createElement('div');
    row.className = 'shortcut-row';

    const label = document.createElement('span');
    label.className = 'shortcut-label';
    label.textContent = 'Max simultaneous';
    row.appendChild(label);

    const select = document.createElement('select');
    select.className = 'notification-preset';
    for (let i = 1; i <= 4; i++) {
      const opt = document.createElement('option');
      opt.value = String(i);
      opt.textContent = String(i);
      select.appendChild(opt);
    }
    select.value = String(aiToolsSettingsStore.getMaxSimultaneous());
    select.onchange = () => {
      aiToolsSettingsStore.setMaxSimultaneous(parseInt(select.value, 10));
    };
    row.appendChild(select);

    section.appendChild(row);

    return section;
  }
}
