import { readFileSync } from 'fs';
import { McpClient } from './mcp-client.js';
import { evaluateAssertion } from './assertions.js';
import { ArtifactBundle } from './artifact-bundle.js';
import type { Contract, Step, CleanupStep, RunResult, StepResult, AssertionResult } from './types.js';

export class ContractRunner {
  constructor(
    private client: McpClient,
    private artifactBaseDir: string,
  ) {}

  async run(contractPath: string): Promise<RunResult> {
    const contract: Contract = JSON.parse(readFileSync(contractPath, 'utf-8'));
    const runId = `${contract.id}-${Date.now()}`;
    const bundle = new ArtifactBundle(this.artifactBaseDir, runId);
    bundle.writeManifest(contract.id);

    const startTime = Date.now();
    const stepResults: StepResult[] = [];
    let aborted = false;

    for (const step of contract.steps) {
      if (aborted) {
        stepResults.push(skipResult(step.id));
        continue;
      }

      const stepStart = Date.now();
      const result = await this.executeStep(step);
      result.duration_ms = Date.now() - stepStart;

      stepResults.push(result);
      bundle.writeStepTrace(step.id, result);

      if (!result.passed && (step.on_failure === 'abort' || step.on_failure === undefined)) {
        aborted = true;
      }
    }

    // Run cleanup (best-effort)
    for (const cleanup of contract.cleanup) {
      try {
        await this.executeCleanup(cleanup);
      } catch (e) {
        console.error(`[cleanup] Error: ${e}`);
      }
    }

    const runResult: RunResult = {
      contract_id: contract.id,
      passed: stepResults.every((s) => s.passed || s.skipped),
      steps: stepResults,
      total_duration_ms: Date.now() - startTime,
      artifact_dir: bundle.artifactDir,
    };

    bundle.writeResult(runResult);
    bundle.finalize();
    return runResult;
  }

  private async executeStep(step: Step): Promise<StepResult> {
    try {
      switch (step.type) {
        case 'action':
          return await this.executeAction(step);
        case 'query':
        case 'assert':
          return await this.executeQuery(step);
        case 'wait':
          return await this.executeWait(step);
        case 'snapshot':
          return await this.executeSnapshot(step);
        default:
          return failResult(step.id, `Unknown step type: ${step.type}`);
      }
    } catch (e: unknown) {
      return failResult(step.id, e instanceof Error ? e.message : String(e));
    }
  }

  private async executeAction(step: Step): Promise<StepResult> {
    await this.client.callTool('ui_act', {
      target: step.target || '',
      action: step.action || '',
      args: step.args,
    });
    return passResult(step.id);
  }

  private async executeQuery(step: Step): Promise<StepResult> {
    const raw = await this.client.callTool('ui_query', {
      target: step.target || '',
      args: step.args,
    });

    const queryData = extractMcpData(raw);
    const assertions: AssertionResult[] = (step.assertions || []).map((a) =>
      evaluateAssertion(a, queryData),
    );

    return {
      step_id: step.id,
      passed: assertions.every((a) => a.passed),
      duration_ms: 0,
      assertions,
      skipped: false,
    };
  }

  private async executeWait(step: Step): Promise<StepResult> {
    const raw = await this.client.callTool('ui_wait', {
      condition: step.condition || '',
      timeout_ms: step.timeout_ms,
      args: step.args,
    });

    const data = extractMcpData(raw) as Record<string, unknown>;
    const passed = data?.ok === true || data?.timed_out === false;

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
    return passResult(step.id);
  }

  private async executeCleanup(cleanup: CleanupStep): Promise<void> {
    if (cleanup.type === 'reset') {
      await this.client.callTool('reset_staging_profile');
    } else if (cleanup.type === 'action') {
      await this.client.callTool('ui_act', {
        target: cleanup.target || '',
        action: cleanup.action || '',
        args: cleanup.args,
      });
    }
    // 'close' and unknown types are no-ops
  }
}

/** Extract the data payload from an MCP tool result (handles content[].text wrapping). */
function extractMcpData(raw: unknown): unknown {
  const obj = raw as Record<string, unknown> | undefined;
  const content = obj?.content as Array<{ text?: string }> | undefined;
  const data = content?.[0]?.text ? JSON.parse(content[0].text) : raw;
  return (data as Record<string, unknown>)?.data ?? data;
}

function passResult(stepId: string): StepResult {
  return { step_id: stepId, passed: true, duration_ms: 0, assertions: [], skipped: false };
}

function failResult(stepId: string, error: string): StepResult {
  return { step_id: stepId, passed: false, duration_ms: 0, assertions: [], error, skipped: false };
}

function skipResult(stepId: string): StepResult {
  return { step_id: stepId, passed: false, duration_ms: 0, assertions: [], skipped: true };
}
