/**
 * Contract schema validator — runs in CI without a live app.
 *
 * Validates that every contract JSON in testing/contracts/ is well-formed:
 *   - Required top-level fields present and typed correctly
 *   - Every step has a valid type, required fields for that type, and unique id
 *   - Assertion types are from the known set
 *   - No duplicate step/assertion ids within a contract
 *   - Cleanup steps have required fields
 *
 * Exit code 0 = all valid, 1 = at least one error.
 */

import { readdirSync, readFileSync } from 'fs';
import { resolve, basename } from 'path';

// --- Known valid values ---

const VALID_STEP_TYPES = ['action', 'query', 'wait'] as const;
const VALID_ASSERTION_TYPES = ['equals', 'gt', 'gte', 'lt', 'lte', 'contains', 'not_null', 'exists', 'matches'] as const;
const VALID_FRONTENDS = ['web', 'native'] as const;

type StepType = (typeof VALID_STEP_TYPES)[number];
type AssertionType = (typeof VALID_ASSERTION_TYPES)[number];

// --- Types ---

interface Assertion {
  id: string;
  type: AssertionType;
  path?: string;
  expected?: unknown;
}

interface Step {
  id: string;
  description: string;
  type: StepType;
  target?: string;
  action?: string;
  args?: Record<string, unknown>;
  condition?: string;
  timeout_ms?: number;
  assertions?: Assertion[];
}

interface Contract {
  id: string;
  description: string;
  frontends: string[];
  fixture: string;
  requires_restart: boolean;
  steps: Step[];
  cleanup?: Record<string, unknown>[];
}

// --- Validation ---

function validateContract(filePath: string): string[] {
  const errors: string[] = [];
  const fileName = basename(filePath);

  let raw: string;
  try {
    raw = readFileSync(filePath, 'utf-8');
  } catch {
    errors.push(`${fileName}: cannot read file`);
    return errors;
  }

  let contract: Contract;
  try {
    contract = JSON.parse(raw);
  } catch (e) {
    errors.push(`${fileName}: invalid JSON — ${e}`);
    return errors;
  }

  // Top-level required fields
  if (!contract.id || typeof contract.id !== 'string') {
    errors.push(`${fileName}: missing or invalid "id"`);
  }
  if (!contract.description || typeof contract.description !== 'string') {
    errors.push(`${fileName}: missing or invalid "description"`);
  }
  if (!Array.isArray(contract.frontends) || contract.frontends.length === 0) {
    errors.push(`${fileName}: "frontends" must be a non-empty array`);
  } else {
    for (const f of contract.frontends) {
      if (!(VALID_FRONTENDS as readonly string[]).includes(f)) {
        errors.push(`${fileName}: unknown frontend "${f}" (valid: ${VALID_FRONTENDS.join(', ')})`);
      }
    }
  }
  if (!contract.fixture || typeof contract.fixture !== 'string') {
    errors.push(`${fileName}: missing or invalid "fixture"`);
  }
  if (typeof contract.requires_restart !== 'boolean') {
    errors.push(`${fileName}: "requires_restart" must be a boolean`);
  }

  // File name should match id
  const expectedFileName = `${contract.id}.json`;
  if (fileName !== expectedFileName) {
    errors.push(`${fileName}: file name doesn't match id "${contract.id}" (expected ${expectedFileName})`);
  }

  // Steps
  if (!Array.isArray(contract.steps) || contract.steps.length === 0) {
    errors.push(`${fileName}: "steps" must be a non-empty array`);
    return errors;
  }

  const stepIds = new Set<string>();
  const assertionIds = new Set<string>();

  for (let i = 0; i < contract.steps.length; i++) {
    const step = contract.steps[i];
    const prefix = `${fileName} step[${i}]`;

    if (!step.id || typeof step.id !== 'string') {
      errors.push(`${prefix}: missing or invalid "id"`);
    } else if (stepIds.has(step.id)) {
      errors.push(`${prefix}: duplicate step id "${step.id}"`);
    } else {
      stepIds.add(step.id);
    }

    if (!step.description || typeof step.description !== 'string') {
      errors.push(`${prefix} (${step.id}): missing "description"`);
    }

    if (!(VALID_STEP_TYPES as readonly string[]).includes(step.type)) {
      errors.push(`${prefix} (${step.id}): invalid type "${step.type}" (valid: ${VALID_STEP_TYPES.join(', ')})`);
      continue;
    }

    // Type-specific validation
    switch (step.type) {
      case 'action':
        if (!step.target) errors.push(`${prefix} (${step.id}): action step missing "target"`);
        if (!step.action) errors.push(`${prefix} (${step.id}): action step missing "action"`);
        break;
      case 'query':
        if (!step.target) errors.push(`${prefix} (${step.id}): query step missing "target"`);
        // assertions are optional for query (some just record values)
        break;
      case 'wait':
        if (!step.condition) errors.push(`${prefix} (${step.id}): wait step missing "condition"`);
        if (step.timeout_ms !== undefined && (typeof step.timeout_ms !== 'number' || step.timeout_ms <= 0)) {
          errors.push(`${prefix} (${step.id}): "timeout_ms" must be a positive number`);
        }
        break;
    }

    // Assertions
    if (step.assertions) {
      if (!Array.isArray(step.assertions)) {
        errors.push(`${prefix} (${step.id}): "assertions" must be an array`);
      } else {
        for (let j = 0; j < step.assertions.length; j++) {
          const a = step.assertions[j];
          const aPrefix = `${prefix} (${step.id}) assertion[${j}]`;

          if (!a.id || typeof a.id !== 'string') {
            errors.push(`${aPrefix}: missing or invalid "id"`);
          } else if (assertionIds.has(a.id)) {
            errors.push(`${aPrefix}: duplicate assertion id "${a.id}"`);
          } else {
            assertionIds.add(a.id);
          }

          if (!(VALID_ASSERTION_TYPES as readonly string[]).includes(a.type)) {
            errors.push(`${aPrefix} (${a.id}): invalid assertion type "${a.type}" (valid: ${VALID_ASSERTION_TYPES.join(', ')})`);
          }

          // Most assertion types need "expected" (except exists and not_null)
          if (!['exists', 'not_null'].includes(a.type) && a.expected === undefined) {
            errors.push(`${aPrefix} (${a.id}): assertion type "${a.type}" requires "expected" value`);
          }
        }
      }
    }
  }

  // Cleanup
  if (contract.cleanup) {
    if (!Array.isArray(contract.cleanup)) {
      errors.push(`${fileName}: "cleanup" must be an array`);
    } else {
      for (let i = 0; i < contract.cleanup.length; i++) {
        const c = contract.cleanup[i];
        if (!c.type) {
          errors.push(`${fileName} cleanup[${i}]: missing "type"`);
        }
      }
    }
  }

  return errors;
}

// --- Main ---

const contractsDir = resolve(import.meta.dirname!, '..', 'contracts');
const files = readdirSync(contractsDir).filter(f => f.endsWith('.json'));

if (files.length === 0) {
  console.error('No contract files found in', contractsDir);
  process.exit(1);
}

let totalErrors = 0;

for (const file of files) {
  const filePath = resolve(contractsDir, file);
  const errors = validateContract(filePath);

  if (errors.length > 0) {
    totalErrors += errors.length;
    for (const e of errors) {
      console.error(`  FAIL  ${e}`);
    }
  } else {
    console.log(`  PASS  ${file}`);
  }
}

console.log('');
console.log(`${files.length} contracts, ${totalErrors} errors`);
process.exit(totalErrors > 0 ? 1 : 0);
