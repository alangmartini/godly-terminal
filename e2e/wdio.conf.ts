import os from 'os';
import path from 'path';
import { ChildProcess, spawn } from 'child_process';
import { clearAppData } from './helpers/persistence';

// Skip WDIO 9.x's built-in browser driver management — we manage tauri-driver ourselves.
// This also hardcodes the connection to localhost:4321.
process.env.WDIO_SKIP_DRIVER_SETUP = '1';

let tauriDriver: ChildProcess;

export const config: WebdriverIO.Config = {
  specs: ['./specs/**/*.e2e.ts'],
  maxInstances: 1,
  capabilities: [
    {
      // No browserName — tauri-driver handles the app launch via tauri:options
      'tauri:options': {
        application: path.resolve(
          'src-tauri/target/debug/godly-terminal.exe'
        ),
      },
    } as any,
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

  // Build the debug binary unless SKIP_BUILD is set
  async onPrepare() {
    if (!process.env.SKIP_BUILD) {
      const { execFileSync } = await import('child_process');
      console.log('Building Tauri debug binary...');
      execFileSync('npm', ['run', 'tauri', 'build', '--', '--debug', '--no-bundle'], {
        stdio: 'inherit',
        timeout: 600000,
        shell: true,
      });
    }
  },

  // Spawn tauri-driver before each worker session.
  // Uses the cargo-installed binary which properly bridges WebDriver to WebView2.
  async beforeSession() {
    // Clear persisted app data so the app starts with a fresh default workspace
    clearAppData();

    // Ensure edgedriver is downloaded and find its path
    const edgedriver = await import('edgedriver');
    const edgedriverProcess = await edgedriver.start({ port: 9516 });
    edgedriverProcess.kill();
    const tempDir = process.env.TEMP || process.env.TMP || '/tmp';
    const nativeDriverPath = path.join(tempDir, 'msedgedriver.exe');

    const driverPath = path.resolve(os.homedir(), '.cargo', 'bin', 'tauri-driver.exe');
    tauriDriver = spawn(
      driverPath,
      ['--port', '4321', '--native-driver', nativeDriverPath],
      { stdio: [null, process.stdout, process.stderr] }
    );

    // Wait for tauri-driver to start listening
    await new Promise<void>((resolve) => setTimeout(resolve, 3000));
  },

  afterSession() {
    if (tauriDriver) {
      tauriDriver.kill();
    }
  },
};
