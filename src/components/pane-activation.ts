/**
 * Activation sequence for terminal panes.
 *
 * Extracted from TerminalPane so the sequence (fit → scrollToBottom → focus)
 * can be unit-tested without a DOM or real terminal renderer instance.
 *
 * Called inside requestAnimationFrame after the pane's CSS visibility is toggled.
 */
export function activatePane(
  terminal: { scrollToBottom: () => void; focus: () => void },
  fit: () => void,
  shouldFocus: boolean,
): void {
  fit();
  terminal.scrollToBottom();
  if (shouldFocus) {
    terminal.focus();
  }
}
