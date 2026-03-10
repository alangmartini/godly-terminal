import type { AssertionResult } from './types.js';

interface Assertion {
  id: string;
  type: string;
  path?: string;
  expected?: unknown;
  pattern?: string;
  tolerance?: number;
  message?: string;
}

export function evaluateAssertion(assertion: Assertion, data: unknown): AssertionResult {
  const actual = assertion.path ? getByPath(data, assertion.path) : data;

  switch (assertion.type) {
    case 'equals':
      return {
        assertion_id: assertion.id,
        passed: deepEqual(actual, assertion.expected),
        expected: assertion.expected,
        actual,
        message: assertion.message,
      };

    case 'contains':
      return {
        assertion_id: assertion.id,
        passed: typeof actual === 'string' && actual.includes(String(assertion.expected)),
        expected: assertion.expected,
        actual,
        message: assertion.message,
      };

    case 'regex': {
      const re = new RegExp(assertion.pattern || '');
      return {
        assertion_id: assertion.id,
        passed: typeof actual === 'string' && re.test(actual),
        expected: assertion.pattern,
        actual,
        message: assertion.message,
      };
    }

    case 'exists':
      return {
        assertion_id: assertion.id,
        passed: actual !== undefined && actual !== null,
        expected: 'exists',
        actual,
        message: assertion.message,
      };

    case 'not_exists':
      return {
        assertion_id: assertion.id,
        passed: actual === undefined || actual === null,
        expected: 'not_exists',
        actual,
        message: assertion.message,
      };

    case 'gt':
      return {
        assertion_id: assertion.id,
        passed: typeof actual === 'number' && actual > (assertion.expected as number),
        expected: `> ${assertion.expected}`,
        actual,
        message: assertion.message,
      };

    case 'lt':
      return {
        assertion_id: assertion.id,
        passed: typeof actual === 'number' && actual < (assertion.expected as number),
        expected: `< ${assertion.expected}`,
        actual,
        message: assertion.message,
      };

    case 'threshold':
      return {
        assertion_id: assertion.id,
        passed: typeof actual === 'number' && Math.abs(actual - (assertion.expected as number)) <= (assertion.tolerance ?? 0),
        expected: `${assertion.expected} +/- ${assertion.tolerance}`,
        actual,
        message: assertion.message,
      };

    case 'bounds': {
      const bounds = assertion.expected as { x: number; y: number; width: number; height: number };
      const actualBounds = actual as { x: number; y: number; width: number; height: number } | null;
      return {
        assertion_id: assertion.id,
        passed: actualBounds != null &&
          actualBounds.x >= bounds.x &&
          actualBounds.y >= bounds.y &&
          actualBounds.width <= bounds.width &&
          actualBounds.height <= bounds.height,
        expected: bounds,
        actual: actualBounds,
        message: assertion.message,
      };
    }

    default:
      return {
        assertion_id: assertion.id,
        passed: false,
        message: `Unknown assertion type: ${assertion.type}`,
      };
  }
}

function getByPath(obj: unknown, path: string): unknown {
  const parts = path.split('.');
  let current: unknown = obj;
  for (const part of parts) {
    if (current == null || typeof current !== 'object') return undefined;
    current = (current as Record<string, unknown>)[part];
  }
  return current;
}

function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (a == null || b == null) return false;
  if (typeof a !== typeof b) return false;
  if (typeof a === 'object') {
    return JSON.stringify(a) === JSON.stringify(b);
  }
  return false;
}
