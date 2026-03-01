import { flowStore } from '../../flow-engine/flow-store';
import { nodeTypeRegistry } from '../../flow-engine/node-type-registry';
import type { Flow, FlowNode, FlowEdge, NodeCategory, ConfigField } from '../../flow-engine/types';
import { NODE_CATEGORY_LABELS, NODE_CATEGORY_COLORS } from '../../flow-engine/types';
import { formatChord, eventToChord, type KeyChord } from '../../state/keybinding-store';
import type { SettingsTabProvider, SettingsDialogContext } from './types';

// ── Flows Settings Tab ──────────────────────────────────────────────

export class FlowsTab implements SettingsTabProvider {
  id = 'flows';
  label = 'Flows';

  private capturing = false;

  buildContent(_dialog: SettingsDialogContext): HTMLDivElement {
    const content = document.createElement('div');
    content.className = 'settings-tab-content';

    let editingFlowId: string | null = null;

    const renderList = () => {
      this.capturing = false;
      editingFlowId = null;
      content.textContent = '';
      this.renderFlowList(content, (flowId) => {
        editingFlowId = flowId;
        renderEditor();
      });
    };

    const renderEditor = () => {
      if (!editingFlowId) return;
      content.textContent = '';
      this.renderFlowEditor(content, editingFlowId, () => renderList());
    };

    renderList();

    // Subscribe to store changes for live updates when on list view
    const unsub = flowStore.subscribe(() => {
      if (!editingFlowId) {
        renderList();
      }
    });

    // Store unsubscribe for cleanup when the dialog closes
    (content as any).__flowsUnsub = unsub;

    return content;
  }

  onDialogClose(): void {
    this.capturing = false;
  }

  isCapturing(): boolean {
    return this.capturing;
  }

  // ── Flow List View ──────────────────────────────────────────────

  private renderFlowList(
    container: HTMLElement,
    onEdit: (flowId: string) => void,
  ): void {
    // Header row with title and action buttons
    const header = document.createElement('div');
    header.className = 'flow-list-header';

    const title = document.createElement('div');
    title.className = 'settings-section-title';
    title.textContent = 'Flows';
    title.style.marginBottom = '0';
    header.appendChild(title);

    const headerActions = document.createElement('div');
    headerActions.className = 'flow-header-actions';

    const importBtn = document.createElement('button');
    importBtn.className = 'flow-btn flow-btn-secondary';
    importBtn.textContent = 'Import';
    importBtn.addEventListener('click', () => this.handleImport(container, onEdit));
    headerActions.appendChild(importBtn);

    const newBtn = document.createElement('button');
    newBtn.className = 'flow-btn flow-btn-primary';
    newBtn.textContent = 'New Flow';
    newBtn.addEventListener('click', () => {
      const flow = flowStore.create({
        name: 'New Flow',
        description: '',
        tags: [],
        enabled: true,
        nodes: [],
        edges: [],
        variables: [],
      });
      onEdit(flow.id);
    });
    headerActions.appendChild(newBtn);

    header.appendChild(headerActions);
    container.appendChild(header);

    const desc = document.createElement('div');
    desc.className = 'settings-description';
    desc.textContent = 'Automate terminal workflows with node-based flows. Assign hotkeys, chain commands, and orchestrate workspaces.';
    container.appendChild(desc);

    // Flow cards
    const flows = flowStore.getAll();

    if (flows.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'flow-empty';
      empty.textContent = 'No flows yet. Create one to get started.';
      container.appendChild(empty);
      return;
    }

    const list = document.createElement('div');
    list.className = 'flow-list';

    for (const flow of flows) {
      const card = this.createFlowCard(flow, onEdit);
      list.appendChild(card);
    }

    container.appendChild(list);
  }

  private createFlowCard(
    flow: Flow,
    onEdit: (flowId: string) => void,
  ): HTMLElement {
    const card = document.createElement('div');
    card.className = 'flow-card';

    const cardHeader = document.createElement('div');
    cardHeader.className = 'flow-card-header';

    const nameEl = document.createElement('span');
    nameEl.className = 'flow-card-name';
    nameEl.textContent = flow.name;
    cardHeader.appendChild(nameEl);

    // Hotkey badge (if any)
    const hotkeyNode = flow.nodes.find(n => n.type === 'trigger.hotkey' && !n.disabled);
    if (hotkeyNode) {
      const chord = hotkeyNode.config.chord as KeyChord | undefined;
      if (chord && chord.key) {
        const badge = document.createElement('span');
        badge.className = 'flow-chord-badge';
        badge.textContent = formatChord(chord);
        cardHeader.appendChild(badge);
      }
    }

    const actions = document.createElement('div');
    actions.className = 'flow-card-actions';

    // Toggle
    const toggle = this.createToggle(flow.enabled, (enabled) => {
      flowStore.setEnabled(flow.id, enabled);
    });
    actions.appendChild(toggle);

    // Edit button
    const editBtn = document.createElement('button');
    editBtn.className = 'flow-btn flow-btn-icon';
    editBtn.title = 'Edit';
    editBtn.textContent = '\u270E'; // pencil
    editBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      onEdit(flow.id);
    });
    actions.appendChild(editBtn);

    // Duplicate button
    const dupBtn = document.createElement('button');
    dupBtn.className = 'flow-btn flow-btn-icon';
    dupBtn.title = 'Duplicate';
    dupBtn.textContent = '\u2398'; // copy symbol
    dupBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      flowStore.duplicate(flow.id);
    });
    actions.appendChild(dupBtn);

    // Export button
    const exportBtn = document.createElement('button');
    exportBtn.className = 'flow-btn flow-btn-icon';
    exportBtn.title = 'Export JSON';
    exportBtn.textContent = '\u21E9'; // download arrow
    exportBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      this.handleExport(flow.id);
    });
    actions.appendChild(exportBtn);

    // Delete button
    const delBtn = document.createElement('button');
    delBtn.className = 'flow-btn flow-btn-icon flow-btn-danger-icon';
    delBtn.title = 'Delete';
    delBtn.textContent = '\u2715'; // x mark
    delBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      flowStore.delete(flow.id);
    });
    actions.appendChild(delBtn);

    cardHeader.appendChild(actions);
    card.appendChild(cardHeader);

    if (flow.description) {
      const descEl = document.createElement('div');
      descEl.className = 'flow-card-description';
      descEl.textContent = flow.description;
      card.appendChild(descEl);
    }

    // Step count
    const enabledNodes = flow.nodes.filter(n => !n.disabled && !n.type.startsWith('trigger.'));
    if (enabledNodes.length > 0) {
      const stepsEl = document.createElement('div');
      stepsEl.className = 'flow-card-meta';
      stepsEl.textContent = `${enabledNodes.length} step${enabledNodes.length !== 1 ? 's' : ''}`;
      card.appendChild(stepsEl);
    }

    return card;
  }

  // ── Flow Editor View ────────────────────────────────────────────

  private renderFlowEditor(
    container: HTMLElement,
    flowId: string,
    onBack: () => void,
  ): void {
    const flow = flowStore.getById(flowId);
    if (!flow) {
      onBack();
      return;
    }

    // Work on a mutable copy of the flow data
    let editName = flow.name;
    let editDescription = flow.description;
    let editNodes = structuredClone(flow.nodes);
    let editVariables = structuredClone(flow.variables);

    const editor = document.createElement('div');
    editor.className = 'flow-editor';

    // ── Header ──
    const header = document.createElement('div');
    header.className = 'flow-editor-header';

    const backBtn = document.createElement('button');
    backBtn.className = 'flow-btn flow-btn-secondary';
    backBtn.textContent = '\u2190 Back';
    backBtn.addEventListener('click', onBack);
    header.appendChild(backBtn);

    const headerTitle = document.createElement('span');
    headerTitle.className = 'flow-editor-title';
    headerTitle.textContent = 'Edit Flow';
    header.appendChild(headerTitle);

    editor.appendChild(header);

    // ── Name ──
    const nameGroup = this.createFieldGroup('Name');
    const nameInput = document.createElement('input');
    nameInput.type = 'text';
    nameInput.className = 'flow-input';
    nameInput.value = editName;
    nameInput.placeholder = 'Flow name';
    nameInput.addEventListener('input', () => {
      editName = nameInput.value;
    });
    nameGroup.appendChild(nameInput);
    editor.appendChild(nameGroup);

    // ── Description ──
    const descGroup = this.createFieldGroup('Description');
    const descInput = document.createElement('input');
    descInput.type = 'text';
    descInput.className = 'flow-input';
    descInput.value = editDescription;
    descInput.placeholder = 'Optional description';
    descInput.addEventListener('input', () => {
      editDescription = descInput.value;
    });
    descGroup.appendChild(descInput);
    editor.appendChild(descGroup);

    // ── Trigger Section ──
    const triggerGroup = this.createFieldGroup('Trigger');
    const triggerNode = editNodes.find(n => n.type.startsWith('trigger.'));
    const triggerTypes = nodeTypeRegistry.getByCategory('trigger');

    const triggerSelect = document.createElement('select');
    triggerSelect.className = 'flow-select';

    const noneOpt = document.createElement('option');
    noneOpt.value = '';
    noneOpt.textContent = 'None (manual only)';
    triggerSelect.appendChild(noneOpt);

    for (const def of triggerTypes) {
      const opt = document.createElement('option');
      opt.value = def.type;
      opt.textContent = def.label;
      triggerSelect.appendChild(opt);
    }

    if (triggerNode) {
      triggerSelect.value = triggerNode.type;
    }

    triggerGroup.appendChild(triggerSelect);

    // Trigger config container
    const triggerConfigContainer = document.createElement('div');
    triggerConfigContainer.className = 'flow-trigger-config';
    triggerGroup.appendChild(triggerConfigContainer);

    const renderTriggerConfig = () => {
      triggerConfigContainer.textContent = '';
      const currentTrigger = editNodes.find(n => n.type.startsWith('trigger.'));
      if (!currentTrigger) return;

      if (currentTrigger.type === 'trigger.hotkey') {
        this.renderChordPicker(triggerConfigContainer, currentTrigger);
      } else {
        const def = nodeTypeRegistry.get(currentTrigger.type);
        if (def) {
          this.renderConfigFields(triggerConfigContainer, def.configSchema, currentTrigger);
        }
      }
    };

    triggerSelect.addEventListener('change', () => {
      // Remove existing trigger nodes
      editNodes = editNodes.filter(n => !n.type.startsWith('trigger.'));

      if (triggerSelect.value) {
        const newTrigger: FlowNode = {
          id: crypto.randomUUID(),
          type: triggerSelect.value,
          label: triggerSelect.options[triggerSelect.selectedIndex].textContent || triggerSelect.value,
          position: { x: 0, y: 0 },
          config: {},
          disabled: false,
        };
        editNodes.unshift(newTrigger);
      }
      renderTriggerConfig();
    });

    renderTriggerConfig();
    editor.appendChild(triggerGroup);

    // ── Steps Section ──
    const stepsGroup = this.createFieldGroup('Steps');
    const stepsContainer = document.createElement('div');
    stepsContainer.className = 'flow-steps';

    const renderSteps = () => {
      stepsContainer.textContent = '';

      // Get non-trigger nodes (these are the "steps")
      const stepNodes = editNodes.filter(n => !n.type.startsWith('trigger.'));

      if (stepNodes.length === 0) {
        const empty = document.createElement('div');
        empty.className = 'flow-empty flow-empty-small';
        empty.textContent = 'No steps yet. Add a step to define your workflow.';
        stepsContainer.appendChild(empty);
      } else {
        stepNodes.forEach((node, idx) => {
          const stepEl = this.createStepElement(node, idx + 1, editNodes, () => renderSteps());
          stepsContainer.appendChild(stepEl);
        });
      }

      // Add Step button
      const addStepBtn = document.createElement('button');
      addStepBtn.className = 'flow-add-step';
      addStepBtn.textContent = '+ Add Step';
      addStepBtn.addEventListener('click', () => {
        this.showNodeSelector(addStepBtn, (nodeType) => {
          const def = nodeTypeRegistry.get(nodeType);
          if (!def) return;

          // Build default config from schema
          const defaultConfig: Record<string, unknown> = {};
          for (const field of def.configSchema) {
            if (field.defaultValue !== undefined) {
              defaultConfig[field.name] = field.defaultValue;
            }
          }

          const newNode: FlowNode = {
            id: crypto.randomUUID(),
            type: nodeType,
            label: def.label,
            position: { x: 0, y: (editNodes.length) * 100 },
            config: defaultConfig,
            disabled: false,
          };
          editNodes.push(newNode);
          renderSteps();
        });
      });
      stepsContainer.appendChild(addStepBtn);
    };

    renderSteps();
    stepsGroup.appendChild(stepsContainer);
    editor.appendChild(stepsGroup);

    // ── Action Buttons ──
    const actionsRow = document.createElement('div');
    actionsRow.className = 'flow-actions';

    const saveBtn = document.createElement('button');
    saveBtn.className = 'flow-btn flow-btn-primary';
    saveBtn.textContent = 'Save';
    saveBtn.addEventListener('click', () => {
      // Auto-generate edges as a linear chain
      const edges = this.generateLinearEdges(editNodes);

      flowStore.update(flowId, {
        name: editName,
        description: editDescription,
        nodes: editNodes,
        edges,
        variables: editVariables,
      });
      onBack();
    });
    actionsRow.appendChild(saveBtn);

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'flow-btn flow-btn-secondary';
    cancelBtn.textContent = 'Cancel';
    cancelBtn.addEventListener('click', onBack);
    actionsRow.appendChild(cancelBtn);

    const runNowBtn = document.createElement('button');
    runNowBtn.className = 'flow-btn flow-btn-accent';
    runNowBtn.textContent = 'Run Now';
    runNowBtn.addEventListener('click', async () => {
      // Save first
      const edges = this.generateLinearEdges(editNodes);
      flowStore.update(flowId, {
        name: editName,
        description: editDescription,
        nodes: editNodes,
        edges,
        variables: editVariables,
      });

      // Trigger execution via the global flow engine
      const engine = (window as any).__FLOW_ENGINE__;
      if (engine && engine.triggerFlow) {
        try {
          await engine.triggerFlow(flowId);
          console.info('[Flows] Flow run initiated:', editName);
        } catch (err) {
          console.warn('[Flows] Run failed:', err);
        }
      }
    });
    actionsRow.appendChild(runNowBtn);

    editor.appendChild(actionsRow);
    container.appendChild(editor);
  }

  // ── Step Element ────────────────────────────────────────────────

  private createStepElement(
    node: FlowNode,
    stepNumber: number,
    allNodes: FlowNode[],
    onChanged: () => void,
  ): HTMLElement {
    const step = document.createElement('div');
    step.className = 'flow-step';

    // Step number
    const numEl = document.createElement('div');
    numEl.className = 'flow-step-number';
    numEl.textContent = String(stepNumber);
    step.appendChild(numEl);

    const stepBody = document.createElement('div');
    stepBody.className = 'flow-step-body';

    // Type selector
    const typeRow = document.createElement('div');
    typeRow.className = 'flow-step-type-row';

    const typeSelect = document.createElement('select');
    typeSelect.className = 'flow-select';

    // Build grouped options (exclude trigger category)
    const categories: NodeCategory[] = ['terminal', 'split', 'workspace', 'voice', 'quick-claude', 'control-flow', 'data'];
    for (const cat of categories) {
      const defs = nodeTypeRegistry.getByCategory(cat);
      if (defs.length === 0) continue;

      const group = document.createElement('optgroup');
      group.label = NODE_CATEGORY_LABELS[cat];

      for (const def of defs) {
        const opt = document.createElement('option');
        opt.value = def.type;
        opt.textContent = `${def.icon} ${def.label}`;
        group.appendChild(opt);
      }
      typeSelect.appendChild(group);
    }

    typeSelect.value = node.type;

    typeSelect.addEventListener('change', () => {
      const newDef = nodeTypeRegistry.get(typeSelect.value);
      if (newDef) {
        node.type = typeSelect.value;
        node.label = newDef.label;
        // Reset config to defaults
        node.config = {};
        for (const field of newDef.configSchema) {
          if (field.defaultValue !== undefined) {
            node.config[field.name] = field.defaultValue;
          }
        }
      }
      onChanged();
    });

    typeRow.appendChild(typeSelect);

    // Remove button
    const removeBtn = document.createElement('button');
    removeBtn.className = 'flow-btn flow-btn-icon flow-btn-danger-icon';
    removeBtn.title = 'Remove step';
    removeBtn.textContent = '\u2715';
    removeBtn.addEventListener('click', () => {
      const idx = allNodes.indexOf(node);
      if (idx !== -1) {
        allNodes.splice(idx, 1);
      }
      onChanged();
    });
    typeRow.appendChild(removeBtn);

    stepBody.appendChild(typeRow);

    // Config fields
    const def = nodeTypeRegistry.get(node.type);
    if (def && def.configSchema.length > 0) {
      const configContainer = document.createElement('div');
      configContainer.className = 'flow-step-config';
      this.renderConfigFields(configContainer, def.configSchema, node);
      stepBody.appendChild(configContainer);
    }

    step.appendChild(stepBody);
    return step;
  }

  // ── Config Field Rendering ──────────────────────────────────────

  private renderConfigFields(
    container: HTMLElement,
    schema: ConfigField[],
    node: FlowNode,
  ): void {
    for (const field of schema) {
      const fieldGroup = document.createElement('div');
      fieldGroup.className = 'flow-config-field';

      const label = document.createElement('label');
      label.className = 'flow-field-label';
      label.textContent = field.label;
      if (field.required) {
        const req = document.createElement('span');
        req.className = 'flow-field-required';
        req.textContent = '*';
        label.appendChild(req);
      }
      fieldGroup.appendChild(label);

      switch (field.type) {
        case 'string': {
          const input = document.createElement('input');
          input.type = 'text';
          input.className = 'flow-input';
          input.value = String(node.config[field.name] ?? field.defaultValue ?? '');
          input.placeholder = field.placeholder ?? '';
          input.addEventListener('input', () => {
            node.config[field.name] = input.value;
          });
          fieldGroup.appendChild(input);
          break;
        }

        case 'number': {
          const input = document.createElement('input');
          input.type = 'number';
          input.className = 'flow-input';
          input.value = String(node.config[field.name] ?? field.defaultValue ?? '');
          input.placeholder = field.placeholder ?? '';
          input.addEventListener('input', () => {
            node.config[field.name] = input.value ? Number(input.value) : undefined;
          });
          fieldGroup.appendChild(input);
          break;
        }

        case 'boolean': {
          const toggle = this.createToggle(
            Boolean(node.config[field.name] ?? field.defaultValue ?? false),
            (val) => {
              node.config[field.name] = val;
            },
          );
          fieldGroup.appendChild(toggle);
          break;
        }

        case 'select': {
          const select = document.createElement('select');
          select.className = 'flow-select';

          if (!field.required) {
            const emptyOpt = document.createElement('option');
            emptyOpt.value = '';
            emptyOpt.textContent = '-- Select --';
            select.appendChild(emptyOpt);
          }

          for (const opt of field.options ?? []) {
            const option = document.createElement('option');
            option.value = opt.value;
            option.textContent = opt.label;
            select.appendChild(option);
          }

          select.value = String(node.config[field.name] ?? field.defaultValue ?? '');
          select.addEventListener('change', () => {
            node.config[field.name] = select.value;
          });
          fieldGroup.appendChild(select);
          break;
        }

        case 'keychord': {
          this.renderChordPicker(fieldGroup, node, field.name);
          break;
        }
      }

      container.appendChild(fieldGroup);
    }
  }

  // ── Chord Picker ────────────────────────────────────────────────

  private renderChordPicker(
    container: HTMLElement,
    node: FlowNode,
    configKey: string = 'chord',
  ): void {
    const chord = node.config[configKey] as KeyChord | undefined;
    const displayText = chord && chord.key ? formatChord(chord) : 'Click to set hotkey';

    const picker = document.createElement('button');
    picker.className = 'flow-chord-picker';
    picker.textContent = displayText;

    let listening = false;

    const keyHandler = (e: KeyboardEvent) => {
      // Ignore modifier-only presses
      if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;

      e.preventDefault();
      e.stopPropagation();

      const newChord = eventToChord(e);
      node.config[configKey] = newChord;
      picker.textContent = formatChord(newChord);
      picker.classList.remove('capturing');
      this.capturing = false;
      listening = false;
      document.removeEventListener('keydown', keyHandler, true);
    };

    picker.addEventListener('click', () => {
      if (listening) {
        // Cancel capture
        picker.classList.remove('capturing');
        picker.textContent = chord && chord.key ? formatChord(chord) : 'Click to set hotkey';
        this.capturing = false;
        listening = false;
        document.removeEventListener('keydown', keyHandler, true);
        return;
      }

      listening = true;
      this.capturing = true;
      picker.classList.add('capturing');
      picker.textContent = 'Press a key combo...';
      document.addEventListener('keydown', keyHandler, true);
    });

    container.appendChild(picker);
  }

  // ── Node Type Selector Dropdown ─────────────────────────────────

  private showNodeSelector(
    anchor: HTMLElement,
    onSelect: (nodeType: string) => void,
  ): void {
    // Remove any existing selector
    const existing = document.querySelector('.flow-node-selector');
    if (existing) existing.remove();

    const dropdown = document.createElement('div');
    dropdown.className = 'flow-node-selector';

    // Position near the anchor
    const rect = anchor.getBoundingClientRect();
    dropdown.style.position = 'fixed';
    dropdown.style.left = `${rect.left}px`;
    dropdown.style.top = `${rect.bottom + 4}px`;

    const categories: NodeCategory[] = ['terminal', 'split', 'workspace', 'voice', 'quick-claude', 'control-flow', 'data'];

    for (const cat of categories) {
      const defs = nodeTypeRegistry.getByCategory(cat);
      if (defs.length === 0) continue;

      const catHeader = document.createElement('div');
      catHeader.className = 'flow-node-selector-category';
      catHeader.style.borderLeftColor = NODE_CATEGORY_COLORS[cat];
      catHeader.textContent = NODE_CATEGORY_LABELS[cat];
      dropdown.appendChild(catHeader);

      for (const def of defs) {
        const item = document.createElement('div');
        item.className = 'flow-node-selector-item';
        item.textContent = `${def.icon} ${def.label}`;
        item.title = def.description;
        item.addEventListener('click', () => {
          onSelect(def.type);
          dropdown.remove();
        });
        dropdown.appendChild(item);
      }
    }

    document.body.appendChild(dropdown);

    // Close on outside click
    const closeHandler = (e: MouseEvent) => {
      if (!dropdown.contains(e.target as Node)) {
        dropdown.remove();
        document.removeEventListener('click', closeHandler, true);
      }
    };
    // Defer so the current click doesn't immediately close it
    requestAnimationFrame(() => {
      document.addEventListener('click', closeHandler, true);
    });
  }

  // ── Import / Export ─────────────────────────────────────────────

  private handleImport(
    _container: HTMLElement,
    onEdit: (flowId: string) => void,
  ): void {
    const modal = document.createElement('div');
    modal.className = 'flow-import-modal';

    const backdrop = document.createElement('div');
    backdrop.className = 'flow-import-backdrop';
    backdrop.addEventListener('click', () => {
      modal.remove();
      backdrop.remove();
    });

    const title = document.createElement('div');
    title.className = 'flow-import-title';
    title.textContent = 'Import Flow';
    modal.appendChild(title);

    const desc = document.createElement('div');
    desc.className = 'settings-description';
    desc.textContent = 'Paste a flow JSON below or load from a file.';
    modal.appendChild(desc);

    const textarea = document.createElement('textarea');
    textarea.className = 'flow-import-textarea';
    textarea.placeholder = '{ "name": "My Flow", ... }';
    textarea.rows = 10;
    modal.appendChild(textarea);

    const errorEl = document.createElement('div');
    errorEl.className = 'flow-import-error';
    errorEl.style.display = 'none';
    modal.appendChild(errorEl);

    const btnRow = document.createElement('div');
    btnRow.className = 'flow-actions';

    const fileBtn = document.createElement('button');
    fileBtn.className = 'flow-btn flow-btn-secondary';
    fileBtn.textContent = 'Load File';
    fileBtn.addEventListener('click', () => {
      const fileInput = document.createElement('input');
      fileInput.type = 'file';
      fileInput.accept = '.json';
      fileInput.addEventListener('change', () => {
        const file = fileInput.files?.[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = () => {
          textarea.value = reader.result as string;
        };
        reader.readAsText(file);
      });
      fileInput.click();
    });
    btnRow.appendChild(fileBtn);

    const importBtn = document.createElement('button');
    importBtn.className = 'flow-btn flow-btn-primary';
    importBtn.textContent = 'Import';
    importBtn.addEventListener('click', () => {
      try {
        const flow = flowStore.importFlow(textarea.value);
        modal.remove();
        backdrop.remove();
        onEdit(flow.id);
      } catch (err) {
        errorEl.textContent = err instanceof Error ? err.message : String(err);
        errorEl.style.display = 'block';
      }
    });
    btnRow.appendChild(importBtn);

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'flow-btn flow-btn-secondary';
    cancelBtn.textContent = 'Cancel';
    cancelBtn.addEventListener('click', () => {
      modal.remove();
      backdrop.remove();
    });
    btnRow.appendChild(cancelBtn);

    modal.appendChild(btnRow);

    document.body.appendChild(backdrop);
    document.body.appendChild(modal);
  }

  private handleExport(flowId: string): void {
    const json = flowStore.exportFlow(flowId);
    if (!json) return;

    // Copy to clipboard
    navigator.clipboard.writeText(json).then(() => {
      console.info('[Flows] Flow JSON copied to clipboard');
    }).catch(() => {
      // Fallback: open in a temporary textarea
      const ta = document.createElement('textarea');
      ta.value = json;
      ta.style.position = 'fixed';
      ta.style.left = '-9999px';
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
      console.info('[Flows] Flow JSON copied to clipboard (fallback)');
    });
  }

  // ── Edge Generation ─────────────────────────────────────────────

  /**
   * Generate linear edges connecting each node to the next.
   * For Phase 1, flows are a simple chain: n1 -> n2 -> n3 -> ...
   */
  private generateLinearEdges(nodes: FlowNode[]): FlowEdge[] {
    const edges: FlowEdge[] = [];

    for (let i = 0; i < nodes.length - 1; i++) {
      const source = nodes[i];
      const target = nodes[i + 1];

      // Determine port names from definitions
      const sourceDef = nodeTypeRegistry.get(source.type);
      const targetDef = nodeTypeRegistry.get(target.type);

      const sourcePort = sourceDef?.ports.find(p => p.direction === 'output')?.name ?? 'output';
      const targetPort = targetDef?.ports.find(p => p.direction === 'input')?.name ?? 'input';

      edges.push({
        id: crypto.randomUUID(),
        sourceNodeId: source.id,
        sourcePort,
        targetNodeId: target.id,
        targetPort,
      });
    }

    return edges;
  }

  // ── Helpers ─────────────────────────────────────────────────────

  private createFieldGroup(labelText: string): HTMLElement {
    const group = document.createElement('div');
    group.className = 'flow-field-group';

    const label = document.createElement('label');
    label.className = 'flow-field-label';
    label.textContent = labelText;
    group.appendChild(label);

    return group;
  }

  private createToggle(
    checked: boolean,
    onChange: (enabled: boolean) => void,
  ): HTMLElement {
    const toggle = document.createElement('label');
    toggle.className = 'flow-toggle';

    const input = document.createElement('input');
    input.type = 'checkbox';
    input.checked = checked;
    input.addEventListener('change', () => {
      onChange(input.checked);
    });
    toggle.appendChild(input);

    const slider = document.createElement('span');
    slider.className = 'flow-toggle-slider';
    toggle.appendChild(slider);

    return toggle;
  }
}
