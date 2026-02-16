import { store, Workspace, ShellType } from '../state/store';
import { workspaceService } from '../services/workspace-service';
import { notificationStore } from '../state/notification-store';
import { open } from '@tauri-apps/plugin-dialog';
import { openPath } from '@tauri-apps/plugin-opener';
import { WorktreePanel } from './WorktreePanel';
import { invoke } from '@tauri-apps/api/core';
import {
  startDrag, getDrag, endDrag,
  createGhost, moveGhost,
  onDragMove, onDragDrop, notifyMove, notifyDrop,
} from '../state/drag-state';

const DRAG_THRESHOLD = 5;

export class WorkspaceSidebar {
  private container: HTMLElement;
  private listContainer: HTMLElement;
  private worktreePanel: WorktreePanel;
  private onDrop: ((workspaceId: string, terminalId: string) => void) | null = null;

  /** Timestamp of the last drag end, used to suppress the click that fires after pointerup. */
  private _lastDragEndTime = 0;

  constructor() {
    this.container = document.createElement('div');
    this.container.className = 'sidebar';

    const header = document.createElement('div');
    header.className = 'sidebar-header';
    header.textContent = 'Workspaces';
    this.container.appendChild(header);

    this.listContainer = document.createElement('div');
    this.listContainer.className = 'workspace-list';
    this.container.appendChild(this.listContainer);

    this.worktreePanel = new WorktreePanel();
    this.worktreePanel.mount(this.container);

    // CLAUDE.md editor buttons
    const claudeMdSection = document.createElement('div');
    claudeMdSection.className = 'claude-md-buttons';

    const projectClaudeBtn = document.createElement('div');
    projectClaudeBtn.className = 'claude-md-btn';
    projectClaudeBtn.title = 'Edit project CLAUDE.md';
    const projectIcon = document.createElement('span');
    projectIcon.textContent = '\uD83D\uDCC4';
    const projectLabel = document.createElement('span');
    projectLabel.textContent = 'Project CLAUDE.md';
    projectClaudeBtn.appendChild(projectIcon);
    projectClaudeBtn.appendChild(projectLabel);
    projectClaudeBtn.onclick = async () => {
      const state = store.getState();
      const ws = state.workspaces.find(w => w.id === state.activeWorkspaceId);
      if (!ws) return;
      const filePath = ws.folderPath.replace(/[\\/]$/, '') + '\\CLAUDE.md';
      const { showFileEditorDialog } = await import('./FileEditorDialog');
      await showFileEditorDialog('Project CLAUDE.md', filePath);
    };
    claudeMdSection.appendChild(projectClaudeBtn);

    const userClaudeBtn = document.createElement('div');
    userClaudeBtn.className = 'claude-md-btn';
    userClaudeBtn.title = 'Edit user CLAUDE.md (~/.claude/CLAUDE.md)';
    const userIcon = document.createElement('span');
    userIcon.textContent = '\uD83D\uDC64';
    const userLabel = document.createElement('span');
    userLabel.textContent = 'User CLAUDE.md';
    userClaudeBtn.appendChild(userIcon);
    userClaudeBtn.appendChild(userLabel);
    userClaudeBtn.onclick = async () => {
      try {
        const filePath = await invoke<string>('get_user_claude_md_path');
        const { showFileEditorDialog } = await import('./FileEditorDialog');
        await showFileEditorDialog('User CLAUDE.md', filePath);
      } catch (err) {
        console.error('Failed to get user CLAUDE.md path:', err);
      }
    };
    claudeMdSection.appendChild(userClaudeBtn);

    this.container.appendChild(claudeMdSection);

    const settingsBtn = document.createElement('div');
    settingsBtn.className = 'settings-btn';
    const settingsIcon = document.createElement('span');
    settingsIcon.textContent = '\u2699';
    const settingsLabel = document.createElement('span');
    settingsLabel.textContent = 'Settings';
    settingsBtn.appendChild(settingsIcon);
    settingsBtn.appendChild(settingsLabel);
    settingsBtn.onclick = async () => {
      const { showSettingsDialog } = await import('./SettingsDialog');
      await showSettingsDialog();
    };
    this.container.appendChild(settingsBtn);

    const addBtn = document.createElement('div');
    addBtn.className = 'add-workspace-btn';
    const addIcon = document.createElement('span');
    addIcon.textContent = '+';
    const addLabel = document.createElement('span');
    addLabel.textContent = 'New Workspace';
    addBtn.appendChild(addIcon);
    addBtn.appendChild(addLabel);
    addBtn.onclick = () => this.handleAddWorkspace();
    this.container.appendChild(addBtn);

    // Register as drop target for workspace reorder + tab-to-workspace drops
    onDragMove((x, y, data) => {
      // Clear all workspace highlights first
      const items = this.listContainer.querySelectorAll('.workspace-item');
      items.forEach(el => {
        el.classList.remove('drag-over', 'drag-over-workspace');
      });

      for (const item of items) {
        const rect = item.getBoundingClientRect();
        if (x < rect.left || x > rect.right || y < rect.top || y > rect.bottom) continue;

        const wsId = (item as HTMLElement).dataset.workspaceId;
        if (!wsId) continue;

        if (data.kind === 'workspace' && wsId !== data.id) {
          item.classList.add('drag-over-workspace');
        } else if (data.kind === 'tab') {
          const state = store.getState();
          const isActive = state.activeWorkspaceId === wsId;
          if (!isActive) {
            item.classList.add('drag-over');
          }
        }
        break;
      }
    });

    onDragDrop((x, y, data) => {
      const items = this.listContainer.querySelectorAll('.workspace-item');
      items.forEach(el => {
        el.classList.remove('drag-over', 'drag-over-workspace');
      });

      for (const item of items) {
        const rect = item.getBoundingClientRect();
        if (x < rect.left || x > rect.right || y < rect.top || y > rect.bottom) continue;

        const wsId = (item as HTMLElement).dataset.workspaceId;
        if (!wsId) continue;

        if (data.kind === 'workspace' && wsId !== data.id) {
          this.handleWorkspaceReorder(data.id, wsId);
        } else if (data.kind === 'tab') {
          this.onDrop?.(wsId, data.id);
        }
        break;
      }
    });

    store.subscribe(() => this.render());
    notificationStore.subscribe(() => this.render());
  }

  setOnTabDrop(handler: (workspaceId: string, terminalId: string) => void) {
    this.onDrop = handler;
  }

  private async handleAddWorkspace() {
    // Check if WSL is available
    const wslAvailable = await workspaceService.isWslAvailable().catch(() => false);

    // Show shell type selection dialog
    const shellType = await this.showShellTypeDialog(wslAvailable);
    if (!shellType) {
      return; // User cancelled
    }

    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select workspace folder',
    });

    if (selected && typeof selected === 'string') {
      const folderName = selected.split(/[/\\]/).pop() || 'New Workspace';
      await workspaceService.createWorkspace(folderName, selected, shellType);

      const state = store.getState();
      const newWorkspace = state.workspaces[state.workspaces.length - 1];
      if (newWorkspace) {
        store.setActiveWorkspace(newWorkspace.id);
      }
    }
  }

  private async showShellTypeDialog(wslAvailable: boolean): Promise<ShellType | null> {
    return new Promise(async resolve => {
      const overlay = document.createElement('div');
      overlay.className = 'dialog-overlay';

      const dialog = document.createElement('div');
      dialog.className = 'dialog';

      let wslDistributions: string[] = [];
      if (wslAvailable) {
        wslDistributions = await workspaceService.getWslDistributions().catch(() => []);
      }

      const showWslOption = wslAvailable && wslDistributions.length > 0;

      const title = document.createElement('div');
      title.className = 'dialog-title';
      title.textContent = 'New Workspace';
      dialog.appendChild(title);

      const options = document.createElement('div');
      options.className = 'shell-type-options';

      // Windows option
      const winLabel = document.createElement('label');
      winLabel.className = 'shell-type-option';
      const winRadio = document.createElement('input');
      winRadio.type = 'radio';
      winRadio.name = 'shellType';
      winRadio.value = 'windows';
      winRadio.checked = true;
      const winText = document.createElement('span');
      winText.textContent = 'Windows (PowerShell)';
      winLabel.appendChild(winRadio);
      winLabel.appendChild(winText);
      options.appendChild(winLabel);

      let distroContainer: HTMLElement | null = null;
      let distroSelect: HTMLSelectElement | null = null;

      if (showWslOption) {
        const wslLabel = document.createElement('label');
        wslLabel.className = 'shell-type-option';
        const wslRadio = document.createElement('input');
        wslRadio.type = 'radio';
        wslRadio.name = 'shellType';
        wslRadio.value = 'wsl';
        const wslText = document.createElement('span');
        wslText.textContent = 'WSL (Linux)';
        wslLabel.appendChild(wslRadio);
        wslLabel.appendChild(wslText);
        options.appendChild(wslLabel);

        distroContainer = document.createElement('div');
        distroContainer.className = 'wsl-distro-container';
        distroContainer.style.display = 'none';

        const distroLabel = document.createElement('label');
        distroLabel.className = 'wsl-distro-label';
        distroLabel.textContent = 'Distribution:';
        distroContainer.appendChild(distroLabel);

        distroSelect = document.createElement('select');
        distroSelect.className = 'wsl-distro-select dialog-input';
        for (const d of wslDistributions) {
          const opt = document.createElement('option');
          opt.value = d;
          opt.textContent = d;
          distroSelect.appendChild(opt);
        }
        distroContainer.appendChild(distroSelect);
        options.appendChild(distroContainer);

        // Toggle distro dropdown visibility
        winRadio.addEventListener('change', () => {
          if (distroContainer) distroContainer.style.display = 'none';
        });
        wslRadio.addEventListener('change', () => {
          if (distroContainer) distroContainer.style.display = 'block';
        });
      }

      dialog.appendChild(options);

      const buttons = document.createElement('div');
      buttons.className = 'dialog-buttons';

      const cancelBtn = document.createElement('button');
      cancelBtn.className = 'dialog-btn dialog-btn-secondary';
      cancelBtn.textContent = 'Cancel';
      buttons.appendChild(cancelBtn);

      const continueBtn = document.createElement('button');
      continueBtn.className = 'dialog-btn dialog-btn-primary';
      continueBtn.textContent = 'Continue';
      buttons.appendChild(continueBtn);

      dialog.appendChild(buttons);

      const close = () => overlay.remove();

      cancelBtn.onclick = () => {
        close();
        resolve(null);
      };

      continueBtn.onclick = () => {
        const selectedRadio = dialog.querySelector<HTMLInputElement>(
          'input[name="shellType"]:checked'
        );
        const selectedType = selectedRadio?.value;

        if (selectedType === 'wsl') {
          const distribution = distroSelect?.value;
          close();
          resolve({ type: 'wsl', distribution });
        } else {
          close();
          resolve({ type: 'windows' });
        }
      };

      overlay.appendChild(dialog);
      document.body.appendChild(overlay);
    });
  }

  private render() {
    // Guard: skip full rebuild while a workspace drag is active
    // (render does clear+recreate which would destroy the dragged element)
    const drag = getDrag();
    if (drag?.kind === 'workspace') return;

    // Clear existing items
    while (this.listContainer.firstChild) {
      this.listContainer.removeChild(this.listContainer.firstChild);
    }

    // Use filtered list: hides Agent workspace in main window
    store.getVisibleWorkspaces().forEach((workspace) => {
      const item = this.createWorkspaceItem(workspace);
      this.listContainer.appendChild(item);
    });
  }

  private createWorkspaceItem(workspace: Workspace): HTMLElement {
    const state = store.getState();
    const isActive = state.activeWorkspaceId === workspace.id;
    const terminalCount = store.getTerminalCount(workspace.id);
    const isWsl = workspace.shellType?.type === 'wsl';

    const item = document.createElement('div');
    item.className = `workspace-item${isActive ? ' active' : ''}`;
    item.dataset.workspaceId = workspace.id;

    const nameContainer = document.createElement('span');
    nameContainer.className = 'workspace-name-container';

    const name = document.createElement('span');
    name.className = 'workspace-name';
    name.textContent = workspace.name;
    name.title = workspace.folderPath;
    nameContainer.appendChild(name);

    if (isWsl) {
      const wslBadge = document.createElement('span');
      wslBadge.className = 'wsl-badge';
      wslBadge.textContent = 'WSL';
      nameContainer.appendChild(wslBadge);
    }

    const wtToggle = document.createElement('button');
    wtToggle.className = `worktree-toggle${workspace.worktreeMode ? ' active' : ''}`;
    wtToggle.textContent = 'WT';
    wtToggle.title = workspace.worktreeMode ? 'Worktree mode: ON' : 'Worktree mode: OFF';
    wtToggle.onclick = async (e) => {
      e.stopPropagation();
      if (!workspace.worktreeMode) {
        const isGit = await workspaceService.isGitRepo(workspace.folderPath).catch(() => false);
        if (!isGit) {
          console.warn('Cannot enable worktree mode: not a git repository');
          return;
        }
      }
      await workspaceService.toggleWorktreeMode(workspace.id, !workspace.worktreeMode);
    };
    nameContainer.appendChild(wtToggle);

    const ccToggle = document.createElement('button');
    ccToggle.className = `claude-code-toggle${workspace.claudeCodeMode ? ' active' : ''}`;
    ccToggle.textContent = 'CC';
    ccToggle.title = workspace.claudeCodeMode ? 'Claude Code mode: ON' : 'Claude Code mode: OFF';
    ccToggle.onclick = async (e) => {
      e.stopPropagation();
      await workspaceService.toggleClaudeCodeMode(workspace.id, !workspace.claudeCodeMode);
    };
    nameContainer.appendChild(ccToggle);

    item.appendChild(nameContainer);

    const hasNotification = !isActive && notificationStore.workspaceHasBadge(
      workspace.id,
      (wsId) => store.getWorkspaceTerminals(wsId),
    );
    if (hasNotification) {
      const notifBadge = document.createElement('span');
      notifBadge.className = 'workspace-notification-badge';
      item.appendChild(notifBadge);
    }

    const badge = document.createElement('span');
    badge.className = 'workspace-badge';
    badge.textContent = terminalCount.toString();
    item.appendChild(badge);

    // Click to activate (suppress if drag just ended)
    item.onclick = () => {
      if (Date.now() - this._lastDragEndTime < 100) return;
      store.setActiveWorkspace(workspace.id);
    };

    item.oncontextmenu = (e) => {
      e.preventDefault();
      this.showContextMenu(e, workspace);
    };

    // Pointer-event drag for workspace reordering
    this.setupPointerDrag(item, workspace.id);

    return item;
  }

  private setupPointerDrag(item: HTMLElement, workspaceId: string): void {
    let startX = 0;
    let startY = 0;
    let dragging = false;

    item.addEventListener('pointerdown', (e: PointerEvent) => {
      // Only left button
      if (e.button !== 0) return;
      // Don't start drag from buttons
      if ((e.target as HTMLElement).closest('button')) return;

      startX = e.clientX;
      startY = e.clientY;
      dragging = false;

      item.setPointerCapture(e.pointerId);

      const onMove = (me: PointerEvent) => {
        const dx = me.clientX - startX;
        const dy = me.clientY - startY;

        if (!dragging) {
          if (Math.abs(dx) < DRAG_THRESHOLD && Math.abs(dy) < DRAG_THRESHOLD) return;
          dragging = true;
          item.classList.add('dragging');
          startDrag({ kind: 'workspace', id: workspaceId, sourceElement: item });
          createGhost(item);
        }

        moveGhost(me.clientX, me.clientY);
        notifyMove(me.clientX, me.clientY);
      };

      const onUp = (ue: PointerEvent) => {
        item.removeEventListener('pointermove', onMove);
        item.removeEventListener('pointerup', onUp);
        item.releasePointerCapture(ue.pointerId);

        if (dragging) {
          notifyDrop(ue.clientX, ue.clientY);
          item.classList.remove('dragging');
          endDrag();
          this._lastDragEndTime = Date.now();
        }
      };

      item.addEventListener('pointermove', onMove);
      item.addEventListener('pointerup', onUp);
    });
  }

  private handleWorkspaceReorder(draggedId: string, targetId: string) {
    const state = store.getState();
    const ids = state.workspaces.map(w => w.id);

    const draggedIndex = ids.indexOf(draggedId);
    const targetIndex = ids.indexOf(targetId);
    if (draggedIndex === -1 || targetIndex === -1) return;

    ids.splice(draggedIndex, 1);
    ids.splice(targetIndex, 0, draggedId);

    store.reorderWorkspaces(ids);
  }

  private showContextMenu(e: MouseEvent, workspace: Workspace) {
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
      this.showRenameDialog(workspace);
    };
    menu.appendChild(renameItem);

    const openFolderItem = document.createElement('div');
    openFolderItem.className = 'context-menu-item';
    openFolderItem.textContent = 'Open in Explorer';
    openFolderItem.onclick = () => {
      menu.remove();
      openPath(workspace.folderPath);
    };
    menu.appendChild(openFolderItem);

    // Worktree mode toggle (only for git repos)
    const worktreeItem = document.createElement('div');
    worktreeItem.className = 'context-menu-item';
    worktreeItem.textContent = workspace.worktreeMode
      ? 'Disable Worktree Mode'
      : 'Enable Worktree Mode';
    worktreeItem.onclick = async () => {
      menu.remove();
      const isGit = await workspaceService.isGitRepo(workspace.folderPath).catch(() => false);
      if (!isGit && !workspace.worktreeMode) {
        console.warn('Cannot enable worktree mode: not a git repository');
        return;
      }
      await workspaceService.toggleWorktreeMode(workspace.id, !workspace.worktreeMode);
    };
    menu.appendChild(worktreeItem);

    const claudeCodeItem = document.createElement('div');
    claudeCodeItem.className = 'context-menu-item';
    claudeCodeItem.textContent = workspace.claudeCodeMode
      ? 'Disable Claude Code Mode'
      : 'Enable Claude Code Mode';
    claudeCodeItem.onclick = async () => {
      menu.remove();
      await workspaceService.toggleClaudeCodeMode(workspace.id, !workspace.claudeCodeMode);
    };
    menu.appendChild(claudeCodeItem);

    const separator = document.createElement('div');
    separator.className = 'context-menu-separator';
    menu.appendChild(separator);

    const deleteItem = document.createElement('div');
    deleteItem.className = 'context-menu-item danger';
    deleteItem.textContent = 'Delete';
    deleteItem.onclick = async () => {
      menu.remove();
      await workspaceService.deleteWorkspace(workspace.id);
    };
    menu.appendChild(deleteItem);

    document.body.appendChild(menu);

    const closeMenu = () => {
      menu.remove();
      document.removeEventListener('click', closeMenu);
    };
    setTimeout(() => {
      document.addEventListener('click', closeMenu);
    }, 0);
  }

  private showRenameDialog(workspace: Workspace) {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const dialog = document.createElement('div');
    dialog.className = 'dialog';

    const title = document.createElement('div');
    title.className = 'dialog-title';
    title.textContent = 'Rename Workspace';
    dialog.appendChild(title);

    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'dialog-input';
    input.value = workspace.name;
    dialog.appendChild(input);

    const buttons = document.createElement('div');
    buttons.className = 'dialog-buttons';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'dialog-btn dialog-btn-secondary';
    cancelBtn.textContent = 'Cancel';
    buttons.appendChild(cancelBtn);

    const renameBtn = document.createElement('button');
    renameBtn.className = 'dialog-btn dialog-btn-primary';
    renameBtn.textContent = 'Rename';
    buttons.appendChild(renameBtn);

    dialog.appendChild(buttons);

    const close = () => overlay.remove();

    cancelBtn.onclick = close;
    renameBtn.onclick = () => {
      const newName = input.value.trim();
      if (newName) {
        store.updateWorkspace(workspace.id, { name: newName });
      }
      close();
    };

    input.onkeydown = (e) => {
      if (e.key === 'Enter') renameBtn.click();
      if (e.key === 'Escape') close();
    };

    overlay.appendChild(dialog);
    document.body.appendChild(overlay);
    input.focus();
    input.select();
  }

  mount(parent: HTMLElement) {
    parent.appendChild(this.container);
    this.render();
  }
}
