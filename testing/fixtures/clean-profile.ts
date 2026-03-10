import type { Fixture, McpClient } from './types';

export const cleanProfile: Fixture = {
  name: 'clean-profile',
  description: 'Reset staging to a clean profile with one workspace and one terminal',

  async create(client: McpClient): Promise<void> {
    await client.call('reset_staging_profile');
  },

  async verifyReady(client: McpClient): Promise<boolean> {
    const result = await client.call('wait_for_app_ready', { timeout_ms: 10000 }) as any;
    return result?.ready === true;
  },

  async tearDown(_client: McpClient): Promise<void> {
    // Clean profile has no special teardown
  },

  async reset(client: McpClient): Promise<void> {
    await client.call('reset_staging_profile');
  },
};
