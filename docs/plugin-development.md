# Plugin Development Guide

This guide covers everything you need to build, test, and publish plugins for Godly Terminal.

## Overview

Plugins extend Godly Terminal with custom functionality -- sound notifications, AI-powered features, UI enhancements, and more. Each plugin lives in its own GitHub repository and is distributed as a `.zip` artifact attached to a GitHub Release.

The plugin system has two tiers:

- **Built-in plugins** ship with the app and have full access to all Tauri IPC commands.
- **External plugins** are installed from GitHub and run in a sandboxed context with restricted IPC access.

## Plugin Repo Structure

```
godly-plugin-example/
  godly-plugin.json      # Manifest (required)
  icon.png               # 64x64 plugin icon (optional)
  src/index.ts           # TypeScript source
  dist/index.js          # Built ES module bundle (required)
  package.json
  README.md
```

The only files required at runtime are `godly-plugin.json` and `dist/index.js`. Everything else is for development.

## Manifest Schema

Every plugin must include a `godly-plugin.json` in its root directory.

```json
{
  "id": "exit-sound",
  "name": "Exit Sound",
  "description": "Plays a sound when a terminal process exits",
  "author": "Your Name",
  "version": "1.0.0",
  "icon": "icon.png",
  "main": "dist/index.js",
  "minAppVersion": "0.4.0",
  "permissions": ["audio", "notifications"],
  "tags": ["sound", "terminal"],
  "homepage": "https://github.com/you/godly-plugin-exit-sound",
  "license": "MIT"
}
```

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Unique kebab-case identifier. Must not contain `/`, `\`, or `..`. |
| `name` | string | yes | Human-readable display name. |
| `description` | string | yes | Short description shown in the plugin browser. |
| `author` | string | yes | Author name or organization. |
| `version` | string | yes | Semver version (e.g., `1.0.0`). |
| `icon` | string | no | Filename of a 64x64 PNG icon. Default: `icon.png`. Max 1MB. |
| `main` | string | no | Entry point ES module. Default: `dist/index.js`. Max 5MB. |
| `minAppVersion` | string | no | Minimum Godly Terminal version required. |
| `permissions` | string[] | no | Required permissions (see [Permissions](#permissions)). |
| `tags` | string[] | no | Searchable tags for the plugin registry. |
| `homepage` | string | no | URL to the plugin's homepage or repo. |
| `license` | string | no | SPDX license identifier (e.g., `MIT`, `Apache-2.0`). |

### Permission values

- `"audio"` -- access to AudioContext and sound playback
- `"settings"` -- read/write plugin-scoped settings
- `"terminal-read"` -- receive terminal output events
- `"notifications"` -- show toast notifications

## GodlyPlugin Interface

Your plugin's default export must implement the `GodlyPlugin` interface:

```typescript
interface GodlyPlugin {
  id: string;
  name: string;
  description: string;
  version: string;

  init(ctx: PluginContext): void | Promise<void>;
  enable?(): void;
  disable?(): void;
  destroy?(): void;
  renderSettings?(): HTMLElement;
}
```

### Lifecycle methods

**`init(ctx: PluginContext)`** -- Called once when the plugin is loaded. This is where you receive the `PluginContext` and set up event subscriptions, preload assets, and prepare internal state. Runs for all registered plugins regardless of enabled state.

**`enable()`** -- Called when the user enables the plugin in Settings. Start actively processing events here.

**`disable()`** -- Called when the user disables the plugin. Stop processing events and release active resources, but keep cached data since the user may re-enable.

**`destroy()`** -- Called when the plugin system shuts down (app closing). Clean up everything: unsubscribe event handlers, release audio buffers, close connections.

**`renderSettings()`** -- Return a DOM element that will be displayed in the Settings dialog under your plugin's section. Use standard DOM APIs to create controls (sliders, checkboxes, dropdowns). The element is mounted/unmounted as the user navigates settings.

## PluginContext API

The `PluginContext` object is passed to `init()` and provides the plugin's interface to the host application.

```typescript
interface PluginContext {
  on(type: PluginEventType, handler: (event: PluginEvent) => void): () => void;
  readSoundFile(packId: string, filename: string): Promise<string>;
  listSoundPackFiles(packId: string): Promise<string[]>;
  listSoundPacks(): Promise<SoundPackManifest[]>;
  getAudioContext(): AudioContext;
  getSetting<T>(key: string, defaultValue: T): T;
  setSetting<T>(key: string, value: T): void;
  playSound(buffer: AudioBuffer, volume: number): void;
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  showToast(message: string, type?: 'info' | 'error' | 'success'): void;
}
```

### Event subscription

```typescript
on(type: PluginEventType, handler: (event: PluginEvent) => void): () => void
```

Subscribe to a plugin event. Returns an unsubscribe function. Always store and call unsubscribe functions in `destroy()` to prevent memory leaks.

```typescript
const unsub = ctx.on('terminal:closed', (event) => {
  console.log(`Terminal ${event.terminalId} closed`);
});

// Later, in destroy():
unsub();
```

### Sound pack access

```typescript
readSoundFile(packId: string, filename: string): Promise<string>
```

Read an audio file from an installed sound pack. Returns the file contents as a base64-encoded string. Use `atob()` and `AudioContext.decodeAudioData()` to convert to an `AudioBuffer`.

```typescript
const base64 = await ctx.readSoundFile('default', 'complete.mp3');
const binary = atob(base64);
const bytes = new Uint8Array(binary.length);
for (let i = 0; i < binary.length; i++) {
  bytes[i] = binary.charCodeAt(i);
}
const buffer = await ctx.getAudioContext().decodeAudioData(bytes.buffer);
```

```typescript
listSoundPackFiles(packId: string): Promise<string[]>
```

List all audio filenames in a sound pack.

```typescript
listSoundPacks(): Promise<SoundPackManifest[]>
```

List all installed sound packs with their manifests, including the sounds they provide for each category (`ready`, `complete`, `error`, `permission`, `notification`).

### Audio playback

```typescript
getAudioContext(): AudioContext
```

Get the shared `AudioContext` instance. Reuse this across your plugin instead of creating your own.

```typescript
playSound(buffer: AudioBuffer, volume: number): void
```

Play an `AudioBuffer` at the given volume (0.0 to 1.0). Uses the shared audio output pipeline.

```typescript
const volume = ctx.getSetting('volume', 0.7);
ctx.playSound(myBuffer, volume);
```

### Plugin settings

Settings are scoped per plugin and persisted to `localStorage`.

```typescript
getSetting<T>(key: string, defaultValue: T): T
```

Read a setting. Returns `defaultValue` if the key has not been set.

```typescript
setSetting<T>(key: string, value: T): void
```

Write a setting. The value is serialized to JSON and persisted immediately.

```typescript
// Read with default
const volume = ctx.getSetting('volume', 0.7);

// Write
ctx.setSetting('volume', 0.5);
ctx.setSetting('selectedPack', 'warcraft');
ctx.setSetting('category.error', false);
```

### Tauri IPC (gated)

```typescript
invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>
```

Call a Tauri backend command. External plugins can only invoke whitelisted commands (currently limited to sound pack operations). Built-in plugins have unrestricted access.

```typescript
const packs = await ctx.invoke<SoundPackManifest[]>('list_sound_packs');
```

### Toast notifications

```typescript
showToast(message: string, type?: 'info' | 'error' | 'success'): void
```

Display a brief toast notification in the app UI.

```typescript
ctx.showToast('Plugin loaded successfully', 'success');
ctx.showToast('Failed to load sound pack', 'error');
```

## Plugin Events

Events are the primary way plugins respond to what happens in the terminal.

```typescript
interface PluginEvent {
  type: PluginEventType;
  terminalId?: string;
  message?: string;
  processName?: string;
  timestamp: number;
}
```

### Event types

| Event | Fired when |
|-------|-----------|
| `notification` | Generic notification received (catch-all) |
| `terminal:output` | Terminal produces output |
| `terminal:closed` | Terminal process exits |
| `process:changed` | Active process in a terminal changes (e.g., `bash` to `node`) |
| `agent:task-complete` | An AI agent completes a task |
| `agent:error` | An AI agent encounters an error |
| `agent:permission` | An AI agent requests user permission |
| `agent:ready` | An AI agent signals it is ready |
| `app:focus` | The application window gains focus |
| `app:blur` | The application window loses focus |

The `agent:*` events are classified from incoming MCP notify messages using keyword heuristics. A message containing words like "error", "fail", or "crash" triggers `agent:error`, while "complete", "done", or "success" triggers `agent:task-complete`.

## Build Instructions

### Project setup

```bash
mkdir godly-plugin-my-plugin && cd godly-plugin-my-plugin
npm init -y
npm install --save-dev esbuild typescript
```

### TypeScript configuration

Create `tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ES2022",
    "moduleResolution": "bundler",
    "outDir": "dist",
    "declaration": true,
    "strict": true
  },
  "include": ["src"]
}
```

### Build script

Add to `package.json`:

```json
{
  "scripts": {
    "build": "esbuild src/index.ts --bundle --format=esm --outfile=dist/index.js --external:@tauri-apps/api"
  }
}
```

The `--external:@tauri-apps/api` flag is important. The Tauri API is provided by the host application at runtime and must not be bundled into your plugin.

### Type definitions

To get type checking for the plugin interfaces, create a `src/types.ts` file with the interfaces from this guide, or copy them from the Godly Terminal source at `src/plugins/types.ts`.

### Source file

Create `src/index.ts` with your plugin class as the default export:

```typescript
import type { GodlyPlugin, PluginContext } from './types';

export default class MyPlugin implements GodlyPlugin {
  id = 'my-plugin';
  name = 'My Plugin';
  description = 'Does something useful';
  version = '1.0.0';

  private ctx!: PluginContext;

  async init(ctx: PluginContext): Promise<void> {
    this.ctx = ctx;
  }

  enable(): void {
    // Start doing things
  }

  disable(): void {
    // Stop doing things
  }

  destroy(): void {
    // Clean up everything
  }
}
```

Build with:

```bash
npm run build
```

## Release Process

1. **Build** the plugin:
   ```bash
   npm run build
   ```

2. **Create the zip** containing only the runtime files:
   ```
   my-plugin.zip
     godly-plugin.json
     icon.png              (if you have one)
     dist/
       index.js
   ```

   On Unix/macOS:
   ```bash
   zip -r my-plugin.zip godly-plugin.json icon.png dist/index.js
   ```

   On Windows (PowerShell):
   ```powershell
   Compress-Archive -Path godly-plugin.json, icon.png, dist -DestinationPath my-plugin.zip
   ```

3. **Create a GitHub Release** with a semver tag:
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

4. **Attach the zip** as a release asset. The Godly Terminal installer fetches the latest release and looks for the first `.zip` asset.

## Submit to the Plugin Registry

To make your plugin discoverable in the Godly Terminal plugin browser:

1. Fork the [godly-terminal](https://github.com/alangmartini/godly-terminal) repository.

2. Add an entry to `src/plugins/registry.json`:
   ```json
   {
     "id": "my-plugin",
     "repo": "your-username/godly-plugin-my-plugin",
     "description": "Short description of what your plugin does",
     "author": "Your Name",
     "tags": ["relevant", "tags"],
     "featured": false
   }
   ```

3. Open a pull request with your addition. Include your plugin name, a link to the repo, and a brief description.

The registry is fetched from the `master` branch at runtime, so your plugin becomes available to all users once the PR is merged.

## Example: Exit Sound Plugin

A complete plugin that plays a sound when any terminal process exits.

### `godly-plugin.json`

```json
{
  "id": "exit-sound",
  "name": "Exit Sound",
  "description": "Plays a sound when a terminal process exits",
  "author": "Your Name",
  "version": "1.0.0",
  "permissions": ["audio"],
  "tags": ["sound", "terminal"]
}
```

### `src/index.ts`

```typescript
import type { GodlyPlugin, PluginContext } from './types';

export default class ExitSoundPlugin implements GodlyPlugin {
  id = 'exit-sound';
  name = 'Exit Sound';
  description = 'Plays a sound when a terminal process exits';
  version = '1.0.0';

  private ctx!: PluginContext;
  private unsub?: () => void;
  private soundBuffer: AudioBuffer | null = null;

  async init(ctx: PluginContext): Promise<void> {
    this.ctx = ctx;
    await this.loadSound();
  }

  enable(): void {
    this.unsub = this.ctx.on('terminal:closed', () => {
      if (this.soundBuffer) {
        const volume = this.ctx.getSetting('volume', 0.5);
        this.ctx.playSound(this.soundBuffer, volume);
      }
    });
  }

  disable(): void {
    this.unsub?.();
    this.unsub = undefined;
  }

  destroy(): void {
    this.disable();
    this.soundBuffer = null;
  }

  renderSettings(): HTMLElement {
    const container = document.createElement('div');

    // Volume slider
    const row = document.createElement('div');
    row.className = 'shortcut-row';

    const label = document.createElement('span');
    label.className = 'shortcut-label';
    label.textContent = 'Volume';
    row.appendChild(label);

    const slider = document.createElement('input');
    slider.type = 'range';
    slider.className = 'notification-volume';
    slider.min = '0';
    slider.max = '100';
    slider.value = String(Math.round(this.ctx.getSetting('volume', 0.5) * 100));
    slider.oninput = () => {
      this.ctx.setSetting('volume', parseInt(slider.value) / 100);
    };
    row.appendChild(slider);

    container.appendChild(row);

    // Test button
    const testRow = document.createElement('div');
    testRow.className = 'shortcut-row';

    const testLabel = document.createElement('span');
    testLabel.className = 'shortcut-label';
    testLabel.textContent = 'Preview';
    testRow.appendChild(testLabel);

    const testBtn = document.createElement('button');
    testBtn.className = 'dialog-btn dialog-btn-secondary';
    testBtn.textContent = 'Test Sound';
    testBtn.onclick = () => {
      if (this.soundBuffer) {
        const volume = this.ctx.getSetting('volume', 0.5);
        this.ctx.playSound(this.soundBuffer, volume);
      }
    };
    testRow.appendChild(testBtn);

    container.appendChild(testRow);
    return container;
  }

  private async loadSound(): Promise<void> {
    try {
      const base64 = await this.ctx.readSoundFile('default', 'work_complete.mp3');
      const binary = atob(base64);
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i++) {
        bytes[i] = binary.charCodeAt(i);
      }
      const audioCtx = this.ctx.getAudioContext();
      this.soundBuffer = await audioCtx.decodeAudioData(bytes.buffer);
    } catch (e) {
      console.warn('[ExitSound] Failed to load sound:', e);
    }
  }
}
```

## Permissions

Godly Terminal uses a tiered permission model to balance extensibility with security.

**Built-in plugins** (shipped with the app) have full, unrestricted access to all Tauri IPC commands. They can read terminal grids, manage workspaces, access the filesystem through Tauri APIs, and call any backend command.

**External plugins** (installed from GitHub) operate in a sandboxed context:

- The `invoke()` method on `PluginContext` is gated. Only whitelisted Tauri commands can be called.
- Currently whitelisted commands: `read_sound_pack_file`, `list_sound_pack_files`, `list_sound_packs`, `get_sound_packs_dir`.
- The `permissions` field in the manifest declares what capabilities the plugin needs. Users see these permissions before enabling a plugin.
- Plugin JavaScript is loaded from a size-limited file (5MB max) read from disk by the Rust backend, not fetched from the network at runtime.

The permission surface will expand as the plugin API stabilizes. If your plugin needs access to a command that is not yet whitelisted, open an issue on the Godly Terminal repository describing your use case.
