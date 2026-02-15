import { invoke } from '@tauri-apps/api/core';
import { terminalService } from '../services/terminal-service';
import { store } from '../state/store';
import { isAppShortcut, isTerminalControlKey } from './keyboard';
import { keybindingStore } from '../state/keybinding-store';
import { TerminalRenderer, RichGridData } from './TerminalRenderer';

/**
 * Terminal pane backed by the godly-vt Canvas2D renderer.
 *
 * The daemon's godly-vt parser maintains the terminal grid. On each
 * `terminal-output` event, we request a grid snapshot via Tauri IPC
 * and paint it on a <canvas> via TerminalRenderer.
 */
export class TerminalPane {
  private renderer: TerminalRenderer;
  private container: HTMLElement;
  private terminalId: string;
  private resizeObserver: ResizeObserver;
  private resizeRAF: number | null = null;
  private unsubscribeOutput: (() => void) | null = null;

  // Debounce grid snapshot requests: on terminal-output we schedule a snapshot
  // fetch via setTimeout(0). Multiple output events within the same frame
  // collapse into a single IPC call.
  private snapshotPending = false;
  private snapshotTimer: ReturnType<typeof setTimeout> | null = null;

  // Scrollback state: tracked from the latest snapshot
  private scrollbackOffset = 0;
  private totalScrollback = 0;

  // Scrollback save interval (every 5 minutes)
  private scrollbackSaveInterval: number | null = null;

  constructor(terminalId: string) {
    this.terminalId = terminalId;

    this.renderer = new TerminalRenderer();

    // Forward OSC title changes to the store
    this.renderer.setOnTitleChange((title) => {
      store.updateTerminal(this.terminalId, { oscTitle: title || undefined });
    });

    // Handle wheel scroll events from the renderer
    this.renderer.setOnScroll((deltaLines) => {
      this.handleScroll(deltaLines);
    });

    this.container = document.createElement('div');
    this.container.className = 'terminal-pane';
    this.container.dataset.terminalId = terminalId;

    this.resizeObserver = new ResizeObserver(() => {
      if (this.resizeRAF) cancelAnimationFrame(this.resizeRAF);
      this.resizeRAF = requestAnimationFrame(() => this.fit());
    });
  }

  mount(parent: HTMLElement) {
    parent.appendChild(this.container);
    this.container.appendChild(this.renderer.getElement());
    const overlay = this.renderer.getOverlayElement();
    if (overlay) {
      this.container.appendChild(overlay);
    }
    this.resizeObserver.observe(this.container);

    // Click-to-focus in split mode: set this terminal as active
    this.container.addEventListener('mousedown', () => {
      if (this.container.classList.contains('split-visible')) {
        store.setActiveTerminal(this.terminalId);
      }
    });

    // Handle keyboard input on the canvas.
    // The canvas has tabIndex so it receives keyboard events.
    const canvas = this.renderer.getElement();
    canvas.addEventListener('keydown', (event) => {
      this.handleKeyEvent(event);
    });
    canvas.addEventListener('keyup', (event) => {
      // Prevent WebView2 from intercepting terminal control keys on keyup
      if (isTerminalControlKey({
        ctrlKey: event.ctrlKey,
        shiftKey: event.shiftKey,
        altKey: event.altKey,
        key: event.key,
        type: 'keydown',
      })) {
        event.preventDefault();
      }
    });

    // Listen for terminal output events.
    // When new PTY output arrives, schedule a grid snapshot fetch.
    this.unsubscribeOutput = terminalService.onTerminalOutput(
      this.terminalId,
      () => {
        this.scheduleSnapshotFetch();
      }
    );

    // Start periodic scrollback saving (every 5 minutes)
    this.startScrollbackSaveInterval();

    // Initial fit + snapshot
    requestAnimationFrame(() => {
      this.fit();
      this.fetchAndRenderSnapshot();
    });
  }

  // ---- Keyboard handling ----

  private handleKeyEvent(event: KeyboardEvent) {
    const action = keybindingStore.matchAction(event);

    // Scroll shortcuts: handle locally without sending to PTY
    if (action === 'scroll.pageUp') {
      event.preventDefault();
      this.handleScroll(this.renderer.getGridSize().rows);
      return;
    }
    if (action === 'scroll.pageDown') {
      event.preventDefault();
      this.handleScroll(-this.renderer.getGridSize().rows);
      return;
    }
    if (action === 'scroll.toTop') {
      event.preventDefault();
      this.handleScroll(this.totalScrollback);
      return;
    }
    if (action === 'scroll.toBottom') {
      event.preventDefault();
      this.handleScroll(-this.scrollbackOffset);
      return;
    }

    // Copy: copy selected text to clipboard
    if (action === 'clipboard.copy') {
      event.preventDefault();
      if (this.renderer.hasSelection()) {
        this.renderer.getSelectedText(this.terminalId).then((text) => {
          if (text) {
            navigator.clipboard.writeText(text);
          }
        });
      }
      return;
    }

    // Paste: paste from clipboard into terminal
    if (action === 'clipboard.paste') {
      event.preventDefault();
      navigator.clipboard.readText().then((text) => {
        if (text) {
          terminalService.writeToTerminal(this.terminalId, text);
        }
      });
      return;
    }

    // App-level shortcuts should bubble to App.ts
    if (isAppShortcut(event)) {
      return; // Don't prevent default -- let it bubble
    }

    // Prevent WebView2 from intercepting terminal control keys
    if (isTerminalControlKey(event)) {
      event.preventDefault();
    }

    // Shift+Enter: send CSI 13;2u (kitty keyboard protocol)
    if (event.shiftKey && !event.ctrlKey && event.key === 'Enter') {
      event.preventDefault();
      terminalService.writeToTerminal(this.terminalId, '\x1b[13;2u');
      return;
    }

    // Snap to bottom on any input when scrolled up
    if (this.scrollbackOffset > 0) {
      this.snapToBottom();
    }

    // Convert keyboard events to terminal input data
    const data = this.keyToTerminalData(event);
    if (data) {
      event.preventDefault();
      terminalService.writeToTerminal(this.terminalId, data);
    }
  }

  /**
   * Convert a keyboard event into the string that should be sent to the PTY.
   * Handles control characters, special keys, and printable input.
   */
  private keyToTerminalData(event: KeyboardEvent): string | null {
    // Control key combinations -> control characters
    if (event.ctrlKey && !event.altKey && !event.shiftKey) {
      const key = event.key.toLowerCase();
      if (key.length === 1 && key >= 'a' && key <= 'z') {
        return String.fromCharCode(key.charCodeAt(0) - 96);
      }
      // Ctrl+[ -> ESC, Ctrl+\ -> FS, Ctrl+] -> GS, Ctrl+^ -> RS, Ctrl+_ -> US
      if (key === '[') return '\x1b';
      if (key === '\\') return '\x1c';
      if (key === ']') return '\x1d';
      // Ctrl+Space -> NUL
      if (key === ' ' || event.code === 'Space') return '\x00';
    }

    // Alt combinations -> ESC + key
    if (event.altKey && !event.ctrlKey && event.key.length === 1) {
      return '\x1b' + event.key;
    }

    // Special keys
    switch (event.key) {
      case 'Enter': return '\r';
      case 'Backspace': return '\x7f';
      case 'Tab': return '\t';
      case 'Escape': return '\x1b';
      case 'Delete': return '\x1b[3~';
      case 'ArrowUp': return '\x1b[A';
      case 'ArrowDown': return '\x1b[B';
      case 'ArrowRight': return '\x1b[C';
      case 'ArrowLeft': return '\x1b[D';
      case 'Home': return '\x1b[H';
      case 'End': return '\x1b[F';
      case 'PageUp': return '\x1b[5~';
      case 'PageDown': return '\x1b[6~';
      case 'Insert': return '\x1b[2~';
      case 'F1': return '\x1bOP';
      case 'F2': return '\x1bOQ';
      case 'F3': return '\x1bOR';
      case 'F4': return '\x1bOS';
      case 'F5': return '\x1b[15~';
      case 'F6': return '\x1b[17~';
      case 'F7': return '\x1b[18~';
      case 'F8': return '\x1b[19~';
      case 'F9': return '\x1b[20~';
      case 'F10': return '\x1b[21~';
      case 'F11': return '\x1b[23~';
      case 'F12': return '\x1b[24~';
    }

    // Printable characters
    if (event.key.length === 1 && !event.ctrlKey && !event.altKey) {
      return event.key;
    }

    return null;
  }

  // ---- Scroll handling ----

  /**
   * Handle a scroll request. deltaLines > 0 = scroll up (into history),
   * deltaLines < 0 = scroll down (toward live view).
   */
  private handleScroll(deltaLines: number) {
    const newOffset = Math.max(0, this.scrollbackOffset + deltaLines);
    if (newOffset === this.scrollbackOffset) return;
    this.scrollbackOffset = newOffset;
    terminalService.setScrollback(this.terminalId, newOffset).then(() => {
      this.fetchAndRenderSnapshot();
    });
  }

  /** Snap viewport back to live view (offset 0). */
  private snapToBottom() {
    if (this.scrollbackOffset === 0) return;
    this.scrollbackOffset = 0;
    terminalService.setScrollback(this.terminalId, 0).then(() => {
      this.fetchAndRenderSnapshot();
    });
  }

  // ---- Grid snapshot fetching ----

  private scheduleSnapshotFetch() {
    if (this.snapshotPending) return;
    this.snapshotPending = true;
    if (this.snapshotTimer === null) {
      this.snapshotTimer = setTimeout(() => {
        this.snapshotTimer = null;
        this.snapshotPending = false;
        this.fetchAndRenderSnapshot();
      }, 0);
    }
  }

  private async fetchAndRenderSnapshot() {
    try {
      const snapshot = await invoke<RichGridData>('get_grid_snapshot', {
        terminalId: this.terminalId,
      });
      // Track scrollback state from the daemon's authoritative values
      this.scrollbackOffset = snapshot.scrollback_offset;
      this.totalScrollback = snapshot.total_scrollback;
      this.renderer.render(snapshot);
    } catch (error) {
      // Ignore errors during initialization or after terminal close
      console.debug('Grid snapshot fetch failed:', error);
    }
  }

  // ---- Scrollback ----

  private startScrollbackSaveInterval() {
    this.scrollbackSaveInterval = window.setInterval(() => {
      this.saveScrollback();
    }, 5 * 60 * 1000);
  }

  /**
   * Get scrollback data using godly-vt's contents_formatted() output.
   * Scrollback data is managed by the daemon's godly-vt parser.
   */
  getScrollbackData(): Uint8Array {
    // The daemon owns the terminal state, so we save scrollback via
    // the existing save_scrollback Tauri command which uses godly-vt.
    // Return empty data here; actual save happens in saveScrollback().
    return new Uint8Array(0);
  }

  async saveScrollback(): Promise<void> {
    // Scrollback is now saved via the daemon's godly-vt grid state.
    // The daemon-side save_scrollback command handles this.
    try {
      await invoke('save_scrollback', {
        terminalId: this.terminalId,
        data: [],
      });
    } catch (error) {
      console.error('Failed to save scrollback:', error);
    }
  }

  async loadScrollback(): Promise<void> {
    // On reconnect, the daemon replays the ring buffer + godly-vt state.
    // We just need to fetch a fresh snapshot.
    await this.fetchAndRenderSnapshot();
  }

  // ---- Fit / Resize ----

  fit() {
    try {
      this.renderer.updateSize();
      const { rows, cols } = this.renderer.getGridSize();
      if (rows > 0 && cols > 0) {
        terminalService.resizeTerminal(this.terminalId, rows, cols);
      }
    } catch {
      // Ignore fit errors during initialization
    }
  }

  // ---- Activation / Visibility ----

  setActive(active: boolean) {
    this.container.classList.remove('split-visible', 'split-focused');
    this.container.classList.toggle('active', active);
    if (active) {
      requestAnimationFrame(() => {
        this.fit();
        this.renderer.scrollToBottom();
        this.renderer.focus();
        this.fetchAndRenderSnapshot();
      });
    }
  }

  setSplitVisible(visible: boolean, focused: boolean) {
    this.container.classList.remove('active');
    this.container.classList.toggle('split-visible', visible);
    this.container.classList.toggle('split-focused', focused);
    if (visible) {
      requestAnimationFrame(() => {
        this.fit();
        this.renderer.scrollToBottom();
        if (focused) {
          this.renderer.focus();
        }
        this.fetchAndRenderSnapshot();
      });
    }
  }

  focus() {
    this.renderer.focus();
  }

  // ---- Lifecycle ----

  async destroy() {
    await this.saveScrollback();

    if (this.snapshotTimer !== null) {
      clearTimeout(this.snapshotTimer);
      this.snapshotTimer = null;
    }
    if (this.scrollbackSaveInterval) {
      clearInterval(this.scrollbackSaveInterval);
    }
    this.resizeObserver.disconnect();
    if (this.unsubscribeOutput) {
      this.unsubscribeOutput();
    }
    this.renderer.dispose();
    this.container.remove();
  }

  getElement(): HTMLElement {
    return this.container;
  }

  getContainer(): HTMLElement {
    return this.container;
  }

  getTerminalId(): string {
    return this.terminalId;
  }
}
