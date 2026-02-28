import { getPluginRegistry } from '../../plugins/index';
import { pluginStore } from '../../plugins/plugin-store';
import { pluginInstaller } from '../../plugins/plugin-installer';
import { createPluginCard } from '../PluginCard';
import type { RegistryEntry, PluginRegistryData } from '../../plugins/types';
import registryData from '../../plugins/registry.json';
import type { SettingsTabProvider, SettingsDialogContext } from './types';

export class PluginsTab implements SettingsTabProvider {
  id = 'plugins';
  label = 'Plugins';

  buildContent(_dialog: SettingsDialogContext): HTMLDivElement {
    const content = document.createElement('div');
    content.className = 'settings-tab-content';

    const installingPlugins = new Set<string>();

    function renderPluginsTab() {
      content.textContent = '';

      const registry = getPluginRegistry();
      if (!registry) {
        const msg = document.createElement('div');
        msg.className = 'settings-description';
        msg.textContent = 'Plugin system not initialized.';
        content.appendChild(msg);
        return;
      }

      // ── Installed section ──
      const installedTitle = document.createElement('div');
      installedTitle.className = 'plugin-section-title';
      installedTitle.textContent = 'Installed';
      content.appendChild(installedTitle);

      const registeredPlugins = registry.getAllWithMeta();
      if (registeredPlugins.length === 0) {
        const msg = document.createElement('div');
        msg.className = 'settings-description';
        msg.textContent = 'No plugins installed.';
        content.appendChild(msg);
      } else {
        const installedGrid = document.createElement('div');
        installedGrid.className = 'plugin-browse-grid';
        for (const entry of registeredPlugins) {
          const card = createPluginCard({
            plugin: entry.plugin,
            manifest: entry.manifest,
            isBuiltin: entry.builtin,
            isEnabled: pluginStore.isEnabled(entry.plugin.id),
            isInstalled: true,
            onToggle: (enabled) => {
              registry.setEnabled(entry.plugin.id, enabled);
              renderPluginsTab();
            },
            onUninstall: async () => {
              try {
                await pluginInstaller.uninstallPlugin(entry.plugin.id);
                renderPluginsTab();
              } catch (e) {
                console.error('[Plugins] Uninstall failed:', e);
              }
            },
          });
          installedGrid.appendChild(card);
        }
        content.appendChild(installedGrid);
      }

      // ── Browse section ──
      const browseTitle = document.createElement('div');
      browseTitle.className = 'plugin-section-title';
      browseTitle.style.marginTop = '16px';
      browseTitle.textContent = 'Browse';
      content.appendChild(browseTitle);

      const installedIds = new Set(registeredPlugins.map(e => e.plugin.id));
      const browseEntries = (registryData as PluginRegistryData).plugins.filter(
        (entry: RegistryEntry) => !installedIds.has(entry.id)
      );

      if (browseEntries.length === 0) {
        const msg = document.createElement('div');
        msg.className = 'settings-description';
        msg.textContent = 'All registry plugins are installed.';
        content.appendChild(msg);
      } else {
        const browseGrid = document.createElement('div');
        browseGrid.className = 'plugin-browse-grid';
        for (const entry of browseEntries) {
          const card = createPluginCard({
            registryEntry: entry,
            isBuiltin: false,
            isEnabled: false,
            isInstalled: false,
            installing: installingPlugins.has(entry.id),
            onInstall: async () => {
              const [owner, repo] = entry.repo.split('/');
              if (!owner || !repo) return;
              installingPlugins.add(entry.id);
              renderPluginsTab();
              try {
                await pluginInstaller.installPlugin(owner, repo);
              } catch (e) {
                console.error('[Plugins] Install failed:', e);
              } finally {
                installingPlugins.delete(entry.id);
                renderPluginsTab();
              }
            },
          });
          browseGrid.appendChild(card);
        }
        content.appendChild(browseGrid);
      }
    }

    renderPluginsTab();

    return content;
  }
}
