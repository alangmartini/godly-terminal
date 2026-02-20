import type { GodlyPlugin, PluginContext, PluginEventType, SoundPackManifest } from '../types';
import type { PluginEventBus } from '../event-bus';

type SoundCategory = 'ready' | 'complete' | 'error' | 'permission' | 'notification';

const EVENT_TO_CATEGORY: Partial<Record<PluginEventType, SoundCategory>> = {
  'agent:ready': 'ready',
  'agent:task-complete': 'complete',
  'agent:error': 'error',
  'agent:permission': 'permission',
  'notification': 'notification',
};

const CATEGORIES: { id: SoundCategory; label: string; eventType: PluginEventType }[] = [
  { id: 'ready', label: 'Agent Ready', eventType: 'agent:ready' },
  { id: 'complete', label: 'Task Complete', eventType: 'agent:task-complete' },
  { id: 'error', label: 'Error', eventType: 'agent:error' },
  { id: 'permission', label: 'Permission Needed', eventType: 'agent:permission' },
  { id: 'notification', label: 'General Notification', eventType: 'notification' },
];

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

    // Load packs asynchronously
    this.ctx.listSoundPacks().then(packs => {
      const activePack = this.ctx.getSetting('activePack', 'default');
      for (const pack of packs) {
        const opt = document.createElement('option');
        opt.value = pack.id;
        opt.textContent = `${pack.name} (${pack.author})`;
        if (pack.id === activePack) opt.selected = true;
        packSelect.appendChild(opt);
      }
    }).catch(() => {
      const opt = document.createElement('option');
      opt.value = 'default';
      opt.textContent = 'Default';
      packSelect.appendChild(opt);
    });

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
