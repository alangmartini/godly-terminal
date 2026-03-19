// lib/yaml-parser.mjs
// Parse YAML test files with front matter (name, tags) and step list.

import { readFileSync } from 'fs';
import yaml from 'js-yaml';

/**
 * Parse a YAML test file.
 *
 * Format:
 *   name: "Test name"
 *   tags: [tag1, tag2]
 *   ---
 *   - stepName
 *   - stepName:
 *       param: value
 *     store: varName
 *
 * Or without front matter separator (single document):
 *   name: "Test name"
 *   tags: [tag1, tag2]
 *   steps:
 *     - stepName
 *     - stepName:
 *         param: value
 *       store: varName
 */
export function parseTestFile(filePath) {
  const raw = readFileSync(filePath, 'utf-8');
  return parseTestYaml(raw, filePath);
}

export function parseTestYaml(raw, filePath = '<inline>') {
  // Try multi-document YAML (front matter --- steps)
  const docs = [];
  yaml.loadAll(raw, (doc) => docs.push(doc));

  let meta, steps;

  if (docs.length === 2 && Array.isArray(docs[1])) {
    // Two documents: front matter object + step array
    meta = docs[0] || {};
    steps = docs[1];
  } else if (docs.length === 1 && docs[0] && typeof docs[0] === 'object') {
    const doc = docs[0];
    if (Array.isArray(doc)) {
      // Single document that is just a step array
      meta = {};
      steps = doc;
    } else if (Array.isArray(doc.steps)) {
      // Single document with explicit steps key
      meta = { name: doc.name, tags: doc.tags };
      steps = doc.steps;
    } else {
      throw new ParseError(`Invalid test file structure`, filePath);
    }
  } else {
    throw new ParseError(`Expected 1-2 YAML documents, got ${docs.length}`, filePath);
  }

  const normalizedSteps = steps.map((step, i) => normalizeStep(step, i, filePath));

  return {
    name: meta.name || fileBaseName(filePath),
    tags: Array.isArray(meta.tags) ? meta.tags : [],
    steps: normalizedSteps,
    filePath,
  };
}

/**
 * Normalize a single step into a structured object.
 *
 * Input forms:
 *   - "stepName"                     → { action: "stepName", args: {} }
 *   - { stepName: { p: v } }         → { action: "stepName", args: { p: v } }
 *   - { stepName: { p: v }, store: x }→ { action: "stepName", args: { p: v }, store: "x" }
 */
function normalizeStep(step, index, filePath) {
  if (typeof step === 'string') {
    return { action: step, args: {}, index };
  }

  if (typeof step !== 'object' || step === null || Array.isArray(step)) {
    throw new ParseError(`Step ${index + 1}: expected string or object, got ${typeof step}`, filePath);
  }

  // Extract reserved keys
  const store = step.store;
  const timeout = step.timeout;

  // The action key is the first non-reserved key
  const reservedKeys = new Set(['store', 'timeout']);
  const actionKeys = Object.keys(step).filter(k => !reservedKeys.has(k));

  if (actionKeys.length === 0) {
    throw new ParseError(`Step ${index + 1}: no action key found`, filePath);
  }
  if (actionKeys.length > 1) {
    throw new ParseError(`Step ${index + 1}: multiple action keys: ${actionKeys.join(', ')}`, filePath);
  }

  const action = actionKeys[0];
  let args = step[action];

  // Handle null args (e.g., `- resetApp:` with no value)
  if (args === null || args === undefined) {
    args = {};
  } else if (typeof args !== 'object' || Array.isArray(args)) {
    // Scalar arg — wrap as { value: X }
    args = { value: args };
  }

  const result = { action, args, index };
  if (store) result.store = store;
  if (timeout) result.timeout = timeout;
  return result;
}

/**
 * Validate a parsed test file structure (without executing).
 * Returns an array of warning/error strings.
 */
export function validateTest(test) {
  const issues = [];

  if (!test.name) {
    issues.push('Missing test name');
  }

  if (test.steps.length === 0) {
    issues.push('Test has no steps');
  }

  // Check for $var references that might be undefined
  const definedVars = new Set();
  for (const step of test.steps) {
    // Check args for $var references
    const refs = findVarRefs(step.args);
    for (const ref of refs) {
      const varName = ref.split('.')[0];
      if (!definedVars.has(varName)) {
        issues.push(`Step ${step.index + 1} (${step.action}): references $${ref} but '${varName}' not yet defined via store:`);
      }
    }

    if (step.store) {
      definedVars.add(step.store);
    }
  }

  return issues;
}

function findVarRefs(obj) {
  const refs = [];
  if (typeof obj === 'string' && obj.startsWith('$')) {
    refs.push(obj.slice(1));
  } else if (obj && typeof obj === 'object') {
    for (const val of Object.values(obj)) {
      refs.push(...findVarRefs(val));
    }
  }
  return refs;
}

function fileBaseName(filePath) {
  const parts = filePath.replace(/\\/g, '/').split('/');
  const name = parts[parts.length - 1];
  return name.replace(/\.(yaml|yml)$/i, '');
}

class ParseError extends Error {
  constructor(message, filePath) {
    super(`${filePath}: ${message}`);
    this.name = 'ParseError';
    this.filePath = filePath;
  }
}
