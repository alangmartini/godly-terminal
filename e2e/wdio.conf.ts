import path from 'path';

export const config: WebdriverIO.Config = {
  specs: ['./specs/**/*.e2e.ts'],
  maxInstances: 1,
  capabilities: [
    {
      'browserName': 'wry',
      'tauri:options': {
        application: path.resolve(
          'src-tauri/target/debug/godly-terminal.exe'
        ),
      },
    },
  ],
  logLevel: 'warn',
  bail: 0,
  waitforTimeout: 30000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: {
    ui: 'bdd',
    timeout: 120000,
  },
  tsConfigPath: './tsconfig.e2e.json',

  async onPrepare() {
    // Build the debug binary unless SKIP_BUILD is set
    if (!process.env.SKIP_BUILD) {
      const { execFileSync } = await import('child_process');
      console.log('Building Tauri debug binary...');
      execFileSync('npm', ['run', 'tauri', 'build', '--', '--debug', '--no-bundle'], {
        stdio: 'inherit',
        timeout: 600000,
        shell: true,
      });
    }

    // Ensure edgedriver is downloaded, then stop it (we just need the binary)
    const edgedriver = await import('edgedriver');
    const edgedriverProcess = await edgedriver.start({ port: 9516 });
    edgedriverProcess.kill();

    // Find the edgedriver binary path
    const tempDir = process.env.TEMP || process.env.TMP || '/tmp';
    const nativeDriverPath = path.join(tempDir, 'msedgedriver.exe');

    // Start tauri-driver using the NAPI programmatic API (runs in background)
    const { run, waitTauriDriverReady } = await import('@crabnebula/tauri-driver');
    run(
      ['--port', '4444', '--native-driver', nativeDriverPath, '--native-port', '9516'],
      'tauri-driver'
    ).catch((err: Error) => {
      // run() resolves when the driver exits; errors during normal shutdown are expected
      if (!err.message?.includes('interrupted')) {
        console.error('[tauri-driver]', err.message);
      }
    });

    // Wait for tauri-driver to be listening
    await waitTauriDriverReady('127.0.0.1', 4444, 200);
    console.log('tauri-driver is ready on port 4444');
  },
};
