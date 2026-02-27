import type { GodlyPlugin, PluginContext } from '../types';
import {
  llmHasApiKey,
  llmSetApiKey,
  llmSetModel,
  llmGenerateBranchName,
} from './llm-service';

export class SmolLM2Plugin implements GodlyPlugin {
  id = 'smollm2';
  name = 'Branch Name AI';
  description = 'Generate branch names from task descriptions using Google Gemini Flash (free tier).';
  version = '2.0.0';

  private ctx!: PluginContext;
  private hasKey = false;
  private selectedModel = 'gemini-2.0-flash-lite';

  static readonly MODELS: { id: string; label: string }[] = [
    { id: 'gemini-2.0-flash-lite', label: 'Gemini 2.0 Flash Lite (fastest, free)' },
    { id: 'gemini-2.0-flash', label: 'Gemini 2.0 Flash (balanced)' },
    { id: 'gemini-1.5-flash', label: 'Gemini 1.5 Flash' },
    { id: 'gemini-1.5-pro', label: 'Gemini 1.5 Pro (highest quality)' },
  ];

  async init(ctx: PluginContext): Promise<void> {
    this.ctx = ctx;
    try {
      this.hasKey = await llmHasApiKey();
    } catch {
      // Assume no key
    }
  }

  async enable(): Promise<void> {
    // Restore saved API key into backend state
    const savedKey = this.ctx.getSetting<string | null>('geminiApiKey', null);
    if (savedKey) {
      try {
        await llmSetApiKey(savedKey);
        this.hasKey = true;
      } catch (e) {
        console.warn('[BranchNameAI] Failed to restore API key:', e);
      }
    }
    // Restore saved model
    const savedModel = this.ctx.getSetting<string | null>('geminiModel', null);
    if (savedModel) {
      this.selectedModel = savedModel;
      try {
        await llmSetModel(savedModel);
      } catch (e) {
        console.warn('[BranchNameAI] Failed to restore model:', e);
      }
    }
  }

  async disable(): Promise<void> {
    // Clear API key from backend memory (stays in settings for next enable)
    try {
      await llmSetApiKey(null);
      this.hasKey = false;
    } catch {
      // ignore
    }
  }

  destroy(): void {}

  renderSettings(): HTMLElement {
    const container = document.createElement('div');
    container.className = 'smollm2-settings';

    // -- API Key section --
    const keySection = document.createElement('div');
    keySection.className = 'settings-section';
    const keyTitle = document.createElement('div');
    keyTitle.className = 'settings-section-title';
    keyTitle.textContent = 'Google Gemini API Key';
    keySection.appendChild(keyTitle);

    const keyHint = document.createElement('div');
    keyHint.style.cssText = 'padding: 0 12px; font-size: 10px; color: var(--text-secondary); margin-bottom: 8px;';
    keyHint.textContent = 'Free tier: 250 req/day. Get a key from Google AI Studio.';
    keySection.appendChild(keyHint);

    const keyRow = document.createElement('div');
    keyRow.className = 'shortcut-row';
    keyRow.style.gap = '8px';

    const keyInput = document.createElement('input');
    keyInput.type = 'password';
    keyInput.className = 'dialog-input';
    keyInput.placeholder = 'AIza...';
    keyInput.style.cssText = 'flex: 1; font-size: 12px; font-family: monospace;';

    // Show saved key if exists
    const savedKey = this.ctx.getSetting<string | null>('geminiApiKey', null);
    if (savedKey) {
      keyInput.value = savedKey;
    }

    const statusDot = document.createElement('span');
    statusDot.style.cssText = 'font-size: 11px; white-space: nowrap;';
    const updateStatus = () => {
      if (this.hasKey) {
        statusDot.textContent = 'Active';
        statusDot.style.color = 'var(--accent)';
      } else {
        statusDot.textContent = 'Not set';
        statusDot.style.color = 'var(--text-secondary)';
      }
    };
    updateStatus();

    const saveBtn = document.createElement('button');
    saveBtn.className = 'dialog-btn dialog-btn-primary';
    saveBtn.textContent = 'Save';
    saveBtn.style.fontSize = '11px';
    saveBtn.onclick = async () => {
      const key = keyInput.value.trim();
      saveBtn.disabled = true;
      saveBtn.textContent = 'Saving...';
      try {
        if (key) {
          await llmSetApiKey(key);
          this.ctx.setSetting('geminiApiKey', key);
          this.hasKey = true;
        } else {
          await llmSetApiKey(null);
          this.ctx.setSetting('geminiApiKey', null);
          this.hasKey = false;
        }
        updateStatus();
      } catch (e) {
        statusDot.textContent = `Error: ${e}`;
        statusDot.style.color = 'var(--error)';
      } finally {
        saveBtn.disabled = false;
        saveBtn.textContent = 'Save';
      }
    };

    keyRow.appendChild(keyInput);
    keyRow.appendChild(saveBtn);
    keyRow.appendChild(statusDot);
    keySection.appendChild(keyRow);
    container.appendChild(keySection);

    // -- Model selector section --
    const modelSection = document.createElement('div');
    modelSection.className = 'settings-section';
    const modelTitle = document.createElement('div');
    modelTitle.className = 'settings-section-title';
    modelTitle.textContent = 'Gemini Model';
    modelSection.appendChild(modelTitle);

    const modelRow = document.createElement('div');
    modelRow.className = 'shortcut-row';
    modelRow.style.gap = '8px';

    const modelSelect = document.createElement('select');
    modelSelect.className = 'dialog-input';
    modelSelect.style.cssText = 'flex: 1; font-size: 12px;';
    for (const m of SmolLM2Plugin.MODELS) {
      const opt = document.createElement('option');
      opt.value = m.id;
      opt.textContent = m.label;
      if (m.id === this.selectedModel) opt.selected = true;
      modelSelect.appendChild(opt);
    }
    modelSelect.onchange = async () => {
      const model = modelSelect.value;
      this.selectedModel = model;
      this.ctx.setSetting('geminiModel', model);
      try {
        await llmSetModel(model);
      } catch (e) {
        console.warn('[BranchNameAI] Failed to set model:', e);
      }
    };

    modelRow.appendChild(modelSelect);
    modelSection.appendChild(modelRow);
    container.appendChild(modelSection);

    // -- Test Generation section --
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
      testBtn.textContent = 'Generating...';
      testResult.textContent = '';
      try {
        const result = await llmGenerateBranchName(desc);
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
}
