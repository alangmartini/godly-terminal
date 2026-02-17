import { store } from '../state/store';

/**
 * Figma embed pane — renders a Figma design in an iframe.
 *
 * Follows the same public interface as TerminalPane so App.ts
 * can manage both types interchangeably.
 */
export class FigmaPane {
  private container: HTMLElement;
  private iframe: HTMLIFrameElement | null = null;
  private terminalId: string;
  private figmaUrl: string;

  constructor(terminalId: string, figmaUrl: string) {
    this.terminalId = terminalId;
    this.figmaUrl = figmaUrl;

    this.container = document.createElement('div');
    this.container.className = 'figma-pane';
    this.container.dataset.terminalId = terminalId;
  }

  mount(parent: HTMLElement) {
    parent.appendChild(this.container);
    this.createIframe();

    // Click-to-focus in split mode
    this.container.addEventListener('mousedown', () => {
      if (this.container.classList.contains('split-visible')) {
        store.setActiveTerminal(this.terminalId);
      }
    });
  }

  private createIframe() {
    this.iframe = document.createElement('iframe');
    this.iframe.src = this.buildEmbedUrl();
    this.iframe.className = 'figma-embed-iframe';
    this.iframe.setAttribute('allowfullscreen', '');
    this.container.appendChild(this.iframe);
  }

  private buildEmbedUrl(): string {
    // If already an embed URL, use as-is
    if (this.figmaUrl.includes('/embed')) {
      return this.figmaUrl;
    }
    // Convert design/file URL to embed URL
    const encoded = encodeURIComponent(this.figmaUrl);
    return `https://www.figma.com/embed?embed_host=godly-terminal&url=${encoded}`;
  }

  /** Update the Figma URL and reload the iframe */
  setUrl(url: string) {
    this.figmaUrl = url;
    if (this.iframe) {
      this.iframe.src = this.buildEmbedUrl();
    }
  }

  setActive(active: boolean) {
    this.container.classList.remove('split-visible', 'split-focused');
    this.container.classList.toggle('active', active);
  }

  setSplitVisible(visible: boolean, focused: boolean) {
    this.container.classList.remove('active');
    this.container.classList.toggle('split-visible', visible);
    this.container.classList.toggle('split-focused', focused);
  }

  focus() {
    this.iframe?.focus();
  }

  async destroy() {
    if (this.iframe) {
      this.iframe.src = 'about:blank';
      this.iframe = null;
    }
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

  // Stubs for TerminalPane interface compatibility
  async saveScrollback(): Promise<void> {}
  async loadScrollback(): Promise<void> {}
}
