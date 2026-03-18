// lib/cleanup.mjs
// Auto-teardown of resources created during test execution.

export class Cleanup {
  constructor(mcpClient) {
    this.mcp = mcpClient;
    this._terminals = [];
    this._workspaces = [];
    this.enabled = true;
  }

  trackTerminal(terminalId) {
    this._terminals.push(terminalId);
  }

  trackWorkspace(workspaceId) {
    this._workspaces.push(workspaceId);
  }

  /**
   * Teardown all tracked resources in reverse order.
   * Terminals first, then workspaces.
   */
  async teardown() {
    if (!this.enabled) return;

    const errors = [];

    // Close terminals in reverse order
    for (const id of [...this._terminals].reverse()) {
      try {
        await this.mcp.callTool('close_terminal', { terminal_id: id }, { timeout: 5000 });
      } catch (err) {
        errors.push(`close_terminal(${id}): ${err.message}`);
      }
    }

    // Small delay between terminal close and workspace delete
    if (this._terminals.length > 0 && this._workspaces.length > 0) {
      await new Promise(r => setTimeout(r, 500));
    }

    // Delete workspaces in reverse order
    for (const id of [...this._workspaces].reverse()) {
      try {
        await this.mcp.callTool('delete_workspace', { workspace_id: id }, { timeout: 5000 });
      } catch (err) {
        errors.push(`delete_workspace(${id}): ${err.message}`);
      }
    }

    this._terminals = [];
    this._workspaces = [];

    return errors;
  }

  /** Reset tracking without actually tearing down (for manual cleanup tests) */
  reset() {
    this._terminals = [];
    this._workspaces = [];
  }

  get trackedCount() {
    return this._terminals.length + this._workspaces.length;
  }
}
