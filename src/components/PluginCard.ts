import type { GodlyPlugin, ExternalPluginManifest, RegistryEntry } from '../plugins/types';

export interface PluginCardOptions {
  plugin?: GodlyPlugin;
  manifest?: ExternalPluginManifest;
  registryEntry?: RegistryEntry;
  isBuiltin: boolean;
  isEnabled: boolean;
  isInstalled: boolean;
  iconDataUrl?: string | null;
  onToggle?: (enabled: boolean) => void;
  onInstall?: () => void;
  onUninstall?: () => void;
  installing?: boolean;
}

const FALLBACK_ICON_SVG = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/></svg>`;

export function createPluginCard(opts: PluginCardOptions): HTMLElement {
  const card = document.createElement('div');
  card.className = 'plugin-card';

  // Determine display values from plugin, manifest, or registry entry
  const name = opts.plugin?.name ?? opts.manifest?.name ?? opts.registryEntry?.id ?? 'Unknown';
  const version = opts.plugin?.version ?? opts.manifest?.version ?? '';
  const author = opts.manifest?.author ?? opts.registryEntry?.author ?? (opts.isBuiltin ? 'Built-in' : '');
  const description = opts.plugin?.description ?? opts.manifest?.description ?? opts.registryEntry?.description ?? '';
  const tags = opts.manifest?.tags ?? opts.registryEntry?.tags ?? [];

  // Icon
  const iconEl = document.createElement('div');
  iconEl.className = 'plugin-card-icon';
  if (opts.iconDataUrl) {
    const img = document.createElement('img');
    img.src = opts.iconDataUrl;
    img.alt = name;
    img.width = 40;
    img.height = 40;
    iconEl.appendChild(img);
  } else {
    iconEl.innerHTML = FALLBACK_ICON_SVG;
  }
  card.appendChild(iconEl);

  // Body
  const body = document.createElement('div');
  body.className = 'plugin-card-body';

  // Header row: name + version + author
  const header = document.createElement('div');
  header.className = 'plugin-card-header';

  const nameEl = document.createElement('span');
  nameEl.className = 'plugin-card-name';
  nameEl.textContent = name;
  header.appendChild(nameEl);

  if (version) {
    const versionEl = document.createElement('span');
    versionEl.className = 'plugin-card-version';
    versionEl.textContent = `v${version}`;
    header.appendChild(versionEl);
  }

  if (author) {
    const authorEl = document.createElement('span');
    authorEl.className = 'plugin-card-author';
    authorEl.textContent = author;
    header.appendChild(authorEl);
  }

  if (opts.isBuiltin) {
    const badge = document.createElement('span');
    badge.className = 'plugin-card-builtin-badge';
    badge.textContent = 'Built-in';
    header.appendChild(badge);
  }

  body.appendChild(header);

  // Description
  if (description) {
    const descEl = document.createElement('div');
    descEl.className = 'plugin-card-description';
    descEl.textContent = description;
    body.appendChild(descEl);
  }

  // Tags
  if (tags.length > 0) {
    const tagsRow = document.createElement('div');
    tagsRow.className = 'plugin-card-tags';
    for (const tag of tags) {
      const tagEl = document.createElement('span');
      tagEl.className = 'plugin-tag';
      tagEl.textContent = tag;
      tagsRow.appendChild(tagEl);
    }
    body.appendChild(tagsRow);
  }

  // Actions row
  const actions = document.createElement('div');
  actions.className = 'plugin-card-actions';

  if (opts.isInstalled) {
    // Toggle switch
    const toggleLabel = document.createElement('label');
    toggleLabel.className = 'plugin-toggle';

    const toggleInput = document.createElement('input');
    toggleInput.type = 'checkbox';
    toggleInput.checked = opts.isEnabled;
    toggleInput.onchange = () => {
      opts.onToggle?.(toggleInput.checked);
    };
    toggleLabel.appendChild(toggleInput);

    const toggleSlider = document.createElement('span');
    toggleSlider.className = 'plugin-toggle-slider';
    toggleLabel.appendChild(toggleSlider);

    const toggleText = document.createElement('span');
    toggleText.className = 'plugin-toggle-text';
    toggleText.textContent = opts.isEnabled ? 'Enabled' : 'Disabled';
    toggleLabel.appendChild(toggleText);

    actions.appendChild(toggleLabel);

    // Uninstall button (only for non-builtin)
    if (!opts.isBuiltin) {
      const uninstallBtn = document.createElement('button');
      uninstallBtn.className = 'plugin-card-btn plugin-card-btn-danger';
      uninstallBtn.textContent = 'Uninstall';
      uninstallBtn.onclick = () => opts.onUninstall?.();
      actions.appendChild(uninstallBtn);
    }
  } else {
    // Install button
    const installBtn = document.createElement('button');
    installBtn.className = 'plugin-card-btn plugin-card-btn-primary';
    if (opts.installing) {
      installBtn.textContent = 'Installing...';
      installBtn.disabled = true;
    } else {
      installBtn.textContent = 'Install';
      installBtn.onclick = () => opts.onInstall?.();
    }
    actions.appendChild(installBtn);
  }

  body.appendChild(actions);
  card.appendChild(body);

  // Expandable settings (only when installed + enabled + has renderSettings)
  if (opts.isInstalled && opts.isEnabled && opts.plugin?.renderSettings) {
    const settingsSection = document.createElement('div');
    settingsSection.className = 'plugin-card-settings';
    try {
      const settingsEl = opts.plugin.renderSettings();
      settingsSection.appendChild(settingsEl);
    } catch (e) {
      const errEl = document.createElement('div');
      errEl.className = 'settings-description';
      errEl.textContent = 'Failed to load plugin settings.';
      settingsSection.appendChild(errEl);
    }
    card.appendChild(settingsSection);
  }

  return card;
}
