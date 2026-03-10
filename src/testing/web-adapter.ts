import type { SemanticQuery, SemanticAction, SemanticWait, QueryResult, ActionResult, WaitResult } from './types';
import type { Store } from '../state/store';

export class WebTestAdapter {
  private getStore(): Store {
    return (window as any).__STORE__;
  }

  async query(q: SemanticQuery): Promise<QueryResult> {
    const now = Date.now();
    try {
      const data = this.resolveQuery(q.target, q.args);
      return { ok: true, target: q.target, data, timestamp_ms: now };
    } catch (e: any) {
      return { ok: false, target: q.target, error: e.message, timestamp_ms: now };
    }
  }

  async act(a: SemanticAction): Promise<ActionResult> {
    const now = Date.now();
    try {
      await this.resolveAction(a.target, a.action, a.args);
      return { ok: true, target: a.target, action: a.action, timestamp_ms: now };
    } catch (e: any) {
      return { ok: false, target: a.target, action: a.action, error: e.message, timestamp_ms: now };
    }
  }

  async wait(w: SemanticWait): Promise<WaitResult> {
    const timeout = w.timeout_ms ?? 10000;
    const poll = w.poll_interval_ms ?? 100;
    const start = Date.now();

    while (Date.now() - start < timeout) {
      const result = this.checkCondition(w.condition, w.args);
      if (result) {
        return { ok: true, condition: w.condition, timed_out: false, elapsed_ms: Date.now() - start };
      }
      await new Promise(resolve => setTimeout(resolve, poll));
    }

    return { ok: false, condition: w.condition, timed_out: true, elapsed_ms: timeout, error: 'Timed out' };
  }

  private resolveQuery(target: string, args?: Record<string, unknown>): unknown {
    const store = this.getStore();
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
        const wsId = (args?.workspace_id as string) || state.activeWorkspaceId;
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
        const wsId = (args?.workspace_id as string) || state.activeWorkspaceId;
        if (!wsId) return null;
        return store.getLayoutTree(wsId);
      }
      case 'terminal.count': {
        const wsId = (args?.workspace_id as string) || state.activeWorkspaceId;
        if (!wsId) return 0;
        return store.getTerminalCount(wsId);
      }
      case 'terminal.list': {
        const wsId = (args?.workspace_id as string) || state.activeWorkspaceId;
        if (!wsId) return [];
        return store.getWorkspaceTerminals(wsId);
      }
      default:
        throw new Error(`Unknown query target: ${target}`);
    }
  }

  private async resolveAction(target: string, action: string, args?: Record<string, unknown>): Promise<void> {
    const store = this.getStore();
    const key = `${target}.${action}`;

    // Import services dynamically to avoid circular deps
    const { terminalService } = await import('../services/terminal-service');
    const { workspaceService } = await import('../services/workspace-service');

    switch (key) {
      case 'workspace.switch': {
        if (!args?.workspace_id) throw new Error('workspace_id required');
        store.setActiveWorkspace(args.workspace_id as string);
        break;
      }
      case 'workspace.create': {
        if (!args?.name || !args?.folder_path) throw new Error('name and folder_path required');
        await workspaceService.createWorkspace(args.name as string, args.folder_path as string);
        break;
      }
      case 'workspace.delete': {
        if (!args?.workspace_id) throw new Error('workspace_id required');
        await workspaceService.deleteWorkspace(args.workspace_id as string);
        break;
      }
      case 'terminal.create': {
        const wsId = (args?.workspace_id as string) || store.getState().activeWorkspaceId;
        if (!wsId) throw new Error('No active workspace');
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
        await terminalService.closeTerminal(args.terminal_id as string);
        break;
      }
      case 'terminal.write': {
        if (!args?.terminal_id || args?.data == null) throw new Error('terminal_id and data required');
        await terminalService.writeToTerminal(args.terminal_id as string, args.data as string);
        break;
      }
      default:
        throw new Error(`Unknown action: ${key}`);
    }
  }

  private checkCondition(condition: string, args?: Record<string, unknown>): boolean {
    const store = this.getStore();
    const state = store.getState();

    switch (condition) {
      case 'app.ready':
        return state.workspaces.length > 0;
      case 'terminal.created': {
        if (args?.terminal_id) {
          return state.terminals.some(t => t.id === args.terminal_id);
        }
        return state.activeTerminalId != null;
      }
      case 'workspace.switched':
        return state.activeWorkspaceId === (args?.workspace_id as string);
      case 'terminal.count': {
        const wsId = (args?.workspace_id as string) || state.activeWorkspaceId;
        if (!wsId) return false;
        const expected = args?.count as number | undefined;
        if (expected == null) return false;
        return store.getTerminalCount(wsId) === expected;
      }
      default:
        return false;
    }
  }
}
