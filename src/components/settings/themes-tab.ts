import { themeStore } from '../../state/theme-store';
import type { ThemeDefinition } from '../../themes/types';
import { createThemePreview } from '../ThemePreview';
import type { SettingsTabProvider, SettingsDialogContext } from './types';

export class ThemesTab implements SettingsTabProvider {
  id = 'themes';
  label = 'Themes';

  buildContent(_dialog: SettingsDialogContext): HTMLDivElement {
    const content = document.createElement('div');
    content.className = 'settings-tab-content';

    const themeGrid = document.createElement('div');
    themeGrid.className = 'theme-grid';
    content.appendChild(themeGrid);

    function renderThemeGrid() {
      themeGrid.textContent = '';
      const allThemes = themeStore.getAllThemes();
      const activeTheme = themeStore.getActiveTheme();

      for (const theme of allThemes) {
        const card = document.createElement('div');
        card.className = 'theme-card' + (theme.id === activeTheme.id ? ' active' : '');

        const preview = createThemePreview(theme, 280, 140);
        card.appendChild(preview);

        const info = document.createElement('div');
        info.className = 'theme-card-info';

        const nameEl = document.createElement('div');
        nameEl.className = 'theme-card-name';
        nameEl.textContent = theme.name;
        info.appendChild(nameEl);

        const descEl = document.createElement('div');
        descEl.className = 'theme-card-description';
        descEl.textContent = theme.description;
        info.appendChild(descEl);

        const authorEl = document.createElement('div');
        authorEl.className = 'theme-card-author';
        authorEl.textContent = theme.author;
        info.appendChild(authorEl);

        card.appendChild(info);

        if (!theme.builtin) {
          const actions = document.createElement('div');
          actions.className = 'theme-card-actions';

          const removeBtn = document.createElement('button');
          removeBtn.className = 'dialog-btn dialog-btn-secondary';
          removeBtn.textContent = 'Remove';
          removeBtn.style.fontSize = '11px';
          removeBtn.style.padding = '2px 10px';
          removeBtn.onclick = (e) => {
            e.stopPropagation();
            themeStore.removeCustomTheme(theme.id);
            renderThemeGrid();
          };
          actions.appendChild(removeBtn);
          card.appendChild(actions);
        }

        card.onclick = () => {
          themeStore.setActiveTheme(theme.id);
          renderThemeGrid();
        };

        themeGrid.appendChild(card);
      }
    }

    renderThemeGrid();

    const importBtn = document.createElement('button');
    importBtn.className = 'dialog-btn dialog-btn-secondary theme-import-btn';
    importBtn.textContent = 'Import Theme (JSON)';
    importBtn.onclick = () => {
      const fileInput = document.createElement('input');
      fileInput.type = 'file';
      fileInput.accept = '.json';
      fileInput.style.display = 'none';
      fileInput.onchange = async () => {
        const file = fileInput.files?.[0];
        if (!file) return;
        try {
          const text = await file.text();
          const parsed = JSON.parse(text) as ThemeDefinition;
          if (
            !parsed.id ||
            !parsed.name ||
            !parsed.terminal ||
            !parsed.ui
          ) {
            alert('Invalid theme file: missing required fields (id, name, terminal, ui).');
            return;
          }
          parsed.builtin = false;
          themeStore.addCustomTheme(parsed);
          renderThemeGrid();
        } catch (err) {
          alert('Failed to import theme: ' + (err instanceof Error ? err.message : String(err)));
        }
        fileInput.remove();
      };
      document.body.appendChild(fileInput);
      fileInput.click();
    };
    content.appendChild(importBtn);

    return content;
  }
}
