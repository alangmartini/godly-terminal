import { store, Terminal } from '../state/store';
import { terminalService } from '../services/terminal-service';
import { workspaceService } from '../services/workspace-service';
import { notificationStore } from '../state/notification-store';

export function getDisplayName(terminal: Terminal): string {
  if (terminal.userRenamed) return terminal.name;
  return terminal.oscTitle || terminal.name || terminal.processName || 'Terminal';
}

export class TabBar {
  private container: HTMLElement;
  private tabsContainer: HTMLElement;
  private draggedTab: HTMLElement | null = null;
  private draggedTerminalId: string | null = null;

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
    addBtn.innerHTML = '+';
    addBtn.title = 'New terminal (Ctrl+T)';
    addBtn.onclick = () => this.handleNewTab();
    this.container.appendChild(addBtn);

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

    this.tabsContainer.innerHTML = '';

    terminals.forEach((terminal) => {
      const tab = this.createTab(terminal);
      this.tabsContainer.appendChild(tab);
    });
  }

  private createTab(terminal: Terminal): HTMLElement {
    const state = store.getState();
    const isActive = state.activeTerminalId === terminal.id;

    const tab = document.createElement('div');
    tab.className = `tab${isActive ? ' active' : ''}`;
    tab.dataset.terminalId = terminal.id;
    tab.draggable = true;

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
    closeBtn.innerHTML = '×';
    closeBtn.onclick = (e) => {
      e.stopPropagation();
      this.handleCloseTab(terminal.id);
    };
    tab.appendChild(closeBtn);

    // Click to activate
    tab.onclick = () => {
      store.setActiveTerminal(terminal.id);
    };

    // Double-click to rename
    title.ondblclick = (e) => {
      e.stopPropagation();
      this.startRename(title, terminal);
    };

    // Context menu
    tab.oncontextmenu = (e) => {
      e.preventDefault();
      this.showContextMenu(e, terminal);
    };

    // Drag events
    tab.ondragstart = (e) => {
      this.draggedTab = tab;
      this.draggedTerminalId = terminal.id;
      tab.classList.add('dragging');
      e.dataTransfer!.effectAllowed = 'move';
      e.dataTransfer!.setData('text/plain', terminal.id);
    };

    tab.ondragend = () => {
      tab.classList.remove('dragging');
      this.draggedTab = null;
      this.draggedTerminalId = null;
      document.querySelectorAll('.drag-over').forEach((el) => {
        el.classList.remove('drag-over');
      });
    };

    tab.ondragover = (e) => {
      e.preventDefault();
      e.dataTransfer!.dropEffect = 'move';
      if (this.draggedTab && this.draggedTab !== tab) {
        tab.classList.add('drag-over');
      }
    };

    tab.ondragleave = () => {
      tab.classList.remove('drag-over');
    };

    tab.ondrop = (e) => {
      e.preventDefault();
      tab.classList.remove('drag-over');

      if (this.draggedTerminalId && this.draggedTerminalId !== terminal.id) {
        this.handleReorder(this.draggedTerminalId, terminal.id);
      }
    };

    return tab;
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

  getDraggedTerminalId(): string | null {
    return this.draggedTerminalId;
  }

  mount(parent: HTMLElement) {
    parent.appendChild(this.container);
    this.render();
  }
}
