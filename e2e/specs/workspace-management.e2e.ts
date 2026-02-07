import {
  waitForAppReady,
  waitForTerminalPane,
  getElementCount,
  elementExists,
  clickElement,
} from '../helpers/app';

/**
 * Create a new workspace via Tauri IPC and add it to the store.
 * Uses IPC instead of the UI dialog flow because the native folder picker
 * cannot be automated via WebDriver.
 */
async function createWorkspaceViaIpc(
  name: string,
  folderPath?: string
): Promise<string | null> {
  return browser.execute(
    (wsName: string, folder: string) => {
      const invoke = (window as any).__TAURI__?.core?.invoke;
      const store = (window as any).__store;
      if (!invoke || !store) return null;

      // Synchronous store prep — IPC fires in background
      invoke('create_workspace', {
        name: wsName,
        folderPath: folder,
        shellType: { type: 'windows' },
      })
        .then((workspaceId: string) => {
          store.addWorkspace({
            id: workspaceId,
            name: wsName,
            folderPath: folder,
            tabOrder: [],
            shellType: { type: 'windows' },
          });
          store.setActiveWorkspace(workspaceId);
        })
        .catch((e: any) => {
          console.error('[e2e] create_workspace failed:', e);
        });
      return 'ok';
    },
    name,
    folderPath ?? 'C:\\Users'
  );
}

/**
 * Get workspace count in the sidebar.
 */
async function getWorkspaceCount(): Promise<number> {
  return getElementCount('.workspace-item');
}

/**
 * Get the text of the active workspace name.
 */
async function getActiveWorkspaceName(): Promise<string> {
  return browser.execute(() => {
    const active = document.querySelector('.workspace-item.active .workspace-name');
    return active?.textContent?.trim() ?? '';
  });
}

/**
 * Get the data-workspace-id of the active workspace.
 */
async function getActiveWorkspaceId(): Promise<string | null> {
  return browser.execute(() => {
    const active = document.querySelector('.workspace-item.active');
    return active?.getAttribute('data-workspace-id') ?? null;
  });
}

/**
 * Open context menu on a workspace item by index (0-based).
 */
async function openWorkspaceContextMenu(index: number): Promise<void> {
  await browser.execute((idx: number) => {
    const items = document.querySelectorAll('.workspace-item');
    const item = items[idx] as HTMLElement;
    if (item) {
      const rect = item.getBoundingClientRect();
      const event = new MouseEvent('contextmenu', {
        bubbles: true,
        clientX: rect.x + 10,
        clientY: rect.y + 10,
      });
      item.dispatchEvent(event);
    }
  }, index);
  await browser.pause(300);
}

/**
 * Click a context menu item by its text content.
 */
async function clickContextMenuItem(text: string): Promise<void> {
  await browser.execute((txt: string) => {
    const items = document.querySelectorAll('.context-menu-item');
    const target = Array.from(items).find(
      (item) => item.textContent?.trim() === txt
    ) as HTMLElement;
    if (target) target.click();
  }, text);
  await browser.pause(300);
}

describe('Workspace Management', () => {
  before(async () => {
    await waitForAppReady();
    await waitForTerminalPane();
    await browser.pause(5000);
  });

  describe('Default workspace', () => {
    it('should start with one workspace in the sidebar', async () => {
      const count = await getWorkspaceCount();
      expect(count).toBe(1);
    });

    it('should have the default workspace marked as active', async () => {
      const hasActive = await elementExists('.workspace-item.active');
      expect(hasActive).toBe(true);
    });

    it('should display a terminal count badge', async () => {
      const badgeText = await browser.execute(() => {
        const badge = document.querySelector('.workspace-item.active .workspace-badge');
        return badge?.textContent?.trim() ?? '';
      });
      // Default workspace should have at least 1 terminal
      const count = parseInt(badgeText, 10);
      expect(count).toBeGreaterThanOrEqual(1);
    });
  });

  describe('Create workspace', () => {
    it('should display the add workspace button', async () => {
      const exists = await elementExists('.add-workspace-btn');
      expect(exists).toBe(true);
    });

    it('should create a second workspace via IPC', async () => {
      const countBefore = await getWorkspaceCount();

      await createWorkspaceViaIpc('Test Workspace');

      await browser.waitUntil(
        async () => (await getWorkspaceCount()) > countBefore,
        { timeout: 10000, timeoutMsg: 'New workspace did not appear' }
      );

      const countAfter = await getWorkspaceCount();
      expect(countAfter).toBe(countBefore + 1);
    });

    it('should set the new workspace as active', async () => {
      const activeName = await getActiveWorkspaceName();
      expect(activeName).toBe('Test Workspace');
    });
  });

  describe('Switch workspace', () => {
    it('should switch to the first workspace on click', async () => {
      const activeIdBefore = await getActiveWorkspaceId();

      // Click the first workspace (non-active)
      await clickElement('.workspace-item:first-child');
      await browser.pause(500);

      const activeIdAfter = await getActiveWorkspaceId();
      expect(activeIdAfter).not.toBe(activeIdBefore);

      // The first workspace should now be active
      const firstIsActive = await browser.execute(() => {
        const first = document.querySelector('.workspace-item:first-child');
        return first?.classList.contains('active') ?? false;
      });
      expect(firstIsActive).toBe(true);
    });

    it('should update the tab bar to show the workspace terminals', async () => {
      // The first (default) workspace should have at least 1 tab
      const tabCount = await getElementCount('.tab');
      expect(tabCount).toBeGreaterThanOrEqual(1);
    });

    it('should switch back to the second workspace', async () => {
      // Click the last workspace item
      await browser.execute(() => {
        const items = document.querySelectorAll('.workspace-item');
        const last = items[items.length - 1] as HTMLElement;
        if (last) last.click();
      });
      await browser.pause(500);

      const activeName = await getActiveWorkspaceName();
      expect(activeName).toBe('Test Workspace');
    });
  });

  describe('Workspace context menu', () => {
    it('should show context menu on right-click', async () => {
      await openWorkspaceContextMenu(0);

      const menuExists = await elementExists('.context-menu');
      expect(menuExists).toBe(true);

      const menuItems = await browser.execute(() => {
        const items = document.querySelectorAll('.context-menu-item');
        return Array.from(items).map((item) => item.textContent?.trim() ?? '');
      });

      expect(menuItems).toContain('Rename');
      expect(menuItems).toContain('Delete');

      // Dismiss the menu
      await browser.execute(() => document.body.click());
      await browser.pause(300);
    });
  });

  describe('Rename workspace', () => {
    it('should open rename dialog from context menu', async () => {
      // Right-click on the first workspace
      await openWorkspaceContextMenu(0);
      await clickContextMenuItem('Rename');

      // Dialog should appear
      await browser.waitUntil(
        async () => elementExists('.dialog-overlay'),
        { timeout: 5000, timeoutMsg: 'Rename dialog did not open' }
      );

      const dialogTitle = await browser.execute(() => {
        const title = document.querySelector('.dialog-title');
        return title?.textContent?.trim() ?? '';
      });
      expect(dialogTitle).toBe('Rename Workspace');
    });

    it('should rename the workspace via the dialog', async () => {
      const newName = 'RenamedWS-' + Date.now();

      await browser.execute((name: string) => {
        const input = document.querySelector('.dialog-input') as HTMLInputElement;
        if (input) {
          input.value = name;
          input.dispatchEvent(new Event('input', { bubbles: true }));
        }
      }, newName);

      // Click the primary (Rename) button
      await clickElement('.dialog-btn.dialog-btn-primary');

      // Dialog should close
      await browser.waitUntil(
        async () => !(await elementExists('.dialog-overlay')),
        { timeout: 5000, timeoutMsg: 'Rename dialog did not close' }
      );

      // Workspace name should be updated in the sidebar
      const firstName = await browser.execute(() => {
        const first = document.querySelector('.workspace-item:first-child .workspace-name');
        return first?.textContent?.trim() ?? '';
      });
      expect(firstName).toBe(newName);
    });

    it('should cancel rename with the cancel button', async () => {
      const nameBefore = await browser.execute(() => {
        const first = document.querySelector('.workspace-item:first-child .workspace-name');
        return first?.textContent?.trim() ?? '';
      });

      await openWorkspaceContextMenu(0);
      await clickContextMenuItem('Rename');

      await browser.waitUntil(
        async () => elementExists('.dialog-overlay'),
        { timeout: 5000 }
      );

      // Type something
      await browser.execute(() => {
        const input = document.querySelector('.dialog-input') as HTMLInputElement;
        if (input) input.value = 'SHOULD_NOT_PERSIST';
      });

      // Click cancel
      await clickElement('.dialog-btn.dialog-btn-secondary');

      await browser.waitUntil(
        async () => !(await elementExists('.dialog-overlay')),
        { timeout: 5000 }
      );

      const nameAfter = await browser.execute(() => {
        const first = document.querySelector('.workspace-item:first-child .workspace-name');
        return first?.textContent?.trim() ?? '';
      });
      expect(nameAfter).toBe(nameBefore);
    });
  });

  describe('Delete workspace', () => {
    it('should delete a workspace via context menu', async () => {
      const countBefore = await getWorkspaceCount();
      // Must have at least 2 to safely delete one
      expect(countBefore).toBeGreaterThanOrEqual(2);

      // Delete the last workspace
      await openWorkspaceContextMenu(countBefore - 1);
      await clickContextMenuItem('Delete');

      await browser.waitUntil(
        async () => (await getWorkspaceCount()) < countBefore,
        { timeout: 10000, timeoutMsg: 'Workspace was not deleted' }
      );

      const countAfter = await getWorkspaceCount();
      expect(countAfter).toBe(countBefore - 1);
    });

    it('should keep at least one workspace active after deletion', async () => {
      const hasActive = await elementExists('.workspace-item.active');
      expect(hasActive).toBe(true);
    });
  });
});
