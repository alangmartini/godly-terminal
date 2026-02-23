// @vitest-environment jsdom
/**
 * Bug #289: Notification sounds overlap — multiple sounds play simultaneously
 *
 * Three sub-bugs:
 * 1. PeonPing double-play: emitMcpNotify fires both classified + notification events,
 *    causing PeonPing to play two sounds for one notification.
 * 2. No global sound throttle: simultaneous notifications from different terminals
 *    each play their own sound with no cross-terminal debounce.
 * 3. No audio queue in playBuffer: every call creates a new AudioBufferSourceNode
 *    and starts immediately — no rate limiting or overlap prevention.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { PluginEventBus } from './event-bus';
import { PeonPingPlugin } from './peon-ping/index';
import type { PluginContext, PluginEventType, SoundPackManifest } from './types';

// Mock localStorage
const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
  clear: () => storage.clear(),
});

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(''),
}));

// Mock @tauri-apps/plugin-opener
vi.mock('@tauri-apps/plugin-opener', () => ({
  revealItemInDir: vi.fn(),
}));

// Mock notification-sound module
vi.mock('../services/notification-sound', () => ({
  isBuiltinPreset: (s: string) => ['chime', 'bell', 'ping'].includes(s),
  isCustomPreset: (s: string) => s.startsWith('custom:'),
  playNotificationSound: vi.fn(),
  playBuffer: vi.fn(),
  getSharedAudioContext: vi.fn().mockReturnValue({
    createBufferSource: vi.fn().mockReturnValue({
      connect: vi.fn(),
      start: vi.fn(),
      buffer: null,
    }),
    createGain: vi.fn().mockReturnValue({
      connect: vi.fn(),
      gain: { value: 1 },
    }),
    destination: {},
  }),
  BUILTIN_PRESETS: ['chime', 'bell', 'ping'],
}));

function createMockAudioBuffer(): AudioBuffer {
  return { duration: 1, length: 44100, sampleRate: 44100, numberOfChannels: 1 } as unknown as AudioBuffer;
}

function createMockContext(overrides: Partial<PluginContext> = {}): PluginContext {
  const handlers = new Map<PluginEventType, ((e: any) => void)[]>();

  return {
    on: vi.fn((type: PluginEventType, handler: (e: any) => void) => {
      const list = handlers.get(type) ?? [];
      list.push(handler);
      handlers.set(type, list);
      return () => {
        const idx = list.indexOf(handler);
        if (idx >= 0) list.splice(idx, 1);
      };
    }),
    readSoundFile: vi.fn().mockResolvedValue(''),
    listSoundPackFiles: vi.fn().mockResolvedValue([]),
    listSoundPacks: vi.fn().mockResolvedValue([]),
    getAudioContext: vi.fn().mockReturnValue({
      decodeAudioData: vi.fn().mockResolvedValue(createMockAudioBuffer()),
    }),
    getSetting: vi.fn().mockImplementation((_key: string, defaultValue: any) => defaultValue),
    setSetting: vi.fn(),
    playSound: vi.fn(),
    invoke: vi.fn().mockResolvedValue(undefined),
    showToast: vi.fn(),
    ...overrides,
  };
}

describe('Bug #289: Notification sound overlap', () => {
  beforeEach(() => {
    storage.clear();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('Sub-bug 1: PeonPing double-play on classified mcp-notify events', () => {
    it('should play only ONE sound when a classified event (e.g. agent:error) is emitted via emitMcpNotify', async () => {
      // Bug #289: emitMcpNotify emits both the classified event AND a generic
      // 'notification' event. PeonPing subscribes to all categories, so it
      // handles both — playing TWO sounds for a single notification.

      const bus = new PluginEventBus();
      const plugin = new PeonPingPlugin();
      plugin.setBus(bus);

      const playSoundSpy = vi.fn();

      // Create a pack with sounds for both 'error' and 'notification' categories
      const packs: SoundPackManifest[] = [{
        id: 'test-pack',
        name: 'Test',
        description: 'test',
        author: 'test',
        version: '1.0.0',
        sounds: {
          error: ['error.wav'],
          notification: ['notif.wav'],
        },
      }];

      // Build a context where the plugin's on() actually wires into the real bus
      const eventHandlers = new Map<PluginEventType, ((e: any) => void)[]>();

      const ctx = createMockContext({
        on: vi.fn((type: PluginEventType, handler: (e: any) => void) => {
          // Wire handler to real bus AND track locally
          const unsub = bus.on(type, handler);
          const list = eventHandlers.get(type) ?? [];
          list.push(handler);
          eventHandlers.set(type, list);
          return unsub;
        }),
        listSoundPacks: vi.fn().mockResolvedValue(packs),
        getSetting: vi.fn().mockImplementation((key: string, defaultValue: any) => {
          if (key === 'activePack') return 'test-pack';
          if (key === 'volume') return 0.7;
          if (key.startsWith('category.')) return true;
          return defaultValue;
        }),
        readSoundFile: vi.fn().mockResolvedValue(
          // Minimal valid WAV file encoded as base64 (44 bytes RIFF header + silence)
          btoa(String.fromCharCode(...new Uint8Array(44)))
        ),
        getAudioContext: vi.fn().mockReturnValue({
          decodeAudioData: vi.fn().mockResolvedValue(createMockAudioBuffer()),
        }),
        playSound: playSoundSpy,
      });

      await plugin.init(ctx);
      plugin.enable();

      // Now emit an mcp-notify that classifies as 'agent:error'
      // This should result in exactly ONE sound, not two
      bus.emitMcpNotify('terminal-1', 'Build failed with errors');

      // BUG: PeonPing fires for 'agent:error' AND 'notification', so playSound
      // is called TWICE. The expected correct behavior is exactly 1 call.
      expect(playSoundSpy).toHaveBeenCalledTimes(1);
    });

    it('should fire the notification handler only ONCE when emitMcpNotify classifies as non-notification', () => {
      // Bug #289: emitMcpNotify emits a 'notification' event in addition to
      // the classified event. A plugin subscribing to both will get called twice.

      const bus = new PluginEventBus();
      const allEvents: string[] = [];

      // Subscribe to both the classified type and the generic notification
      bus.on('agent:task-complete', (e) => allEvents.push(e.type));
      bus.on('notification', (e) => allEvents.push(e.type));

      bus.emitMcpNotify('t1', 'Task completed successfully');

      // BUG: The bus emits both 'agent:task-complete' AND 'notification',
      // so a listener on both gets called twice. A single mcp-notify should
      // result in at most one sound-eligible event per listener.
      // If a plugin subscribes to 'agent:task-complete', it shouldn't also
      // receive a redundant 'notification' event for the same mcp-notify.
      expect(allEvents).toHaveLength(1);
    });
  });

  describe('Sub-bug 2: No global sound throttle across terminals', () => {
    it('should coalesce sounds when multiple terminals notify within a short window', () => {
      // Bug #289: When multiple terminals go idle simultaneously, each fires
      // its own notification. The store debounces per-terminal (2s) but there
      // is no global cross-terminal debounce.

      // Re-import fresh store
      vi.resetModules();
      vi.stubGlobal('localStorage', {
        getItem: (key: string) => storage.get(key) ?? null,
        setItem: (key: string, value: string) => storage.set(key, value),
        removeItem: (key: string) => storage.delete(key),
        clear: () => storage.clear(),
      });

      // Simulate the notification flow from App.ts where each recordNotify
      // triggers a sound play
      const soundPlayCount = { value: 0 };

      // Simulate 5 terminals going idle at the exact same time
      const terminals = ['term-1', 'term-2', 'term-3', 'term-4', 'term-5'];

      // Each terminal's recordNotify returns true (not debounced) because
      // each has its own independent debounce timer
      for (const _termId of terminals) {
        // In real code: notificationStore.recordNotify(termId) returns true
        // because it's per-terminal debounce. Then playNotificationSound is called.
        // We simulate the effect:
        soundPlayCount.value++;
      }

      // BUG: All 5 sounds play simultaneously. Expected: at most 1 sound
      // (or sounds spaced out with minimum interval between them).
      expect(soundPlayCount.value).toBeLessThanOrEqual(1);
    });

    it('notificationStore.recordNotify should have a global debounce across all terminals', async () => {
      // Bug #289: recordNotify only debounces per terminal_id.
      // Multiple different terminals can all return true simultaneously.

      vi.resetModules();
      vi.stubGlobal('localStorage', {
        getItem: (key: string) => storage.get(key) ?? null,
        setItem: (key: string, value: string) => storage.set(key, value),
        removeItem: (key: string) => storage.delete(key),
        clear: () => storage.clear(),
      });

      // Need to re-mock notification-sound for the fresh module
      vi.mock('../services/notification-sound', () => ({
        isBuiltinPreset: (s: string) => ['chime', 'bell'].includes(s),
        isCustomPreset: (s: string) => s.startsWith('custom:'),
      }));

      const { notificationStore } = await import('../state/notification-store');

      // Fire recordNotify for 5 different terminals in rapid succession
      const results: boolean[] = [];
      for (let i = 0; i < 5; i++) {
        results.push(notificationStore.recordNotify(`terminal-${i}`));
      }

      // BUG: All 5 return true because debounce is per-terminal only.
      // Expected: Only the first should return true (global debounce),
      // or at most 1-2 within a short window.
      const playedCount = results.filter(Boolean).length;
      expect(playedCount).toBeLessThanOrEqual(1);
    });
  });

  describe('Sub-bug 3: playBuffer has no overlap prevention', () => {
    it('should not allow multiple AudioBufferSourceNodes to start simultaneously', () => {
      // Bug #289: playBuffer creates a new source and calls start() every time.
      // Concurrent calls produce overlapping audio with no rate-limiting.

      const startCalls: number[] = [];
      const mockCtx = {
        currentTime: 0,
        createBufferSource: vi.fn().mockImplementation(() => ({
          connect: vi.fn(),
          start: vi.fn().mockImplementation(() => startCalls.push(Date.now())),
          buffer: null,
        })),
        createGain: vi.fn().mockReturnValue({
          connect: vi.fn(),
          gain: { value: 1, setValueAtTime: vi.fn(), exponentialRampToValueAtTime: vi.fn() },
        }),
        destination: {},
        state: 'running',
        resume: vi.fn(),
      };

      // Simulate what playBuffer does: create source → connect → start
      // Call it 5 times in rapid succession (as happens with overlapping notifications)
      for (let i = 0; i < 5; i++) {
        const source = mockCtx.createBufferSource();
        const gain = mockCtx.createGain();
        source.connect(gain);
        gain.connect(mockCtx.destination);
        source.start();
      }

      // BUG: All 5 sounds start simultaneously. Expected: only 1 should play,
      // or sounds should be queued with minimum spacing.
      expect(startCalls).toHaveLength(1);
    });
  });
});
