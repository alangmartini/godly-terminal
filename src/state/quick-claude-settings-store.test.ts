// @vitest-environment jsdom
import { describe, it, expect, beforeEach, vi } from 'vitest';

// Reset localStorage and re-import store for each test
let store: typeof import('./quick-claude-settings-store');

beforeEach(() => {
  localStorage.clear();
  vi.resetModules();
});

async function getStore() {
  store = await import('./quick-claude-settings-store');
  return store.quickClaudeSettingsStore;
}

describe('QuickClaudeSettingsStore', () => {
  describe('built-in presets', () => {
    it('seeds 3 built-in presets on first run', async () => {
      const s = await getStore();
      const presets = s.getPresets();
      expect(presets.length).toBe(3);
      const names = presets.map(p => p.name);
      expect(names).toContain('Solo Claude');
      expect(names).toContain('Solo Codex');
      expect(names).toContain('Claude + Codex Split');
    });

    it('marks Solo Claude as default', async () => {
      const s = await getStore();
      const def = s.getDefaultPreset();
      expect(def?.name).toBe('Solo Claude');
      expect(def?.isDefault).toBe(true);
    });

    it('does not duplicate built-ins when already present', async () => {
      const s1 = await getStore();
      expect(s1.getPresets().length).toBe(3);
      // Re-import to simulate restart
      vi.resetModules();
      const s2 = await getStore();
      expect(s2.getPresets().length).toBe(3);
    });
  });

  describe('preset CRUD', () => {
    it('adds a custom preset', async () => {
      const s = await getStore();
      s.addPreset({
        id: 'custom-1',
        name: 'Custom',
        description: 'test',
        layout: 'horizontal',
        agents: [],
        isDefault: false,
        createdAt: Date.now(),
      });
      expect(s.getPresets().length).toBe(4);
      expect(s.getPreset('custom-1')?.name).toBe('Custom');
    });

    it('ignores duplicate add', async () => {
      const s = await getStore();
      s.addPreset({
        id: 'custom-1', name: 'A', description: '', layout: 'single',
        agents: [], isDefault: false, createdAt: 0,
      });
      s.addPreset({
        id: 'custom-1', name: 'B', description: '', layout: 'single',
        agents: [], isDefault: false, createdAt: 0,
      });
      expect(s.getPreset('custom-1')?.name).toBe('A');
    });

    it('updates a preset', async () => {
      const s = await getStore();
      s.updatePreset('builtin-solo-claude', { name: 'Renamed', layout: 'horizontal' });
      const p = s.getPreset('builtin-solo-claude');
      expect(p?.name).toBe('Renamed');
      expect(p?.layout).toBe('horizontal');
    });

    it('setting isDefault clears other defaults', async () => {
      const s = await getStore();
      s.updatePreset('builtin-solo-codex', { isDefault: true });
      expect(s.getPreset('builtin-solo-claude')?.isDefault).toBe(false);
      expect(s.getPreset('builtin-solo-codex')?.isDefault).toBe(true);
    });

    it('deletes a custom preset', async () => {
      const s = await getStore();
      s.addPreset({
        id: 'to-delete', name: 'Delete me', description: '', layout: 'single',
        agents: [], isDefault: false, createdAt: 0,
      });
      s.deletePreset('to-delete');
      expect(s.getPreset('to-delete')).toBeUndefined();
    });

    it('cannot delete built-in presets', async () => {
      const s = await getStore();
      s.deletePreset('builtin-solo-claude');
      expect(s.getPreset('builtin-solo-claude')).toBeDefined();
    });

    it('reassigns default when default preset deleted', async () => {
      const s = await getStore();
      s.addPreset({
        id: 'custom-def', name: 'Def', description: '', layout: 'single',
        agents: [], isDefault: true, createdAt: 0,
      });
      s.deletePreset('custom-def');
      const presets = s.getPresets();
      expect(presets.some(p => p.isDefault)).toBe(true);
    });

    it('duplicates a preset with new IDs', async () => {
      const s = await getStore();
      const copy = s.duplicatePreset('builtin-solo-claude');
      expect(copy).toBeDefined();
      expect(copy!.id).not.toBe('builtin-solo-claude');
      expect(copy!.name).toBe('Solo Claude (Copy)');
      expect(copy!.isDefault).toBe(false);
      expect(copy!.agents[0].id).not.toBe(s.getPreset('builtin-solo-claude')!.agents[0].id);
    });
  });

  describe('agent CRUD', () => {
    it('respects layout max agents', async () => {
      const s = await getStore();
      // Solo Claude is single layout with 1 agent — can't add more
      const agent = s.addAgent('builtin-solo-claude', 'codex');
      expect(agent).toBeUndefined();
    });

    it('adds agent to multi-agent layout', async () => {
      const s = await getStore();
      s.addPreset({
        id: 'grid-test', name: 'Grid', description: '', layout: 'grid',
        agents: [], isDefault: false, createdAt: 0,
      });
      const a1 = s.addAgent('grid-test', 'claude');
      const a2 = s.addAgent('grid-test', 'codex');
      const a3 = s.addAgent('grid-test', 'claude');
      const a4 = s.addAgent('grid-test', 'codex');
      const a5 = s.addAgent('grid-test', 'claude');
      expect(a1).toBeDefined();
      expect(a2).toBeDefined();
      expect(a3).toBeDefined();
      expect(a4).toBeDefined();
      expect(a5).toBeUndefined(); // grid max = 4
    });

    it('updates agent fields', async () => {
      const s = await getStore();
      const preset = s.getPreset('builtin-solo-claude')!;
      const agentId = preset.agents[0].id;
      s.updateAgent('builtin-solo-claude', agentId, { label: 'My Claude', commandOverride: 'test' });
      const updated = s.getPreset('builtin-solo-claude')!.agents[0];
      expect(updated.label).toBe('My Claude');
      expect(updated.commandOverride).toBe('test');
    });

    it('removes an agent', async () => {
      const s = await getStore();
      const preset = s.getPreset('builtin-claude-codex-split')!;
      const agentId = preset.agents[1].id;
      s.removeAgent('builtin-claude-codex-split', agentId);
      expect(s.getPreset('builtin-claude-codex-split')!.agents.length).toBe(1);
    });
  });

  describe('step operations', () => {
    it('updates a step', async () => {
      const s = await getStore();
      const preset = s.getPreset('builtin-solo-claude')!;
      const agent = preset.agents[0];
      const step = agent.steps.find(st => st.type === 'wait-idle')!;
      s.updateStep('builtin-solo-claude', agent.id, step.id, { enabled: false, config: { idleMs: 5000 } });
      const updated = s.getPreset('builtin-solo-claude')!.agents[0].steps.find(st => st.id === step.id)!;
      expect(updated.enabled).toBe(false);
      expect(updated.config.idleMs).toBe(5000);
    });

    it('resets steps to default', async () => {
      const s = await getStore();
      const preset = s.getPreset('builtin-solo-claude')!;
      const agent = preset.agents[0];
      s.updateStep('builtin-solo-claude', agent.id, agent.steps[0].id, { enabled: false });
      s.resetStepsToDefault('builtin-solo-claude', agent.id);
      const refreshed = s.getPreset('builtin-solo-claude')!.agents[0];
      expect(refreshed.steps.every(st => st.enabled)).toBe(true);
      expect(refreshed.steps[0].type).toBe('create-terminal');
    });
  });

  describe('subscriptions', () => {
    it('notifies on changes', async () => {
      const s = await getStore();
      const fn = vi.fn();
      s.subscribe(fn);
      s.updatePreset('builtin-solo-claude', { name: 'test' });
      expect(fn).toHaveBeenCalledTimes(1);
    });

    it('unsubscribes correctly', async () => {
      const s = await getStore();
      const fn = vi.fn();
      const unsub = s.subscribe(fn);
      unsub();
      s.updatePreset('builtin-solo-claude', { name: 'test' });
      expect(fn).not.toHaveBeenCalled();
    });
  });

  describe('localStorage roundtrip', () => {
    it('persists and restores presets', async () => {
      const s1 = await getStore();
      s1.addPreset({
        id: 'roundtrip-test', name: 'RT', description: 'roundtrip', layout: 'horizontal',
        agents: [{
          id: 'a1', toolId: 'claude', label: 'C',
          steps: [{ id: 's1', type: 'delay', enabled: true, config: { ms: 100 } }],
        }],
        isDefault: false, createdAt: 123,
      });

      vi.resetModules();
      const s2 = await getStore();
      const p = s2.getPreset('roundtrip-test');
      expect(p).toBeDefined();
      expect(p!.name).toBe('RT');
      expect(p!.agents[0].steps[0].config.ms).toBe(100);
    });
  });

  describe('setDefault', () => {
    it('changes the default preset', async () => {
      const s = await getStore();
      s.setDefault('builtin-solo-codex');
      expect(s.getDefaultPreset()?.id).toBe('builtin-solo-codex');
      expect(s.getPreset('builtin-solo-claude')?.isDefault).toBe(false);
    });
  });
});
