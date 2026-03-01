import { store } from '../state/store';
import { terminalSettingsStore } from '../state/terminal-settings-store';
import { terminalService } from '../services/terminal-service';
import { workspaceService } from '../services/workspace-service';
import {
  BackendShellType,
  convertShellType,
  shellTypeToProcessName,
} from '../utils/shell-type-utils';
import { fromLegacySplitView } from '../state/split-types';

export interface ReconnectionDeps {
  /** Marks a terminal ID for scrollback restoration. */
  markRestoredTerminal(id: string): void;
  /** Marks a terminal ID as reattached (no scrollback needed). */
  markReattachedTerminal(id: string): void;
}

/**
 * Restore layout from persisted state. Reconnects live daemon sessions,
 * creates fresh terminals for dead ones, and cleans up orphans.
 */
export async function restoreLayout(deps: ReconnectionDeps): Promise<void> {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    console.log('[App] Loading layout...');
    const layout = await invoke<{
      workspaces: Array<{
        id: string;
        name: string;
        folder_path: string;
        tab_order: string[];
        shell_type?: BackendShellType;
        worktree_mode?: boolean;
        claude_code_mode?: boolean;
      }>;
      terminals: Array<{
        id: string;
        workspace_id: string;
        name: string;
        shell_type?: BackendShellType;
        cwd?: string | null;
        worktree_path?: string | null;
        worktree_branch?: string | null;
      }>;
      active_workspace_id: string | null;
      split_views?: Record<string, {
        left_terminal_id: string;
        right_terminal_id: string;
        direction: string;
        ratio: number;
      }>;
    }>('load_layout');

    console.log('[App] Layout loaded:', JSON.stringify(layout, null, 2));
    console.log('[App] Workspaces count:', layout.workspaces.length);
    console.log('[App] Terminals count:', layout.terminals.length);

    if (layout.workspaces.length > 0) {
      console.log('[App] Restoring workspaces...');
      // Restore workspaces
      layout.workspaces.forEach((w) => {
        console.log('[App] Adding workspace:', w.id, w.name);
        store.addWorkspace({
          id: w.id,
          name: w.name,
          folderPath: w.folder_path,
          tabOrder: w.tab_order,
          shellType: convertShellType(w.shell_type),
          worktreeMode: w.worktree_mode ?? false,
          claudeCodeMode: w.claude_code_mode ?? false,
        });
      });

      // Set active workspace
      const activeWsId = layout.active_workspace_id || layout.workspaces[0].id;
      console.log('[App] Setting active workspace:', activeWsId);
      store.setActiveWorkspace(activeWsId);

      // Check daemon for live sessions that can be reattached
      const liveSessions = await terminalService.reconnectSessions();
      // Filter out dead sessions (PTY exited but session still in daemon HashMap)
      const liveSessionIds = new Set(
        liveSessions.filter((s) => s.running).map((s) => s.id)
      );
      console.log('[App] Live daemon sessions:', liveSessionIds.size);

      // Restore terminals: reattach if alive, or create fresh with scrollback
      console.log('[App] Restoring terminals...');
      for (const t of layout.terminals) {
        const shellType = convertShellType(t.shell_type);
        const processName = shellTypeToProcessName(shellType);

        const tabName = t.worktree_branch || t.name;

        if (liveSessionIds.has(t.id)) {
          // Session is still alive in daemon - reattach
          console.log('[App] Reattaching to live session:', t.id);
          try {
            await terminalService.attachSession(t.id, t.workspace_id, tabName);
            deps.markReattachedTerminal(t.id);

            store.addTerminal({
              id: t.id,
              workspaceId: t.workspace_id,
              name: tabName,
              processName,
              order: 0,
            });
            continue;
          } catch (error) {
            console.warn('[App] Failed to reattach session:', t.id, error);
            // Fall through to create fresh terminal
          }
        }

        // Session is dead or reattach failed - create fresh terminal with saved CWD
        console.log('[App] Creating fresh terminal:', t.id, 'in workspace:', t.workspace_id);
        const result = await terminalService.createTerminal(t.workspace_id, {
          cwdOverride: t.cwd ?? undefined,
          shellTypeOverride: shellType,
          idOverride: t.id,
          nameOverride: tabName,
        });

        console.log('[App] Terminal created with ID:', result.id, '(requested:', t.id, ')');

        // Mark for scrollback restoration (only for fresh terminals, not reattached ones)
        deps.markRestoredTerminal(result.id);

        store.addTerminal({
          id: result.id,
          workspaceId: t.workspace_id,
          name: tabName,
          processName,
          order: 0,
        });
      }

      // Prune stale terminal IDs from backend layout trees, tab orders,
      // split views, and zoomed panes. This handles the case where persisted
      // data references terminals that failed to restore (crash, dead sessions).
      const liveTerminalIds = store.getState().terminals.map((t) => t.id);
      await invoke('prune_stale_terminal_ids', {
        liveTerminalIds,
      });
      console.log('[App] Pruned stale terminal IDs from backend state');

      // Clean up orphaned daemon sessions not in the saved layout.
      // These accumulate when the app crashes before autosave.
      const layoutTerminalIds = new Set(layout.terminals.map((t) => t.id));
      const orphanSessions = liveSessions.filter((s) => !layoutTerminalIds.has(s.id));
      if (orphanSessions.length > 0) {
        console.log(
          '[App] Closing',
          orphanSessions.length,
          'orphaned daemon sessions:',
          orphanSessions.map((s) => s.id),
        );
        for (const orphan of orphanSessions) {
          try {
            await terminalService.closeTerminal(orphan.id);
          } catch {
            // Session may already be gone — ignore
          }
        }
      }

      // Restore split views (create layout trees from persisted flat splits)
      if (layout.split_views) {
        const knownTerminalIds = new Set(liveTerminalIds);
        for (const [wsId, sv] of Object.entries(layout.split_views)) {
          if (knownTerminalIds.has(sv.left_terminal_id) && knownTerminalIds.has(sv.right_terminal_id)) {
            const dir = sv.direction === 'vertical' ? 'vertical' : 'horizontal';
            // Create layout tree from legacy split
            const tree = fromLegacySplitView({
              leftTerminalId: sv.left_terminal_id,
              rightTerminalId: sv.right_terminal_id,
              direction: dir,
              ratio: sv.ratio,
            });
            store.setLayoutTree(wsId, tree);
            // Also set legacy split for backend persistence
            store.setSplitView(wsId, sv.left_terminal_id, sv.right_terminal_id, dir, sv.ratio);
            await syncSplitToBackend(wsId, {
              leftTerminalId: sv.left_terminal_id,
              rightTerminalId: sv.right_terminal_id,
              direction: dir,
              ratio: sv.ratio,
            });
          }
        }
        console.log('[App] Split views restored:', Object.keys(layout.split_views).length);
      }

      console.log('[App] Restore complete!');
    } else {
      console.log('[App] No workspaces in layout, creating default...');
      // No layout — close all stale daemon sessions from previous runs
      await closeAllDaemonSessions();
      await createDefaultWorkspace();
    }
  } catch (error) {
    console.error('[App] Error loading layout:', error);
    (window as any).__app_init_error = String(error);

    // Layout failed — close all daemon sessions since none are in use
    await closeAllDaemonSessions();

    try {
      await createDefaultWorkspace();
    } catch (e2) {
      console.error('[App] Error creating default workspace:', e2);
      (window as any).__app_init_error2 = String(e2);
    }
  }
}

/** Close all daemon sessions. Used when no layout is loaded. */
export async function closeAllDaemonSessions(): Promise<void> {
  try {
    const sessions = await terminalService.reconnectSessions();
    if (sessions.length > 0) {
      console.log('[App] Closing', sessions.length, 'stale daemon sessions');
      for (const s of sessions) {
        try {
          await terminalService.closeTerminal(s.id);
        } catch {
          // Ignore — session may already be gone
        }
      }
    }
  } catch {
    // Daemon may not be reachable — nothing to clean up
  }
}

/** Create a default workspace with a single terminal. */
export async function createDefaultWorkspace(): Promise<void> {
  // Get user home directory via Tauri
  const { homeDir } = await import('@tauri-apps/api/path');
  const homePath = await homeDir().catch(() => 'C:\\');
  console.log('[App] Home path:', homePath);

  const workspaceId = await workspaceService.createWorkspace(
    'Default',
    homePath
  );
  console.log('[App] Workspace created:', workspaceId);
  store.setActiveWorkspace(workspaceId);

  // Create initial terminal
  console.log('[App] Creating terminal in workspace:', workspaceId);
  const result = await terminalService.createTerminal(workspaceId);
  console.log('[App] Terminal created:', result.id);
  store.addTerminal({
    id: result.id,
    workspaceId,
    name: result.worktree_branch ?? 'Terminal',
    processName: shellTypeToProcessName(terminalSettingsStore.getDefaultShell()),
    order: 0,
  });
  console.log('[App] Terminal added to store');
}

/** Sync a split view to the backend for persistence. */
export async function syncSplitToBackend(
  workspaceId: string,
  split: { leftTerminalId: string; rightTerminalId: string; direction: string; ratio: number },
): Promise<void> {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('set_split_view', {
      workspaceId,
      leftTerminalId: split.leftTerminalId,
      rightTerminalId: split.rightTerminalId,
      direction: split.direction,
      ratio: split.ratio,
    });
  } catch (error) {
    console.error('[App] Failed to sync split view to backend:', error);
  }
}
