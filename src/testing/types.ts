export interface SemanticQuery {
  target: string;
  args?: Record<string, unknown>;
}

export interface SemanticAction {
  target: string;
  action: string;
  args?: Record<string, unknown>;
}

export interface SemanticWait {
  condition: string;
  timeout_ms?: number;
  poll_interval_ms?: number;
  args?: Record<string, unknown>;
}

export interface QueryResult {
  ok: boolean;
  target: string;
  data?: unknown;
  error?: string;
  timestamp_ms: number;
}

export interface ActionResult {
  ok: boolean;
  target: string;
  action: string;
  error?: string;
  timestamp_ms: number;
}

export interface WaitResult {
  ok: boolean;
  condition: string;
  timed_out: boolean;
  elapsed_ms: number;
  error?: string;
}

export interface TestHarnessStatus {
  ready: boolean;
  frontend_type: string;
  harness_mode: boolean;
  run_id?: string;
  uptime_ms: number;
}
