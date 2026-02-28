import { settingsTabStore } from '../state/settings-tab-store';
import { getSettingsTabRegistry } from './settings/index';
import type { SettingsDialogContext } from './settings/types';

/**
 * Show the settings dialog for customising themes, notifications, and
 * keyboard shortcuts. Returns a promise that resolves when the dialog
 * is closed.
 */
export function showSettingsDialog(): Promise<void> {
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const dialog = document.createElement('div');
    dialog.className = 'dialog settings-dialog';

    // ── Header ──────────────────────────────────────────────────
    const header = document.createElement('div');
    header.className = 'settings-header';

    const title = document.createElement('div');
    title.className = 'dialog-title';
    title.textContent = 'Settings';
    header.appendChild(title);

    dialog.appendChild(header);

    // ── Tab registry ────────────────────────────────────────────
    const registry = getSettingsTabRegistry();
    const allTabs: Record<string, string> = {};
    for (const provider of registry) {
      allTabs[provider.id] = provider.label;
    }

    let tabOrder = settingsTabStore.getTabOrder();
    let activeTab = tabOrder[0];

    const tabBar = document.createElement('div');
    tabBar.className = 'settings-tabs';

    const tabButtons: Record<string, HTMLButtonElement> = {};
    const tabContents: Record<string, HTMLDivElement> = {};

    const dialogContext: SettingsDialogContext = {
      renderTabBar: () => buildTabBar(),
    };

    // Build tab contents from registry
    for (const provider of registry) {
      const content = provider.buildContent(dialogContext);
      tabContents[provider.id] = content;
    }

    function buildTabBar() {
      tabBar.textContent = '';
      for (const id of tabOrder) {
        if (!allTabs[id]) continue;
        let btn = tabButtons[id];
        if (!btn) {
          btn = document.createElement('button');
          btn.dataset.tabId = id;
          tabButtons[id] = btn;
        }
        btn.className = 'settings-tab' + (id === activeTab ? ' active' : '');
        btn.textContent = allTabs[id];
        btn.onclick = () => switchTab(id);
        tabBar.appendChild(btn);
      }
    }

    buildTabBar();
    dialog.appendChild(tabBar);

    // ── Drag-and-drop reorder ──────────────────────────────────
    {
      let dragTabId: string | null = null;
      let startX = 0;
      let dragging = false;
      const THRESHOLD = 5;

      tabBar.addEventListener('pointerdown', (e: PointerEvent) => {
        const target = (e.target as HTMLElement).closest('.settings-tab') as HTMLButtonElement | null;
        if (!target || !target.dataset.tabId) return;
        dragTabId = target.dataset.tabId;
        startX = e.clientX;
        dragging = false;

        const onMove = (ev: PointerEvent) => {
          if (!dragging && Math.abs(ev.clientX - startX) >= THRESHOLD) {
            dragging = true;
            tabButtons[dragTabId!]?.classList.add('dragging');
          }
          if (!dragging) return;

          for (const id of tabOrder) {
            const btn = tabButtons[id];
            if (!btn || id === dragTabId) continue;
            const rect = btn.getBoundingClientRect();
            if (ev.clientX >= rect.left && ev.clientX <= rect.right) {
              const fromIdx = tabOrder.indexOf(dragTabId!);
              const toIdx = tabOrder.indexOf(id);
              if (fromIdx !== -1 && toIdx !== -1 && fromIdx !== toIdx) {
                tabOrder.splice(fromIdx, 1);
                tabOrder.splice(toIdx, 0, dragTabId!);
                buildTabBar();
              }
              break;
            }
          }
        };

        const onUp = () => {
          document.removeEventListener('pointermove', onMove);
          document.removeEventListener('pointerup', onUp);
          if (dragging && dragTabId) {
            tabButtons[dragTabId]?.classList.remove('dragging');
            settingsTabStore.setTabOrder(tabOrder);
          }
          dragTabId = null;
          dragging = false;
        };

        document.addEventListener('pointermove', onMove);
        document.addEventListener('pointerup', onUp);
      });
    }

    function switchTab(tabId: string) {
      activeTab = tabId;
      for (const id of Object.keys(tabButtons)) {
        tabButtons[id].className = 'settings-tab' + (id === tabId ? ' active' : '');
        tabContents[id].className = 'settings-tab-content' + (id === tabId ? ' active' : '');
      }
    }

    // Append tab contents to dialog (first tab is active)
    for (const provider of registry) {
      const content = tabContents[provider.id];
      if (provider.id === activeTab) {
        content.classList.add('active');
      }
      dialog.appendChild(content);
    }

    // ── Info footer ──────────────────────────────────────────────
    const footer = document.createElement('div');
    footer.className = 'settings-footer';
    footer.textContent = 'Renderer: GPU';
    dialog.appendChild(footer);

    const versionLine = document.createElement('div');
    versionLine.className = 'settings-footer settings-version';
    versionLine.textContent = `Version: ${__APP_VERSION__}`;
    dialog.appendChild(versionLine);

    // ── Close handling ──────────────────────────────────────────
    const close = () => {
      for (const provider of registry) {
        provider.onDialogClose?.();
      }
      overlay.remove();
      resolve();
    };

    overlay.onclick = (e) => {
      if (e.target === overlay) close();
    };

    const escHandler = (e: KeyboardEvent) => {
      // Don't close if any tab is in a capturing/modal state
      const anyCapturing = registry.some(p => p.isCapturing?.());
      if (e.key === 'Escape' && !anyCapturing) {
        close();
        document.removeEventListener('keydown', escHandler);
      }
    };
    document.addEventListener('keydown', escHandler);

    overlay.appendChild(dialog);
    document.body.appendChild(overlay);
  });
}
