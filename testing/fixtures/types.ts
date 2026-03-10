export interface McpClient {
  call(tool: string, args?: Record<string, unknown>): Promise<unknown>;
}

/** Core fixture operations that each fixture must implement. */
export interface FixtureOps {
  name: string;
  description: string;
  create(client: McpClient): Promise<void>;
  verifyReady(client: McpClient): Promise<boolean>;
  tearDown(client: McpClient): Promise<void>;
}

/** Full fixture interface including derived `reset`. */
export interface Fixture extends FixtureOps {
  reset(client: McpClient): Promise<void>;
}

/**
 * Create a Fixture from core ops. `reset` defaults to tearDown + create.
 * Avoids repeating the same reset pattern in every fixture.
 */
export function defineFixture(ops: FixtureOps): Fixture {
  return {
    ...ops,
    async reset(client: McpClient) {
      await ops.tearDown(client);
      await ops.create(client);
    },
  };
}

/** Reset staging to a clean profile. Shared across all fixtures for teardown. */
export async function resetProfile(client: McpClient): Promise<void> {
  await client.call('reset_staging_profile');
}

/** Wait for the staging app to be ready. Returns true if ready. */
export async function waitForReady(client: McpClient, timeout_ms = 10_000): Promise<boolean> {
  const result = await client.call('wait_for_app_ready', { timeout_ms }) as { ready?: boolean } | null;
  return result?.ready === true;
}

/** Get the active workspace ID, or throw if none. */
export async function getActiveWorkspaceId(client: McpClient): Promise<string> {
  const result = await client.call('get_active_workspace') as { workspace?: { id?: string } } | null;
  const id = result?.workspace?.id;
  if (!id) throw new Error('No active workspace');
  return id;
}

/** List terminal IDs in the current workspace. */
export async function listTerminalIds(client: McpClient): Promise<string[]> {
  const result = await client.call('list_terminals') as { terminals?: { id: string }[] } | null;
  return result?.terminals?.map(t => t.id) ?? [];
}
