import type { GodlyPlugin, PluginContext } from '../types';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import {
  llmGetStatus,
  llmDownloadModel,
  llmLoadModel,
  llmUnloadModel,
  llmGenerateBranchName,
  llmCheckModelFiles,
  isModelReady,
  isModelDownloaded,
  getStatusLabel,
  type LlmStatus,
} from './llm-service';
import { MODEL_PRESETS, DEFAULT_PRESET_ID, type ModelPreset } from './model-presets';

export class SmolLM2Plugin implements GodlyPlugin {
  id = 'smollm2';
  name = 'SmolLM2 Local LLM';
  description = 'Run SmolLM2-135M locally for AI-powered branch name suggestions. ~110MB download, runs on CPU.';
  version = '1.0.0';

  private ctx!: PluginContext;
  private status: LlmStatus = { status: 'NotDownloaded' };

  async init(ctx: PluginContext): Promise<void> {
    this.ctx = ctx;
    try {
      this.status = await llmGetStatus();
    } catch {
      // Status check failed, assume not downloaded
    }
  }

  async enable(): Promise<void> {
    const autoLoad = this.ctx.getSetting('autoLoad', true);
    if (autoLoad && isModelDownloaded(this.status) && !isModelReady(this.status)) {
      try {
        // Load using the saved model source settings
        const modelSource = this.ctx.getSetting<string>('modelSource', 'preset');
        if (modelSource === 'custom') {
          const ggufPath = this.ctx.getSetting<string | null>('customGgufPath', null);
          const tokenizerPath = this.ctx.getSetting<string | null>('customTokenizerPath', null);
          if (ggufPath && tokenizerPath) {
            await llmLoadModel({ ggufPath, tokenizerPath });
          }
        } else {
          const presetId = this.ctx.getSetting<string>('selectedPreset', DEFAULT_PRESET_ID);
          const preset = MODEL_PRESETS.find(p => p.id === presetId);
          if (preset) {
            await llmLoadModel({ subdir: preset.subdir, ggufFilename: preset.hfFilename });
          } else {
            await llmLoadModel();
          }
        }
        this.status = await llmGetStatus();
      } catch (e) {
        console.warn('[SmolLM2] Auto-load failed:', e);
      }
    }
  }

  async disable(): Promise<void> {
    if (isModelReady(this.status)) {
      try {
        await llmUnloadModel();
        this.status = await llmGetStatus();
      } catch (e) {
        console.warn('[SmolLM2] Unload failed:', e);
      }
    }
  }

  destroy(): void {
    // Nothing to clean up
  }

  renderSettings(): HTMLElement {
    const container = document.createElement('div');
    container.className = 'smollm2-settings';

    // ── Status indicator ──
    const statusRow = this.createRow('Status');
    const statusValue = document.createElement('span');
    statusValue.className = 'shortcut-keys';
    statusValue.textContent = getStatusLabel(this.status);
    statusRow.appendChild(statusValue);
    container.appendChild(statusRow);

    // ── Section A: Branch Name Engine Toggle ──
    const engineSection = document.createElement('div');
    engineSection.className = 'settings-section';
    const engineTitle = document.createElement('div');
    engineTitle.className = 'settings-section-title';
    engineTitle.textContent = 'Branch Name Engine';
    engineSection.appendChild(engineTitle);

    const engineRow = this.createRow('Engine');
    const engineSelect = document.createElement('select');
    engineSelect.className = 'dialog-input';
    engineSelect.style.cssText = 'width: auto; font-size: 12px; padding: 4px 8px;';

    const currentEngine = this.ctx.getSetting<string>('branchNameEngine', 'tiny');

    for (const opt of [
      { value: 'tiny', label: 'Tiny (branch-name-gen) — Fast, ~20M params' },
      { value: 'smollm2', label: 'SmolLM2 — Better quality, requires model loaded' },
    ]) {
      const option = document.createElement('option');
      option.value = opt.value;
      option.textContent = opt.label;
      if (opt.value === currentEngine) option.selected = true;
      engineSelect.appendChild(option);
    }
    engineSelect.onchange = () => {
      this.ctx.setSetting('branchNameEngine', engineSelect.value);
    };
    engineRow.appendChild(engineSelect);
    engineSection.appendChild(engineRow);
    container.appendChild(engineSection);

    // ── Section B: Model Source ──
    const modelSection = document.createElement('div');
    modelSection.className = 'settings-section';
    const modelTitle = document.createElement('div');
    modelTitle.className = 'settings-section-title';
    modelTitle.textContent = 'Model Source';
    modelSection.appendChild(modelTitle);

    const modelSource = this.ctx.getSetting<string>('modelSource', 'preset');
    const presetContent = document.createElement('div');
    const customContent = document.createElement('div');

    // Source toggle
    const sourceRow = this.createRow('Source');
    const sourceSelect = document.createElement('select');
    sourceSelect.className = 'dialog-input';
    sourceSelect.style.cssText = 'width: auto; font-size: 12px; padding: 4px 8px;';
    for (const opt of [
      { value: 'preset', label: 'Preset Models' },
      { value: 'custom', label: 'Custom GGUF File' },
    ]) {
      const option = document.createElement('option');
      option.value = opt.value;
      option.textContent = opt.label;
      if (opt.value === modelSource) option.selected = true;
      sourceSelect.appendChild(option);
    }
    sourceSelect.onchange = () => {
      this.ctx.setSetting('modelSource', sourceSelect.value);
      presetContent.style.display = sourceSelect.value === 'preset' ? '' : 'none';
      customContent.style.display = sourceSelect.value === 'custom' ? '' : 'none';
    };
    sourceRow.appendChild(sourceSelect);
    modelSection.appendChild(sourceRow);

    // ── Preset content ──
    this.buildPresetContent(presetContent, statusValue);
    presetContent.style.display = modelSource === 'preset' ? '' : 'none';
    modelSection.appendChild(presetContent);

    // ── Custom content ──
    this.buildCustomContent(customContent, statusValue);
    customContent.style.display = modelSource === 'custom' ? '' : 'none';
    modelSection.appendChild(customContent);

    container.appendChild(modelSection);

    // ── Auto-load toggle ──
    const autoLoadRow = this.createRow('Auto-load on enable');
    const autoLoadCheckbox = document.createElement('input');
    autoLoadCheckbox.type = 'checkbox';
    autoLoadCheckbox.className = 'notification-checkbox';
    autoLoadCheckbox.checked = this.ctx.getSetting('autoLoad', true);
    autoLoadCheckbox.onchange = () => {
      this.ctx.setSetting('autoLoad', autoLoadCheckbox.checked);
    };
    autoLoadRow.appendChild(autoLoadCheckbox);
    container.appendChild(autoLoadRow);

    // ── Section C: Test Generation ──
    const testSection = document.createElement('div');
    testSection.className = 'settings-section';
    const testTitle = document.createElement('div');
    testTitle.className = 'settings-section-title';
    testTitle.textContent = 'Test Generation';
    testSection.appendChild(testTitle);

    const testRow = document.createElement('div');
    testRow.className = 'shortcut-row';
    testRow.style.flexDirection = 'column';
    testRow.style.alignItems = 'stretch';
    testRow.style.gap = '8px';

    const testInput = document.createElement('input');
    testInput.type = 'text';
    testInput.className = 'dialog-input';
    testInput.placeholder = 'Describe a feature to generate a branch name...';
    testInput.style.fontSize = '12px';
    testRow.appendChild(testInput);

    const testResultRow = document.createElement('div');
    testResultRow.style.display = 'flex';
    testResultRow.style.gap = '8px';
    testResultRow.style.alignItems = 'center';

    const testBtn = document.createElement('button');
    testBtn.className = 'dialog-btn dialog-btn-secondary';
    testBtn.textContent = 'Generate Branch Name';
    testBtn.style.fontSize = '11px';
    testBtn.onclick = async () => {
      const desc = testInput.value.trim();
      if (!desc) return;
      testBtn.disabled = true;
      testBtn.textContent = 'Thinking...';
      testResult.textContent = '';
      try {
        const engine = this.ctx.getSetting<string>('branchNameEngine', 'tiny');
        const result = await llmGenerateBranchName(desc, engine === 'tiny');
        testResult.textContent = result;
        testResult.style.color = 'var(--accent)';
      } catch (e) {
        testResult.textContent = `Error: ${e}`;
        testResult.style.color = 'var(--error)';
      } finally {
        testBtn.disabled = false;
        testBtn.textContent = 'Generate Branch Name';
      }
    };
    testResultRow.appendChild(testBtn);

    const testResult = document.createElement('span');
    testResult.style.cssText = 'font-family: monospace; font-size: 12px;';
    testResultRow.appendChild(testResult);

    testRow.appendChild(testResultRow);
    testSection.appendChild(testRow);
    container.appendChild(testSection);

    return container;
  }

  // ── Helper: create a shortcut-row with a label ──
  private createRow(label: string): HTMLElement {
    const row = document.createElement('div');
    row.className = 'shortcut-row';
    const lbl = document.createElement('span');
    lbl.className = 'shortcut-label';
    lbl.textContent = label;
    row.appendChild(lbl);
    return row;
  }

  // ── Build preset dropdown + download/load buttons ──
  private buildPresetContent(container: HTMLElement, statusValue: HTMLElement): void {
    const selectedPresetId = this.ctx.getSetting<string>('selectedPreset', DEFAULT_PRESET_ID);

    // Preset dropdown
    const presetRow = this.createRow('Model');
    const presetSelect = document.createElement('select');
    presetSelect.className = 'dialog-input';
    presetSelect.style.cssText = 'width: auto; font-size: 12px; padding: 4px 8px;';

    for (const preset of MODEL_PRESETS) {
      const option = document.createElement('option');
      option.value = preset.id;
      option.textContent = `${preset.label} — ${preset.size}`;
      if (preset.id === selectedPresetId) option.selected = true;
      presetSelect.appendChild(option);
    }
    presetSelect.onchange = () => {
      this.ctx.setSetting('selectedPreset', presetSelect.value);
      updatePresetButtons();
    };
    presetRow.appendChild(presetSelect);
    container.appendChild(presetRow);

    // Quality hint
    const hintRow = document.createElement('div');
    hintRow.style.cssText = 'padding: 0 12px; font-size: 10px; color: var(--text-secondary);';
    const updateHint = () => {
      const preset = MODEL_PRESETS.find(p => p.id === presetSelect.value);
      hintRow.textContent = preset ? preset.quality : '';
    };
    presetSelect.addEventListener('change', updateHint);
    updateHint();
    container.appendChild(hintRow);

    // Action buttons
    const actionRow = this.createRow('');
    const btnContainer = document.createElement('div');
    btnContainer.style.display = 'flex';
    btnContainer.style.gap = '8px';
    btnContainer.style.alignItems = 'center';

    const progressBar = document.createElement('div');
    progressBar.style.cssText = 'width: 100px; height: 6px; background: var(--bg-tertiary); border-radius: 3px; overflow: hidden; display: none;';
    const progressFill = document.createElement('div');
    progressFill.style.cssText = 'height: 100%; background: var(--accent); width: 0%; transition: width 0.3s;';
    progressBar.appendChild(progressFill);

    const getSelectedPreset = (): ModelPreset | undefined =>
      MODEL_PRESETS.find(p => p.id === presetSelect.value);

    const updatePresetButtons = async () => {
      btnContainer.innerHTML = '';
      const preset = getSelectedPreset();
      if (!preset) return;

      // Check if this preset's files exist
      let downloaded = false;
      try {
        downloaded = await llmCheckModelFiles({
          subdir: preset.subdir,
          ggufFilename: preset.hfFilename,
        });
      } catch { /* assume not downloaded */ }

      if (!downloaded) {
        const downloadBtn = document.createElement('button');
        downloadBtn.className = 'dialog-btn dialog-btn-primary';
        downloadBtn.textContent = `Download (${preset.size})`;
        downloadBtn.onclick = async () => {
          downloadBtn.disabled = true;
          downloadBtn.textContent = 'Downloading...';
          progressBar.style.display = 'block';

          const unlisten = await listen<number>('llm-download-progress', (event) => {
            progressFill.style.width = `${Math.round(event.payload * 100)}%`;
          });

          try {
            await llmDownloadModel(
              preset.hfRepo,
              preset.hfFilename,
              preset.tokenizerRepo,
              preset.subdir,
            );
            this.status = await llmGetStatus();
            statusValue.textContent = getStatusLabel(this.status);
          } catch (e) {
            statusValue.textContent = `Error: ${e}`;
          } finally {
            unlisten();
            progressBar.style.display = 'none';
            updatePresetButtons();
          }
        };
        btnContainer.appendChild(downloadBtn);
        btnContainer.appendChild(progressBar);
      } else if (!isModelReady(this.status)) {
        const loadBtn = document.createElement('button');
        loadBtn.className = 'dialog-btn dialog-btn-primary';
        loadBtn.textContent = 'Load Model';
        loadBtn.onclick = async () => {
          loadBtn.disabled = true;
          loadBtn.textContent = 'Loading...';
          try {
            await llmLoadModel({ subdir: preset.subdir, ggufFilename: preset.hfFilename });
            this.status = await llmGetStatus();
            statusValue.textContent = getStatusLabel(this.status);
          } catch (e) {
            statusValue.textContent = `Error: ${e}`;
          }
          updatePresetButtons();
        };
        btnContainer.appendChild(loadBtn);
      } else {
        const unloadBtn = document.createElement('button');
        unloadBtn.className = 'dialog-btn dialog-btn-secondary';
        unloadBtn.textContent = 'Unload Model';
        unloadBtn.onclick = async () => {
          try {
            await llmUnloadModel();
            this.status = await llmGetStatus();
            statusValue.textContent = getStatusLabel(this.status);
          } catch (e) {
            statusValue.textContent = `Error: ${e}`;
          }
          updatePresetButtons();
        };
        btnContainer.appendChild(unloadBtn);
      }
    };

    actionRow.appendChild(btnContainer);
    container.appendChild(actionRow);

    // Initial button state
    updatePresetButtons();
  }

  // ── Build custom GGUF file picker ──
  private buildCustomContent(container: HTMLElement, statusValue: HTMLElement): void {
    let customGguf = this.ctx.getSetting<string | null>('customGgufPath', null);
    let customTokenizer = this.ctx.getSetting<string | null>('customTokenizerPath', null);

    // GGUF file picker
    const ggufRow = this.createRow('GGUF File');
    const ggufLabel = document.createElement('span');
    ggufLabel.style.cssText = 'font-size: 11px; font-family: monospace; color: var(--text-secondary); max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;';
    ggufLabel.textContent = customGguf ? this.basename(customGguf) : 'None selected';
    ggufLabel.title = customGguf || '';

    const ggufBtn = document.createElement('button');
    ggufBtn.className = 'dialog-btn dialog-btn-secondary';
    ggufBtn.textContent = 'Choose...';
    ggufBtn.style.fontSize = '11px';
    ggufBtn.onclick = async () => {
      const result = await open({
        filters: [{ name: 'GGUF Models', extensions: ['gguf'] }],
        multiple: false,
      });
      if (result) {
        customGguf = result as string;
        this.ctx.setSetting('customGgufPath', customGguf);
        ggufLabel.textContent = this.basename(customGguf);
        ggufLabel.title = customGguf;
        updateLoadBtn();
      }
    };

    const ggufContainer = document.createElement('div');
    ggufContainer.style.display = 'flex';
    ggufContainer.style.gap = '8px';
    ggufContainer.style.alignItems = 'center';
    ggufContainer.appendChild(ggufLabel);
    ggufContainer.appendChild(ggufBtn);
    ggufRow.appendChild(ggufContainer);
    container.appendChild(ggufRow);

    // Tokenizer file picker
    const tokRow = this.createRow('Tokenizer');
    const tokLabel = document.createElement('span');
    tokLabel.style.cssText = 'font-size: 11px; font-family: monospace; color: var(--text-secondary); max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;';
    tokLabel.textContent = customTokenizer ? this.basename(customTokenizer) : 'None selected';
    tokLabel.title = customTokenizer || '';

    const tokBtn = document.createElement('button');
    tokBtn.className = 'dialog-btn dialog-btn-secondary';
    tokBtn.textContent = 'Choose...';
    tokBtn.style.fontSize = '11px';
    tokBtn.onclick = async () => {
      const result = await open({
        filters: [{ name: 'Tokenizer JSON', extensions: ['json'] }],
        multiple: false,
      });
      if (result) {
        customTokenizer = result as string;
        this.ctx.setSetting('customTokenizerPath', customTokenizer);
        tokLabel.textContent = this.basename(customTokenizer);
        tokLabel.title = customTokenizer;
        updateLoadBtn();
      }
    };

    const tokContainer = document.createElement('div');
    tokContainer.style.display = 'flex';
    tokContainer.style.gap = '8px';
    tokContainer.style.alignItems = 'center';
    tokContainer.appendChild(tokLabel);
    tokContainer.appendChild(tokBtn);
    tokRow.appendChild(tokContainer);
    container.appendChild(tokRow);

    // Load/Unload button
    const actionRow = this.createRow('');
    const loadBtn = document.createElement('button');
    loadBtn.className = 'dialog-btn dialog-btn-primary';

    const updateLoadBtn = () => {
      actionRow.querySelectorAll('button').forEach(b => b.remove());

      if (isModelReady(this.status)) {
        const unloadBtn = document.createElement('button');
        unloadBtn.className = 'dialog-btn dialog-btn-secondary';
        unloadBtn.textContent = 'Unload Model';
        unloadBtn.onclick = async () => {
          try {
            await llmUnloadModel();
            this.status = await llmGetStatus();
            statusValue.textContent = getStatusLabel(this.status);
          } catch (e) {
            statusValue.textContent = `Error: ${e}`;
          }
          updateLoadBtn();
        };
        actionRow.appendChild(unloadBtn);
      } else {
        const btn = document.createElement('button');
        btn.className = 'dialog-btn dialog-btn-primary';
        btn.textContent = 'Load Custom Model';
        btn.disabled = !customGguf || !customTokenizer;
        btn.onclick = async () => {
          if (!customGguf || !customTokenizer) return;
          btn.disabled = true;
          btn.textContent = 'Loading...';
          try {
            await llmLoadModel({ ggufPath: customGguf!, tokenizerPath: customTokenizer! });
            this.status = await llmGetStatus();
            statusValue.textContent = getStatusLabel(this.status);
          } catch (e) {
            statusValue.textContent = `Error: ${e}`;
          }
          updateLoadBtn();
        };
        actionRow.appendChild(btn);
      }
    };

    container.appendChild(actionRow);
    updateLoadBtn();
  }

  private basename(path: string): string {
    return path.split(/[\\/]/).pop() || path;
  }
}
