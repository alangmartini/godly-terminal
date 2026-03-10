import type { Fixture, McpClient } from './types';
import { twoTerminals } from './two-terminals';

export const splitBasic: Fixture = {
  name: 'split-basic',
  description: 'Two terminals with a horizontal split already created',

  async create(client: McpClient): Promise<void> {
    // First set up two terminals
    await twoTerminals.create(client);
    await twoTerminals.verifyReady(client);

    // Get workspace and terminal IDs
    const workspace = await client.call('get_active_workspace') as any;
    const workspaceId = workspace?.workspace?.id;
    const terminals = await client.call('list_terminals') as any;
    const terminalIds = terminals?.terminals?.map((t: any) => t.id) ?? [];

    if (terminalIds.length < 2) throw new Error('Need at least 2 terminals for split fixture');

    // Create the split
    await client.call('split_terminal', {
      workspace_id: workspaceId,
      target_terminal_id: terminalIds[0],
      new_terminal_id: terminalIds[1],
      direction: 'horizontal',
      ratio: 0.5,
    });
  },

  async verifyReady(client: McpClient): Promise<boolean> {
    const workspace = await client.call('get_active_workspace') as any;
    const workspaceId = workspace?.workspace?.id;
    if (!workspaceId) return false;

    const layout = await client.call('get_layout_tree', { workspace_id: workspaceId }) as any;
    return layout?.tree?.type === 'split' || layout?.tree?.Split !== undefined;
  },

  async tearDown(client: McpClient): Promise<void> {
    await client.call('reset_staging_profile');
  },

  async reset(client: McpClient): Promise<void> {
    await this.tearDown(client);
    await this.create(client);
  },
};
