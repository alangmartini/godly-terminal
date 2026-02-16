/**
 * Theme type definitions for Godly Terminal.
 *
 * Themes control both the terminal ANSI color palette and the surrounding
 * UI chrome (sidebar, tabs, dialogs). A single ThemeDefinition bundles
 * both layers so switching themes feels cohesive.
 */

/** Terminal color palette — the 16 ANSI colors plus cursor/selection. */
export interface TerminalTheme {
  background: string;
  foreground: string;
  cursor: string;
  cursorAccent: string;
  selectionBackground: string;
  black: string;
  red: string;
  green: string;
  yellow: string;
  blue: string;
  magenta: string;
  cyan: string;
  white: string;
  brightBlack: string;
  brightRed: string;
  brightGreen: string;
  brightYellow: string;
  brightBlue: string;
  brightMagenta: string;
  brightCyan: string;
  brightWhite: string;
}

/** UI chrome colors derived from (or complementing) the terminal palette. */
export interface UiTheme {
  bgPrimary: string;
  bgSecondary: string;
  bgTertiary: string;
  bgActive: string;
  textPrimary: string;
  textSecondary: string;
  textActive: string;
  accent: string;
  accentHover: string;
  borderColor: string;
  danger: string;
  success: string;
}

/** Full theme definition with metadata. */
export interface ThemeDefinition {
  /** Machine-readable id (e.g. 'tokyo-night', 'dusk'). Must be unique. */
  id: string;
  /** Human-readable name shown in settings UI. */
  name: string;
  /** Short description shown below the name. */
  description: string;
  /** Author / attribution. */
  author: string;
  /** True for themes that ship with the app. User themes are false. */
  builtin: boolean;
  /** Terminal ANSI palette. */
  terminal: TerminalTheme;
  /** UI chrome colors. */
  ui: UiTheme;
}
