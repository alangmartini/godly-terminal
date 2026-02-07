import {
  waitForAppReady,
  waitForTerminalPane,
  getElementCount,
  createNewTerminalTab,
} from '../helpers/app';

/**
 * Get the ordered list of terminal IDs from the tab bar.
 */
async function getTabOrder(): Promise<string[]> {
  return browser.execute(() => {
    const tabs = document.querySelectorAll('.tab');
    return Array.from(tabs).map(
      (tab) => tab.getAttribute('data-terminal-id') ?? ''
    );
  });
}

/**
 * Get the terminal IDs for a specific workspace from the store.
 */
async function getStoreTabOrder(workspaceId: string): Promise<string[]> {
  return browser.execute((wsId: string) => {
    const store = (window as any).__store;
    if (!store) return [];
    const terminals = store.getWorkspaceTerminals(wsId);
    return terminals.map((t: any) => t.id);
  }, workspaceId);
}

/**
 * Reorder tabs via the workspaceService IPC call (bypasses drag-drop UI
 * which is unreliable in WebDriver).
 */
async function reorderTabsViaIpc(
  workspaceId: string,
  newOrder: string[]
): Promise<void> {
  await browser.execute(
    (wsId: string, order: string[]) => {
      const invoke = (window as any).__TAURI__?.core?.invoke;
      const store = (window as any).__store;
      if (!invoke || !store) return;

      invoke('reorder_tabs', { workspaceId: wsId, tabOrder: order })
        .then(() => {
          store.reorderTerminals(wsId, order);
        })
        .catch((e: any) => {
          console.error('[e2e] reorder_tabs failed:', e);
        });
    },
    workspaceId,
    newOrder
  );
  // Wait for DOM re-render
  await browser.pause(1000);
}

/**
 * Move a tab to a different workspace via IPC.
 */
async function moveTabToWorkspaceViaIpc(
  terminalId: string,
  targetWorkspaceId: string
): Promise<void> {
  await browser.execute(
    (tId: string, wsId: string) => {
      const invoke = (window as any).__TAURI__?.core?.invoke;
      const store = (window as any).__store;
      if (!invoke || !store) return;

      invoke('move_tab_to_workspace', {
        terminalId: tId,
        targetWorkspaceId: wsId,
      })
        .then(() => {
          store.moveTerminalToWorkspace(tId, wsId);
        })
        .catch((e: any) => {
          console.error('[e2e] move_tab_to_workspace failed:', e);
        });
    },
    terminalId,
    targetWorkspaceId
  );
  await browser.pause(1000);
}

/**
 * Create a workspace via IPC for test setup.
 */
async function createWorkspaceForTest(name: string): Promise<string | null> {
  return browser.execute((wsName: string) => {
    const invoke = (window as any).__TAURI__?.core?.invoke;
    const store = (window as any).__store;
    if (!invoke || !store) return null;

    // We need to wait for the ID — use a synchronous return via promise + global
    invoke('create_workspace', {
      name: wsName,
      folderPath: 'C:\\Users',
      shellType: { type: 'windows' },
    })
      .then((workspaceId: string) => {
        store.addWorkspace({
          id: workspaceId,
          name: wsName,
          folderPath: 'C:\\Users',
          tabOrder: [],
          shellType: { type: 'windows' },
        });
        (window as any).__lastCreatedWorkspaceId = workspaceId;
      })
      .catch((e: any) => {
        console.error('[e2e] create_workspace failed:', e);
      });
    return 'ok';
  }, name);
}

describe('Tab Reorder & Move', () => {
  before(async () => {
    await waitForAppReady();
    await waitForTerminalPane();
    await browser.pause(5000);
  });

  describe('Tab reorder within workspace', () => {
    before(async () => {
      // Ensure we have at least 3 tabs for meaningful reorder tests
      while ((await getElementCount('.tab')) < 3) {
        await createNewTerminalTab();
        await browser.pause(1000);
      }
    });

    it('should have 3 tabs in the tab bar', async () => {
      const count = await getElementCount('.tab');
      expect(count).toBeGreaterThanOrEqual(3);
    });

    it('should reverse tab order via IPC', async () => {
      const orderBefore = await getTabOrder();
      expect(orderBefore.length).toBeGreaterThanOrEqual(3);

      const workspaceId = await browser.execute(() => {
        return (window as any).__store?.getState()?.activeWorkspaceId ?? '';
      });

      const reversed = [...orderBefore].reverse();
      await reorderTabsViaIpc(workspaceId, reversed);

      const orderAfter = await getTabOrder();
      expect(orderAfter).toEqual(reversed);
    });

    it('should persist reorder in the store', async () => {
      const workspaceId = await browser.execute(() => {
        return (window as any).__store?.getState()?.activeWorkspaceId ?? '';
      });

      const domOrder = await getTabOrder();
      const storeOrder = await getStoreTabOrder(workspaceId);

      expect(storeOrder).toEqual(domOrder);
    });

    it('should swap first and last tab', async () => {
      const orderBefore = await getTabOrder();
      const swapped = [...orderBefore];
      const first = swapped[0];
      swapped[0] = swapped[swapped.length - 1];
      swapped[swapped.length - 1] = first;

      const workspaceId = await browser.execute(() => {
        return (window as any).__store?.getState()?.activeWorkspaceId ?? '';
      });

      await reorderTabsViaIpc(workspaceId, swapped);

      const orderAfter = await getTabOrder();
      expect(orderAfter[0]).toBe(swapped[0]);
      expect(orderAfter[orderAfter.length - 1]).toBe(swapped[swapped.length - 1]);
    });
  });

  describe('Move tab to another workspace', () => {
    let secondWorkspaceId: string;

    before(async () => {
      // Create a second workspace
      await createWorkspaceForTest('MoveTarget');
      await browser.pause(2000);

      secondWorkspaceId = await browser.execute(() => {
        return (window as any).__lastCreatedWorkspaceId ?? '';
      });

      // Switch back to the first workspace
      await browser.execute(() => {
        const store = (window as any).__store;
        const workspaces = store.getState().workspaces;
        if (workspaces.length > 0) {
          store.setActiveWorkspace(workspaces[0].id);
        }
      });
      await browser.pause(500);
    });

    it('should have tabs in the source workspace', async () => {
      const tabCount = await getElementCount('.tab');
      expect(tabCount).toBeGreaterThanOrEqual(2);
    });

    it('should move a tab to the target workspace', async () => {
      const tabsBefore = await getTabOrder();
      const tabToMove = tabsBefore[tabsBefore.length - 1];

      await moveTabToWorkspaceViaIpc(tabToMove, secondWorkspaceId);

      // The tab should no longer be in the current workspace's tab bar
      const tabsAfter = await getTabOrder();
      expect(tabsAfter).not.toContain(tabToMove);
      expect(tabsAfter.length).toBe(tabsBefore.length - 1);
    });

    it('should show the moved tab in the target workspace', async () => {
      // Switch to the target workspace
      await browser.execute((wsId: string) => {
        (window as any).__store?.setActiveWorkspace(wsId);
      }, secondWorkspaceId);
      await browser.pause(1000);

      const tabCount = await getElementCount('.tab');
      // The target workspace should now have the moved tab
      expect(tabCount).toBeGreaterThanOrEqual(1);
    });

    it('should update the store correctly after move', async () => {
      const storeState = await browser.execute((wsId: string) => {
        const store = (window as any).__store;
        if (!store) return { source: 0, target: 0 };
        const state = store.getState();
        const sourceTerminals = state.terminals.filter(
          (t: any) => t.workspaceId === state.workspaces[0]?.id
        );
        const targetTerminals = state.terminals.filter(
          (t: any) => t.workspaceId === wsId
        );
        return {
          source: sourceTerminals.length,
          target: targetTerminals.length,
        };
      }, secondWorkspaceId);

      expect(storeState.target).toBeGreaterThanOrEqual(1);
    });
  });
});
