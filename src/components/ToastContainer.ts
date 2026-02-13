import { store } from '../state/store';

interface Toast {
  id: number;
  terminalId: string;
  title: string;
  body: string;
  element: HTMLElement;
  timeout: ReturnType<typeof setTimeout>;
}

const TOAST_DURATION_MS = 4000;
const FADE_OUT_MS = 300;

export class ToastContainer {
  private container: HTMLElement;
  private toasts: Toast[] = [];
  private nextId = 0;

  constructor() {
    this.container = document.createElement('div');
    this.container.className = 'toast-container';
  }

  mount(parent: HTMLElement) {
    parent.appendChild(this.container);
  }

  show(title: string, body: string, terminalId: string) {
    const id = this.nextId++;

    const el = document.createElement('div');
    el.className = 'toast';

    const titleEl = document.createElement('div');
    titleEl.className = 'toast-title';
    titleEl.textContent = title;

    const bodyEl = document.createElement('div');
    bodyEl.className = 'toast-body';
    bodyEl.textContent = body;

    el.appendChild(titleEl);
    el.appendChild(bodyEl);

    el.addEventListener('click', () => {
      this.dismiss(id);
      // Switch to the terminal's workspace and activate the terminal
      const terminal = store.getState().terminals.find(t => t.id === terminalId);
      if (terminal) {
        store.setActiveWorkspace(terminal.workspaceId);
        store.setActiveTerminal(terminalId);
      }
    });

    const timeout = setTimeout(() => this.dismiss(id), TOAST_DURATION_MS);

    const toast: Toast = { id, terminalId, title, body, element: el, timeout };
    this.toasts.push(toast);
    this.container.appendChild(el);
  }

  private dismiss(id: number) {
    const index = this.toasts.findIndex(t => t.id === id);
    if (index === -1) return;

    const toast = this.toasts[index];
    clearTimeout(toast.timeout);
    toast.element.classList.add('toast-exit');

    setTimeout(() => {
      toast.element.remove();
      this.toasts = this.toasts.filter(t => t.id !== id);
    }, FADE_OUT_MS);
  }
}
