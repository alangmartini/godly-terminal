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

export interface Step {
  id: string;
  description: string;
  type: 'action' | 'query' | 'wait' | 'assert' | 'snapshot';
  target?: string;
  action?: string;
  condition?: string;
  args?: Record<string, unknown>;
  assertions?: Assertion[];
  timeout_ms?: number;
  on_failure?: 'abort' | 'continue' | 'skip-rest';
}

export interface Assertion {
  id: string;
  type: 'equals' | 'contains' | 'regex' | 'exists' | 'not_exists' | 'gt' | 'lt' | 'bounds' | 'threshold';
  path?: string;  // JSON path into query result
  expected?: unknown;
  pattern?: string;  // for regex
  tolerance?: number;  // for threshold
  message?: string;  // human-readable failure description
}

export interface CleanupStep {
  type: 'action' | 'close' | 'reset';
  target?: string;
  action?: string;
  args?: Record<string, unknown>;
}

export interface FixtureRef {
  name: string;
  args?: Record<string, unknown>;
}

// Validation
export function validateContract(contract: unknown): contract is Contract {
  if (!contract || typeof contract !== 'object') return false;
  const c = contract as Record<string, unknown>;
  return (
    typeof c.id === 'string' &&
    typeof c.description === 'string' &&
    Array.isArray(c.frontends) &&
    typeof c.fixture === 'string' &&
    typeof c.requires_restart === 'boolean' &&
    Array.isArray(c.steps) &&
    Array.isArray(c.cleanup)
  );
}
