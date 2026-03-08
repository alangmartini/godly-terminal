/**
 * Bug #611: Quick Claude shortcut label should be "Quick Prompt" in Settings → Shortcuts.
 *
 * The feature formerly called "Quick Claude" has been renamed to "Quick Prompt", but
 * the shortcut label in DEFAULT_SHORTCUTS still says "Quick Claude". This makes it
 * impossible to find via the shortcuts search when searching "prompt", and shows the
 * wrong name in the UI.
 *
 * Additionally, the dialog title in dialogs.ts and the flow engine display name still
 * say "Quick Claude" instead of "Quick Prompt".
 */
import { describe, it, expect, vi } from 'vitest';
import { DEFAULT_SHORTCUTS } from './keybinding-store';

// Mock localStorage (required by keybinding-store module init)
const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
});

describe('Bug #611: Quick Claude → Quick Prompt rename', () => {
  it('has a shortcut entry for tabs.quickClaude', () => {
    // Bug #611: The shortcut must exist in DEFAULT_SHORTCUTS
    const def = DEFAULT_SHORTCUTS.find((d) => d.id === 'tabs.quickClaude');
    expect(def).toBeDefined();
  });

  it('shortcut label should say "Quick Prompt" not "Quick Claude"', () => {
    // Bug #611: The label shown in Settings → Shortcuts must be "Quick Prompt"
    const def = DEFAULT_SHORTCUTS.find((d) => d.id === 'tabs.quickClaude')!;
    expect(def.label).toBe('Quick Prompt');
  });

  it('searching "prompt" in shortcuts should find the quick prompt shortcut', () => {
    // Bug #611: User searched "prompt" in the shortcuts filter and couldn't find it
    // because the label was "Quick Claude" not "Quick Prompt"
    const query = 'prompt';
    const matches = DEFAULT_SHORTCUTS.filter((d) =>
      d.label.toLowerCase().includes(query)
    );
    expect(matches.length).toBeGreaterThanOrEqual(1);
    expect(matches.some((d) => d.id === 'tabs.quickClaude')).toBe(true);
  });

  it('no user-facing shortcut label should contain "Quick Claude"', () => {
    // Bug #611: All references to "Quick Claude" in shortcut labels should be renamed
    const badLabels = DEFAULT_SHORTCUTS.filter((d) =>
      d.label.toLowerCase().includes('quick claude')
    );
    expect(badLabels).toEqual([]);
  });
});

describe('Bug #611: Quick Prompt flow engine label', () => {
  it('flow engine category label should say "Quick Prompt" not "Quick Claude"', async () => {
    // Bug #611: NODE_CATEGORY_LABELS['quick-claude'] says "Quick Claude"
    const { NODE_CATEGORY_LABELS } = await import('../flow-engine/types');
    expect(NODE_CATEGORY_LABELS['quick-claude']).toBe('Quick Prompt');
  });
});
