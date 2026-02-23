import type { GodlyPlugin, PluginContext, PluginEventType, SoundPackManifest } from '../types';
import type { PluginEventBus } from '../event-bus';

type SoundCategory = 'ready' | 'acknowledge' | 'complete' | 'error' | 'permission' | 'notification';

const EVENT_TO_CATEGORY: Partial<Record<PluginEventType, SoundCategory>> = {
  'agent:ready': 'ready',
  'agent:acknowledge': 'acknowledge',
  'agent:task-complete': 'complete',
  'agent:error': 'error',
  'agent:permission': 'permission',
  'notification': 'notification',
};

const CATEGORIES: { id: SoundCategory; label: string; eventType: PluginEventType }[] = [
  { id: 'ready', label: 'Agent Ready', eventType: 'agent:ready' },
  { id: 'acknowledge', label: 'Task Acknowledged', eventType: 'agent:acknowledge' },
  { id: 'complete', label: 'Task Complete', eventType: 'agent:task-complete' },
  { id: 'error', label: 'Error', eventType: 'agent:error' },
  { id: 'permission', label: 'Permission Needed', eventType: 'agent:permission' },
  { id: 'notification', label: 'General Notification', eventType: 'notification' },
];

const REGISTRY_URL = 'https://peonping.github.io/registry/index.json';

// CESP category → our category mapping
const CESP_TO_CATEGORY: Record<string, SoundCategory> = {
  'session.start': 'ready',
  'task.acknowledge': 'acknowledge',
  'task.complete': 'complete',
  'task.error': 'error',
  'input.required': 'permission',
  'resource.limit': 'notification',
  'user.spam': 'notification',
};

interface CespSound {
  file: string;
  label?: string;
  sha256?: string;
}

interface CespManifest {
  cesp_version: string;
  name: string;
  display_name: string;
  version: string;
  author: { name: string; github?: string };
  license?: string;
  language?: string;
  categories: Record<string, { sounds: CespSound[] }>;
}

interface RegistryEntry {
  name: string;
  display_name: string;
  version: string;
  description: string;
  author: { name: string; github?: string };
  trust_tier?: string;
  categories: string[];
  language?: string;
  license?: string;
  sound_count: number;
  total_size_bytes: number;
  source_repo: string;
  source_ref?: string;
  source_path: string;
  tags?: string[];
  added?: string;
}

/** Convert a CESP manifest to our SoundPackManifest format. */
function cespToManifest(cespId: string, cesp: CespManifest): SoundPackManifest {
  const sounds: Record<string, string[]> = {};

  for (const [cespCat, data] of Object.entries(cesp.categories)) {
    const ourCat = CESP_TO_CATEGORY[cespCat];
    if (!ourCat) continue;

    const filenames = data.sounds.map(s => {
      // CESP files are stored as "sounds/Filename.wav", strip the "sounds/" prefix
      const name = s.file.replace(/^sounds\//, '');
      return name;
    });

    if (!sounds[ourCat]) {
      sounds[ourCat] = [];
    }
    // Merge (e.g., resource.limit and user.spam both map to notification)
    for (const f of filenames) {
      if (!sounds[ourCat].includes(f)) {
        sounds[ourCat].push(f);
      }
    }
  }

  return {
    id: cespId,
    name: cesp.display_name,
    description: `${cesp.display_name} — powered by PeonPing`,
    author: cesp.author.name,
    version: cesp.version,
    sounds,
  };
}

export class PeonPingPlugin implements GodlyPlugin {
  id = 'peon-ping';
  name = 'Peon Ping';
  description = 'Play sound pack voice lines when AI agents complete tasks, hit errors, or need permission.';
  version = '1.0.0';

  private ctx!: PluginContext;
  private bus!: PluginEventBus;
  private enabled = false;
  private unsubscribes: (() => void)[] = [];

  // Audio cache: packId -> category -> AudioBuffer[]
  private audioCache = new Map<string, Map<SoundCategory, AudioBuffer[]>>();
  // No-repeat: last played index per category
  private lastPlayed = new Map<SoundCategory, number>();
  // Current manifest
  private currentManifest: SoundPackManifest | null = null;

  setBus(bus: PluginEventBus): void {
    this.bus = bus;
  }

  async init(ctx: PluginContext): Promise<void> {
    this.ctx = ctx;

    // Subscribe to relevant events
    for (const cat of CATEGORIES) {
      const unsub = ctx.on(cat.eventType, (event) => {
        if (!this.enabled) return;
        const category = EVENT_TO_CATEGORY[event.type];
        if (!category) return;
        if (!this.ctx.getSetting(`category.${category}`, true)) return;
        this.playCategorySound(category);
      });
      this.unsubscribes.push(unsub);
    }

    // Load active sound pack
    await this.loadActivePack();
  }

  enable(): void {
    this.enabled = true;
  }

  disable(): void {
    this.enabled = false;
  }

  destroy(): void {
    for (const unsub of this.unsubscribes) unsub();
    this.unsubscribes = [];
    this.audioCache.clear();
    this.lastPlayed.clear();
  }

  renderSettings(): HTMLElement {
    const container = document.createElement('div');
    container.className = 'peon-ping-settings';

    // Volume slider
    const volumeRow = document.createElement('div');
    volumeRow.className = 'shortcut-row';
    const volumeLabel = document.createElement('span');
    volumeLabel.className = 'shortcut-label';
    volumeLabel.textContent = 'Volume';
    volumeRow.appendChild(volumeLabel);
    const volumeSlider = document.createElement('input');
    volumeSlider.type = 'range';
    volumeSlider.className = 'notification-volume';
    volumeSlider.min = '0';
    volumeSlider.max = '100';
    volumeSlider.value = String(Math.round(this.ctx.getSetting('volume', 0.7) * 100));
    volumeSlider.oninput = () => {
      this.ctx.setSetting('volume', parseInt(volumeSlider.value) / 100);
    };
    volumeRow.appendChild(volumeSlider);
    container.appendChild(volumeRow);

    // Sound pack selector
    const packRow = document.createElement('div');
    packRow.className = 'shortcut-row';
    const packLabel = document.createElement('span');
    packLabel.className = 'shortcut-label';
    packLabel.textContent = 'Sound Pack';
    packRow.appendChild(packLabel);

    const packSelect = document.createElement('select');
    packSelect.className = 'notification-preset';
    packSelect.style.minWidth = '140px';

    // Show placeholder while loading
    const loadingOpt = document.createElement('option');
    loadingOpt.value = '';
    loadingOpt.textContent = 'Loading...';
    loadingOpt.disabled = true;
    loadingOpt.selected = true;
    packSelect.appendChild(loadingOpt);

    const refreshPackList = () => {
      this.ctx.listSoundPacks().then(packs => {
        // Clear all options
        while (packSelect.firstChild) packSelect.removeChild(packSelect.firstChild);
        if (packs.length === 0) {
          const emptyOpt = document.createElement('option');
          emptyOpt.value = '';
          emptyOpt.textContent = 'No packs found';
          emptyOpt.disabled = true;
          emptyOpt.selected = true;
          packSelect.appendChild(emptyOpt);
          return;
        }
        const activePack = this.ctx.getSetting('activePack', 'default');
        for (const pack of packs) {
          const opt = document.createElement('option');
          opt.value = pack.id;
          opt.textContent = `${pack.name} (${pack.author})`;
          if (pack.id === activePack) opt.selected = true;
          packSelect.appendChild(opt);
        }
      }).catch(() => {
        while (packSelect.firstChild) packSelect.removeChild(packSelect.firstChild);
        const opt = document.createElement('option');
        opt.value = 'default';
        opt.textContent = 'Default';
        packSelect.appendChild(opt);
      });
    };
    refreshPackList();

    packSelect.onchange = async () => {
      this.ctx.setSetting('activePack', packSelect.value);
      await this.loadActivePack();
    };
    packRow.appendChild(packSelect);
    container.appendChild(packRow);

    // Category toggles with test buttons
    const catSection = document.createElement('div');
    catSection.className = 'settings-section';
    const catTitle = document.createElement('div');
    catTitle.className = 'settings-section-title';
    catTitle.textContent = 'Sound Categories';
    catSection.appendChild(catTitle);

    for (const cat of CATEGORIES) {
      const row = document.createElement('div');
      row.className = 'shortcut-row';

      const label = document.createElement('span');
      label.className = 'shortcut-label';
      label.textContent = cat.label;
      row.appendChild(label);

      const rightSide = document.createElement('div');
      rightSide.style.display = 'flex';
      rightSide.style.alignItems = 'center';
      rightSide.style.gap = '8px';

      const checkbox = document.createElement('input');
      checkbox.type = 'checkbox';
      checkbox.className = 'notification-checkbox';
      checkbox.checked = this.ctx.getSetting(`category.${cat.id}`, true);
      checkbox.onchange = () => {
        this.ctx.setSetting(`category.${cat.id}`, checkbox.checked);
      };
      rightSide.appendChild(checkbox);

      const testBtn = document.createElement('button');
      testBtn.className = 'dialog-btn dialog-btn-secondary';
      testBtn.textContent = 'Test';
      testBtn.style.fontSize = '11px';
      testBtn.style.padding = '2px 10px';
      testBtn.onclick = () => {
        this.playCategorySound(cat.id);
      };
      rightSide.appendChild(testBtn);

      row.appendChild(rightSide);
      catSection.appendChild(row);
    }

    container.appendChild(catSection);

    // PeonPing registry browser section
    const registrySection = document.createElement('div');
    registrySection.className = 'settings-section';
    const registryTitle = document.createElement('div');
    registryTitle.className = 'settings-section-title';
    registryTitle.textContent = 'PeonPing Sound Packs';
    registrySection.appendChild(registryTitle);

    const registryContainer = document.createElement('div');
    registryContainer.className = 'peon-ping-registry';
    registrySection.appendChild(registryContainer);

    const browseBtn = document.createElement('button');
    browseBtn.className = 'dialog-btn dialog-btn-secondary';
    browseBtn.textContent = 'Browse 127+ Sound Packs';
    browseBtn.style.width = '100%';
    browseBtn.onclick = () => {
      browseBtn.style.display = 'none';
      this.renderRegistryBrowser(registryContainer, packSelect, refreshPackList);
    };
    registryContainer.appendChild(browseBtn);

    container.appendChild(registrySection);

    // Open sound packs folder button
    const folderRow = document.createElement('div');
    folderRow.className = 'shortcut-row';
    const folderLabel = document.createElement('span');
    folderLabel.className = 'shortcut-label';
    folderLabel.textContent = 'Sound packs';
    folderRow.appendChild(folderLabel);

    const openFolderBtn = document.createElement('button');
    openFolderBtn.className = 'dialog-btn dialog-btn-secondary';
    openFolderBtn.textContent = 'Open Sound Packs Folder';
    openFolderBtn.onclick = async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
        const dir: string = await invoke('get_sound_packs_dir');
        await revealItemInDir(dir);
      } catch (e) {
        console.warn('Failed to open sound packs folder:', e);
      }
    };
    folderRow.appendChild(openFolderBtn);
    container.appendChild(folderRow);

    return container;
  }

  private async renderRegistryBrowser(
    container: HTMLElement,
    _packSelect: HTMLSelectElement,
    refreshPackList: () => void,
  ): Promise<void> {
    const statusEl = document.createElement('div');
    statusEl.style.color = 'var(--text-secondary)';
    statusEl.style.fontSize = '12px';
    statusEl.style.padding = '8px 0';
    statusEl.textContent = 'Loading registry...';
    container.appendChild(statusEl);

    let entries: RegistryEntry[];
    try {
      const resp = await fetch(REGISTRY_URL);
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      entries = await resp.json();
    } catch (e) {
      statusEl.textContent = `Failed to load registry: ${e}`;
      return;
    }

    // Get installed packs to show install status
    let installedIds: Set<string>;
    try {
      const packs = await this.ctx.listSoundPacks();
      installedIds = new Set(packs.map(p => p.id));
    } catch {
      installedIds = new Set();
    }

    statusEl.textContent = `${entries.length} packs available`;

    // Search input
    const searchInput = document.createElement('input');
    searchInput.type = 'text';
    searchInput.placeholder = 'Search packs...';
    searchInput.className = 'notification-preset';
    searchInput.style.width = '100%';
    searchInput.style.marginBottom = '8px';
    searchInput.style.boxSizing = 'border-box';
    container.appendChild(searchInput);

    // Pack list container
    const listContainer = document.createElement('div');
    listContainer.style.maxHeight = '300px';
    listContainer.style.overflowY = 'auto';
    listContainer.style.border = '1px solid var(--border-color)';
    listContainer.style.borderRadius = '4px';
    container.appendChild(listContainer);

    const renderList = (filter: string) => {
      listContainer.innerHTML = '';
      const filtered = filter
        ? entries.filter(e =>
            e.display_name.toLowerCase().includes(filter) ||
            e.description.toLowerCase().includes(filter) ||
            (e.tags ?? []).some(t => t.includes(filter))
          )
        : entries;

      for (const entry of filtered) {
        const row = document.createElement('div');
        row.style.display = 'flex';
        row.style.alignItems = 'center';
        row.style.justifyContent = 'space-between';
        row.style.padding = '6px 10px';
        row.style.borderBottom = '1px solid var(--border-color)';
        row.style.gap = '8px';

        const info = document.createElement('div');
        info.style.flex = '1';
        info.style.minWidth = '0';

        const nameEl = document.createElement('div');
        nameEl.style.fontWeight = '500';
        nameEl.style.fontSize = '12px';
        nameEl.style.whiteSpace = 'nowrap';
        nameEl.style.overflow = 'hidden';
        nameEl.style.textOverflow = 'ellipsis';
        nameEl.textContent = entry.display_name;
        info.appendChild(nameEl);

        const metaEl = document.createElement('div');
        metaEl.style.fontSize = '11px';
        metaEl.style.color = 'var(--text-secondary)';
        const sizeKB = Math.round(entry.total_size_bytes / 1024);
        metaEl.textContent = `${entry.sound_count} sounds · ${sizeKB}KB · ${entry.language ?? 'en'}`;
        info.appendChild(metaEl);

        row.appendChild(info);

        const isInstalled = installedIds.has(entry.name);
        const btn = document.createElement('button');
        btn.className = 'dialog-btn dialog-btn-secondary';
        btn.style.fontSize = '11px';
        btn.style.padding = '2px 10px';
        btn.style.flexShrink = '0';
        btn.textContent = isInstalled ? 'Installed' : 'Install';
        btn.disabled = isInstalled;

        if (!isInstalled) {
          btn.onclick = async () => {
            btn.textContent = 'Installing...';
            btn.disabled = true;
            try {
              await this.installFromRegistry(entry);
              btn.textContent = 'Installed';
              installedIds.add(entry.name);
              refreshPackList();
              this.ctx.showToast(`Installed: ${entry.display_name}`, 'success');
            } catch (e) {
              btn.textContent = 'Failed';
              btn.disabled = false;
              console.warn('[PeonPing] Install failed:', e);
              this.ctx.showToast(`Failed to install ${entry.display_name}: ${e}`, 'error');
            }
          };
        }

        row.appendChild(btn);
        listContainer.appendChild(row);
      }

      if (filtered.length === 0) {
        const empty = document.createElement('div');
        empty.style.padding = '16px';
        empty.style.textAlign = 'center';
        empty.style.color = 'var(--text-secondary)';
        empty.style.fontSize = '12px';
        empty.textContent = 'No packs match your search';
        listContainer.appendChild(empty);
      }
    };

    renderList('');
    searchInput.oninput = () => renderList(searchInput.value.toLowerCase().trim());
  }

  private async installFromRegistry(entry: RegistryEntry): Promise<void> {
    const ref = entry.source_ref ?? 'main';
    const baseUrl = `https://raw.githubusercontent.com/${entry.source_repo}/${ref}/${entry.source_path}`;

    // 1. Fetch CESP manifest
    const manifestResp = await fetch(`${baseUrl}/openpeon.json`);
    if (!manifestResp.ok) throw new Error(`Failed to fetch manifest: HTTP ${manifestResp.status}`);
    const cesp: CespManifest = await manifestResp.json();

    // 2. Collect all unique sound filenames
    const allFiles = new Set<string>();
    for (const cat of Object.values(cesp.categories)) {
      for (const sound of cat.sounds) {
        const filename = sound.file.replace(/^sounds\//, '');
        allFiles.add(filename);
      }
    }

    // 3. Download all sound files (parallel, batched)
    const files: [string, string][] = [];
    const batch = Array.from(allFiles);
    const BATCH_SIZE = 5;
    for (let i = 0; i < batch.length; i += BATCH_SIZE) {
      const chunk = batch.slice(i, i + BATCH_SIZE);
      const results = await Promise.all(
        chunk.map(async (filename): Promise<[string, string]> => {
          const resp = await fetch(`${baseUrl}/sounds/${filename}`);
          if (!resp.ok) throw new Error(`Failed to download ${filename}: HTTP ${resp.status}`);
          const buf = await resp.arrayBuffer();
          const bytes = new Uint8Array(buf);
          // Convert to base64
          let binary = '';
          for (let j = 0; j < bytes.length; j++) {
            binary += String.fromCharCode(bytes[j]);
          }
          return [filename, btoa(binary)];
        })
      );
      files.push(...results);
    }

    // 4. Convert CESP manifest to our format
    const manifest = cespToManifest(entry.name, cesp);
    const manifestJson = JSON.stringify(manifest, null, 2);

    // 5. Install via Tauri command
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('install_sound_pack', {
      packId: entry.name,
      manifestJson,
      files,
    });
  }

  private async loadActivePack(): Promise<void> {
    const packId = this.ctx.getSetting('activePack', 'default');
    try {
      const packs = await this.ctx.listSoundPacks();
      this.currentManifest = packs.find(p => p.id === packId) ?? null;
      if (this.currentManifest) {
        await this.preloadPackSounds(packId, this.currentManifest);
      }
    } catch (e) {
      console.warn('[PeonPing] Failed to load sound pack:', e);
    }
  }

  private async preloadPackSounds(packId: string, manifest: SoundPackManifest): Promise<void> {
    const packCache = new Map<SoundCategory, AudioBuffer[]>();
    const ctx = this.ctx.getAudioContext();

    for (const category of CATEGORIES) {
      const files = manifest.sounds[category.id];
      if (!files || files.length === 0) continue;

      const buffers: AudioBuffer[] = [];
      for (const file of files) {
        try {
          const base64 = await this.ctx.readSoundFile(packId, file);
          const binaryString = atob(base64);
          const bytes = new Uint8Array(binaryString.length);
          for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i);
          }
          const buffer = await ctx.decodeAudioData(bytes.buffer);
          buffers.push(buffer);
        } catch (e) {
          console.warn(`[PeonPing] Failed to load sound ${file}:`, e);
        }
      }
      if (buffers.length > 0) {
        packCache.set(category.id, buffers);
      }
    }

    this.audioCache.set(packId, packCache);
  }

  private playCategorySound(category: SoundCategory): void {
    const packId = this.ctx.getSetting('activePack', 'default');
    const packCache = this.audioCache.get(packId);
    if (!packCache) return;

    const buffers = packCache.get(category);
    if (!buffers || buffers.length === 0) return;

    // Random selection with no-repeat
    let index: number;
    if (buffers.length === 1) {
      index = 0;
    } else {
      const last = this.lastPlayed.get(category) ?? -1;
      do {
        index = Math.floor(Math.random() * buffers.length);
      } while (index === last);
    }
    this.lastPlayed.set(category, index);

    const volume = this.ctx.getSetting('volume', 0.7);
    this.ctx.playSound(buffers[index], volume);

    // Mark that this plugin handled the sound
    this.bus.markSoundHandled(
      CATEGORIES.find(c => c.id === category)?.eventType ?? 'notification'
    );
  }
}
