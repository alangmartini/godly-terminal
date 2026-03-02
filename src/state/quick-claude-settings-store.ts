const STORAGE_KEY = 'godly-quick-claude-presets';

// ── Types ───────────────────────────────────────────────────────────

export type LaunchStepType =
  | 'create-terminal' | 'wait-idle' | 'run-command'
  | 'wait-ready' | 'send-prompt' | 'send-enter' | 'delay';

export interface LaunchStep {
  id: string;
  type: LaunchStepType;
  enabled: boolean;
  config: Record<string, unknown>;
}

export interface PresetAgent {
  id: string;
  toolId: string;
  label: string;
  commandOverride?: string;
  branchSuffixOverride?: string;
  steps: LaunchStep[];
}

export type PresetLayout = 'single' | 'vertical' | 'horizontal' | 'grid';

export interface QuickClaudePreset {
  id: string;
  name: string;
  description: string;
  layout: PresetLayout;
  agents: PresetAgent[];
  isDefault: boolean;
  createdAt: number;
}

// ── Layout constraints ──────────────────────────────────────────────

export const LAYOUT_MAX_AGENTS: Record<PresetLayout, number> = {
  single: 1,
  vertical: 2,
  horizontal: 2,
  grid: 4,
};

// ── Default step templates ──────────────────────────────────────────

function defaultStepsForTool(toolId: string): LaunchStep[] {
  const base: LaunchStep[] = [
    { id: crypto.randomUUID(), type: 'create-terminal', enabled: true, config: {} },
    { id: crypto.randomUUID(), type: 'wait-idle', enabled: true, config: { idleMs: 2000, timeoutMs: 30000 } },
  ];

  if (toolId === 'claude') {
    base.push(
      { id: crypto.randomUUID(), type: 'wait-ready', enabled: true, config: { marker: 'trust', timeoutMs: 15000 } },
      { id: crypto.randomUUID(), type: 'send-enter', enabled: true, config: {} },
      { id: crypto.randomUUID(), type: 'wait-ready', enabled: true, config: { marker: 'ready', timeoutMs: 30000 } },
      { id: crypto.randomUUID(), type: 'send-prompt', enabled: true, config: {} },
    );
  } else if (toolId === 'codex') {
    base.push(
      { id: crypto.randomUUID(), type: 'wait-ready', enabled: true, config: { marker: 'ready', timeoutMs: 30000 } },
      { id: crypto.randomUUID(), type: 'send-prompt', enabled: true, config: {} },
    );
  } else {
    base.push(
      { id: crypto.randomUUID(), type: 'delay', enabled: true, config: { ms: 3000 } },
      { id: crypto.randomUUID(), type: 'send-prompt', enabled: true, config: {} },
    );
  }

  return base;
}

// ── Built-in presets ────────────────────────────────────────────────

const BUILT_IN_PRESET_IDS = ['builtin-solo-claude', 'builtin-solo-codex', 'builtin-claude-codex-split'];

function createBuiltInPresets(): QuickClaudePreset[] {
  return [
    {
      id: 'builtin-solo-claude',
      name: 'Solo Claude',
      description: 'Single Claude Code session',
      layout: 'single',
      agents: [{
        id: crypto.randomUUID(),
        toolId: 'claude',
        label: 'Claude Code',
        steps: defaultStepsForTool('claude'),
      }],
      isDefault: true,
      createdAt: 0,
    },
    {
      id: 'builtin-solo-codex',
      name: 'Solo Codex',
      description: 'Single Codex session',
      layout: 'single',
      agents: [{
        id: crypto.randomUUID(),
        toolId: 'codex',
        label: 'Codex',
        steps: defaultStepsForTool('codex'),
      }],
      isDefault: false,
      createdAt: 0,
    },
    {
      id: 'builtin-claude-codex-split',
      name: 'Claude + Codex Split',
      description: 'Claude and Codex side by side',
      layout: 'vertical',
      agents: [
        {
          id: crypto.randomUUID(),
          toolId: 'claude',
          label: 'Claude Code',
          steps: defaultStepsForTool('claude'),
        },
        {
          id: crypto.randomUUID(),
          toolId: 'codex',
          label: 'Codex',
          steps: defaultStepsForTool('codex'),
        },
      ],
      isDefault: false,
      createdAt: 0,
    },
  ];
}

// ── Store ───────────────────────────────────────────────────────────

type Subscriber = () => void;

class QuickClaudeSettingsStore {
  private presets: QuickClaudePreset[] = [];
  private subscribers: Subscriber[] = [];

  constructor() {
    this.loadFromStorage();
    this.ensureBuiltInPresets();
  }

  // ── Preset CRUD ─────────────────────────────────────────────────

  getPresets(): QuickClaudePreset[] {
    return this.presets.map(p => structuredClone(p));
  }

  getPreset(id: string): QuickClaudePreset | undefined {
    const preset = this.presets.find(p => p.id === id);
    return preset ? structuredClone(preset) : undefined;
  }

  getDefaultPreset(): QuickClaudePreset | undefined {
    const preset = this.presets.find(p => p.isDefault) ?? this.presets[0];
    return preset ? structuredClone(preset) : undefined;
  }

  addPreset(preset: QuickClaudePreset): void {
    if (this.presets.some(p => p.id === preset.id)) return;
    this.presets.push(structuredClone(preset));
    this.saveAndNotify();
  }

  updatePreset(id: string, updates: Partial<Omit<QuickClaudePreset, 'id'>>): void {
    const preset = this.presets.find(p => p.id === id);
    if (!preset) return;

    if (updates.name !== undefined) preset.name = updates.name;
    if (updates.description !== undefined) preset.description = updates.description;
    if (updates.layout !== undefined) preset.layout = updates.layout;
    if (updates.agents !== undefined) preset.agents = structuredClone(updates.agents);
    if (updates.isDefault !== undefined) {
      if (updates.isDefault) {
        for (const p of this.presets) p.isDefault = false;
      }
      preset.isDefault = updates.isDefault;
    }

    this.saveAndNotify();
  }

  deletePreset(id: string): void {
    if (BUILT_IN_PRESET_IDS.includes(id)) return;
    const wasDefault = this.presets.find(p => p.id === id)?.isDefault;
    this.presets = this.presets.filter(p => p.id !== id);
    if (wasDefault && this.presets.length > 0) {
      this.presets[0].isDefault = true;
    }
    this.saveAndNotify();
  }

  duplicatePreset(id: string): QuickClaudePreset | undefined {
    const source = this.presets.find(p => p.id === id);
    if (!source) return undefined;

    const copy = structuredClone(source);
    copy.id = crypto.randomUUID();
    copy.name = `${source.name} (Copy)`;
    copy.isDefault = false;
    copy.createdAt = Date.now();
    // Give new IDs to agents and steps
    for (const agent of copy.agents) {
      agent.id = crypto.randomUUID();
      for (const step of agent.steps) {
        step.id = crypto.randomUUID();
      }
    }

    this.presets.push(copy);
    this.saveAndNotify();
    return structuredClone(copy);
  }

  setDefault(id: string): void {
    const preset = this.presets.find(p => p.id === id);
    if (!preset) return;
    for (const p of this.presets) p.isDefault = false;
    preset.isDefault = true;
    this.saveAndNotify();
  }

  // ── Agent CRUD ──────────────────────────────────────────────────

  addAgent(presetId: string, toolId: string): PresetAgent | undefined {
    const preset = this.presets.find(p => p.id === presetId);
    if (!preset) return undefined;
    const max = LAYOUT_MAX_AGENTS[preset.layout];
    if (preset.agents.length >= max) return undefined;

    const agent: PresetAgent = {
      id: crypto.randomUUID(),
      toolId,
      label: toolId === 'claude' ? 'Claude Code' : toolId === 'codex' ? 'Codex' : toolId,
      steps: defaultStepsForTool(toolId),
    };
    preset.agents.push(agent);
    this.saveAndNotify();
    return structuredClone(agent);
  }

  updateAgent(presetId: string, agentId: string, updates: Partial<Omit<PresetAgent, 'id' | 'steps'>>): void {
    const preset = this.presets.find(p => p.id === presetId);
    if (!preset) return;
    const agent = preset.agents.find(a => a.id === agentId);
    if (!agent) return;
    Object.assign(agent, updates);
    this.saveAndNotify();
  }

  removeAgent(presetId: string, agentId: string): void {
    const preset = this.presets.find(p => p.id === presetId);
    if (!preset) return;
    preset.agents = preset.agents.filter(a => a.id !== agentId);
    this.saveAndNotify();
  }

  reorderAgents(presetId: string, agentIds: string[]): void {
    const preset = this.presets.find(p => p.id === presetId);
    if (!preset) return;
    const reordered: PresetAgent[] = [];
    for (const id of agentIds) {
      const agent = preset.agents.find(a => a.id === id);
      if (agent) reordered.push(agent);
    }
    preset.agents = reordered;
    this.saveAndNotify();
  }

  // ── Step operations ─────────────────────────────────────────────

  updateStep(presetId: string, agentId: string, stepId: string, updates: Partial<Omit<LaunchStep, 'id'>>): void {
    const step = this.findStep(presetId, agentId, stepId);
    if (!step) return;
    Object.assign(step, updates);
    this.saveAndNotify();
  }

  reorderSteps(presetId: string, agentId: string, stepIds: string[]): void {
    const preset = this.presets.find(p => p.id === presetId);
    if (!preset) return;
    const agent = preset.agents.find(a => a.id === agentId);
    if (!agent) return;
    const reordered: LaunchStep[] = [];
    for (const id of stepIds) {
      const step = agent.steps.find(s => s.id === id);
      if (step) reordered.push(step);
    }
    agent.steps = reordered;
    this.saveAndNotify();
  }

  resetStepsToDefault(presetId: string, agentId: string): void {
    const preset = this.presets.find(p => p.id === presetId);
    if (!preset) return;
    const agent = preset.agents.find(a => a.id === agentId);
    if (!agent) return;
    agent.steps = defaultStepsForTool(agent.toolId);
    this.saveAndNotify();
  }

  // ── Subscriptions ───────────────────────────────────────────────

  subscribe(fn: Subscriber): () => void {
    this.subscribers.push(fn);
    return () => {
      this.subscribers = this.subscribers.filter(s => s !== fn);
    };
  }

  // ── Helpers for default steps ───────────────────────────────────

  getDefaultStepsForTool(toolId: string): LaunchStep[] {
    return defaultStepsForTool(toolId);
  }

  // ── Private ─────────────────────────────────────────────────────

  private findStep(presetId: string, agentId: string, stepId: string): LaunchStep | undefined {
    const preset = this.presets.find(p => p.id === presetId);
    if (!preset) return undefined;
    const agent = preset.agents.find(a => a.id === agentId);
    if (!agent) return undefined;
    return agent.steps.find(s => s.id === stepId);
  }

  private ensureBuiltInPresets(): void {
    const existing = new Set(this.presets.map(p => p.id));
    const builtIns = createBuiltInPresets();
    let changed = false;
    for (const preset of builtIns) {
      if (!existing.has(preset.id)) {
        this.presets.unshift(preset);
        changed = true;
      }
    }
    if (changed) this.saveToStorage();
  }

  private saveAndNotify(): void {
    this.saveToStorage();
    this.notify();
  }

  private notify(): void {
    for (const fn of this.subscribers) fn();
  }

  private loadFromStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return;
      const data = JSON.parse(raw) as unknown;
      if (!Array.isArray(data)) return;
      this.presets = data.filter(
        (p): p is QuickClaudePreset =>
          typeof p === 'object' && p !== null &&
          typeof p.id === 'string' && typeof p.name === 'string' &&
          typeof p.layout === 'string' && Array.isArray(p.agents),
      );
    } catch {
      // Corrupt data — use defaults
    }
  }

  private saveToStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.presets));
    } catch {
      // No localStorage available
    }
  }
}

export const quickClaudeSettingsStore = new QuickClaudeSettingsStore();
