import { store, Terminal } from '../state/store';
import { terminalService } from '../services/terminal-service';
import { workspaceService } from '../services/workspace-service';
import { notificationStore } from '../state/notification-store';
import {
  startDrag, endDrag,
  createGhost, moveGhost,
  onDragMove, onDragDrop, notifyMove, notifyDrop,
} from '../state/drag-state';

export function getDisplayName(terminal: Terminal): string {
  if (terminal.userRenamed) return terminal.name;
  return terminal.oscTitle || terminal.name || terminal.processName || 'Terminal';
}

const DRAG_THRESHOLD = 5; // px of movement before drag starts

export class TabBar {
  private container: HTMLElement;
  private tabsContainer: HTMLElement;
  private tabElements: Map<string, HTMLElement> = new Map();
  private lastRenderedOrder: string[] = [];
  private onSplitCallback: ((terminalId: string, direction: 'horizontal' | 'vertical') => void) | null = null;
  private onUnsplitCallback: (() => void) | null = null;

  /** Timestamp of the last drag end, used to suppress the click that fires after pointerup. */
  private _lastDragEndTime = 0;

  constructor() {
    this.container = document.createElement('div');
    this.container.className = 'tab-bar';

    this.tabsContainer = document.createElement('div');
    this.tabsContainer.style.display = 'flex';
    this.tabsContainer.style.flex = '1';
    this.tabsContainer.style.minWidth = '0';
    this.container.appendChild(this.tabsContainer);

    const addBtn = document.createElement('div');
    addBtn.className = 'add-tab-btn';
    addBtn.textContent = '+';
    addBtn.title = 'New terminal (Ctrl+T)';
    addBtn.onclick = () => this.handleNewTab();
    this.container.appendChild(addBtn);

    // Register as a drop target for tab drags (reorder)
    onDragMove((x, y, data) => {
      if (data.kind !== 'tab') return;
      // Clear all drag-over highlights first
      for (const el of this.tabElements.values()) {
        el.classList.remove('drag-over');
      }
      // Find which tab the pointer is over
      for (const [id, el] of this.tabElements) {
        if (id === data.id) continue;
        const rect = el.getBoundingClientRect();
        if (x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) {
          el.classList.add('drag-over');
          break;
        }
      }
    });

    onDragDrop((x, y, data) => {
      if (data.kind !== 'tab') return;
      for (const el of this.tabElements.values()) {
        el.classList.remove('drag-over');
      }
      // Find drop target
      for (const [id, el] of this.tabElements) {
        if (id === data.id) continue;
        const rect = el.getBoundingClientRect();
        if (x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) {
          this.handleReorder(data.id, id);
          break;
        }
      }
    });

    store.subscribe(() => this.render());
    notificationStore.subscribe(() => this.render());
  }

  private async handleNewTab() {
    const state = store.getState();
    if (!state.activeWorkspaceId) return;

    const workspace = state.workspaces.find(w => w.id === state.activeWorkspaceId);
    let worktreeName: string | undefined;

    if (workspace?.worktreeMode) {
      const { showWorktreeNamePrompt } = await import('./dialogs');
      const name = await showWorktreeNamePrompt();
      if (name === null) return; // user cancelled
      worktreeName = name || undefined; // empty string = auto-generate
    }

    const result = await terminalService.createTerminal(
      state.activeWorkspaceId,
      { worktreeName }
    );
    store.addTerminal({
      id: result.id,
      workspaceId: state.activeWorkspaceId,
      name: result.worktree_branch ?? 'Terminal',
      processName: 'powershell',
      order: 0,
    });

    if (workspace?.claudeCodeMode) {
      setTimeout(() => {
        terminalService.writeToTerminal(result.id, 'claude -dangerously-skip-permissions\r');
      }, 500);
    }
  }

  private render() {
    const state = store.getState();
    const terminals = store.getWorkspaceTerminals(
      state.activeWorkspaceId || ''
    );

    const currentIds = terminals.map(t => t.id);
    const currentIdSet = new Set(currentIds);

    // Remove tabs that no longer exist
    for (const [id, el] of this.tabElements) {
      if (!currentIdSet.has(id)) {
        el.remove();
        this.tabElements.delete(id);
      }
    }

    // Check if order changed (needs DOM reorder)
    const orderChanged =
      currentIds.length !== this.lastRenderedOrder.length ||
      currentIds.some((id, i) => this.lastRenderedOrder[i] !== id);

    // Update existing tabs in-place, create new ones
    for (const terminal of terminals) {
      const existing = this.tabElements.get(terminal.id);
      if (existing) {
        this.updateTabInPlace(existing, terminal, state.activeTerminalId);
      } else {
        const tab = this.createTab(terminal);
        this.tabElements.set(terminal.id, tab);
        this.tabsContainer.appendChild(tab);
      }
    }

    // Reorder DOM if needed
    if (orderChanged) {
      for (const terminal of terminals) {
        const el = this.tabElements.get(terminal.id);
        if (el) {
          this.tabsContainer.appendChild(el);
        }
      }
      this.lastRenderedOrder = currentIds;
    }
  }

  private updateTabInPlace(tab: HTMLElement, terminal: Terminal, activeTerminalId: string | null) {
    const isActive = activeTerminalId === terminal.id;
    const shouldBeActive = tab.classList.contains('active');
    if (isActive !== shouldBeActive) {
      tab.classList.toggle('active', isActive);
    }

    // Split indicator: highlight tabs that are part of a split
    const state = store.getState();
    const split = state.activeWorkspaceId
      ? store.getSplitView(state.activeWorkspaceId)
      : null;
    const isInSplit = split &&
      (split.leftTerminalId === terminal.id || split.rightTerminalId === terminal.id);
    tab.classList.toggle('in-split', !!isInSplit);

    const titleEl = tab.querySelector('.tab-title') as HTMLSpanElement | null;
    if (titleEl) {
      const displayName = getDisplayName(terminal);
      if (titleEl.textContent !== displayName) {
        titleEl.textContent = displayName;
      }
    }

    const hasBadge = notificationStore.hasBadge(terminal.id) && !isActive;
    const existingBadge = tab.querySelector('.tab-notification-badge');
    if (hasBadge && !existingBadge) {
      const badge = document.createElement('span');
      badge.className = 'tab-notification-badge';
      // Insert before close button
      const closeBtn = tab.querySelector('.tab-close');
      if (closeBtn) {
        tab.insertBefore(badge, closeBtn);
      } else {
        tab.appendChild(badge);
      }
    } else if (!hasBadge && existingBadge) {
      existingBadge.remove();
    }
  }

  private createTab(terminal: Terminal): HTMLElement {
    const state = store.getState();
    const isActive = state.activeTerminalId === terminal.id;

    const tab = document.createElement('div');
    tab.className = `tab${isActive ? ' active' : ''}`;
    tab.dataset.terminalId = terminal.id;

    const displayName = getDisplayName(terminal);

    const title = document.createElement('span');
    title.className = 'tab-title';
    title.textContent = displayName;
    tab.appendChild(title);

    if (notificationStore.hasBadge(terminal.id) && !isActive) {
      const badge = document.createElement('span');
      badge.className = 'tab-notification-badge';
      tab.appendChild(badge);
    }

    const closeBtn = document.createElement('span');
    closeBtn.className = 'tab-close';
    closeBtn.textContent = '\u00d7';
    closeBtn.onclick = (e) => {
      e.stopPropagation();
      this.handleCloseTab(terminal.id);
    };
    tab.appendChild(closeBtn);

    // Click to activate (suppress if drag just ended)
    tab.onclick = () => {
      if (Date.now() - this._lastDragEndTime < 100) return;
      store.setActiveTerminal(terminal.id);
    };

    // Double-click: rename in single mode, unsplit in split mode
    title.ondblclick = (e) => {
      e.stopPropagation();
      const currentState = store.getState();
      const split = currentState.activeWorkspaceId
        ? store.getSplitView(currentState.activeWorkspaceId)
        : null;
      if (split) {
        // Double-click in split mode: unsplit (maximize this terminal)
        store.setActiveTerminal(terminal.id);
        this.onUnsplitCallback?.();
      } else {
        this.startRename(title, terminal);
      }
    };

    // Context menu
    tab.oncontextmenu = (e) => {
      e.preventDefault();
      this.showContextMenu(e, terminal);
    };

    // Pointer-event drag (replaces HTML5 DnD)
    this.setupPointerDrag(tab, terminal.id);

    return tab;
  }

  private setupPointerDrag(tab: HTMLElement, terminalId: string): void {
    let startX = 0;
    let startY = 0;
    let dragging = false;

    tab.addEventListener('pointerdown', (e: PointerEvent) => {
      // Only left button
      if (e.button !== 0) return;
      // Don't start drag from close button
      if ((e.target as HTMLElement).closest('.tab-close')) return;

      startX = e.clientX;
      startY = e.clientY;
      dragging = false;

      tab.setPointerCapture(e.pointerId);

      const onMove = (me: PointerEvent) => {
        const dx = me.clientX - startX;
        const dy = me.clientY - startY;

        if (!dragging) {
          if (Math.abs(dx) < DRAG_THRESHOLD && Math.abs(dy) < DRAG_THRESHOLD) return;
          // Start drag
          dragging = true;
          tab.classList.add('dragging');
          startDrag({ kind: 'tab', id: terminalId, sourceElement: tab });
          createGhost(tab);
        }

        moveGhost(me.clientX, me.clientY);
        notifyMove(me.clientX, me.clientY);
      };

      const onUp = (ue: PointerEvent) => {
        tab.removeEventListener('pointermove', onMove);
        tab.removeEventListener('pointerup', onUp);
        tab.releasePointerCapture(ue.pointerId);

        if (dragging) {
          notifyDrop(ue.clientX, ue.clientY);
          tab.classList.remove('dragging');
          endDrag();
          this._lastDragEndTime = Date.now();
        }
      };

      tab.addEventListener('pointermove', onMove);
      tab.addEventListener('pointerup', onUp);
    });
  }

  private async handleCloseTab(terminalId: string) {
    await terminalService.closeTerminal(terminalId);
    store.removeTerminal(terminalId);
  }

  private startRename(titleEl: HTMLSpanElement, terminal: Terminal) {
    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'tab-title editing';
    input.value = terminal.name || terminal.processName || '';

    const finishRename = async () => {
      const newName = input.value.trim();
      if (newName) {
        await terminalService.renameTerminal(terminal.id, newName);
        store.updateTerminal(terminal.id, { userRenamed: true });
      }
      this.render();
    };

    input.onblur = finishRename;
    input.onkeydown = (e) => {
      if (e.key === 'Enter') {
        finishRename();
      } else if (e.key === 'Escape') {
        this.render();
      }
    };

    titleEl.replaceWith(input);
    input.focus();
    input.select();
  }

  private showContextMenu(e: MouseEvent, terminal: Terminal) {
    // Remove existing context menu
    document.querySelector('.context-menu')?.remove();

    const menu = document.createElement('div');
    menu.className = 'context-menu';
    menu.style.left = `${e.clientX}px`;
    menu.style.top = `${e.clientY}px`;

    const renameItem = document.createElement('div');
    renameItem.className = 'context-menu-item';
    renameItem.textContent = 'Rename';
    renameItem.onclick = () => {
      menu.remove();
      const titleEl = document.querySelector(
        `.tab[data-terminal-id="${terminal.id}"] .tab-title`
      ) as HTMLSpanElement;
      if (titleEl) {
        this.startRename(titleEl, terminal);
      }
    };
    menu.appendChild(renameItem);

    // Split options
    const state = store.getState();
    const wsTerminals = store.getWorkspaceTerminals(state.activeWorkspaceId || '');
    const split = state.activeWorkspaceId
      ? store.getSplitView(state.activeWorkspaceId)
      : null;

    if (wsTerminals.length >= 2) {
      const splitSep = document.createElement('div');
      splitSep.className = 'context-menu-separator';
      menu.appendChild(splitSep);

      if (split) {
        const unsplitItem = document.createElement('div');
        unsplitItem.className = 'context-menu-item';
        unsplitItem.textContent = 'Unsplit';
        unsplitItem.onclick = () => {
          menu.remove();
          this.onUnsplitCallback?.();
        };
        menu.appendChild(unsplitItem);
      } else {
        const splitRightItem = document.createElement('div');
        splitRightItem.className = 'context-menu-item';
        splitRightItem.textContent = 'Split Right';
        splitRightItem.onclick = () => {
          menu.remove();
          this.onSplitCallback?.(terminal.id, 'horizontal');
        };
        menu.appendChild(splitRightItem);

        const splitDownItem = document.createElement('div');
        splitDownItem.className = 'context-menu-item';
        splitDownItem.textContent = 'Split Down';
        splitDownItem.onclick = () => {
          menu.remove();
          this.onSplitCallback?.(terminal.id, 'vertical');
        };
        menu.appendChild(splitDownItem);
      }
    }

    const separator = document.createElement('div');
    separator.className = 'context-menu-separator';
    menu.appendChild(separator);

    const closeItem = document.createElement('div');
    closeItem.className = 'context-menu-item danger';
    closeItem.textContent = 'Close';
    closeItem.onclick = () => {
      menu.remove();
      this.handleCloseTab(terminal.id);
    };
    menu.appendChild(closeItem);

    document.body.appendChild(menu);

    const closeMenu = () => {
      menu.remove();
      document.removeEventListener('click', closeMenu);
    };
    setTimeout(() => {
      document.addEventListener('click', closeMenu);
    }, 0);
  }

  private async handleReorder(
    draggedId: string,
    targetId: string
  ) {
    const state = store.getState();
    if (!state.activeWorkspaceId) return;

    const terminals = store.getWorkspaceTerminals(state.activeWorkspaceId);
    const ids = terminals.map((t) => t.id);

    const draggedIndex = ids.indexOf(draggedId);
    const targetIndex = ids.indexOf(targetId);

    if (draggedIndex === -1 || targetIndex === -1) return;

    ids.splice(draggedIndex, 1);
    ids.splice(targetIndex, 0, draggedId);

    await workspaceService.reorderTabs(state.activeWorkspaceId, ids);
  }

  setOnSplit(callback: (terminalId: string, direction: 'horizontal' | 'vertical') => void) {
    this.onSplitCallback = callback;
  }

  setOnUnsplit(callback: () => void) {
    this.onUnsplitCallback = callback;
  }

  mount(parent: HTMLElement) {
    parent.appendChild(this.container);
    this.render();
  }
}
