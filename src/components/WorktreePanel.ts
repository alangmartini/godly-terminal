import { store } from '../state/store';
import { workspaceService, WorktreeInfo } from '../services/workspace-service';

export class WorktreePanel {
  private container: HTMLElement;
  private listContainer: HTMLElement;
  private headerToggle: HTMLElement;
  private cleanAllBtn: HTMLElement;
  private collapsed = false;
  private worktrees: WorktreeInfo[] = [];

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

    this.cleanAllBtn = document.createElement('button');
    this.cleanAllBtn.className = 'worktree-clean-all';
    this.cleanAllBtn.textContent = 'Clean All';
    this.cleanAllBtn.title = 'Remove all godly-managed worktrees';
    this.cleanAllBtn.onclick = () => this.handleCleanAll();
    header.appendChild(this.cleanAllBtn);

    this.container.appendChild(header);

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
      this.refresh(activeWorkspace.folderPath);
    } else {
      this.container.style.display = 'none';
    }
  }

  async refresh(folderPath: string) {
    try {
      this.worktrees = await workspaceService.listWorktrees(folderPath);
    } catch {
      this.worktrees = [];
    }
    this.render();
  }

  private render() {
    // Clear children safely
    while (this.listContainer.firstChild) {
      this.listContainer.removeChild(this.listContainer.firstChild);
    }

    if (this.collapsed) {
      this.listContainer.style.display = 'none';
      this.cleanAllBtn.style.display = 'none';
      return;
    }

    this.listContainer.style.display = '';
    this.cleanAllBtn.style.display = '';

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

      const branch = document.createElement('span');
      branch.className = 'worktree-item-branch';
      branch.textContent = wt.branch;
      branch.title = wt.path;
      info.appendChild(branch);

      const commit = document.createElement('span');
      commit.className = 'worktree-item-commit';
      commit.textContent = wt.commit.substring(0, 7);
      info.appendChild(commit);

      item.appendChild(info);

      const deleteBtn = document.createElement('button');
      deleteBtn.className = 'worktree-item-delete';
      deleteBtn.textContent = '\u00d7';
      deleteBtn.title = 'Remove this worktree';
      deleteBtn.onclick = async (e) => {
        e.stopPropagation();
        await this.handleRemove(wt.path);
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

  private async handleRemove(worktreePath: string) {
    const state = store.getState();
    const activeWorkspace = state.workspaces.find(
      w => w.id === state.activeWorkspaceId
    );
    if (!activeWorkspace) return;

    try {
      await workspaceService.removeWorktree(
        activeWorkspace.folderPath,
        worktreePath,
        true
      );
      await this.refresh(activeWorkspace.folderPath);
    } catch (e) {
      console.error('Failed to remove worktree:', e);
    }
  }

  private async handleCleanAll() {
    const state = store.getState();
    const activeWorkspace = state.workspaces.find(
      w => w.id === state.activeWorkspaceId
    );
    if (!activeWorkspace) return;

    try {
      const removed = await workspaceService.cleanupAllWorktrees(
        activeWorkspace.folderPath
      );
      console.log(`Cleaned up ${removed} worktrees`);
      await this.refresh(activeWorkspace.folderPath);
    } catch (e) {
      console.error('Failed to clean worktrees:', e);
    }
  }

  mount(parent: HTMLElement) {
    parent.appendChild(this.container);
    this.onStoreChange();
  }
}
