import {
  defineFixture,
  resetProfile,
  waitForReady,
  getActiveWorkspaceId,
  listTerminalIds,
  extractToolData,
} from './types';

export const multiWorkspace = defineFixture({
  name: 'multi-workspace',
  description: 'Two workspaces: default (2 terminals) + "Persist WS" (1 terminal)',

  async create(client) {
    await resetProfile(client);
    await waitForReady(client);

    // Default workspace already has 1 terminal — add a second
    const defaultWsId = await getActiveWorkspaceId(client);
    await client.callTool('create_terminal', { workspace_id: defaultWsId });

    // Create a second workspace (automatically gets 1 terminal)
    await client.callTool('create_workspace', { name: 'Persist WS' });

    // Switch back to the default workspace so it's active at test start
    const workspaces = extractToolData(
      await client.callTool('list_workspaces'),
    ) as { workspaces?: { id: string; name: string }[] } | null;
    const defaultWs = workspaces?.workspaces?.find((w) => w.id === defaultWsId);
    if (defaultWs) {
      await client.callTool('switch_workspace', { workspace_id: defaultWsId });
    }
  },

  async verifyReady(client) {
    const workspaces = extractToolData(
      await client.callTool('list_workspaces'),
    ) as { workspaces?: { id: string }[] } | null;
    const terms = await listTerminalIds(client);
    return (workspaces?.workspaces?.length ?? 0) >= 2 && terms.length >= 2;
  },

  async tearDown(client) {
    await resetProfile(client);
  },
});
