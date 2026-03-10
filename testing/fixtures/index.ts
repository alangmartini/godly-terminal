import { cleanProfile } from './clean-profile.js';
import { splitBasic } from './split-basic.js';
import { twoTerminals } from './two-terminals.js';
import type { Fixture } from './types.js';

const FIXTURES: Record<string, Fixture> = {
  'clean-profile': cleanProfile,
  'split-basic': splitBasic,
  'two-terminals': twoTerminals,
};

export function getFixture(name: string): Fixture | undefined {
  return FIXTURES[name];
}
