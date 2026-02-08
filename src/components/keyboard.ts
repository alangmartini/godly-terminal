/**
 * Returns true if the keyboard event is an app-level shortcut that should NOT
 * be processed by xterm.js. When the custom key handler returns false for these,
 * the event bubbles to the document-level listener in App.ts.
 *
 * Bug: without this, shortcuts like Ctrl+T stop working when text is selected
 * in the terminal because xterm.js consumes the event as terminal input.
 */
export function isAppShortcut(event: { ctrlKey: boolean; shiftKey: boolean; key: string; type: string }): boolean {
  if (event.type !== 'keydown') return false;
  if (!event.ctrlKey) return false;

  const key = event.key;

  // Ctrl+T (new terminal), Ctrl+W (close terminal)
  if (!event.shiftKey && (key === 't' || key === 'w')) return true;

  // Ctrl+Tab / Ctrl+Shift+Tab (switch tabs)
  if (key === 'Tab') return true;

  // Ctrl+Shift+S (manual save), Ctrl+Shift+L (manual load), Ctrl+Shift+C (copy)
  if (event.shiftKey && (key === 'S' || key === 'L' || key === 'C')) return true;

  return false;
}
