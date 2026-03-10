export const SEMANTIC_IDS = {
  // Workspace
  'workspace.sidebar': 'workspace-sidebar',
  'workspace.sidebar.toggle': 'workspace-sidebar-toggle',
  'workspace.list': 'workspace-list',
  'workspace.active': 'workspace-active',
  'workspace.add': 'workspace-add',

  // Tabs
  'tab.bar': 'tab-bar',
  'tab.active': 'tab-active',
  'tab.add': 'tab-add',

  // Panes
  'pane.active': 'pane-active',
  'pane.container': 'pane-container',

  // Terminal
  'terminal.surface': 'terminal-surface',

  // Settings
  'settings.dialog': 'settings-dialog',
  'settings.theme.select': 'settings-theme-select',

  // Quick Claude
  'quick-claude.prompt': 'quick-claude-prompt',
} as const;

export type SemanticId = keyof typeof SEMANTIC_IDS;

// Dynamic ID helpers
export function terminalSurfaceId(terminalId: string): string {
  return `terminal-surface:${terminalId}`;
}

export function tabCloseId(terminalId: string): string {
  return `tab-close:${terminalId}`;
}

export function paneDividerId(workspaceId: string): string {
  return `pane-divider:${workspaceId}`;
}
