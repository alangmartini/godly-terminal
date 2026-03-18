#!/usr/bin/env node
// godly-test.mjs — CLI entry point for the YAML test runner.
//
// Usage:
//   node godly-test/godly-test.mjs run tests/e2e-yaml/           # Run all
//   node godly-test/godly-test.mjs run tests/e2e-yaml/smoke.yaml # Single file
//   node godly-test/godly-test.mjs run tests/e2e-yaml/ --filter smoke --bail --verbose
//   node godly-test/godly-test.mjs list tests/e2e-yaml/           # List without running
//   node godly-test/godly-test.mjs validate tests/e2e-yaml/       # Validate YAML syntax

import { runTests, listTests, validateTests } from './lib/runner.mjs';

const args = process.argv.slice(2);
const command = args[0];

// Parse flags
const flags = {};
const positional = [];
for (let i = 1; i < args.length; i++) {
  const arg = args[i];
  if (arg === '--bail') {
    flags.bail = true;
  } else if (arg === '--verbose' || arg === '-v') {
    flags.verbose = true;
  } else if (arg === '--no-cleanup') {
    flags.noCleanup = true;
  } else if (arg === '--no-color') {
    flags.noColor = true;
  } else if (arg === '--filter' && i + 1 < args.length) {
    flags.filter = args[++i];
  } else if (arg.startsWith('--filter=')) {
    flags.filter = arg.split('=')[1];
  } else if (arg === '--port' && i + 1 < args.length) {
    flags.port = parseInt(args[++i], 10);
  } else if (arg.startsWith('--port=')) {
    flags.port = parseInt(arg.split('=')[1], 10);
  } else if (!arg.startsWith('-')) {
    positional.push(arg);
  }
}

function usage() {
  console.log(`
godly-test v0.1.0 — Maestro-like YAML testing for Godly Terminal

Usage:
  godly-test run <path...>       Run test files or directories
  godly-test list <path...>      List test files without running
  godly-test validate <path...>  Validate YAML syntax

Options:
  --filter <name>   Only run tests matching name
  --bail            Stop after first failure
  --verbose, -v     Verbose output
  --no-cleanup      Skip resource teardown after tests
  --no-color        Disable ANSI colors
  --port <number>   MCP server port (overrides discovery)
`);
}

async function main() {
  switch (command) {
    case 'run': {
      if (positional.length === 0) {
        console.error('Error: specify test file(s) or directory\n');
        usage();
        process.exit(1);
      }
      const success = await runTests(positional, flags);
      process.exit(success ? 0 : 1);
      break;
    }

    case 'list': {
      if (positional.length === 0) {
        console.error('Error: specify test file(s) or directory\n');
        usage();
        process.exit(1);
      }
      listTests(positional, flags.filter);
      break;
    }

    case 'validate': {
      if (positional.length === 0) {
        console.error('Error: specify test file(s) or directory\n');
        usage();
        process.exit(1);
      }
      const valid = validateTests(positional, flags.filter);
      process.exit(valid ? 0 : 1);
      break;
    }

    case '--help':
    case '-h':
    case 'help':
    case undefined:
      usage();
      break;

    default:
      console.error(`Unknown command: ${command}\n`);
      usage();
      process.exit(1);
  }
}

main().catch((err) => {
  console.error(`\x1b[31mFatal: ${err.message}\x1b[0m`);
  if (flags.verbose) console.error(err.stack);
  process.exit(1);
});
