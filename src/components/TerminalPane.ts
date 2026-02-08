import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { SerializeAddon } from '@xterm/addon-serialize';
import { openUrl } from '@tauri-apps/plugin-opener';
import { terminalService } from '../services/terminal-service';

export class TerminalPane {
  private terminal: Terminal;
  private fitAddon: FitAddon;
  private serializeAddon: SerializeAddon;
  private container: HTMLElement;
  private terminalId: string;
  private resizeObserver: ResizeObserver;
  private unsubscribeOutput: (() => void) | null = null;
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
      this.fit();
    });
  }

  mount(parent: HTMLElement) {
    parent.appendChild(this.container);
    this.terminal.open(this.container);
    (this.container as any).__xterm = this.terminal;
    (this.container as any).__serializeAddon = this.serializeAddon;
    this.resizeObserver.observe(this.container);

    // Handle Ctrl+Shift+C to copy selection to clipboard
    this.terminal.attachCustomKeyEventHandler((event) => {
      if (event.ctrlKey && event.shiftKey && event.key === 'C' && event.type === 'keydown') {
        const selection = this.terminal.getSelection();
        if (selection) {
          navigator.clipboard.writeText(selection);
        }
        return false;
      }
      return true;
    });

    // Handle input
    this.terminal.onData((data) => {
      terminalService.writeToTerminal(this.terminalId, data);
    });

    // Handle output and capture for scrollback
    this.unsubscribeOutput = terminalService.onTerminalOutput(
      this.terminalId,
      (data) => {
        this.terminal.write(data);
      }
    );

    // Start periodic scrollback saving (every 5 minutes)
    this.startScrollbackSaveInterval();

    // Initial fit
    requestAnimationFrame(() => {
      this.fit();
    });
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
