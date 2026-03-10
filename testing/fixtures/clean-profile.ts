import { defineFixture, resetProfile, waitForReady } from './types';

export const cleanProfile = defineFixture({
  name: 'clean-profile',
  description: 'Reset staging to a clean profile with one workspace and one terminal',

  async create(client) {
    await resetProfile(client);
  },

  async verifyReady(client) {
    return waitForReady(client);
  },

  async tearDown(_client) {
    // Clean profile has no special teardown — reset IS the setup
  },
});
