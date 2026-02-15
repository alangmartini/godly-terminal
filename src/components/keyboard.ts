import { keybindingStore } from '../state/keybinding-store';

/**
 * Returns true if the keyboard event is an app-level shortcut that should NOT
 * be sent to the PTY. These events bubble to the document-level listener in App.ts.
 *
 * Delegates to KeybindingStore so users can customise bindings via settings.
 *
 * Hardcoded escapes (not customisable):
 * - Ctrl+Shift+S (debug save), Ctrl+Shift+L (debug load), Ctrl+, (settings)
 */
export function isAppShortcut(event: { ctrlKey: boolean; shiftKey: boolean; altKey?: boolean; key: string; type: string }): boolean {
  if (event.type !== 'keydown') return false;
  if (!event.ctrlKey) return false;

  // Hardcoded shortcuts that should always bubble (not customisable)
  if (event.shiftKey && (event.key === 'S' || event.key === 'L')) return true;
  // Ctrl+, (open settings)
  if (!event.shiftKey && event.key === ',') return true;

  return keybindingStore.isAppShortcut({
    ctrlKey: event.ctrlKey,
    shiftKey: event.shiftKey,
    altKey: (event as any).altKey ?? false,
    key: event.key,
    type: event.type,
  });
}

/**
 * Returns true if the keyboard event is a terminal control key whose browser
 * default action (clipboard copy, undo, etc.) must be prevented so the
 * key reaches the PTY as a control character instead.
 *
 * Delegates to KeybindingStore so users can customise bindings via settings.
 */
export function isTerminalControlKey(event: { ctrlKey: boolean; shiftKey: boolean; altKey?: boolean; key: string; type: string }): boolean {
  return keybindingStore.isTerminalControlKey({
    ctrlKey: event.ctrlKey,
    shiftKey: event.shiftKey,
    altKey: (event as any).altKey ?? false,
    key: event.key,
    type: event.type,
  });
}
