const STORAGE_KEY = 'godly-ai-tools-settings';

export interface CustomAiTool {
  id: string;
  name: string;
  binaryPath: string;
  launchCommand: string;
  branchSuffix: string;
}

/** Overrides for built-in agents (claude, codex). Empty strings = use default. */
export interface BuiltInOverride {
  binaryPath: string;
  args: string;
}

/** Unified view of any agent (built-in or custom). */
export interface AgentDefinition {
  id: string;
  name: string;
  builtin: boolean;
  binaryPath: string;
  args: string;
  branchSuffix: string;
}

export interface AiToolsSettings {
  customTools: CustomAiTool[];
  maxSimultaneous: number;
  branchSuffixes: Record<string, string>;
  builtInOverrides: Record<string, BuiltInOverride>;
}

const BUILT_IN_SUFFIXES: Record<string, string> = {
  claude: '-cc',
  codex: '-c',
};

const BUILT_IN_DEFAULTS: Record<string, { name: string; binaryPath: string; args: string }> = {
  claude: { name: 'Claude Code', binaryPath: 'claude', args: '' },
  codex: { name: 'Codex', binaryPath: 'codex', args: '' },
};

type Subscriber = () => void;

class AiToolsSettingsStore {
  static readonly MAX_SIMULTANEOUS = 4;
  static readonly MIN_SIMULTANEOUS = 1;

  private settings: AiToolsSettings = {
    customTools: [],
    maxSimultaneous: 2,
    branchSuffixes: { ...BUILT_IN_SUFFIXES },
    builtInOverrides: {},
  };

  private subscribers: Subscriber[] = [];

  constructor() {
    this.loadFromStorage();
  }

  getCustomTools(): CustomAiTool[] {
    return [...this.settings.customTools];
  }

  getCustomTool(id: string): CustomAiTool | undefined {
    return this.settings.customTools.find(t => t.id === id);
  }

  addCustomTool(tool: CustomAiTool): void {
    if (this.settings.customTools.some(t => t.id === tool.id)) return;
    this.settings.customTools.push({ ...tool });
    this.saveToStorage();
    this.notify();
  }

  updateCustomTool(id: string, updates: Partial<Omit<CustomAiTool, 'id'>>): void {
    const tool = this.settings.customTools.find(t => t.id === id);
    if (!tool) return;
    Object.assign(tool, updates);
    this.saveToStorage();
    this.notify();
  }

  removeCustomTool(id: string): void {
    this.settings.customTools = this.settings.customTools.filter(t => t.id !== id);
    delete this.settings.branchSuffixes[id];
    this.saveToStorage();
    this.notify();
  }

  getMaxSimultaneous(): number {
    return this.settings.maxSimultaneous;
  }

  setMaxSimultaneous(n: number): void {
    const clamped = Math.max(
      AiToolsSettingsStore.MIN_SIMULTANEOUS,
      Math.min(AiToolsSettingsStore.MAX_SIMULTANEOUS, Math.round(n)),
    );
    if (clamped === this.settings.maxSimultaneous) return;
    this.settings.maxSimultaneous = clamped;
    this.saveToStorage();
    this.notify();
  }

  getBranchSuffix(toolId: string): string {
    return this.settings.branchSuffixes[toolId] ?? '';
  }

  setBranchSuffix(toolId: string, suffix: string): void {
    this.settings.branchSuffixes[toolId] = suffix;
    this.saveToStorage();
    this.notify();
  }

  getAllBranchSuffixes(): Record<string, string> {
    return { ...this.settings.branchSuffixes };
  }

  /** Returns all available tool options (built-in + custom) for Quick Claude dialog. */
  getAllToolOptions(): { id: string; name: string; builtin: boolean }[] {
    const options: { id: string; name: string; builtin: boolean }[] = [
      { id: 'claude', name: 'Claude Code', builtin: true },
      { id: 'codex', name: 'Codex', builtin: true },
      { id: 'both', name: 'Both (Claude + Codex)', builtin: true },
    ];
    for (const tool of this.settings.customTools) {
      options.push({ id: tool.id, name: tool.name, builtin: false });
    }
    return options;
  }

  // ── Built-in overrides ──────────────────────────────────────────

  getBuiltInOverride(toolId: string): BuiltInOverride {
    return this.settings.builtInOverrides[toolId] ?? { binaryPath: '', args: '' };
  }

  setBuiltInOverride(toolId: string, override: Partial<BuiltInOverride>): void {
    if (!BUILT_IN_DEFAULTS[toolId]) return;
    const current = this.settings.builtInOverrides[toolId] ?? { binaryPath: '', args: '' };
    this.settings.builtInOverrides[toolId] = { ...current, ...override };
    this.saveToStorage();
    this.notify();
  }

  // ── Unified agent definitions ───────────────────────────────────

  /** Returns a unified AgentDefinition for any tool (built-in or custom). */
  getAgentDefinition(toolId: string): AgentDefinition | undefined {
    const builtIn = BUILT_IN_DEFAULTS[toolId];
    if (builtIn) {
      const override = this.settings.builtInOverrides[toolId];
      return {
        id: toolId,
        name: builtIn.name,
        builtin: true,
        binaryPath: override?.binaryPath || builtIn.binaryPath,
        args: override?.args || builtIn.args,
        branchSuffix: this.settings.branchSuffixes[toolId] ?? '',
      };
    }
    const custom = this.settings.customTools.find(t => t.id === toolId);
    if (custom) {
      return {
        id: custom.id,
        name: custom.name,
        builtin: false,
        binaryPath: custom.binaryPath,
        args: custom.launchCommand,
        branchSuffix: custom.branchSuffix,
      };
    }
    return undefined;
  }

  /** Returns all agent definitions (built-in + custom), excluding meta-options like 'both'. */
  getAllAgentDefinitions(): AgentDefinition[] {
    const agents: AgentDefinition[] = [];
    for (const [id, defaults] of Object.entries(BUILT_IN_DEFAULTS)) {
      const override = this.settings.builtInOverrides[id];
      agents.push({
        id,
        name: defaults.name,
        builtin: true,
        binaryPath: override?.binaryPath || defaults.binaryPath,
        args: override?.args || defaults.args,
        branchSuffix: this.settings.branchSuffixes[id] ?? '',
      });
    }
    for (const tool of this.settings.customTools) {
      agents.push({
        id: tool.id,
        name: tool.name,
        builtin: false,
        binaryPath: tool.binaryPath,
        args: tool.launchCommand,
        branchSuffix: tool.branchSuffix,
      });
    }
    return agents;
  }

  subscribe(fn: Subscriber): () => void {
    this.subscribers.push(fn);
    return () => {
      this.subscribers = this.subscribers.filter(s => s !== fn);
    };
  }

  private notify(): void {
    for (const fn of this.subscribers) fn();
  }

  private loadFromStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return;
      const data = JSON.parse(raw) as Partial<AiToolsSettings>;
      if (Array.isArray(data.customTools)) {
        this.settings.customTools = data.customTools.filter(
          (t): t is CustomAiTool =>
            typeof t === 'object' && t !== null &&
            typeof t.id === 'string' && typeof t.name === 'string' &&
            typeof t.binaryPath === 'string' && typeof t.launchCommand === 'string' &&
            typeof t.branchSuffix === 'string',
        );
      }
      if (typeof data.maxSimultaneous === 'number' &&
          data.maxSimultaneous >= AiToolsSettingsStore.MIN_SIMULTANEOUS &&
          data.maxSimultaneous <= AiToolsSettingsStore.MAX_SIMULTANEOUS) {
        this.settings.maxSimultaneous = data.maxSimultaneous;
      }
      if (data.branchSuffixes && typeof data.branchSuffixes === 'object') {
        this.settings.branchSuffixes = {
          ...BUILT_IN_SUFFIXES,
          ...data.branchSuffixes,
        };
      }
      if (data.builtInOverrides && typeof data.builtInOverrides === 'object') {
        const overrides: Record<string, BuiltInOverride> = {};
        for (const [key, val] of Object.entries(data.builtInOverrides)) {
          if (val && typeof val === 'object' && typeof (val as any).binaryPath === 'string') {
            overrides[key] = {
              binaryPath: (val as BuiltInOverride).binaryPath,
              args: typeof (val as BuiltInOverride).args === 'string' ? (val as BuiltInOverride).args : '',
            };
          }
        }
        this.settings.builtInOverrides = overrides;
      }
    } catch {
      // Corrupt data — use defaults
    }
  }

  private saveToStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.settings));
    } catch {
      // No localStorage available
    }
  }
}

export const aiToolsSettingsStore = new AiToolsSettingsStore();
