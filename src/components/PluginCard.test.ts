// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { createPluginCard } from './PluginCard';
import type { GodlyPlugin } from '../plugins/types';

function makePlugin(overrides: Partial<GodlyPlugin> = {}): GodlyPlugin {
  return {
    id: 'test',
    name: 'Test Plugin',
    description: 'A test plugin',
    version: '1.0.0',
    init: vi.fn(),
    ...overrides,
  };
}

describe('createPluginCard', () => {
  it('renders plugin name and version', () => {
    const card = createPluginCard({
      plugin: makePlugin(),
      isBuiltin: false,
      isEnabled: false,
      isInstalled: true,
    });

    expect(card.querySelector('.plugin-card-name')?.textContent).toBe('Test Plugin');
    expect(card.querySelector('.plugin-card-version')?.textContent).toBe('v1.0.0');
  });

  it('shows Built-in badge for builtin plugins', () => {
    const card = createPluginCard({
      plugin: makePlugin(),
      isBuiltin: true,
      isEnabled: false,
      isInstalled: true,
    });

    const badge = card.querySelector('.plugin-card-builtin-badge');
    expect(badge).not.toBeNull();
    expect(badge?.textContent).toBe('Built-in');
  });

  it('does not show Built-in badge for external plugins', () => {
    const card = createPluginCard({
      plugin: makePlugin(),
      isBuiltin: false,
      isEnabled: false,
      isInstalled: true,
    });

    expect(card.querySelector('.plugin-card-builtin-badge')).toBeNull();
  });

  it('renders toggle switch when installed', () => {
    const card = createPluginCard({
      plugin: makePlugin(),
      isBuiltin: false,
      isEnabled: true,
      isInstalled: true,
    });

    const toggle = card.querySelector<HTMLInputElement>('.plugin-toggle input');
    expect(toggle).not.toBeNull();
    expect(toggle?.checked).toBe(true);
  });

  it('calls onToggle when toggle changes', () => {
    const onToggle = vi.fn();
    const card = createPluginCard({
      plugin: makePlugin(),
      isBuiltin: false,
      isEnabled: false,
      isInstalled: true,
      onToggle,
    });

    const toggle = card.querySelector<HTMLInputElement>('.plugin-toggle input')!;
    toggle.checked = true;
    toggle.dispatchEvent(new Event('change'));

    expect(onToggle).toHaveBeenCalledWith(true);
  });

  it('shows Uninstall button for non-builtin installed plugins', () => {
    const card = createPluginCard({
      plugin: makePlugin(),
      isBuiltin: false,
      isEnabled: false,
      isInstalled: true,
    });

    const uninstallBtn = card.querySelector('.plugin-card-btn-danger');
    expect(uninstallBtn).not.toBeNull();
    expect(uninstallBtn?.textContent).toBe('Uninstall');
  });

  it('hides Uninstall button for builtin plugins', () => {
    const card = createPluginCard({
      plugin: makePlugin(),
      isBuiltin: true,
      isEnabled: false,
      isInstalled: true,
    });

    expect(card.querySelector('.plugin-card-btn-danger')).toBeNull();
  });

  it('shows Install button when not installed', () => {
    const card = createPluginCard({
      registryEntry: { id: 'new-plugin', repo: 'org/repo', description: 'New', author: 'Author' },
      isBuiltin: false,
      isEnabled: false,
      isInstalled: false,
    });

    const installBtn = card.querySelector('.plugin-card-btn-primary');
    expect(installBtn).not.toBeNull();
    expect(installBtn?.textContent).toBe('Install');
  });

  it('shows Installing... when installing', () => {
    const card = createPluginCard({
      registryEntry: { id: 'new-plugin', repo: 'org/repo', description: 'New', author: 'Author' },
      isBuiltin: false,
      isEnabled: false,
      isInstalled: false,
      installing: true,
    });

    const installBtn = card.querySelector<HTMLButtonElement>('.plugin-card-btn-primary');
    expect(installBtn?.textContent).toBe('Installing...');
    expect(installBtn?.disabled).toBe(true);
  });

  it('renders tags when provided', () => {
    const card = createPluginCard({
      plugin: makePlugin(),
      manifest: { id: 'test', name: 'Test', description: 'd', author: 'a', version: '1', tags: ['sound', 'ai'] },
      isBuiltin: false,
      isEnabled: false,
      isInstalled: true,
    });

    const tags = card.querySelectorAll('.plugin-tag');
    expect(tags).toHaveLength(2);
    expect(tags[0].textContent).toBe('sound');
    expect(tags[1].textContent).toBe('ai');
  });

  it('renders settings section when enabled with renderSettings', () => {
    const settingsEl = document.createElement('div');
    settingsEl.textContent = 'Settings content';
    const plugin = makePlugin({ renderSettings: () => settingsEl });

    const card = createPluginCard({
      plugin,
      isBuiltin: true,
      isEnabled: true,
      isInstalled: true,
    });

    const settingsSection = card.querySelector('.plugin-card-settings');
    expect(settingsSection).not.toBeNull();
    expect(settingsSection?.textContent).toContain('Settings content');
  });

  it('does not render settings section when disabled', () => {
    const plugin = makePlugin({ renderSettings: () => document.createElement('div') });

    const card = createPluginCard({
      plugin,
      isBuiltin: true,
      isEnabled: false,
      isInstalled: true,
    });

    expect(card.querySelector('.plugin-card-settings')).toBeNull();
  });

  it('renders description', () => {
    const card = createPluginCard({
      plugin: makePlugin({ description: 'Does great things' }),
      isBuiltin: false,
      isEnabled: false,
      isInstalled: true,
    });

    expect(card.querySelector('.plugin-card-description')?.textContent).toBe('Does great things');
  });

  it('renders fallback icon when no iconDataUrl provided', () => {
    const card = createPluginCard({
      plugin: makePlugin(),
      isBuiltin: false,
      isEnabled: false,
      isInstalled: true,
    });

    const iconEl = card.querySelector('.plugin-card-icon');
    expect(iconEl?.querySelector('svg')).not.toBeNull();
  });

  it('renders image when iconDataUrl provided', () => {
    const card = createPluginCard({
      plugin: makePlugin(),
      isBuiltin: false,
      isEnabled: false,
      isInstalled: true,
      iconDataUrl: 'data:image/png;base64,abc',
    });

    const img = card.querySelector<HTMLImageElement>('.plugin-card-icon img');
    expect(img).not.toBeNull();
    expect(img?.src).toBe('data:image/png;base64,abc');
  });
});
