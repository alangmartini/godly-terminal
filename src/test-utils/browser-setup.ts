/**
 * Shared setup for Vitest Browser Mode tests.
 *
 * Mocks Tauri APIs that are unavailable in a standalone browser context.
 * Runs before each browser test file via `setupFiles` in vitest.workspace.ts.
 */
import { vi } from 'vitest';

// ── @tauri-apps/api/core ────────────────────────────────────────────
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(null),
  transformCallback: vi.fn(),
}));

// ── @tauri-apps/api/event ───────────────────────────────────────────
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
  emit: vi.fn().mockResolvedValue(undefined),
  once: vi.fn().mockResolvedValue(() => {}),
}));

// ── @tauri-apps/plugin-store ────────────────────────────────────────
vi.mock('@tauri-apps/plugin-store', () => ({
  Store: class MockStore {
    async get() { return null; }
    async set() {}
    async save() {}
    async load() {}
  },
  load: vi.fn().mockResolvedValue({
    get: vi.fn().mockResolvedValue(null),
    set: vi.fn().mockResolvedValue(undefined),
    save: vi.fn().mockResolvedValue(undefined),
  }),
}));

// ── @tauri-apps/plugin-dialog ───────────────────────────────────────
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn().mockResolvedValue(null),
  save: vi.fn().mockResolvedValue(null),
  message: vi.fn().mockResolvedValue(undefined),
  ask: vi.fn().mockResolvedValue(false),
  confirm: vi.fn().mockResolvedValue(false),
}));

// ── @tauri-apps/plugin-notification ─────────────────────────────────
vi.mock('@tauri-apps/plugin-notification', () => ({
  sendNotification: vi.fn(),
  requestPermission: vi.fn().mockResolvedValue('granted'),
  isPermissionGranted: vi.fn().mockResolvedValue(true),
}));

// ── @tauri-apps/plugin-opener ───────────────────────────────────────
vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: vi.fn().mockResolvedValue(undefined),
}));
