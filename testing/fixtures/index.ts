import { cleanProfile } from './clean-profile.js';
import { multiWorkspace } from './multi-workspace.js';
import { splitBasic } from './split-basic.js';
import { twoTerminals } from './two-terminals.js';
import type { Fixture } from './types.js';

const FIXTURES: Record<string, Fixture> = {
  'clean-profile': cleanProfile,
  'multi-workspace': multiWorkspace,
  'split-basic': splitBasic,
  'two-terminals': twoTerminals,
};

export function getFixture(name: string): Fixture | undefined {
  return FIXTURES[name];
}
