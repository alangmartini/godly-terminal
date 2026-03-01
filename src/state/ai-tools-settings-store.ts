const STORAGE_KEY = 'godly-ai-tools-settings';

export interface CustomAiTool {
  id: string;
  name: string;
  binaryPath: string;
  launchCommand: string;
  branchSuffix: string;
}

export interface AiToolsSettings {
  customTools: CustomAiTool[];
  maxSimultaneous: number;
  branchSuffixes: Record<string, string>;
}

const BUILT_IN_SUFFIXES: Record<string, string> = {
  claude: '-cc',
  codex: '-c',
};

type Subscriber = () => void;

class AiToolsSettingsStore {
  static readonly MAX_SIMULTANEOUS = 4;
  static readonly MIN_SIMULTANEOUS = 1;

  private settings: AiToolsSettings = {
    customTools: [],
    maxSimultaneous: 2,
    branchSuffixes: { ...BUILT_IN_SUFFIXES },
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
