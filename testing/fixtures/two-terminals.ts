import { defineFixture, resetProfile, waitForReady, getActiveWorkspaceId, listTerminalIds } from './types';

export const twoTerminals = defineFixture({
  name: 'two-terminals',
  description: 'Clean profile with two terminals in the default workspace',

  async create(client) {
    await resetProfile(client);
    await waitForReady(client);
    const workspaceId = await getActiveWorkspaceId(client);
    await client.callTool('create_terminal', { workspace_id: workspaceId });
  },

  async verifyReady(client) {
    const ids = await listTerminalIds(client);
    return ids.length >= 2;
  },

  async tearDown(client) {
    await resetProfile(client);
  },
});
