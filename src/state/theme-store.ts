import type { ThemeDefinition, TerminalTheme, UiTheme } from '../themes/types';
import { BUILTIN_THEMES, TOKYO_NIGHT } from '../themes/builtin';

const STORAGE_KEY = 'godly-theme-settings';

interface ThemeSettings {
  activeThemeId: string;
  customThemes: ThemeDefinition[];
}

type Subscriber = () => void;

class ThemeStore {
  private activeThemeId: string = 'tokyo-night';
  private customThemes: ThemeDefinition[] = [];
  private subscribers: Subscriber[] = [];

  constructor() {
    this.loadFromStorage();
    this.applyUiTheme();
  }

  // ── Queries ──────────────────────────────────────────────────────

  getActiveTheme(): ThemeDefinition {
    const all = this.getAllThemes();
    return all.find(t => t.id === this.activeThemeId) ?? TOKYO_NIGHT;
  }

  getTerminalTheme(): TerminalTheme {
    return this.getActiveTheme().terminal;
  }

  getUiTheme(): UiTheme {
    return this.getActiveTheme().ui;
  }

  getAllThemes(): ThemeDefinition[] {
    return [...BUILTIN_THEMES, ...this.customThemes];
  }

  // ── Mutations ────────────────────────────────────────────────────

  setActiveTheme(id: string): void {
    this.activeThemeId = id;
    this.saveToStorage();
    this.applyUiTheme();
    this.notify();
  }

  addCustomTheme(theme: ThemeDefinition): void {
    this.customThemes.push(theme);
    this.saveToStorage();
    this.notify();
  }

  removeCustomTheme(id: string): void {
    const isBuiltin = BUILTIN_THEMES.some(t => t.id === id);
    if (isBuiltin) return;
    this.customThemes = this.customThemes.filter(t => t.id !== id);
    if (this.activeThemeId === id) {
      this.activeThemeId = 'tokyo-night';
      this.applyUiTheme();
    }
    this.saveToStorage();
    this.notify();
  }

  // ── Subscriptions ────────────────────────────────────────────────

  subscribe(fn: Subscriber): () => void {
    this.subscribers.push(fn);
    return () => {
      this.subscribers = this.subscribers.filter(s => s !== fn);
    };
  }

  private notify(): void {
    for (const fn of this.subscribers) fn();
  }

  // ── Persistence ──────────────────────────────────────────────────

  private loadFromStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return;
      const data = JSON.parse(raw) as Partial<ThemeSettings>;
      if (typeof data.activeThemeId === 'string') this.activeThemeId = data.activeThemeId;
      if (Array.isArray(data.customThemes)) this.customThemes = data.customThemes;
    } catch {
      // Corrupt data — use defaults
    }
  }

  private saveToStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      const data: ThemeSettings = {
        activeThemeId: this.activeThemeId,
        customThemes: this.customThemes,
      };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
    } catch {
      // No localStorage available
    }
  }

  // ── UI theme application ─────────────────────────────────────────

  private applyUiTheme(): void {
    if (typeof document === 'undefined') return;
    const ui = this.getUiTheme();
    const style = document.documentElement.style;
    style.setProperty('--bg-primary', ui.bgPrimary);
    style.setProperty('--bg-secondary', ui.bgSecondary);
    style.setProperty('--bg-tertiary', ui.bgTertiary);
    style.setProperty('--bg-active', ui.bgActive);
    style.setProperty('--text-primary', ui.textPrimary);
    style.setProperty('--text-secondary', ui.textSecondary);
    style.setProperty('--text-active', ui.textActive);
    style.setProperty('--accent', ui.accent);
    style.setProperty('--accent-hover', ui.accentHover);
    style.setProperty('--border-color', ui.borderColor);
    style.setProperty('--danger', ui.danger);
    style.setProperty('--success', ui.success);
  }
}

export const themeStore = new ThemeStore();
