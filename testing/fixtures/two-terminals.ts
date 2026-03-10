import type { Fixture, McpClient } from './types';

export const twoTerminals: Fixture = {
  name: 'two-terminals',
  description: 'Clean profile with two terminals in the default workspace',

  async create(client: McpClient): Promise<void> {
    await client.call('reset_staging_profile');
    // Wait for initial terminal
    await client.call('wait_for_app_ready', { timeout_ms: 10000 });

    // Get active workspace
    const workspace = await client.call('get_active_workspace') as any;
    const workspaceId = workspace?.workspace?.id;
    if (!workspaceId) throw new Error('No active workspace after reset');

    // Create second terminal
    await client.call('create_terminal', { workspace_id: workspaceId });
  },

  async verifyReady(client: McpClient): Promise<boolean> {
    const terminals = await client.call('list_terminals') as any;
    return Array.isArray(terminals?.terminals) && terminals.terminals.length >= 2;
  },

  async tearDown(client: McpClient): Promise<void> {
    await client.call('reset_staging_profile');
  },

  async reset(client: McpClient): Promise<void> {
    await this.tearDown(client);
    await this.create(client);
  },
};
