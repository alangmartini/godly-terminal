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

export type FailureType = 'assertion' | 'timeout' | 'crash' | 'infrastructure' | 'unknown';
