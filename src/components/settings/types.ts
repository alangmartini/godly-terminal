export interface SettingsTabProvider {
  id: string;
  label: string;
  buildContent(dialog: SettingsDialogContext): HTMLDivElement;
  onDialogClose?(): void;
  /** Returns true if the tab is in a modal state that should block Escape from closing the dialog. */
  isCapturing?(): boolean;
}

/** Minimal interface for tab providers that need dialog context. */
export interface SettingsDialogContext {
  /** Re-render the tab bar (used after drag-drop or order changes). */
  renderTabBar(): void;
}
