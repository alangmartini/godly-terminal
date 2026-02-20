const STORAGE_KEY = 'godly-settings-tab-order';

const DEFAULT_ORDER: string[] = ['themes', 'terminal', 'notifications', 'plugins', 'shortcuts'];

type Subscriber = () => void;

class SettingsTabStore {
  private tabOrder: string[] = [...DEFAULT_ORDER];
  private subscribers: Subscriber[] = [];

  constructor() {
    this.loadFromStorage();
  }

  getTabOrder(): string[] {
    return [...this.tabOrder];
  }

  setTabOrder(order: string[]): void {
    this.tabOrder = this.reconcile(order);
    this.saveToStorage();
    this.notify();
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

  /** Ensure stored order contains exactly the known tab IDs. */
  private reconcile(order: string[]): string[] {
    const known = new Set(DEFAULT_ORDER);
    // Keep only known IDs, preserving the user's order
    const result = order.filter(id => known.has(id));
    // Append any missing tabs (e.g. newly added)
    for (const id of DEFAULT_ORDER) {
      if (!result.includes(id)) result.push(id);
    }
    return result;
  }

  private loadFromStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return;
      const data = JSON.parse(raw) as unknown;
      if (!Array.isArray(data)) return;
      this.tabOrder = this.reconcile(data.filter((x): x is string => typeof x === 'string'));
    } catch {
      // Corrupt data — use defaults
    }
  }

  private saveToStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.tabOrder));
    } catch {
      // No localStorage available
    }
  }
}

export const settingsTabStore = new SettingsTabStore();
