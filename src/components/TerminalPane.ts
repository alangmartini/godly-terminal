import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { SerializeAddon } from '@xterm/addon-serialize';
import { openUrl } from '@tauri-apps/plugin-opener';
import { terminalService } from '../services/terminal-service';
import { store } from '../state/store';
import { isAppShortcut, isTerminalControlKey } from './keyboard';
import { keybindingStore } from '../state/keybinding-store';

export class TerminalPane {
  private terminal: Terminal;
  private fitAddon: FitAddon;
  private serializeAddon: SerializeAddon;
  private container: HTMLElement;
  private terminalId: string;
  private resizeObserver: ResizeObserver;
  private resizeRAF: number | null = null;
  private unsubscribeOutput: (() => void) | null = null;
  private outputBuffer: Uint8Array[] = [];
  private outputFlushTimer: ReturnType<typeof setTimeout> | null = null;
  private scrollbackSaveInterval: number | null = null;
  private maxScrollbackLines = 10000;

  constructor(terminalId: string) {
    this.terminalId = terminalId;

    this.terminal = new Terminal({
      fontFamily: 'Cascadia Code, Consolas, monospace',
      fontSize: 14,
      theme: {
        background: '#1e1e1e',
        foreground: '#cccccc',
        cursor: '#aeafad',
        cursorAccent: '#1e1e1e',
        selectionBackground: '#264f78',
        black: '#000000',
        red: '#cd3131',
        green: '#0dbc79',
        yellow: '#e5e510',
        blue: '#2472c8',
        magenta: '#bc3fbc',
        cyan: '#11a8cd',
        white: '#e5e5e5',
        brightBlack: '#666666',
        brightRed: '#f14c4c',
        brightGreen: '#23d18b',
        brightYellow: '#f5f543',
        brightBlue: '#3b8eea',
        brightMagenta: '#d670d6',
        brightCyan: '#29b8db',
        brightWhite: '#e5e5e5',
      },
      cursorBlink: true,
      scrollback: this.maxScrollbackLines,
      allowProposedApi: true,
    });

    this.fitAddon = new FitAddon();
    this.serializeAddon = new SerializeAddon();
    this.terminal.loadAddon(this.fitAddon);
    this.terminal.loadAddon(this.serializeAddon);
    this.terminal.loadAddon(new WebLinksAddon((event: MouseEvent, uri: string) => {
      if (event.ctrlKey) {
        openUrl(uri).catch((err) => {
          console.error('Failed to open URL:', err);
        });
      }
    }));

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
    this.terminal.open(this.container);
    (this.container as any).__xterm = this.terminal;
    (this.container as any).__serializeAddon = this.serializeAddon;
    this.resizeObserver.observe(this.container);

    // Block app-level shortcuts from being consumed by xterm.js so they
    // bubble to the document-level handler in App.ts. Also handle
    // copy/paste inline since we need access to the terminal.
    this.terminal.attachCustomKeyEventHandler((event) => {
      const action = keybindingStore.matchAction(event);

      // Copy: copy selected text to clipboard
      if (action === 'clipboard.copy') {
        event.preventDefault();
        const selection = this.terminal.getSelection();
        if (selection) {
          navigator.clipboard.writeText(selection);
        }
        return false;
      }
      // Paste: paste from clipboard into terminal
      if (action === 'clipboard.paste') {
        event.preventDefault();
        navigator.clipboard.readText().then((text) => {
          if (text) {
            terminalService.writeToTerminal(this.terminalId, text);
          }
        });
        return false;
      }
      if (isAppShortcut(event)) {
        return false;
      }
      // Prevent WebView2 from intercepting terminal control keys as browser
      // clipboard/undo shortcuts. Without this, these keys never reach the
      // PTY as control characters (SIGINT, SIGTSTP, etc.) on Windows.
      // Must handle both keydown AND keyup — WebView2 can intercept either.
      if (isTerminalControlKey(event)) {
        event.preventDefault();
      } else if (event.type === 'keyup' && isTerminalControlKey({
        ctrlKey: event.ctrlKey,
        shiftKey: event.shiftKey,
        altKey: event.altKey,
        key: event.key,
        type: 'keydown',
      })) {
        event.preventDefault();
      }
      // Shift+Enter: send CSI 13;2u (kitty keyboard protocol) so CLI tools
      // like Claude Code can distinguish it from plain Enter.
      if (event.shiftKey && !event.ctrlKey && event.key === 'Enter') {
        if (event.type === 'keydown') {
          terminalService.writeToTerminal(this.terminalId, '\x1b[13;2u');
        }
        return false;
      }
      return true;
    });

    // Handle input
    this.terminal.onData((data) => {
      terminalService.writeToTerminal(this.terminalId, data);
    });

    // Handle output: buffer chunks and flush once per event-loop turn.
    // Bug C1: unbatched write() calls under heavy output saturate the main
    // thread because each call triggers xterm.js's parser synchronously.
    // setTimeout(0) fires in ~1ms (vs ~16ms for RAF), reducing echo latency
    // while still coalescing burst output within the same event-loop turn.
    this.unsubscribeOutput = terminalService.onTerminalOutput(
      this.terminalId,
      (data) => {
        this.outputBuffer.push(data);
        if (this.outputFlushTimer === null) {
          this.outputFlushTimer = setTimeout(() => this.flushOutputBuffer(), 0);
        }
      }
    );

    // Forward OSC 0/2 title changes to the store for tab display
    this.terminal.onTitleChange((title) => {
      store.updateTerminal(this.terminalId, { oscTitle: title || undefined });
    });

    // Start periodic scrollback saving (every 5 minutes)
    this.startScrollbackSaveInterval();

    // Initial fit
    requestAnimationFrame(() => {
      this.fit();
    });
  }

  private flushOutputBuffer() {
    this.outputFlushTimer = null;
    const chunks = this.outputBuffer;
    if (chunks.length === 0) return;
    this.outputBuffer = [];

    if (chunks.length === 1) {
      this.terminal.write(chunks[0]);
      return;
    }

    // Concatenate all buffered chunks into a single Uint8Array
    let totalLength = 0;
    for (const chunk of chunks) {
      totalLength += chunk.byteLength;
    }
    const merged = new Uint8Array(totalLength);
    let offset = 0;
    for (const chunk of chunks) {
      merged.set(chunk, offset);
      offset += chunk.byteLength;
    }
    this.terminal.write(merged);
  }

  private startScrollbackSaveInterval() {
    // Save scrollback every 5 minutes
    this.scrollbackSaveInterval = window.setInterval(() => {
      this.saveScrollback();
    }, 5 * 60 * 1000);
  }

  /**
   * Get scrollback data as raw terminal content
   */
  getScrollbackData(): Uint8Array {
    // Use the serialize addon to get the terminal buffer content
    const serialized = this.serializeAddon.serialize();
    return new TextEncoder().encode(serialized);
  }

  /**
   * Save scrollback to persistent storage
   */
  async saveScrollback(): Promise<void> {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const data = this.getScrollbackData();
      await invoke('save_scrollback', {
        terminalId: this.terminalId,
        data: Array.from(data),
      });
    } catch (error) {
      console.error('Failed to save scrollback:', error);
    }
  }

  /**
   * Load and restore scrollback from persistent storage
   */
  async loadScrollback(): Promise<void> {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const data = await invoke<number[]>('load_scrollback', {
        terminalId: this.terminalId,
      });

      if (data && data.length > 0) {
        const text = new TextDecoder().decode(new Uint8Array(data));
        // Write the restored content to the terminal
        this.terminal.write(text);
      }
    } catch (error) {
      console.error('Failed to load scrollback:', error);
    }
  }

  fit() {
    try {
      this.fitAddon.fit();
      const { rows, cols } = this.terminal;
      terminalService.resizeTerminal(this.terminalId, rows, cols);
    } catch {
      // Ignore fit errors during initialization
    }
  }

  setActive(active: boolean) {
    this.container.classList.toggle('active', active);
    if (active) {
      requestAnimationFrame(() => {
        this.fit();
        this.terminal.focus();
      });
    }
  }

  focus() {
    this.terminal.focus();
  }

  async destroy() {
    // Save scrollback before destroying
    await this.saveScrollback();

    if (this.outputFlushTimer !== null) {
      clearTimeout(this.outputFlushTimer);
      this.outputFlushTimer = null;
    }
    this.outputBuffer = [];
    if (this.scrollbackSaveInterval) {
      clearInterval(this.scrollbackSaveInterval);
    }
    this.resizeObserver.disconnect();
    if (this.unsubscribeOutput) {
      this.unsubscribeOutput();
    }
    this.terminal.dispose();
    this.container.remove();
  }

  getElement(): HTMLElement {
    return this.container;
  }

  getTerminalId(): string {
    return this.terminalId;
  }
}
