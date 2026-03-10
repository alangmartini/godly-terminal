import type { SemanticQuery, SemanticAction, SemanticWait, QueryResult, ActionResult, WaitResult } from './types';
import type { Store } from '../state/store';

function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function getStore(): Store {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return (window as any).__STORE__ as Store;
}

function resolveWorkspaceId(args: Record<string, unknown> | undefined): string | null {
  return (args?.workspace_id as string) || getStore().getState().activeWorkspaceId;
}

export class WebTestAdapter {
  async query(q: SemanticQuery): Promise<QueryResult> {
    const now = Date.now();
    try {
      const data = this.resolveQuery(q.target, q.args);
      return { ok: true, target: q.target, data, timestamp_ms: now };
    } catch (e: unknown) {
      return { ok: false, target: q.target, error: errorMessage(e), timestamp_ms: now };
    }
  }

  async act(a: SemanticAction): Promise<ActionResult> {
    const now = Date.now();
    try {
      await this.resolveAction(a.target, a.action, a.args);
      return { ok: true, target: a.target, action: a.action, timestamp_ms: now };
    } catch (e: unknown) {
      return { ok: false, target: a.target, action: a.action, error: errorMessage(e), timestamp_ms: now };
    }
  }

  async wait(w: SemanticWait): Promise<WaitResult> {
    const timeout = w.timeout_ms ?? 10000;
    const poll = w.poll_interval_ms ?? 100;
    const start = Date.now();

    while (Date.now() - start < timeout) {
      if (this.checkCondition(w.condition, w.args)) {
        return { ok: true, condition: w.condition, timed_out: false, elapsed_ms: Date.now() - start };
      }
      await new Promise(resolve => setTimeout(resolve, poll));
    }

    return { ok: false, condition: w.condition, timed_out: true, elapsed_ms: timeout, error: 'Timed out' };
  }

  /** Register on window for backend access via execute_js. */
  initBridge(): void {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).__TEST_HARNESS__ = {
      query: (q: SemanticQuery) => this.query(q),
      act: (a: SemanticAction) => this.act(a),
      wait: (w: SemanticWait) => this.wait(w),
      isReady: () => true,
    };
    console.log('[test-harness] Web adapter bridge initialized');
  }

  destroyBridge(): void {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    delete (window as any).__TEST_HARNESS__;
  }

  private resolveQuery(target: string, args?: Record<string, unknown>): unknown {
    const store = getStore();
    const state = store.getState();

    switch (target) {
      case 'workspace.active': {
        const wsId = state.activeWorkspaceId;
        return wsId ? state.workspaces.find(w => w.id === wsId) ?? null : null;
      }
      case 'workspace.list':
        return state.workspaces;
      case 'tab.active':
        return state.activeTerminalId;
      case 'tab.list': {
        const wsId = resolveWorkspaceId(args);
        if (!wsId) return [];
        const ws = state.workspaces.find(w => w.id === wsId);
        return ws?.tabOrder ?? [];
      }
      case 'pane.active': {
        const wsId = state.activeWorkspaceId;
        if (!wsId) return null;
        return store.getFocusedPaneId(wsId);
      }
      case 'layout.tree': {
        const wsId = resolveWorkspaceId(args);
        if (!wsId) return null;
        return store.getLayoutTree(wsId);
      }
      case 'terminal.count': {
        const wsId = resolveWorkspaceId(args);
        if (!wsId) return 0;
        return store.getTerminalCount(wsId);
      }
      case 'terminal.list': {
        const wsId = resolveWorkspaceId(args);
        if (!wsId) return [];
        return store.getWorkspaceTerminals(wsId);
      }
      default:
        throw new Error(`Unknown query target: ${target}`);
    }
  }

  private async resolveAction(target: string, action: string, args?: Record<string, unknown>): Promise<void> {
    const store = getStore();
    const key = `${target}.${action}`;

    switch (key) {
      case 'workspace.switch': {
        if (!args?.workspace_id) throw new Error('workspace_id required');
        store.setActiveWorkspace(args.workspace_id as string);
        break;
      }
      case 'workspace.create': {
        if (!args?.name || !args?.folder_path) throw new Error('name and folder_path required');
        const { workspaceService } = await import('../services/workspace-service');
        await workspaceService.createWorkspace(args.name as string, args.folder_path as string);
        break;
      }
      case 'workspace.delete': {
        if (!args?.workspace_id) throw new Error('workspace_id required');
        const { workspaceService } = await import('../services/workspace-service');
        await workspaceService.deleteWorkspace(args.workspace_id as string);
        break;
      }
      case 'terminal.create': {
        const wsId = resolveWorkspaceId(args);
        if (!wsId) throw new Error('No active workspace');
        const { terminalService } = await import('../services/terminal-service');
        await terminalService.createTerminal(wsId, {
          cwdOverride: args?.cwd as string | undefined,
          nameOverride: args?.name as string | undefined,
        });
        break;
      }
      case 'terminal.focus': {
        if (!args?.terminal_id) throw new Error('terminal_id required');
        store.setActiveTerminal(args.terminal_id as string);
        break;
      }
      case 'terminal.close': {
        if (!args?.terminal_id) throw new Error('terminal_id required');
        const { terminalService } = await import('../services/terminal-service');
        await terminalService.closeTerminal(args.terminal_id as string);
        break;
      }
      case 'terminal.write': {
        if (!args?.terminal_id || args?.data == null) throw new Error('terminal_id and data required');
        const { terminalService } = await import('../services/terminal-service');
        await terminalService.writeToTerminal(args.terminal_id as string, args.data as string);
        break;
      }
      default:
        throw new Error(`Unknown action: ${key}`);
    }
  }

  private checkCondition(condition: string, args?: Record<string, unknown>): boolean {
    const store = getStore();
    const state = store.getState();

    switch (condition) {
      case 'app.ready':
        return state.workspaces.length > 0;
      case 'terminal.created':
        return args?.terminal_id
          ? state.terminals.some(t => t.id === args.terminal_id)
          : state.activeTerminalId != null;
      case 'workspace.switched':
        return state.activeWorkspaceId === (args?.workspace_id as string);
      case 'terminal.count': {
        const wsId = resolveWorkspaceId(args);
        const expected = args?.count as number | undefined;
        if (!wsId || expected == null) return false;
        return store.getTerminalCount(wsId) === expected;
      }
      default:
        return false;
    }
  }
}
