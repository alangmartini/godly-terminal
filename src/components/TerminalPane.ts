import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { terminalService } from '../services/terminal-service';

export class TerminalPane {
  private terminal: Terminal;
  private fitAddon: FitAddon;
  private container: HTMLElement;
  private terminalId: string;
  private resizeObserver: ResizeObserver;
  private unsubscribeOutput: (() => void) | null = null;

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
      scrollback: 10000,
      allowProposedApi: true,
    });

    this.fitAddon = new FitAddon();
    this.terminal.loadAddon(this.fitAddon);
    this.terminal.loadAddon(new WebLinksAddon());

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
    this.resizeObserver.observe(this.container);

    // Handle input
    this.terminal.onData((data) => {
      terminalService.writeToTerminal(this.terminalId, data);
    });

    // Handle output
    this.unsubscribeOutput = terminalService.onTerminalOutput(
      this.terminalId,
      (data) => {
        this.terminal.write(data);
      }
    );

    // Initial fit
    requestAnimationFrame(() => {
      this.fit();
    });
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

  destroy() {
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
}
