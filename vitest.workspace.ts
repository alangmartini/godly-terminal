import { defineWorkspace } from 'vitest/config';

export default defineWorkspace([
  {
    extends: './vite.config.ts',
    test: {
      name: 'unit',
      environment: 'node',
      include: ['src/**/*.test.ts'],
      exclude: ['src/**/*.browser.test.ts'],
      testTimeout: 10_000,
      hookTimeout: 10_000,
    },
  },
  {
    extends: './vite.config.ts',
    optimizeDeps: {
      include: [
        '@tauri-apps/api/core',
        '@tauri-apps/api/event',
        '@tauri-apps/plugin-store',
        '@tauri-apps/plugin-dialog',
        '@tauri-apps/plugin-notification',
        '@tauri-apps/plugin-opener',
      ],
    },
    test: {
      name: 'browser',
      include: ['src/**/*.browser.test.ts'],
      testTimeout: 15_000,
      hookTimeout: 15_000,
      setupFiles: ['src/test-utils/browser-setup.ts'],
      browser: {
        enabled: true,
        name: 'chromium',
        provider: 'playwright',
        headless: true,
      },
    },
  },
  {
    test: {
      name: 'integration',
      environment: 'node',
      include: ['integration/tests/**/*.integration.test.ts'],
      testTimeout: 120_000,
      hookTimeout: 30_000,
    },
  },
]);
