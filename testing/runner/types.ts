export interface RunResult {
  contract_id: string;
  passed: boolean;
  steps: StepResult[];
  total_duration_ms: number;
  artifact_dir?: string;
  error?: string;
}

export interface StepResult {
  step_id: string;
  passed: boolean;
  duration_ms: number;
  assertions: AssertionResult[];
  error?: string;
  skipped: boolean;
}

export interface AssertionResult {
  assertion_id: string;
  passed: boolean;
  expected?: unknown;
  actual?: unknown;
  message?: string;
}

export interface Assertion {
  id: string;
  type: string;
  path?: string;
  expected?: unknown;
  pattern?: string;
  tolerance?: number;
  message?: string;
}

export interface Step {
  id: string;
  description: string;
  type: string;
  target?: string;
  action?: string;
  condition?: string;
  args?: Record<string, unknown>;
  assertions?: Assertion[];
  timeout_ms?: number;
  on_failure?: string;
}

export interface CleanupStep {
  type: string;
  target?: string;
  action?: string;
  args?: Record<string, unknown>;
}

export interface Contract {
  id: string;
  description: string;
  frontends: string[];
  fixture: string;
  requires_restart: boolean;
  steps: Step[];
  cleanup: CleanupStep[];
}
