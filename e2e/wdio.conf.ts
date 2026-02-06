import path from 'path';
import { ChildProcess, spawn } from 'child_process';

let tauriDriver: ChildProcess;

export const config: WebdriverIO.Config = {
  specs: ['./e2e/specs/**/*.e2e.ts'],
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
      const { execSync } = await import('child_process');
      console.log('Building Tauri debug binary...');
      execSync('npm run tauri build -- --debug --no-bundle', {
        stdio: 'inherit',
        timeout: 600000,
      });
    }

    // Ensure edgedriver is downloaded
    const edgedriver = await import('edgedriver');
    const edgedriverProcess = await edgedriver.start({ port: 9516 });
    // We just need it to download the binary; kill the process
    edgedriverProcess.kill();

    // Find the edgedriver binary path
    const tempDir = process.env.TEMP || process.env.TMP || '/tmp';
    const nativeDriverPath = path.join(tempDir, 'msedgedriver.exe');

    // Spawn tauri-driver
    const tauriDriverBin = path.resolve(
      'node_modules/.bin/tauri-driver'
    );
    tauriDriver = spawn(tauriDriverBin, [
      '--port', '4444',
      '--native-driver', nativeDriverPath,
      '--native-port', '9516',
    ], {
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    tauriDriver.stderr?.on('data', (data: Buffer) => {
      const msg = data.toString();
      if (msg.includes('error') || msg.includes('Error')) {
        console.error('[tauri-driver]', msg.trim());
      }
    });

    // Wait for tauri-driver to be ready
    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('tauri-driver did not start within 15s'));
      }, 15000);

      const checkReady = async () => {
        try {
          const res = await fetch('http://localhost:4444/status');
          if (res.ok) {
            clearTimeout(timeout);
            resolve();
            return;
          }
        } catch {
          // Not ready yet
        }
        setTimeout(checkReady, 500);
      };
      checkReady();
    });

    console.log('tauri-driver is ready on port 4444');
  },

  afterSession() {
    if (tauriDriver) {
      tauriDriver.kill();
    }
  },
};
