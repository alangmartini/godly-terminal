import { defineFixture, resetProfile, getActiveWorkspaceId, listTerminalIds } from './types';
import { twoTerminals } from './two-terminals';

export const splitBasic = defineFixture({
  name: 'split-basic',
  description: 'Two terminals with a horizontal split already created',

  async create(client) {
    await twoTerminals.create(client);
    await twoTerminals.verifyReady(client);

    const workspaceId = await getActiveWorkspaceId(client);
    const terminalIds = await listTerminalIds(client);
    if (terminalIds.length < 2) throw new Error('Need at least 2 terminals for split fixture');

    await client.call('split_terminal', {
      workspace_id: workspaceId,
      target_terminal_id: terminalIds[0],
      new_terminal_id: terminalIds[1],
      direction: 'horizontal',
      ratio: 0.5,
    });
  },

  async verifyReady(client) {
    const workspaceId = await getActiveWorkspaceId(client);
    const layout = await client.call('get_layout_tree', { workspace_id: workspaceId }) as
      { tree?: { type?: string; Split?: unknown } } | null;
    return layout?.tree?.type === 'split' || layout?.tree?.Split !== undefined;
  },

  async tearDown(client) {
    await resetProfile(client);
  },
});
