import { store, Workspace, ShellType } from '../state/store';
import { workspaceService } from '../services/workspace-service';
import { open } from '@tauri-apps/plugin-dialog';
import { WorktreePanel } from './WorktreePanel';

export class WorkspaceSidebar {
  private container: HTMLElement;
  private listContainer: HTMLElement;
  private worktreePanel: WorktreePanel;
  private onDrop: ((workspaceId: string, terminalId: string) => void) | null = null;

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

    const addBtn = document.createElement('div');
    addBtn.className = 'add-workspace-btn';
    addBtn.innerHTML = '<span>+</span><span>New Workspace</span>';
    addBtn.onclick = () => this.handleAddWorkspace();
    this.container.appendChild(addBtn);

    store.subscribe(() => this.render());
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

      dialog.innerHTML = `
        <div class="dialog-title">New Workspace</div>
        <div class="shell-type-options">
          <label class="shell-type-option">
            <input type="radio" name="shellType" value="windows" checked />
            <span>Windows (PowerShell)</span>
          </label>
          ${
            showWslOption
              ? `
          <label class="shell-type-option">
            <input type="radio" name="shellType" value="wsl" />
            <span>WSL (Linux)</span>
          </label>
          <div class="wsl-distro-container" style="display: none;">
            <label class="wsl-distro-label">Distribution:</label>
            <select class="wsl-distro-select dialog-input">
              ${wslDistributions.map(d => `<option value="${d}">${d}</option>`).join('')}
            </select>
          </div>
          `
              : ''
          }
        </div>
        <div class="dialog-buttons">
          <button class="dialog-btn dialog-btn-secondary">Cancel</button>
          <button class="dialog-btn dialog-btn-primary">Continue</button>
        </div>
      `;

      const close = () => overlay.remove();

      const [cancelBtn, continueBtn] = dialog.querySelectorAll('button');
      const radioInputs = dialog.querySelectorAll<HTMLInputElement>('input[name="shellType"]');
      const distroContainer = dialog.querySelector<HTMLElement>('.wsl-distro-container');
      const distroSelect = dialog.querySelector<HTMLSelectElement>('.wsl-distro-select');

      // Toggle distro dropdown visibility
      radioInputs.forEach(radio => {
        radio.addEventListener('change', () => {
          if (distroContainer) {
            distroContainer.style.display = radio.value === 'wsl' ? 'block' : 'none';
          }
        });
      });

      cancelBtn.onclick = () => {
        close();
        resolve(null);
      };

      continueBtn.onclick = () => {
        const selectedType = dialog.querySelector<HTMLInputElement>(
          'input[name="shellType"]:checked'
        )?.value;

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
    const state = store.getState();

    this.listContainer.innerHTML = '';

    state.workspaces.forEach((workspace) => {
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

    const badge = document.createElement('span');
    badge.className = 'workspace-badge';
    badge.textContent = terminalCount.toString();
    item.appendChild(badge);

    item.onclick = () => {
      store.setActiveWorkspace(workspace.id);
    };

    item.oncontextmenu = (e) => {
      e.preventDefault();
      this.showContextMenu(e, workspace);
    };

    // Drop zone for tabs
    item.ondragover = (e) => {
      e.preventDefault();
      const terminalId = e.dataTransfer?.types.includes('text/plain');
      if (terminalId && !isActive) {
        item.classList.add('drag-over');
      }
    };

    item.ondragleave = () => {
      item.classList.remove('drag-over');
    };

    item.ondrop = (e) => {
      e.preventDefault();
      item.classList.remove('drag-over');

      const terminalId = e.dataTransfer?.getData('text/plain');
      if (terminalId && this.onDrop) {
        this.onDrop(workspace.id, terminalId);
      }
    };

    return item;
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

    dialog.innerHTML = `
      <div class="dialog-title">Rename Workspace</div>
      <input type="text" class="dialog-input" value="${workspace.name}" />
      <div class="dialog-buttons">
        <button class="dialog-btn dialog-btn-secondary">Cancel</button>
        <button class="dialog-btn dialog-btn-primary">Rename</button>
      </div>
    `;

    const input = dialog.querySelector('input')!;
    const [cancelBtn, renameBtn] = dialog.querySelectorAll('button');

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
