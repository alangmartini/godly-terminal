import { readFileSync } from 'fs';
import { McpClient } from './mcp-client.js';
import { evaluateAssertion } from './assertions.js';
import { ArtifactBundle } from './artifact-bundle.js';
import type { RunResult, StepResult, AssertionResult } from './types.js';

interface Contract {
  id: string;
  description: string;
  frontends: string[];
  fixture: string;
  requires_restart: boolean;
  steps: Step[];
  cleanup: CleanupStep[];
}

interface Step {
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

interface Assertion {
  id: string;
  type: string;
  path?: string;
  expected?: unknown;
  pattern?: string;
  tolerance?: number;
  message?: string;
}

interface CleanupStep {
  type: string;
  target?: string;
  action?: string;
  args?: Record<string, unknown>;
}

export class ContractRunner {
  private client: McpClient;
  private bundle: ArtifactBundle | null = null;

  constructor(client: McpClient, private artifactBaseDir: string) {
    this.client = client;
  }

  async run(contractPath: string): Promise<RunResult> {
    const contract: Contract = JSON.parse(readFileSync(contractPath, 'utf-8'));
    const runId = `${contract.id}-${Date.now()}`;
    this.bundle = new ArtifactBundle(this.artifactBaseDir, runId);
    this.bundle.writeManifest(contract.id);

    const startTime = Date.now();
    const stepResults: StepResult[] = [];
    let allPassed = true;
    let aborted = false;

    // Execute steps
    for (const step of contract.steps) {
      if (aborted) {
        stepResults.push({
          step_id: step.id,
          passed: false,
          duration_ms: 0,
          assertions: [],
          skipped: true,
        });
        continue;
      }

      const stepStart = Date.now();
      const result = await this.executeStep(step);
      result.duration_ms = Date.now() - stepStart;

      stepResults.push(result);
      this.bundle.writeStepTrace(step.id, result);

      if (!result.passed) {
        allPassed = false;
        if (step.on_failure === 'abort' || step.on_failure === undefined) {
          aborted = true;
        }
      }
    }

    // Run cleanup (best-effort)
    for (const cleanup of contract.cleanup) {
      try {
        await this.executeCleanup(cleanup);
      } catch (e) {
        // Log but don't fail
        console.error(`[cleanup] Error: ${e}`);
      }
    }

    const runResult: RunResult = {
      contract_id: contract.id,
      passed: allPassed,
      steps: stepResults,
      total_duration_ms: Date.now() - startTime,
      artifact_dir: this.bundle.artifactDir,
    };

    this.bundle.writeResult(runResult);
    this.bundle.finalize();

    return runResult;
  }

  private async executeStep(step: Step): Promise<StepResult> {
    try {
      switch (step.type) {
        case 'action':
          return await this.executeAction(step);
        case 'query':
          return await this.executeQuery(step);
        case 'wait':
          return await this.executeWait(step);
        case 'assert':
          return await this.executeQuery(step); // assert is a query with mandatory assertions
        case 'snapshot':
          return await this.executeSnapshot(step);
        default:
          return {
            step_id: step.id,
            passed: false,
            duration_ms: 0,
            assertions: [],
            error: `Unknown step type: ${step.type}`,
            skipped: false,
          };
      }
    } catch (e: unknown) {
      const message = e instanceof Error ? e.message : String(e);
      return {
        step_id: step.id,
        passed: false,
        duration_ms: 0,
        assertions: [],
        error: message,
        skipped: false,
      };
    }
  }

  private async executeAction(step: Step): Promise<StepResult> {
    await this.client.callTool('ui_act', {
      target: step.target || '',
      action: step.action || '',
      args: step.args,
    });

    return {
      step_id: step.id,
      passed: true,
      duration_ms: 0,
      assertions: [],
      skipped: false,
    };
  }

  private async executeQuery(step: Step): Promise<StepResult> {
    const result = await this.client.callTool('ui_query', {
      target: step.target || '',
      args: step.args,
    });

    const resultObj = result as Record<string, unknown> | undefined;
    const content = resultObj?.content as Array<{ text?: string }> | undefined;
    const data = content?.[0]?.text ? JSON.parse(content[0].text) : result;
    const queryData = (data as Record<string, unknown>)?.data ?? data;
    const assertions: AssertionResult[] = (step.assertions || []).map((a) =>
      evaluateAssertion(a, queryData)
    );

    const allPassed = assertions.every((a) => a.passed);

    return {
      step_id: step.id,
      passed: allPassed,
      duration_ms: 0,
      assertions,
      skipped: false,
    };
  }

  private async executeWait(step: Step): Promise<StepResult> {
    const result = await this.client.callTool('ui_wait', {
      condition: step.condition || '',
      timeout_ms: step.timeout_ms,
      args: step.args,
    });

    const resultObj = result as Record<string, unknown> | undefined;
    const content = resultObj?.content as Array<{ text?: string }> | undefined;
    const data = content?.[0]?.text ? JSON.parse(content[0].text) : result;
    const dataObj = data as Record<string, unknown>;
    const passed = dataObj?.ok === true || dataObj?.timed_out === false;

    return {
      step_id: step.id,
      passed,
      duration_ms: 0,
      assertions: [],
      error: passed ? undefined : 'Wait timed out',
      skipped: false,
    };
  }

  private async executeSnapshot(step: Step): Promise<StepResult> {
    await this.client.callTool('capture_screenshot', {
      terminal_id: step.args?.terminal_id,
    });

    return {
      step_id: step.id,
      passed: true,
      duration_ms: 0,
      assertions: [],
      skipped: false,
    };
  }

  private async executeCleanup(cleanup: CleanupStep): Promise<void> {
    switch (cleanup.type) {
      case 'reset':
        await this.client.callTool('reset_staging_profile');
        break;
      case 'action':
        await this.client.callTool('ui_act', {
          target: cleanup.target || '',
          action: cleanup.action || '',
          args: cleanup.args,
        });
        break;
      case 'close':
        // Close resources — no-op for now
        break;
    }
  }
}
