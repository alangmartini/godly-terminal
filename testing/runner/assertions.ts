import type { Assertion, AssertionResult } from './types.js';

export function evaluateAssertion(assertion: Assertion, data: unknown): AssertionResult {
  const actual = assertion.path ? getByPath(data, assertion.path) : data;
  const { passed, expected } = evaluate(assertion, actual);
  return {
    assertion_id: assertion.id,
    passed,
    expected,
    actual,
    message: assertion.message,
  };
}

function evaluate(a: Assertion, actual: unknown): { passed: boolean; expected: unknown } {
  switch (a.type) {
    case 'equals':
      return { passed: deepEqual(actual, a.expected), expected: a.expected };

    case 'contains':
      if (Array.isArray(actual)) {
        return {
          passed: actual.some(el => el === a.expected || (typeof el === 'string' && el.includes(String(a.expected)))),
          expected: a.expected,
        };
      }
      return {
        passed: typeof actual === 'string' && actual.includes(String(a.expected)),
        expected: a.expected,
      };

    case 'regex':
      return {
        passed: typeof actual === 'string' && new RegExp(a.pattern || '').test(actual),
        expected: a.pattern,
      };

    case 'exists':
    case 'not_null':
      return { passed: actual !== undefined && actual !== null, expected: 'exists' };

    case 'not_exists':
      return { passed: actual === undefined || actual === null, expected: 'not_exists' };

    case 'gt':
      return { passed: typeof actual === 'number' && actual > (a.expected as number), expected: `> ${a.expected}` };

    case 'lt':
      return { passed: typeof actual === 'number' && actual < (a.expected as number), expected: `< ${a.expected}` };

    case 'threshold':
      return {
        passed: typeof actual === 'number' && Math.abs(actual - (a.expected as number)) <= (a.tolerance ?? 0),
        expected: `${a.expected} +/- ${a.tolerance}`,
      };

    case 'bounds': {
      const bounds = a.expected as { x: number; y: number; width: number; height: number };
      const ab = actual as { x: number; y: number; width: number; height: number } | null;
      return {
        passed: ab != null && ab.x >= bounds.x && ab.y >= bounds.y && ab.width <= bounds.width && ab.height <= bounds.height,
        expected: bounds,
      };
    }

    default:
      return { passed: false, expected: `Unknown assertion type: ${a.type}` };
  }
}

function getByPath(obj: unknown, path: string): unknown {
  const parts = path.split('.');
  let current: unknown = obj;
  for (let i = 0; i < parts.length; i++) {
    const part = parts[i];
    if (current == null) return undefined;
    if (part === '[*]') {
      if (!Array.isArray(current)) return undefined;
      const rest = parts.slice(i + 1).join('.');
      if (!rest) return current;
      return current.map(el => getByPath(el, rest));
    }
    if (typeof current !== 'object') return undefined;
    current = (current as Record<string, unknown>)[part];
  }
  return current;
}

function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (a == null || b == null) return false;
  if (typeof a !== typeof b) return false;
  if (typeof a === 'object') return JSON.stringify(a) === JSON.stringify(b);
  return false;
}
