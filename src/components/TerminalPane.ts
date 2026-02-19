import { invoke } from '@tauri-apps/api/core';
import { terminalService } from '../services/terminal-service';
import { store } from '../state/store';
import { isAppShortcut, isTerminalControlKey } from './keyboard';
import { keybindingStore } from '../state/keybinding-store';
import { TerminalRenderer, RichGridData, RichGridDiff } from './TerminalRenderer';
import { showCopyDialog } from './CopyDialog';
import { perfTracer } from '../utils/PerfTracer';
import { themeStore } from '../state/theme-store';
import { terminalSettingsStore } from '../state/terminal-settings-store';

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
  private unsubscribeTheme: (() => void) | null = null;

  // Debounce grid snapshot requests: on terminal-output we schedule a snapshot
  // fetch via setTimeout(SNAPSHOT_MIN_INTERVAL_MS). Multiple output events
  // within the interval collapse into a single IPC call, capping snapshot
  // fetches to ~60fps under sustained output.
  private static readonly SNAPSHOT_MIN_INTERVAL_MS = 16;
  private snapshotPending = false;
  private snapshotTimer: ReturnType<typeof setTimeout> | null = null;

  // Scrollback state: tracked from the latest snapshot
  private scrollbackOffset = 0;
  private totalScrollback = 0;

  // True when the user has explicitly scrolled up into history.
  // Prevents output-triggered snapshot fetches from overwriting the scroll
  // position due to a race with the async setScrollback IPC.
  private isUserScrolled = false;

  // Monotonic counter to discard stale scroll responses.
  // Incremented on every user-initiated scroll; async fetches that
  // complete with a stale seq are discarded to prevent rollbacks.
  private scrollSeq = 0;

  // Scrollback save interval (every 5 minutes)
  private scrollbackSaveInterval: number | null = null;

  // Cached full snapshot for differential updates.
  // null until first full snapshot is received.
  private cachedSnapshot: RichGridData | null = null;

  // Hidden textarea for keyboard input (handles dead keys, IME composition).
  // Canvas elements don't support text composition, so dead keys (e.g. quote
  // on ABNT2 keyboards) produce event.key="Dead" and the composed character
  // is lost. A textarea receives proper composition/input events from the OS.
  private inputTextarea!: HTMLTextAreaElement;
  private isComposing = false;

  // Exited overlay element (hidden until showExitedOverlay() is called)
  private exitedOverlay: HTMLElement | null = null;
  private isExited = false;

  constructor(terminalId: string) {
    this.terminalId = terminalId;

    this.renderer = new TerminalRenderer();

    this.unsubscribeTheme = themeStore.subscribe(() => {
      this.renderer.setTheme(themeStore.getTerminalTheme());
    });

    // Forward OSC title changes to the store
    this.renderer.setOnTitleChange((title) => {
      store.updateTerminal(this.terminalId, { oscTitle: title || undefined });
    });

    // Handle wheel scroll events from the renderer
    this.renderer.setOnScroll((deltaLines) => {
      this.handleScroll(deltaLines);
    });

    // Handle absolute scroll-to events from scrollbar drag
    this.renderer.setOnScrollTo((absoluteOffset) => {
      this.handleScrollTo(absoluteOffset);
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

    // ---- Hidden textarea for keyboard input ----
    // Canvas elements don't participate in OS text composition, so dead keys
    // (e.g. ' and " on ABNT2 keyboards) fire event.key="Dead" and the
    // composed character is lost.  A hidden <textarea> receives proper
    // composition and input events from the OS input method.
    this.inputTextarea = document.createElement('textarea');
    this.inputTextarea.className = 'terminal-input-hidden';
    this.inputTextarea.setAttribute('autocomplete', 'off');
    this.inputTextarea.setAttribute('autocapitalize', 'off');
    this.inputTextarea.setAttribute('autocorrect', 'off');
    this.inputTextarea.setAttribute('spellcheck', 'false');
    this.inputTextarea.tabIndex = 0;
    this.inputTextarea.style.cssText =
      'position:absolute;left:-9999px;top:0;width:1px;height:1px;' +
      'opacity:0;overflow:hidden;resize:none;border:none;padding:0;' +
      'white-space:pre;z-index:-1;';
    this.container.appendChild(this.inputTextarea);

    // Remove canvas from tab order — the textarea is now the keyboard target.
    this.renderer.getElement().tabIndex = -1;

    // Click-to-focus: always focus the textarea when clicking in the terminal
    // area. In split mode, also set this terminal as the active one.
    this.container.addEventListener('mousedown', () => {
      if (this.container.classList.contains('split-visible')) {
        store.setActiveTerminal(this.terminalId);
      }
      // Always focus the textarea — this is the primary keyboard recovery mechanism.
      // requestAnimationFrame ensures the mousedown default behavior completes first.
      requestAnimationFrame(() => this.focusInput());
    });

    // Handle keyboard input on the hidden textarea.
    // Special keys (arrows, Enter, Ctrl combos, etc.) are handled in keydown.
    // Printable characters and dead-key compositions flow through the textarea's
    // input event instead, which correctly resolves dead keys and IME sequences.
    this.inputTextarea.addEventListener('keydown', (event) => {
      this.handleKeyEvent(event);
    });
    this.inputTextarea.addEventListener('keyup', (event) => {
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

    // Composed text input: captures dead key results and IME output.
    this.inputTextarea.addEventListener('input', () => {
      if (this.isComposing) return;
      const text = this.inputTextarea.value;
      if (text) {
        perfTracer.mark('write_ipc_start');
        terminalService.writeToTerminal(this.terminalId, text).then(() => {
          perfTracer.measure('write_to_terminal_ipc', 'write_ipc_start');
        });
        this.inputTextarea.value = '';
      }
    });

    // IME composition tracking — don't send intermediate text.
    this.inputTextarea.addEventListener('compositionstart', () => {
      this.isComposing = true;
    });
    this.inputTextarea.addEventListener('compositionend', () => {
      this.isComposing = false;
      const text = this.inputTextarea.value;
      if (text) {
        terminalService.writeToTerminal(this.terminalId, text);
      }
      this.inputTextarea.value = '';
    });

    // Focus diagnostics: detect when the textarea loses focus while this pane
    // is active. This helps diagnose keyboard input freezes — if the textarea
    // loses focus, keydown events stop reaching handleKeyEvent entirely.
    this.inputTextarea.addEventListener('blur', () => {
      if (this.container.classList.contains('active') ||
          this.container.classList.contains('split-focused')) {
        const thief = document.activeElement;
        console.warn(
          `[TerminalPane] Input lost focus while active (terminal=${this.terminalId}, ` +
          `now focused: ${thief?.tagName}${thief?.className ? '.' + thief.className : ''})`
        );
      }
    });

    // Listen for terminal output events.
    // When new PTY output arrives, schedule a grid snapshot fetch.
    // If the user has scrolled up and auto-scroll is enabled, snap back first.
    this.unsubscribeOutput = terminalService.onTerminalOutput(
      this.terminalId,
      () => {
        perfTracer.mark('terminal_output_event');
        perfTracer.measure('keydown_to_output', 'keydown');
        if (this.isUserScrolled && terminalSettingsStore.getAutoScrollOnOutput()) {
          this.snapToBottom();
          return;
        }
        this.scheduleSnapshotFetch();
      }
    );

    // Start periodic scrollback saving (every 5 minutes)
    this.startScrollbackSaveInterval();

    // Sync canvas bitmap immediately if container is already visible,
    // preventing a zoom flash on the initially active terminal after reopen.
    if (this.container.offsetWidth && this.container.offsetHeight) {
      this.renderer.updateSize();
    }

    // Initial fit + snapshot
    requestAnimationFrame(() => {
      this.fit();
      this.fetchAndRenderSnapshot();
    });
  }

  // ---- Keyboard handling ----

  private handleKeyEvent(event: KeyboardEvent) {
    // Block keyboard input to dead terminals (allow app shortcuts to bubble)
    if (this.isExited) {
      if (isAppShortcut(event)) return; // let close shortcut etc. bubble
      event.preventDefault();
      return;
    }

    perfTracer.mark('keydown');
    const action = keybindingStore.matchAction(event);

    // Scroll shortcuts: handle locally without sending to PTY.
    // On alternate screen (vim, less, htop), pass these keys through to the app.
    const onAlternateScreen = this.cachedSnapshot?.alternate_screen ?? false;
    if (!onAlternateScreen && action === 'scroll.pageUp') {
      event.preventDefault();
      this.handleScroll(this.renderer.getGridSize().rows);
      return;
    }
    if (!onAlternateScreen && action === 'scroll.pageDown') {
      event.preventDefault();
      this.handleScroll(-this.renderer.getGridSize().rows);
      return;
    }
    if (!onAlternateScreen && action === 'scroll.toTop') {
      event.preventDefault();
      this.handleScroll(this.totalScrollback);
      return;
    }
    if (!onAlternateScreen && action === 'scroll.toBottom') {
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

    // Copy (Clean): open dialog with cleaned text
    if (action === 'clipboard.copyClean') {
      event.preventDefault();
      if (this.renderer.hasSelection()) {
        this.renderer.getSelectedText(this.terminalId).then((text) => {
          if (text) {
            showCopyDialog(text).then(() => {
              this.renderer.focus();
            });
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
      perfTracer.mark('write_ipc_start');
      terminalService.writeToTerminal(this.terminalId, data).then(() => {
        perfTracer.measure('write_to_terminal_ipc', 'write_ipc_start');
      });
    }
  }

  /**
   * Convert a keyboard event into the string that should be sent to the PTY.
   * Handles control characters and special keys only. Printable characters
   * (including dead-key compositions) are handled by the textarea's input event.
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

    // Ctrl+Alt combinations -> ESC + control character
    // Standard terminal behavior: Ctrl+Alt+letter sends ESC followed by the
    // control character for that letter (e.g. Ctrl+Alt+C → ESC + \x03).
    if (event.ctrlKey && event.altKey && !event.shiftKey) {
      const key = event.key.toLowerCase();
      if (key.length === 1 && key >= 'a' && key <= 'z') {
        return '\x1b' + String.fromCharCode(key.charCodeAt(0) - 96);
      }
    }

    // Alt combinations -> ESC + key
    if (event.altKey && !event.ctrlKey && event.key.length === 1) {
      return '\x1b' + event.key;
    }

    // CSI modifier parameter for special keys:
    // 1 + (shift ? 1 : 0) + (alt ? 2 : 0) + (ctrl ? 4 : 0)
    // mod=1 means no modifiers; mod>1 triggers the extended CSI format.
    const mod = 1
      + (event.shiftKey ? 1 : 0)
      + (event.altKey ? 2 : 0)
      + (event.ctrlKey ? 4 : 0);

    // Special keys
    switch (event.key) {
      case 'Enter': return '\r';
      case 'Backspace': return '\x7f';
      case 'Tab': return '\t';
      case 'Escape': return '\x1b';

      // Arrow keys: \x1b[X or \x1b[1;{mod}X
      case 'ArrowUp':    return mod > 1 ? `\x1b[1;${mod}A` : '\x1b[A';
      case 'ArrowDown':  return mod > 1 ? `\x1b[1;${mod}B` : '\x1b[B';
      case 'ArrowRight': return mod > 1 ? `\x1b[1;${mod}C` : '\x1b[C';
      case 'ArrowLeft':  return mod > 1 ? `\x1b[1;${mod}D` : '\x1b[D';

      // Home/End: \x1b[H/F or \x1b[1;{mod}H/F
      case 'Home': return mod > 1 ? `\x1b[1;${mod}H` : '\x1b[H';
      case 'End':  return mod > 1 ? `\x1b[1;${mod}F` : '\x1b[F';

      // Tilde keys: \x1b[{num}~ or \x1b[{num};{mod}~
      case 'Delete':   return mod > 1 ? `\x1b[3;${mod}~` : '\x1b[3~';
      case 'PageUp':   return mod > 1 ? `\x1b[5;${mod}~` : '\x1b[5~';
      case 'PageDown': return mod > 1 ? `\x1b[6;${mod}~` : '\x1b[6~';
      case 'Insert':   return mod > 1 ? `\x1b[2;${mod}~` : '\x1b[2~';

      // F1-F4: SS3 without modifiers, CSI with modifiers
      case 'F1': return mod > 1 ? `\x1b[1;${mod}P` : '\x1bOP';
      case 'F2': return mod > 1 ? `\x1b[1;${mod}Q` : '\x1bOQ';
      case 'F3': return mod > 1 ? `\x1b[1;${mod}R` : '\x1bOR';
      case 'F4': return mod > 1 ? `\x1b[1;${mod}S` : '\x1bOS';

      // F5-F12: \x1b[{num}~ or \x1b[{num};{mod}~
      case 'F5':  return mod > 1 ? `\x1b[15;${mod}~` : '\x1b[15~';
      case 'F6':  return mod > 1 ? `\x1b[17;${mod}~` : '\x1b[17~';
      case 'F7':  return mod > 1 ? `\x1b[18;${mod}~` : '\x1b[18~';
      case 'F8':  return mod > 1 ? `\x1b[19;${mod}~` : '\x1b[19~';
      case 'F9':  return mod > 1 ? `\x1b[20;${mod}~` : '\x1b[20~';
      case 'F10': return mod > 1 ? `\x1b[21;${mod}~` : '\x1b[21~';
      case 'F11': return mod > 1 ? `\x1b[23;${mod}~` : '\x1b[23~';
      case 'F12': return mod > 1 ? `\x1b[24;${mod}~` : '\x1b[24~';
    }

    // Printable characters are NOT handled here — they flow through the
    // hidden textarea's input event, which correctly resolves dead keys
    // (e.g. ' and " on ABNT2 keyboards) and IME compositions.
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
    this.isUserScrolled = newOffset > 0;
    const seq = ++this.scrollSeq;
    // Scroll changes the viewport — invalidate cache and do a full snapshot
    this.cachedSnapshot = null;
    terminalService.setScrollback(this.terminalId, newOffset).then(() => {
      if (seq === this.scrollSeq) {
        this.fetchFullSnapshot();
      }
    });
  }

  /**
   * Handle an absolute scroll-to request (e.g. from scrollbar drag).
   * Sets the viewport to the given offset directly.
   */
  private handleScrollTo(absoluteOffset: number) {
    const newOffset = Math.max(0, Math.round(absoluteOffset));
    if (newOffset === this.scrollbackOffset) return;
    this.scrollbackOffset = newOffset;
    const seq = ++this.scrollSeq;
    this.cachedSnapshot = null;
    terminalService.setScrollback(this.terminalId, newOffset).then(() => {
      if (seq === this.scrollSeq) {
        this.fetchFullSnapshot();
      }
    });
  }

  /** Snap viewport back to live view (offset 0). */
  private snapToBottom() {
    if (this.scrollbackOffset === 0) return;
    this.scrollbackOffset = 0;
    this.isUserScrolled = false;
    const seq = ++this.scrollSeq;
    this.cachedSnapshot = null;
    terminalService.setScrollback(this.terminalId, 0).then(() => {
      if (seq === this.scrollSeq) {
        this.fetchFullSnapshot();
      }
    });
  }

  // ---- Grid snapshot fetching ----

  private scheduleSnapshotFetch() {
    if (this.snapshotPending) return;
    this.snapshotPending = true;
    perfTracer.mark('schedule_snapshot');
    if (this.snapshotTimer === null) {
      this.snapshotTimer = setTimeout(async () => {
        this.snapshotTimer = null;
        perfTracer.measure('snapshot_schedule_delay', 'schedule_snapshot');
        try {
          await this.fetchAndRenderSnapshot();
        } finally {
          // Reset AFTER fetch completes to prevent cascading snapshot requests.
          // Without this, events during the ~85ms fetch would schedule overlapping
          // IPC requests that saturate the Tauri thread pool.
          this.snapshotPending = false;
        }
      }, TerminalPane.SNAPSHOT_MIN_INTERVAL_MS);
    }
  }

  private useDiffSnapshots = true;

  private async fetchAndRenderSnapshot() {
    const seqBefore = this.scrollSeq;
    try {
      // Use diff snapshots when we have a cached full snapshot and diff is supported.
      if (this.cachedSnapshot && this.useDiffSnapshots) {
        try {
          const diff = await invoke<RichGridDiff>('get_grid_snapshot_diff', {
            terminalId: this.terminalId,
          });

          // Discard stale response if the user scrolled while we were fetching
          if (seqBefore !== this.scrollSeq) return;

          // Merge dirty rows into the cached snapshot
          if (diff.full_repaint) {
            const rows: RichGridData['rows'] = this.cachedSnapshot.rows;
            while (rows.length < diff.dimensions.rows) {
              rows.push({ cells: [], wrapped: false });
            }
            rows.length = diff.dimensions.rows;
            for (const [rowIdx, rowData] of diff.dirty_rows) {
              rows[rowIdx] = rowData;
            }
            this.cachedSnapshot = {
              rows,
              cursor: diff.cursor,
              dimensions: diff.dimensions,
              alternate_screen: diff.alternate_screen,
              cursor_hidden: diff.cursor_hidden,
              title: diff.title,
              scrollback_offset: diff.scrollback_offset,
              total_scrollback: diff.total_scrollback,
            };
          } else {
            for (const [rowIdx, rowData] of diff.dirty_rows) {
              if (rowIdx < this.cachedSnapshot.rows.length) {
                this.cachedSnapshot.rows[rowIdx] = rowData;
              }
            }
            this.cachedSnapshot.cursor = diff.cursor;
            this.cachedSnapshot.cursor_hidden = diff.cursor_hidden;
            this.cachedSnapshot.scrollback_offset = diff.scrollback_offset;
            this.cachedSnapshot.total_scrollback = diff.total_scrollback;
            this.cachedSnapshot.alternate_screen = diff.alternate_screen;
            if (diff.title) {
              this.cachedSnapshot.title = diff.title;
            }
          }

          // Only sync scroll offset from daemon when the user hasn't explicitly
          // scrolled up. This prevents a race where output-triggered snapshot
          // fetches return a stale offset (0) before the setScrollback IPC
          // completes, snapping the view to bottom.
          if (!this.isUserScrolled) {
            this.scrollbackOffset = diff.scrollback_offset;
          }
          this.totalScrollback = diff.total_scrollback;
          this.renderer.render(this.cachedSnapshot);
          return;
        } catch {
          // Diff not supported by daemon — fall back to full snapshots permanently
          console.debug('Diff snapshots not available, falling back to full snapshots');
          this.useDiffSnapshots = false;
        }
      }
      // Full snapshot path (initial render, after scroll, or diff not supported)
      await this.fetchFullSnapshot(seqBefore);
    } catch (error) {
      console.debug('Grid snapshot fetch failed:', error);
    }
  }

  /**
   * Fetch a full grid snapshot (used for initial render and after scroll).
   * If scrollSeqAtStart is provided, the result is discarded when the user
   * has scrolled again since the fetch started (prevents rollbacks).
   */
  private async fetchFullSnapshot(scrollSeqAtStart?: number) {
    try {
      perfTracer.mark('snapshot_ipc_start');
      const snapshot = await invoke<RichGridData>('get_grid_snapshot', {
        terminalId: this.terminalId,
      });
      perfTracer.measure('snapshot_ipc', 'snapshot_ipc_start');
      // Discard stale response if the user scrolled while we were fetching
      if (scrollSeqAtStart !== undefined && scrollSeqAtStart !== this.scrollSeq) return;
      this.cachedSnapshot = snapshot;
      if (!this.isUserScrolled) {
        this.scrollbackOffset = snapshot.scrollback_offset;
      }
      this.totalScrollback = snapshot.total_scrollback;
      this.renderer.render(snapshot);
    } catch (error) {
      console.debug('Full grid snapshot fetch failed:', error);
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
      // Guard: skip resize when container is hidden (display:none).
      // Hidden containers have 0×0 dimensions, causing getGridSize() to
      // return 1×1. Sending resize(1,1) to the daemon truncates the
      // godly-vt grid and permanently destroys all terminal content.
      if (!this.container.offsetWidth || !this.container.offsetHeight) {
        return;
      }
      this.renderer.updateSize();
      const { rows, cols } = this.renderer.getGridSize();
      if (rows > 0 && cols > 0) {
        // Resize changes grid dimensions — invalidate cache
        if (this.cachedSnapshot &&
            (this.cachedSnapshot.dimensions.rows !== rows ||
             this.cachedSnapshot.dimensions.cols !== cols)) {
          this.cachedSnapshot = null;
        }
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
      // Sync canvas bitmap to container size immediately to prevent the browser
      // from stretching the stale bitmap (300×150 default) for one frame,
      // which causes a "zoomed in" flash on tab switch / reopen.
      this.renderer.updateSize();
      requestAnimationFrame(() => {
        this.fit();
        this.renderer.scrollToBottom();
        this.focusInput();
        this.fetchAndRenderSnapshot();
      });
      // Double-tap focus: some WebView2 focus changes race with RAF.
      // Schedule a second focus attempt after a short delay to catch cases
      // where the first RAF-based focus is stolen by tab bar click cleanup,
      // dialog dismissal, or WebView2 native frame focus events.
      setTimeout(() => {
        if (this.container.classList.contains('active')) {
          this.focusInput();
        }
      }, 50);
    }
  }

  setSplitVisible(visible: boolean, focused: boolean) {
    this.container.classList.remove('active');
    this.container.classList.toggle('split-visible', visible);
    this.container.classList.toggle('split-focused', focused);
    if (visible) {
      // Sync canvas bitmap to container size immediately to prevent zoom flash.
      this.renderer.updateSize();
      requestAnimationFrame(() => {
        this.fit();
        this.renderer.scrollToBottom();
        if (focused) {
          this.focusInput();
        }
        this.fetchAndRenderSnapshot();
      });
    }
  }

  focus() {
    this.focusInput();
  }

  /** Focus the hidden textarea for keyboard input. */
  private focusInput() {
    this.inputTextarea.focus();
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
    this.unsubscribeTheme?.();
    this.renderer.dispose();
    this.container.remove();
  }

  showExitedOverlay() {
    if (this.isExited) return;
    this.isExited = true;

    this.exitedOverlay = document.createElement('div');
    this.exitedOverlay.className = 'terminal-exited-overlay';
    this.exitedOverlay.textContent = 'Process exited';
    this.container.appendChild(this.exitedOverlay);
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
