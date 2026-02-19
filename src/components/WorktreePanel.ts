import { listen } from '@tauri-apps/api/event';
import { store } from '../state/store';
import { workspaceService, WorktreeInfo } from '../services/workspace-service';
import { terminalService } from '../services/terminal-service';

interface CleanupProgress {
  step: 'listing' | 'removing' | 'done';
  current: number;
  total: number;
  worktree_name: string;
}

export class WorktreePanel {
  private container: HTMLElement;
  private listContainer: HTMLElement;
  private statusContainer: HTMLElement;
  private headerToggle: HTMLElement;
  private cleanAllBtn: HTMLElement;
  private refreshBtn: HTMLElement;
  private collapsed = false;
  private worktrees: WorktreeInfo[] = [];
  private deleting = new Set<string>();
  private refreshTimer: ReturnType<typeof setTimeout> | null = null;
  private refreshing = false;
  private cleaningAll = false;

  constructor() {
    this.container = document.createElement('div');
    this.container.className = 'worktree-panel';

    const header = document.createElement('div');
    header.className = 'worktree-panel-header';

    this.headerToggle = document.createElement('span');
    this.headerToggle.className = 'worktree-panel-toggle';
    this.headerToggle.textContent = 'Worktrees';
    this.headerToggle.onclick = () => this.toggleCollapse();
    header.appendChild(this.headerToggle);

    const headerActions = document.createElement('div');
    headerActions.className = 'worktree-header-actions';

    this.refreshBtn = document.createElement('button');
    this.refreshBtn.className = 'worktree-refresh-btn';
    this.refreshBtn.textContent = '\u21BB';
    this.refreshBtn.title = 'Refresh worktree list';
    this.refreshBtn.onclick = (e) => {
      e.stopPropagation();
      this.manualRefresh();
    };
    headerActions.appendChild(this.refreshBtn);

    this.cleanAllBtn = document.createElement('button');
    this.cleanAllBtn.className = 'worktree-clean-all';
    this.cleanAllBtn.textContent = 'Clean All';
    this.cleanAllBtn.title = 'Remove all godly-managed worktrees';
    this.cleanAllBtn.onclick = () => this.handleCleanAll();
    headerActions.appendChild(this.cleanAllBtn);

    header.appendChild(headerActions);
    this.container.appendChild(header);

    this.statusContainer = document.createElement('div');
    this.statusContainer.className = 'worktree-status';
    this.statusContainer.style.display = 'none';
    this.container.appendChild(this.statusContainer);

    this.listContainer = document.createElement('div');
    this.listContainer.className = 'worktree-list';
    this.container.appendChild(this.listContainer);

    store.subscribe(() => this.onStoreChange());
  }

  private onStoreChange() {
    const state = store.getState();
    const activeWorkspace = state.workspaces.find(
      w => w.id === state.activeWorkspaceId
    );

    if (activeWorkspace?.worktreeMode) {
      this.container.style.display = '';
      this.debouncedRefresh(activeWorkspace.folderPath);
    } else {
      this.container.style.display = 'none';
    }
  }

  private debouncedRefresh(folderPath: string) {
    if (this.refreshTimer) {
      clearTimeout(this.refreshTimer);
    }
    this.refreshTimer = setTimeout(() => {
      this.refreshTimer = null;
      this.refresh(folderPath);
    }, 500);
  }

  private async manualRefresh() {
    if (this.refreshing) return;
    const state = store.getState();
    const activeWorkspace = state.workspaces.find(
      w => w.id === state.activeWorkspaceId
    );
    if (!activeWorkspace) return;
    await this.refresh(activeWorkspace.folderPath);
  }

  async refresh(folderPath: string) {
    if (this.refreshing) return;
    this.refreshing = true;
    this.refreshBtn.classList.add('spinning');
    try {
      this.worktrees = await workspaceService.listWorktrees(folderPath);
    } catch {
      this.worktrees = [];
    }
    this.refreshing = false;
    this.refreshBtn.classList.remove('spinning');
    this.render();
  }

  private render() {
    while (this.listContainer.firstChild) {
      this.listContainer.removeChild(this.listContainer.firstChild);
    }

    if (this.collapsed) {
      this.listContainer.style.display = 'none';
      this.cleanAllBtn.style.display = 'none';
      this.refreshBtn.style.display = 'none';
      return;
    }

    this.listContainer.style.display = '';
    this.cleanAllBtn.style.display = '';
    this.refreshBtn.style.display = '';

    const nonMain = this.worktrees.filter(wt => !wt.is_main);
    if (nonMain.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'worktree-empty';
      empty.textContent = 'No worktrees';
      this.listContainer.appendChild(empty);
      this.cleanAllBtn.style.display = 'none';
      return;
    }

    for (const wt of nonMain) {
      const item = document.createElement('div');
      item.className = 'worktree-item';

      const info = document.createElement('div');
      info.className = 'worktree-item-info';
      info.title = `Open terminal in ${wt.path}`;
      info.onclick = (e) => {
        e.stopPropagation();
        this.handleOpen(wt);
      };

      const branch = document.createElement('span');
      branch.className = 'worktree-item-branch';
      branch.textContent = wt.branch;
      info.appendChild(branch);

      const commit = document.createElement('span');
      commit.className = 'worktree-item-commit';
      commit.textContent = wt.commit.substring(0, 7);
      info.appendChild(commit);

      item.appendChild(info);

      const openBtn = document.createElement('button');
      openBtn.className = 'worktree-item-open';
      openBtn.textContent = '\u25B6';
      openBtn.title = `Open terminal in ${wt.branch}`;
      openBtn.onclick = (e) => {
        e.stopPropagation();
        this.handleOpen(wt);
      };
      item.appendChild(openBtn);

      const isDeleting = this.deleting.has(wt.path);
      const deleteBtn = document.createElement('button');
      deleteBtn.className = 'worktree-item-delete';
      if (isDeleting) {
        deleteBtn.classList.add('deleting');
        deleteBtn.textContent = '\u23F3';
        deleteBtn.disabled = true;
      } else {
        deleteBtn.textContent = '\u00d7';
      }
      deleteBtn.title = isDeleting ? 'Removing...' : 'Remove this worktree';
      deleteBtn.onclick = async (e) => {
        e.stopPropagation();
        if (!isDeleting) {
          await this.handleRemove(wt.path);
        }
      };
      item.appendChild(deleteBtn);

      this.listContainer.appendChild(item);
    }
  }

  private toggleCollapse() {
    this.collapsed = !this.collapsed;
    this.headerToggle.classList.toggle('collapsed', this.collapsed);
    this.render();
  }

  private async handleOpen(wt: WorktreeInfo) {
    const state = store.getState();
    const workspace = state.workspaces.find(
      w => w.id === state.activeWorkspaceId
    );
    if (!workspace) return;

    const result = await terminalService.createTerminal(workspace.id, {
      cwdOverride: wt.path,
      nameOverride: wt.branch,
    });

    store.addTerminal({
      id: result.id,
      workspaceId: workspace.id,
      name: wt.branch,
      processName: 'powershell',
      order: 0,
    });

    if (workspace.claudeCodeMode) {
      setTimeout(() => {
        terminalService.writeToTerminal(result.id, 'claude --dangerously-skip-permissions\r');
      }, 500);
    }
  }

  private async handleRemove(worktreePath: string) {
    const state = store.getState();
    const activeWorkspace = state.workspaces.find(
      w => w.id === state.activeWorkspaceId
    );
    if (!activeWorkspace) return;

    this.deleting.add(worktreePath);
    this.render();

    try {
      await workspaceService.removeWorktree(
        activeWorkspace.folderPath,
        worktreePath,
        true
      );
    } catch (e) {
      console.error('Failed to remove worktree:', e);
    } finally {
      this.deleting.delete(worktreePath);
      await this.refresh(activeWorkspace.folderPath);
    }
  }

  private async handleCleanAll() {
    if (this.cleaningAll) return;

    const state = store.getState();
    const activeWorkspace = state.workspaces.find(
      w => w.id === state.activeWorkspaceId
    );
    if (!activeWorkspace) return;

    this.cleaningAll = true;
    this.renderCleanAllButton();
    this.setStatus('Starting cleanup...');

    try {
      const removed = await workspaceService.cleanupAllWorktrees(
        activeWorkspace.folderPath
      );
      console.log(`Cleaned up ${removed} worktrees`);
      await this.refresh(activeWorkspace.folderPath);
    } catch (e) {
      console.error('Failed to clean worktrees:', e);
      this.setStatus(`Error: ${e}`);
      setTimeout(() => this.clearStatus(), 5000);
    } finally {
      this.cleaningAll = false;
      this.renderCleanAllButton();
    }
  }

  private setupCleanupListener() {
    listen<CleanupProgress>('worktree-cleanup-progress', (event) => {
      const { step, current, total, worktree_name } = event.payload;
      switch (step) {
        case 'listing':
          this.setStatus('Listing worktrees...');
          break;
        case 'removing':
          this.setStatus(`Removing ${worktree_name} (${current}/${total})...`);
          break;
        case 'done':
          this.setStatus(`Done! Removed ${current} worktree${current !== 1 ? 's' : ''}`);
          setTimeout(() => this.clearStatus(), 3000);
          break;
      }
    });
  }

  private renderCleanAllButton() {
    if (this.cleaningAll) {
      this.cleanAllBtn.classList.add('cleaning');
      this.cleanAllBtn.textContent = '\u23F3 Cleaning...';
      (this.cleanAllBtn as HTMLButtonElement).disabled = true;
    } else {
      this.cleanAllBtn.classList.remove('cleaning');
      this.cleanAllBtn.textContent = 'Clean All';
      (this.cleanAllBtn as HTMLButtonElement).disabled = false;
    }
  }

  private setStatus(text: string) {
    this.statusContainer.textContent = text;
    this.statusContainer.style.display = '';
  }

  private clearStatus() {
    this.statusContainer.textContent = '';
    this.statusContainer.style.display = 'none';
  }

  mount(parent: HTMLElement) {
    parent.appendChild(this.container);
    this.setupCleanupListener();
    this.onStoreChange();
  }
}
