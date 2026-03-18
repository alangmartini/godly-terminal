// lib/runner.mjs
// Orchestrates test execution: load files → connect MCP → execute steps → cleanup → report.

import { readdirSync, statSync } from 'fs';
import { join, resolve } from 'path';
import { McpClient } from './mcp-client.mjs';
import { discoverMcpPort, checkHealth } from './discovery.mjs';
import { parseTestFile, validateTest } from './yaml-parser.mjs';
import { StepExecutor } from './step-executor.mjs';
import { Cleanup } from './cleanup.mjs';
import { Reporter } from './reporter.mjs';

/**
 * Run all test files matching the given path(s).
 */
export async function runTests(paths, options = {}) {
  const {
    filter,
    bail = false,
    verbose = false,
    noCleanup = false,
    noColor = false,
    port: explicitPort,
  } = options;

  const reporter = new Reporter({ verbose, noColor });
  reporter.header();

  // Discover MCP server
  const { port, source } = explicitPort
    ? { port: explicitPort, source: 'cli' }
    : discoverMcpPort();

  // Health check
  const health = await checkHealth(port);
  if (!health) {
    console.error(`\x1b[31mError: MCP server not reachable at 127.0.0.1:${port}\x1b[0m`);
    console.error(`Start Godly Terminal and ensure MCP HTTP server is running.`);
    console.error(`Source: ${source}${source === 'default' ? ' (no discovery file found)' : ''}`);
    process.exit(1);
  }

  if (verbose) {
    console.log(`  MCP server: 127.0.0.1:${port} (${source})`);
    console.log('');
  }

  // Collect test files
  const files = collectTestFiles(paths, filter);
  if (files.length === 0) {
    console.error('No test files found.');
    process.exit(1);
  }

  // Parse all test files
  const tests = [];
  for (const f of files) {
    try {
      tests.push(parseTestFile(f));
    } catch (err) {
      console.error(`\x1b[31mParse error: ${err.message}\x1b[0m`);
      if (bail) process.exit(1);
    }
  }

  // Connect MCP
  const mcp = new McpClient(port);
  try {
    await mcp.connect();
  } catch (err) {
    console.error(`\x1b[31mFailed to connect to MCP: ${err.message}\x1b[0m`);
    process.exit(1);
  }

  const totalStart = performance.now();
  let allPassed = true;

  try {
    for (const test of tests) {
      const cleanup = new Cleanup(mcp);
      if (noCleanup) cleanup.enabled = false;

      const executor = new StepExecutor(mcp, cleanup);
      const fileStart = performance.now();
      let filePassed = 0;
      let fileFailed = 0;
      let shouldSkipRest = false;

      reporter.fileStart(test.name);

      for (const step of test.steps) {
        const label = StepExecutor.getStepLabel(step);

        if (shouldSkipRest) {
          reporter.stepSkip(step.index, test.steps.length, label);
          continue;
        }

        reporter.stepStart(step.index, test.steps.length, label);

        const result = await executor.execute(step);

        if (result.success) {
          reporter.stepPass(step.index, test.steps.length, label, result.duration);
          filePassed++;
        } else {
          reporter.stepFail(step.index, test.steps.length, label, result.duration, result.error);
          fileFailed++;
          allPassed = false;

          // Capture screenshot on failure
          try {
            const screenshotResult = await mcp.callTool('capture_screenshot', {}, { timeout: 5000 });
            if (screenshotResult?.content) {
              reporter.screenshotCaptured('(screenshot captured)');
            }
          } catch {
            // Screenshot is best-effort
          }

          if (bail) {
            shouldSkipRest = true;
          } else {
            // Skip remaining steps in this file on first failure
            shouldSkipRest = true;
          }
        }
      }

      const fileDuration = performance.now() - fileStart;
      reporter.fileEnd(test.name, filePassed, fileFailed, fileDuration);

      // Cleanup resources
      const cleanupErrors = await cleanup.teardown();
      if (verbose && cleanupErrors.length > 0) {
        for (const err of cleanupErrors) {
          console.log(`   \x1b[33mcleanup warning: ${err}\x1b[0m`);
        }
      }

      if (bail && fileFailed > 0) break;
    }
  } finally {
    await mcp.close();
  }

  const totalDuration = performance.now() - totalStart;
  const success = reporter.summary(totalDuration);

  return success;
}

/**
 * List test files without running them.
 */
export function listTests(paths, filter) {
  const files = collectTestFiles(paths, filter);

  if (files.length === 0) {
    console.log('No test files found.');
    return;
  }

  console.log(`\nFound ${files.length} test file(s):\n`);
  for (const f of files) {
    try {
      const test = parseTestFile(f);
      const tags = test.tags.length > 0 ? ` [${test.tags.join(', ')}]` : '';
      console.log(`  ${test.name}${tags} — ${test.steps.length} steps`);
      console.log(`    ${f}`);
    } catch (err) {
      console.log(`  \x1b[31m${f}: ${err.message}\x1b[0m`);
    }
  }
  console.log('');
}

/**
 * Validate test files without running them.
 */
export function validateTests(paths, filter) {
  const files = collectTestFiles(paths, filter);

  if (files.length === 0) {
    console.log('No test files found.');
    return true;
  }

  let allValid = true;

  console.log(`\nValidating ${files.length} test file(s):\n`);
  for (const f of files) {
    try {
      const test = parseTestFile(f);
      const issues = validateTest(test);
      if (issues.length === 0) {
        console.log(`  \x1b[32m\u2713\x1b[0m ${test.name} — ${test.steps.length} steps`);
      } else {
        console.log(`  \x1b[33m!\x1b[0m ${test.name}`);
        for (const issue of issues) {
          console.log(`    \x1b[33m- ${issue}\x1b[0m`);
        }
        allValid = false;
      }
    } catch (err) {
      console.log(`  \x1b[31m\u2717\x1b[0m ${f}`);
      console.log(`    \x1b[31m${err.message}\x1b[0m`);
      allValid = false;
    }
  }
  console.log('');

  return allValid;
}

/**
 * Collect YAML test files from paths.
 */
function collectTestFiles(paths, filter) {
  const files = [];

  for (const p of paths) {
    const resolved = resolve(p);
    try {
      const stat = statSync(resolved);
      if (stat.isDirectory()) {
        const entries = readdirSync(resolved);
        for (const entry of entries) {
          if (entry.match(/\.(yaml|yml)$/i)) {
            files.push(join(resolved, entry));
          }
        }
      } else if (stat.isFile()) {
        files.push(resolved);
      }
    } catch {
      console.error(`\x1b[33mWarning: path not found: ${p}\x1b[0m`);
    }
  }

  if (filter) {
    const filterLower = filter.toLowerCase();
    return files.filter(f => {
      const name = f.replace(/\\/g, '/').split('/').pop().toLowerCase();
      return name.includes(filterLower);
    });
  }

  return files.sort();
}
