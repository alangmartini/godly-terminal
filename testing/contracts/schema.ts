export interface Contract {
  id: string;
  description: string;
  frontends: ('web' | 'native')[];
  fixture: string;
  requires_restart: boolean;
  tags?: string[];
  steps: Step[];
  cleanup: CleanupStep[];
}

export type StepType = 'action' | 'query' | 'wait' | 'assert' | 'snapshot';
export type FailureMode = 'abort' | 'continue' | 'skip-rest';
export type AssertionType = 'equals' | 'contains' | 'regex' | 'exists' | 'not_exists' | 'gt' | 'lt' | 'bounds' | 'threshold';

export interface Step {
  id: string;
  description: string;
  type: StepType;
  target?: string;
  action?: string;
  condition?: string;
  args?: Record<string, unknown>;
  assertions?: Assertion[];
  timeout_ms?: number;
  on_failure?: FailureMode;
}

export interface Assertion {
  id: string;
  type: AssertionType;
  path?: string;
  expected?: unknown;
  pattern?: string;
  tolerance?: number;
  message?: string;
}

export interface CleanupStep {
  type: 'action' | 'close' | 'reset';
  target?: string;
  action?: string;
  args?: Record<string, unknown>;
}

// Validation
export function validateContract(contract: unknown): contract is Contract {
  if (!contract || typeof contract !== 'object') return false;
  const c = contract as Record<string, unknown>;
  if (
    typeof c.id !== 'string' ||
    typeof c.description !== 'string' ||
    !Array.isArray(c.frontends) ||
    typeof c.fixture !== 'string' ||
    typeof c.requires_restart !== 'boolean' ||
    !Array.isArray(c.steps) ||
    !Array.isArray(c.cleanup)
  ) return false;

  return (c.steps as unknown[]).every(isValidStep)
    && (c.cleanup as unknown[]).every(isValidCleanupStep);
}

const STEP_TYPES: ReadonlySet<string> = new Set(['action', 'query', 'wait', 'assert', 'snapshot']);
const CLEANUP_TYPES: ReadonlySet<string> = new Set(['action', 'close', 'reset']);

function isValidStep(step: unknown): step is Step {
  if (!step || typeof step !== 'object') return false;
  const s = step as Record<string, unknown>;
  return typeof s.id === 'string'
    && typeof s.description === 'string'
    && STEP_TYPES.has(s.type as string);
}

function isValidCleanupStep(step: unknown): step is CleanupStep {
  if (!step || typeof step !== 'object') return false;
  const s = step as Record<string, unknown>;
  return CLEANUP_TYPES.has(s.type as string);
}
