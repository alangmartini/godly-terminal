/**
 * Creates a key event handler for terminal input.
 *
 * Returns false to block the event from reaching the terminal,
 * true to let it pass through normally.
 */
export function createTerminalKeyHandler(
  writeData: (data: string) => void,
  getSelection: () => string,
  copyToClipboard: (text: string) => void,
): (event: KeyboardEvent) => boolean {
  return (event: KeyboardEvent): boolean => {
    // Ctrl+Shift+C: copy selection to clipboard
    if (event.ctrlKey && event.shiftKey && event.key === 'C' && event.type === 'keydown') {
      const selection = getSelection();
      if (selection) {
        copyToClipboard(selection);
      }
      return false;
    }

    // Shift+Enter: send CSI 13;2u (kitty keyboard protocol) so CLI tools
    // like Claude Code can distinguish it from plain Enter.
    if (event.shiftKey && !event.ctrlKey && event.key === 'Enter') {
      if (event.type === 'keydown') {
        writeData('\x1b[13;2u');
      }
      return false;
    }

    return true;
  };
}
