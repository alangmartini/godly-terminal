import {
  quickClaudeSettingsStore,
  LAYOUT_MAX_AGENTS,
  type QuickClaudePreset,
  type PresetAgent,
  type PresetLayout,
  type LaunchStep,
  type LaunchStepType,
} from '../../state/quick-claude-settings-store';
import { aiToolsSettingsStore } from '../../state/ai-tools-settings-store';
import type { SettingsTabProvider, SettingsDialogContext } from './types';

// ── Step type metadata ──────────────────────────────────────────────

const STEP_TYPE_LABELS: Record<LaunchStepType, string> = {
  'create-terminal': 'Create Terminal',
  'wait-idle': 'Wait for Idle',
  'run-command': 'Run Command',
  'wait-ready': 'Wait for Ready',
  'send-prompt': 'Send Prompt',
  'send-enter': 'Send Enter',
  'delay': 'Delay',
};

const LAYOUT_LABELS: Record<PresetLayout, string> = {
  single: 'Single',
  vertical: 'Left / Right',
  horizontal: 'Top / Bottom',
  grid: '2x2 Grid',
};

const LAYOUT_ICONS: Record<PresetLayout, string> = {
  single: '\u25A0',
  vertical: '\u25EB',
  horizontal: '\u2B12',
  grid: '\u2B1A',
};

// ── Quick Claude Settings Tab ───────────────────────────────────────

export class QuickClaudeTab implements SettingsTabProvider {
  id = 'quick-claude';
  label = 'Quick Claude';

  buildContent(_dialog: SettingsDialogContext): HTMLDivElement {
    const content = document.createElement('div');
    content.className = 'settings-tab-content';

    let editingPresetId: string | null = null;

    const renderList = () => {
      editingPresetId = null;
      content.textContent = '';
      this.renderPresetList(content, (presetId) => {
        editingPresetId = presetId;
        renderEditor();
      });
    };

    const renderEditor = () => {
      if (!editingPresetId) return;
      content.textContent = '';
      this.renderPresetEditor(content, editingPresetId, () => renderList());
    };

    renderList();

    const unsub = quickClaudeSettingsStore.subscribe(() => {
      if (!editingPresetId) renderList();
    });
    (content as any).__qcUnsub = unsub;

    return content;
  }

  // ── List View ─────────────────────────────────────────────────────

  private renderPresetList(
    container: HTMLElement,
    onEdit: (presetId: string) => void,
  ): void {
    const header = document.createElement('div');
    header.className = 'flow-list-header';

    const title = document.createElement('div');
    title.className = 'settings-section-title';
    title.textContent = 'Quick Claude Presets';
    title.style.marginBottom = '0';
    header.appendChild(title);

    const headerActions = document.createElement('div');
    headerActions.className = 'flow-header-actions';

    const newBtn = document.createElement('button');
    newBtn.className = 'flow-btn flow-btn-primary';
    newBtn.textContent = 'New Preset';
    newBtn.addEventListener('click', () => {
      const preset: QuickClaudePreset = {
        id: crypto.randomUUID(),
        name: 'New Preset',
        description: '',
        layout: 'single',
        agents: [{
          id: crypto.randomUUID(),
          toolId: 'claude',
          label: 'Claude Code',
          steps: quickClaudeSettingsStore.getDefaultStepsForTool('claude'),
        }],
        isDefault: false,
        createdAt: Date.now(),
      };
      quickClaudeSettingsStore.addPreset(preset);
      onEdit(preset.id);
    });
    headerActions.appendChild(newBtn);
    header.appendChild(headerActions);
    container.appendChild(header);

    const desc = document.createElement('div');
    desc.className = 'settings-description';
    desc.textContent = 'Named launch configurations for Quick Claude. Each preset defines which agents to launch, their layout, and step sequence.';
    container.appendChild(desc);

    const presets = quickClaudeSettingsStore.getPresets();

    if (presets.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'flow-empty';
      empty.textContent = 'No presets yet. Create one to get started.';
      container.appendChild(empty);
      return;
    }

    const list = document.createElement('div');
    list.className = 'flow-list';

    for (const preset of presets) {
      list.appendChild(this.createPresetCard(preset, onEdit));
    }

    container.appendChild(list);
  }

  private createPresetCard(
    preset: QuickClaudePreset,
    onEdit: (presetId: string) => void,
  ): HTMLElement {
    const card = document.createElement('div');
    card.className = 'flow-card';

    const cardHeader = document.createElement('div');
    cardHeader.className = 'flow-card-header';

    const nameEl = document.createElement('span');
    nameEl.className = 'flow-card-name';
    nameEl.textContent = preset.name;
    cardHeader.appendChild(nameEl);

    // Layout badge
    const layoutBadge = document.createElement('span');
    layoutBadge.className = 'qc-layout-badge';
    layoutBadge.textContent = `${LAYOUT_ICONS[preset.layout]} ${LAYOUT_LABELS[preset.layout]}`;
    cardHeader.appendChild(layoutBadge);

    // Default star
    if (preset.isDefault) {
      const star = document.createElement('span');
      star.className = 'qc-default-star';
      star.textContent = '\u2605';
      star.title = 'Default preset';
      cardHeader.appendChild(star);
    }

    const actions = document.createElement('div');
    actions.className = 'flow-card-actions';

    // Set as default
    if (!preset.isDefault) {
      const defaultBtn = document.createElement('button');
      defaultBtn.className = 'flow-btn flow-btn-icon';
      defaultBtn.title = 'Set as default';
      defaultBtn.textContent = '\u2606'; // empty star
      defaultBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        quickClaudeSettingsStore.setDefault(preset.id);
      });
      actions.appendChild(defaultBtn);
    }

    // Edit
    const editBtn = document.createElement('button');
    editBtn.className = 'flow-btn flow-btn-icon';
    editBtn.title = 'Edit';
    editBtn.textContent = '\u270E';
    editBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      onEdit(preset.id);
    });
    actions.appendChild(editBtn);

    // Duplicate
    const dupBtn = document.createElement('button');
    dupBtn.className = 'flow-btn flow-btn-icon';
    dupBtn.title = 'Duplicate';
    dupBtn.textContent = '\u2398';
    dupBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      quickClaudeSettingsStore.duplicatePreset(preset.id);
    });
    actions.appendChild(dupBtn);

    // Delete (not for built-ins)
    if (!preset.id.startsWith('builtin-')) {
      const delBtn = document.createElement('button');
      delBtn.className = 'flow-btn flow-btn-icon flow-btn-danger-icon';
      delBtn.title = 'Delete';
      delBtn.textContent = '\u2715';
      delBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        quickClaudeSettingsStore.deletePreset(preset.id);
      });
      actions.appendChild(delBtn);
    }

    cardHeader.appendChild(actions);
    card.appendChild(cardHeader);

    if (preset.description) {
      const descEl = document.createElement('div');
      descEl.className = 'flow-card-description';
      descEl.textContent = preset.description;
      card.appendChild(descEl);
    }

    // Agent count
    const meta = document.createElement('div');
    meta.className = 'flow-card-meta';
    const agentNames = preset.agents.map(a => a.label).join(', ');
    meta.textContent = `${preset.agents.length} agent${preset.agents.length !== 1 ? 's' : ''}: ${agentNames}`;
    card.appendChild(meta);

    return card;
  }

  // ── Editor View ───────────────────────────────────────────────────

  private renderPresetEditor(
    container: HTMLElement,
    presetId: string,
    onBack: () => void,
  ): void {
    const preset = quickClaudeSettingsStore.getPreset(presetId);
    if (!preset) { onBack(); return; }

    let editName = preset.name;
    let editDescription = preset.description;
    let editLayout = preset.layout;
    let editAgents = structuredClone(preset.agents);

    const editor = document.createElement('div');
    editor.className = 'flow-editor';

    // Header
    const header = document.createElement('div');
    header.className = 'flow-editor-header';
    const backBtn = document.createElement('button');
    backBtn.className = 'flow-btn flow-btn-secondary';
    backBtn.textContent = '\u2190 Back';
    backBtn.addEventListener('click', onBack);
    header.appendChild(backBtn);
    const headerTitle = document.createElement('span');
    headerTitle.className = 'flow-editor-title';
    headerTitle.textContent = 'Edit Preset';
    header.appendChild(headerTitle);
    editor.appendChild(header);

    // Name
    const nameGroup = this.createFieldGroup('Name');
    const nameInput = document.createElement('input');
    nameInput.type = 'text';
    nameInput.className = 'flow-input';
    nameInput.value = editName;
    nameInput.placeholder = 'Preset name';
    nameInput.addEventListener('input', () => { editName = nameInput.value; });
    nameGroup.appendChild(nameInput);
    editor.appendChild(nameGroup);

    // Description
    const descGroup = this.createFieldGroup('Description');
    const descInput = document.createElement('input');
    descInput.type = 'text';
    descInput.className = 'flow-input';
    descInput.value = editDescription;
    descInput.placeholder = 'Optional description';
    descInput.addEventListener('input', () => { editDescription = descInput.value; });
    descGroup.appendChild(descInput);
    editor.appendChild(descGroup);

    // Layout selector
    const layoutGroup = this.createFieldGroup('Layout');
    const layoutSelector = document.createElement('div');
    layoutSelector.className = 'qc-layout-selector';

    const layouts: PresetLayout[] = ['single', 'vertical', 'horizontal', 'grid'];
    for (const layout of layouts) {
      const radio = document.createElement('label');
      radio.className = 'qc-layout-option' + (layout === editLayout ? ' qc-layout-option-active' : '');

      const input = document.createElement('input');
      input.type = 'radio';
      input.name = 'qc-layout';
      input.value = layout;
      input.checked = layout === editLayout;
      input.style.display = 'none';
      input.addEventListener('change', () => {
        editLayout = layout;
        // Update active class
        layoutSelector.querySelectorAll('.qc-layout-option').forEach(el => el.classList.remove('qc-layout-option-active'));
        radio.classList.add('qc-layout-option-active');
        // Trim agents if needed
        const max = LAYOUT_MAX_AGENTS[layout];
        if (editAgents.length > max) {
          editAgents = editAgents.slice(0, max);
        }
        renderAgents();
      });

      const icon = document.createElement('span');
      icon.className = 'qc-layout-icon';
      icon.textContent = LAYOUT_ICONS[layout];

      const label = document.createElement('span');
      label.className = 'qc-layout-label';
      label.textContent = LAYOUT_LABELS[layout];

      const maxLabel = document.createElement('span');
      maxLabel.className = 'qc-layout-max';
      maxLabel.textContent = `max ${LAYOUT_MAX_AGENTS[layout]}`;

      radio.append(input, icon, label, maxLabel);
      layoutSelector.appendChild(radio);
    }

    layoutGroup.appendChild(layoutSelector);
    editor.appendChild(layoutGroup);

    // Agents section
    const agentsGroup = this.createFieldGroup('Agents');
    const agentsContainer = document.createElement('div');
    agentsContainer.className = 'qc-agents';

    const renderAgents = () => {
      agentsContainer.textContent = '';

      editAgents.forEach((agent, idx) => {
        agentsContainer.appendChild(
          this.createAgentCard(agent, idx, editAgents, () => renderAgents()),
        );
      });

      // Add Agent button
      const max = LAYOUT_MAX_AGENTS[editLayout];
      if (editAgents.length < max) {
        const addBtn = document.createElement('button');
        addBtn.className = 'flow-add-step';
        addBtn.textContent = '+ Add Agent';
        addBtn.addEventListener('click', () => {
          const toolId = 'claude';
          editAgents.push({
            id: crypto.randomUUID(),
            toolId,
            label: 'Claude Code',
            steps: quickClaudeSettingsStore.getDefaultStepsForTool(toolId),
          });
          renderAgents();
        });
        agentsContainer.appendChild(addBtn);
      }
    };

    renderAgents();
    agentsGroup.appendChild(agentsContainer);
    editor.appendChild(agentsGroup);

    // Action buttons
    const actionsRow = document.createElement('div');
    actionsRow.className = 'flow-actions';

    const saveBtn = document.createElement('button');
    saveBtn.className = 'flow-btn flow-btn-primary';
    saveBtn.textContent = 'Save';
    saveBtn.addEventListener('click', () => {
      quickClaudeSettingsStore.updatePreset(presetId, {
        name: editName,
        description: editDescription,
        layout: editLayout,
        agents: editAgents,
      });
      onBack();
    });
    actionsRow.appendChild(saveBtn);

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'flow-btn flow-btn-secondary';
    cancelBtn.textContent = 'Cancel';
    cancelBtn.addEventListener('click', onBack);
    actionsRow.appendChild(cancelBtn);

    editor.appendChild(actionsRow);
    container.appendChild(editor);
  }

  // ── Agent Card ────────────────────────────────────────────────────

  private createAgentCard(
    agent: PresetAgent,
    index: number,
    allAgents: PresetAgent[],
    onChanged: () => void,
  ): HTMLElement {
    const card = document.createElement('div');
    card.className = 'qc-agent-card';

    // Header row
    const headerRow = document.createElement('div');
    headerRow.className = 'qc-agent-header';

    const posLabel = document.createElement('span');
    posLabel.className = 'qc-agent-position';
    posLabel.textContent = `Agent ${index + 1}`;
    headerRow.appendChild(posLabel);

    // Tool selector
    const toolSelect = document.createElement('select');
    toolSelect.className = 'flow-select';
    const toolOptions = aiToolsSettingsStore.getAllToolOptions().filter(t => t.id !== 'both');
    for (const tool of toolOptions) {
      const opt = document.createElement('option');
      opt.value = tool.id;
      opt.textContent = tool.name;
      toolSelect.appendChild(opt);
    }
    toolSelect.value = agent.toolId;
    toolSelect.addEventListener('change', () => {
      agent.toolId = toolSelect.value;
      const selected = toolOptions.find(t => t.id === toolSelect.value);
      if (selected) {
        agent.label = selected.name;
        labelInput.value = selected.name;
      }
    });
    headerRow.appendChild(toolSelect);

    // Remove button
    const removeBtn = document.createElement('button');
    removeBtn.className = 'flow-btn flow-btn-icon flow-btn-danger-icon';
    removeBtn.title = 'Remove agent';
    removeBtn.textContent = '\u2715';
    removeBtn.addEventListener('click', () => {
      const idx = allAgents.indexOf(agent);
      if (idx !== -1) allAgents.splice(idx, 1);
      onChanged();
    });
    headerRow.appendChild(removeBtn);

    card.appendChild(headerRow);

    // Label
    const labelRow = document.createElement('div');
    labelRow.className = 'qc-agent-field';
    const labelLabel = document.createElement('span');
    labelLabel.className = 'qc-agent-field-label';
    labelLabel.textContent = 'Label';
    labelRow.appendChild(labelLabel);
    const labelInput = document.createElement('input');
    labelInput.type = 'text';
    labelInput.className = 'flow-input';
    labelInput.value = agent.label;
    labelInput.placeholder = 'Display name';
    labelInput.addEventListener('input', () => { agent.label = labelInput.value; });
    labelRow.appendChild(labelInput);
    card.appendChild(labelRow);

    // Command override
    const cmdRow = document.createElement('div');
    cmdRow.className = 'qc-agent-field';
    const cmdLabel = document.createElement('span');
    cmdLabel.className = 'qc-agent-field-label';
    cmdLabel.textContent = 'Command Override';
    cmdRow.appendChild(cmdLabel);
    const cmdInput = document.createElement('input');
    cmdInput.type = 'text';
    cmdInput.className = 'flow-input';
    cmdInput.value = agent.commandOverride ?? '';
    cmdInput.placeholder = 'Optional (uses tool default)';
    cmdInput.addEventListener('input', () => {
      agent.commandOverride = cmdInput.value || undefined;
    });
    cmdRow.appendChild(cmdInput);
    card.appendChild(cmdRow);

    // Branch suffix override
    const suffixRow = document.createElement('div');
    suffixRow.className = 'qc-agent-field';
    const suffixLabel = document.createElement('span');
    suffixLabel.className = 'qc-agent-field-label';
    suffixLabel.textContent = 'Branch Suffix';
    suffixRow.appendChild(suffixLabel);
    const suffixInput = document.createElement('input');
    suffixInput.type = 'text';
    suffixInput.className = 'flow-input';
    suffixInput.value = agent.branchSuffixOverride ?? '';
    suffixInput.placeholder = 'Optional (uses tool default)';
    suffixInput.style.width = '120px';
    suffixInput.addEventListener('input', () => {
      agent.branchSuffixOverride = suffixInput.value || undefined;
    });
    suffixRow.appendChild(suffixInput);
    card.appendChild(suffixRow);

    // Steps section
    const stepsHeader = document.createElement('div');
    stepsHeader.className = 'qc-steps-header';
    const stepsTitle = document.createElement('span');
    stepsTitle.className = 'qc-agent-field-label';
    stepsTitle.textContent = 'Launch Steps';
    stepsHeader.appendChild(stepsTitle);

    const resetBtn = document.createElement('button');
    resetBtn.className = 'flow-btn flow-btn-secondary';
    resetBtn.style.cssText = 'font-size: 11px; padding: 2px 8px;';
    resetBtn.textContent = 'Reset to Default';
    resetBtn.addEventListener('click', () => {
      agent.steps = quickClaudeSettingsStore.getDefaultStepsForTool(agent.toolId);
      onChanged();
    });
    stepsHeader.appendChild(resetBtn);
    card.appendChild(stepsHeader);

    const stepsContainer = document.createElement('div');
    stepsContainer.className = 'qc-step-list';

    for (const step of agent.steps) {
      stepsContainer.appendChild(this.createStepRow(step));
    }

    card.appendChild(stepsContainer);

    return card;
  }

  // ── Step Row ──────────────────────────────────────────────────────

  private createStepRow(step: LaunchStep): HTMLElement {
    const row = document.createElement('div');
    row.className = 'qc-step-row';

    // Checkbox
    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox';
    checkbox.checked = step.enabled;
    checkbox.className = 'qc-step-checkbox';
    checkbox.addEventListener('change', () => { step.enabled = checkbox.checked; });
    row.appendChild(checkbox);

    // Type label
    const typeLabel = document.createElement('span');
    typeLabel.className = 'qc-step-type';
    typeLabel.textContent = STEP_TYPE_LABELS[step.type] ?? step.type;
    row.appendChild(typeLabel);

    // Inline config
    const configEl = document.createElement('span');
    configEl.className = 'qc-step-config';
    configEl.textContent = this.formatStepConfig(step);
    row.appendChild(configEl);

    // Expandable config fields
    if (this.hasEditableConfig(step)) {
      const expandBtn = document.createElement('button');
      expandBtn.className = 'flow-btn flow-btn-icon';
      expandBtn.title = 'Configure';
      expandBtn.textContent = '\u2699';
      expandBtn.style.cssText = 'font-size: 12px; padding: 1px 4px;';

      const configPanel = document.createElement('div');
      configPanel.className = 'qc-step-config-panel';
      configPanel.style.display = 'none';
      this.renderStepConfig(configPanel, step, () => {
        configEl.textContent = this.formatStepConfig(step);
      });

      expandBtn.addEventListener('click', () => {
        const visible = configPanel.style.display !== 'none';
        configPanel.style.display = visible ? 'none' : '';
      });

      row.appendChild(expandBtn);
      row.appendChild(configPanel);
    }

    return row;
  }

  private formatStepConfig(step: LaunchStep): string {
    switch (step.type) {
      case 'wait-idle':
        return `${step.config.idleMs ?? 2000}ms idle, ${step.config.timeoutMs ?? 30000}ms timeout`;
      case 'run-command':
        return step.config.command ? String(step.config.command) : '(no command)';
      case 'wait-ready':
        return `marker: ${step.config.marker ?? 'ready'}`;
      case 'delay':
        return `${step.config.ms ?? 1000}ms`;
      default:
        return '';
    }
  }

  private hasEditableConfig(step: LaunchStep): boolean {
    return ['wait-idle', 'run-command', 'wait-ready', 'delay'].includes(step.type);
  }

  private renderStepConfig(
    container: HTMLElement,
    step: LaunchStep,
    onChanged: () => void,
  ): void {
    switch (step.type) {
      case 'wait-idle': {
        container.appendChild(this.createNumberField('Idle (ms)', step.config, 'idleMs', 2000, onChanged));
        container.appendChild(this.createNumberField('Timeout (ms)', step.config, 'timeoutMs', 30000, onChanged));
        break;
      }
      case 'run-command': {
        container.appendChild(this.createStringField('Command', step.config, 'command', 'echo hello', onChanged));
        break;
      }
      case 'wait-ready': {
        container.appendChild(this.createStringField('Marker', step.config, 'marker', 'ready', onChanged));
        container.appendChild(this.createNumberField('Timeout (ms)', step.config, 'timeoutMs', 30000, onChanged));
        break;
      }
      case 'delay': {
        container.appendChild(this.createNumberField('Delay (ms)', step.config, 'ms', 1000, onChanged));
        break;
      }
    }
  }

  // ── Helper: field builders ────────────────────────────────────────

  private createFieldGroup(labelText: string): HTMLElement {
    const group = document.createElement('div');
    group.className = 'flow-field-group';
    const label = document.createElement('label');
    label.className = 'flow-field-label';
    label.textContent = labelText;
    group.appendChild(label);
    return group;
  }

  private createNumberField(
    label: string,
    config: Record<string, unknown>,
    key: string,
    defaultVal: number,
    onChanged: () => void,
  ): HTMLElement {
    const row = document.createElement('div');
    row.className = 'qc-config-field';
    const labelEl = document.createElement('span');
    labelEl.className = 'qc-config-label';
    labelEl.textContent = label;
    row.appendChild(labelEl);
    const input = document.createElement('input');
    input.type = 'number';
    input.className = 'flow-input';
    input.style.width = '100px';
    input.value = String(config[key] ?? defaultVal);
    input.addEventListener('input', () => {
      config[key] = input.value ? Number(input.value) : defaultVal;
      onChanged();
    });
    row.appendChild(input);
    return row;
  }

  private createStringField(
    label: string,
    config: Record<string, unknown>,
    key: string,
    placeholder: string,
    onChanged: () => void,
  ): HTMLElement {
    const row = document.createElement('div');
    row.className = 'qc-config-field';
    const labelEl = document.createElement('span');
    labelEl.className = 'qc-config-label';
    labelEl.textContent = label;
    row.appendChild(labelEl);
    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'flow-input';
    input.value = String(config[key] ?? '');
    input.placeholder = placeholder;
    input.addEventListener('input', () => {
      config[key] = input.value;
      onChanged();
    });
    row.appendChild(input);
    return row;
  }
}
