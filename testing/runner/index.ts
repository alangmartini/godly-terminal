import { resolve } from 'path';
import { McpClient } from './mcp-client.js';
import { ContractRunner } from './contract-runner.js';

async function main() {
  const args = process.argv.slice(2);

  if (args.length === 0 || args.includes('--help')) {
    console.log('Usage: godly-test-runner <contract.json> [--artifacts-dir <dir>] [--mcp-binary <path>]');
    console.log('');
    console.log('Options:');
    console.log('  --artifacts-dir  Directory for test artifacts (default: ./artifacts)');
    console.log('  --mcp-binary     Path to godly-mcp binary');
    process.exit(args.includes('--help') ? 0 : 1);
  }

  const contractPath = resolve(args[0]);
  const artifactsIdx = args.indexOf('--artifacts-dir');
  const artifactsDir = artifactsIdx > -1 ? resolve(args[artifactsIdx + 1]) : resolve('./artifacts');
  const mcpIdx = args.indexOf('--mcp-binary');
  const mcpBinary = mcpIdx > -1 ? resolve(args[mcpIdx + 1]) : undefined;

  console.log(`[runner] Contract: ${contractPath}`);
  console.log(`[runner] Artifacts: ${artifactsDir}`);

  const client = new McpClient(mcpBinary);

  try {
    console.log('[runner] Connecting to MCP...');
    await client.connect();
    console.log('[runner] Connected.');

    const runner = new ContractRunner(client, artifactsDir);
    const result = await runner.run(contractPath);

    console.log('');
    console.log('='.repeat(60));
    console.log(`Contract: ${result.contract_id}`);
    console.log(`Result: ${result.passed ? 'PASS' : 'FAIL'}`);
    console.log(`Duration: ${result.total_duration_ms}ms`);
    console.log(`Steps: ${result.steps.filter(s => s.passed).length}/${result.steps.length} passed`);

    if (!result.passed) {
      const firstFail = result.steps.find(s => !s.passed && !s.skipped);
      if (firstFail) {
        console.log(`First failure: ${firstFail.step_id}`);
        if (firstFail.error) console.log(`  Error: ${firstFail.error}`);
        const failedAssertions = firstFail.assertions.filter(a => !a.passed);
        for (const a of failedAssertions) {
          console.log(`  Assertion ${a.assertion_id}: expected=${JSON.stringify(a.expected)}, actual=${JSON.stringify(a.actual)}`);
        }
      }
    }

    if (result.artifact_dir) {
      console.log(`Artifacts: ${result.artifact_dir}`);
    }
    console.log('='.repeat(60));

    process.exit(result.passed ? 0 : 1);
  } catch (e: unknown) {
    const message = e instanceof Error ? e.message : String(e);
    console.error(`[runner] Fatal: ${message}`);
    process.exit(2);
  } finally {
    await client.disconnect();
  }
}

main();
