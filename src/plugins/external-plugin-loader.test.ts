import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock @tauri-apps/api/core
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

// Mock URL.createObjectURL / revokeObjectURL
const mockCreateObjectURL = vi.fn().mockReturnValue('blob:test-url');
const mockRevokeObjectURL = vi.fn();
vi.stubGlobal('URL', {
  createObjectURL: mockCreateObjectURL,
  revokeObjectURL: mockRevokeObjectURL,
});

import { loadExternalPlugin, loadPluginIconDataUrl } from './external-plugin-loader';

describe('loadExternalPlugin', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('reads JS via invoke and creates a Blob URL', async () => {
    const fakeJs = 'export default class P { id="t"; name="T"; description="d"; version="1"; init(){} }';
    mockInvoke.mockResolvedValueOnce(fakeJs);

    // dynamic import will fail in test environment, but we can verify the invoke call
    try {
      await loadExternalPlugin('test-plugin');
    } catch {
      // Expected: dynamic import of blob URL won't work in vitest
    }

    expect(mockInvoke).toHaveBeenCalledWith('read_plugin_js', { pluginId: 'test-plugin' });
    expect(mockCreateObjectURL).toHaveBeenCalled();
    // Blob should have been created with the JS content
    const blobArg = mockCreateObjectURL.mock.calls[0][0];
    expect(blobArg).toBeInstanceOf(Blob);
  });

  it('throws when invoke fails', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('Plugin not found'));

    await expect(loadExternalPlugin('missing')).rejects.toThrow('Plugin not found');
  });
});

describe('loadPluginIconDataUrl', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns data URL when icon exists', async () => {
    mockInvoke.mockResolvedValueOnce('iVBORw0KGgo=');

    const result = await loadPluginIconDataUrl('test-plugin');

    expect(mockInvoke).toHaveBeenCalledWith('read_plugin_icon', { pluginId: 'test-plugin' });
    expect(result).toBe('data:image/png;base64,iVBORw0KGgo=');
  });

  it('returns null when no icon exists', async () => {
    mockInvoke.mockResolvedValueOnce(null);

    const result = await loadPluginIconDataUrl('test-plugin');

    expect(result).toBeNull();
  });

  it('returns null on error', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('read failed'));

    const result = await loadPluginIconDataUrl('test-plugin');

    expect(result).toBeNull();
  });
});
