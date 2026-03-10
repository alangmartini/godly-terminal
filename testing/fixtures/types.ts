export interface McpClient {
  call(tool: string, args?: Record<string, unknown>): Promise<unknown>;
  callTool(tool: string, args?: Record<string, unknown>): Promise<unknown>;
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

export function extractToolData(raw: unknown): unknown {
  const obj = raw as Record<string, unknown> | undefined;
  const content = obj?.content as Array<{ text?: string }> | undefined;
  const text = content?.[0]?.text;
  if (!text) return raw;

  try {
    return JSON.parse(text) as unknown;
  } catch {
    return text;
  }
}

/** Reset staging to a clean profile. Shared across all fixtures for teardown. */
export async function resetProfile(client: McpClient): Promise<void> {
  await client.callTool('reset_staging_profile');
}

/** Wait for the staging app to be ready. Returns true if ready. */
export async function waitForReady(client: McpClient, timeout_ms = 10_000): Promise<boolean> {
  const result = extractToolData(
    await client.callTool('wait_for_app_ready', { timeout_ms }),
  ) as { ready?: boolean } | null;
  return result?.ready === true;
}

/** Get the active workspace ID, or throw if none. */
export async function getActiveWorkspaceId(client: McpClient): Promise<string> {
  const result = extractToolData(
    await client.callTool('get_active_workspace'),
  ) as { id?: string, workspace?: { id?: string } | null } | null;
  const id = result?.id ?? result?.workspace?.id;
  if (!id) throw new Error('No active workspace');
  return id;
}

/** List terminal IDs in the current workspace. */
export async function listTerminalIds(client: McpClient): Promise<string[]> {
  const result = extractToolData(
    await client.callTool('list_terminals'),
  ) as { terminals?: { id: string }[] } | null;
  return result?.terminals?.map(t => t.id) ?? [];
}
