import type { GodlyPlugin, PluginContext } from '../types';
import {
  llmGenerateBranchName,
  llmHasApiKey,
  llmSetApiBaseUrl,
  llmSetApiKey,
  llmSetModel,
  llmSetProvider,
  type BranchAiProvider,
} from './llm-service';

const PROVIDER_SETTINGS_KEY = 'llmProvider';
const GEMINI_API_KEY_SETTINGS_KEY = 'geminiApiKey';
const GEMINI_MODEL_SETTINGS_KEY = 'geminiModel';

function normalizeProvider(provider: string | null | undefined): BranchAiProvider {
  return provider === 'openai-compatible' ? 'openai-compatible' : 'gemini';
}

function defaultModelForProvider(provider: BranchAiProvider): string {
  return provider === 'openai-compatible' ? 'gpt-4o-mini' : 'gemini-2.0-flash-lite';
}

function keyStorageKey(provider: BranchAiProvider): string {
  return `llmApiKey.${provider}`;
}

function modelStorageKey(provider: BranchAiProvider): string {
  return `llmModel.${provider}`;
}

function apiBaseUrlStorageKey(provider: BranchAiProvider): string {
  return `llmApiBaseUrl.${provider}`;
}

function providerLabel(provider: BranchAiProvider): string {
  return provider === 'openai-compatible' ? 'OpenAI-Compatible' : 'Google Gemini';
}

export class SmolLM2Plugin implements GodlyPlugin {
  id = 'smollm2';
  name = 'Branch Name AI';
  description = 'Generate branch names from task descriptions with a configurable LLM provider.';
  version = '2.0.0';

  private ctx!: PluginContext;
  private hasKey = false;
  private selectedProvider: BranchAiProvider = 'gemini';
  private selectedModel = defaultModelForProvider('gemini');
  private apiBaseUrl = '';

  private getSavedApiKey(provider: BranchAiProvider): string | null {
    if (provider === 'gemini') {
      return this.ctx.getSetting<string | null>(
        keyStorageKey(provider),
        this.ctx.getSetting<string | null>(GEMINI_API_KEY_SETTINGS_KEY, null),
      );
    }
    return this.ctx.getSetting<string | null>(keyStorageKey(provider), null);
  }

  private setSavedApiKey(provider: BranchAiProvider, key: string | null): void {
    this.ctx.setSetting(keyStorageKey(provider), key);
    if (provider === 'gemini') {
      this.ctx.setSetting(GEMINI_API_KEY_SETTINGS_KEY, key);
    }
  }

  private getSavedModel(provider: BranchAiProvider): string | null {
    if (provider === 'gemini') {
      return this.ctx.getSetting<string | null>(
        modelStorageKey(provider),
        this.ctx.getSetting<string | null>(GEMINI_MODEL_SETTINGS_KEY, null),
      );
    }
    return this.ctx.getSetting<string | null>(modelStorageKey(provider), null);
  }

  private setSavedModel(provider: BranchAiProvider, model: string): void {
    this.ctx.setSetting(modelStorageKey(provider), model);
    if (provider === 'gemini') {
      this.ctx.setSetting(GEMINI_MODEL_SETTINGS_KEY, model);
    }
  }

  private getSavedApiBaseUrl(provider: BranchAiProvider): string | null {
    return this.ctx.getSetting<string | null>(apiBaseUrlStorageKey(provider), null);
  }

  private setSavedApiBaseUrl(provider: BranchAiProvider, apiBaseUrl: string | null): void {
    this.ctx.setSetting(apiBaseUrlStorageKey(provider), apiBaseUrl);
  }

  private getProviderMeta(provider: BranchAiProvider): {
    keyTitle: string;
    keyHint: string;
    modelPlaceholder: string;
    modelLabel: string;
    showBaseUrl: boolean;
    baseUrlPlaceholder: string;
    baseUrlHint: string;
  } {
    if (provider === 'openai-compatible') {
      return {
        keyTitle: 'Provider API Key',
        keyHint: 'Works with OpenAI and any OpenAI-compatible provider.',
        modelLabel: 'Model',
        modelPlaceholder: 'e.g. gpt-4o-mini',
        showBaseUrl: true,
        baseUrlPlaceholder: 'https://api.openai.com/v1/chat/completions',
        baseUrlHint: 'Use a custom endpoint for OpenRouter, Groq, local gateways, etc.',
      };
    }
    return {
      keyTitle: 'Google Gemini API Key',
      keyHint: 'Free tier: 250 req/day. Get a key from Google AI Studio.',
      modelLabel: 'Model',
      modelPlaceholder: 'e.g. gemini-2.0-flash-lite',
      showBaseUrl: false,
      baseUrlPlaceholder: '',
      baseUrlHint: '',
    };
  }

  async init(ctx: PluginContext): Promise<void> {
    this.ctx = ctx;
    try {
      this.hasKey = await llmHasApiKey();
    } catch {
      // Assume not configured
    }
  }

  async enable(): Promise<void> {
    const provider = normalizeProvider(
      this.ctx.getSetting<string | null>(PROVIDER_SETTINGS_KEY, this.selectedProvider),
    );
    this.selectedProvider = provider;
    this.ctx.setSetting(PROVIDER_SETTINGS_KEY, provider);

    const savedKey = this.getSavedApiKey(provider);
    const savedModel = this.getSavedModel(provider) ?? defaultModelForProvider(provider);
    const savedBaseUrl = this.getSavedApiBaseUrl(provider) ?? '';

    this.selectedModel = savedModel;
    this.apiBaseUrl = savedBaseUrl;
    this.hasKey = Boolean(savedKey);

    // Persist migrated/default model so provider switches keep stable defaults.
    this.setSavedModel(provider, savedModel);

    try {
      await llmSetProvider(provider);
      await llmSetApiKey(savedKey);
      await llmSetModel(savedModel);
      await llmSetApiBaseUrl(provider === 'openai-compatible' ? (savedBaseUrl || null) : null);
      this.hasKey = await llmHasApiKey();
    } catch (e) {
      console.warn('[BranchNameAI] Failed to restore provider settings:', e);
    }
  }

  async disable(): Promise<void> {
    // Clear sensitive values from backend memory (settings stay persisted).
    try {
      await llmSetApiKey(null);
      await llmSetApiBaseUrl(null);
      this.hasKey = false;
    } catch {
      // ignore
    }
  }

  destroy(): void {}

  renderSettings(): HTMLElement {
    const container = document.createElement('div');
    container.className = 'smollm2-settings';

    // -- Provider section --
    const providerSection = document.createElement('div');
    providerSection.className = 'settings-section';
    const providerTitle = document.createElement('div');
    providerTitle.className = 'settings-section-title';
    providerTitle.textContent = 'Provider';
    providerSection.appendChild(providerTitle);

    const providerRow = document.createElement('div');
    providerRow.className = 'shortcut-row';
    providerRow.style.gap = '8px';

    const providerInput = document.createElement('select');
    providerInput.className = 'dialog-input';
    providerInput.style.cssText = 'flex: 1; font-size: 12px;';

    const geminiOption = document.createElement('option');
    geminiOption.value = 'gemini';
    geminiOption.textContent = 'Google Gemini';
    providerInput.appendChild(geminiOption);

    const openaiOption = document.createElement('option');
    openaiOption.value = 'openai-compatible';
    openaiOption.textContent = 'OpenAI-Compatible';
    providerInput.appendChild(openaiOption);

    providerInput.value = this.selectedProvider;
    providerRow.appendChild(providerInput);
    providerSection.appendChild(providerRow);
    container.appendChild(providerSection);

    // -- API Key section --
    const keySection = document.createElement('div');
    keySection.className = 'settings-section';
    const keyTitle = document.createElement('div');
    keyTitle.className = 'settings-section-title';
    keySection.appendChild(keyTitle);

    const keyHint = document.createElement('div');
    keyHint.style.cssText =
      'padding: 0 12px; font-size: 10px; color: var(--text-secondary); margin-bottom: 8px;';
    keySection.appendChild(keyHint);

    const keyRow = document.createElement('div');
    keyRow.className = 'shortcut-row';
    keyRow.style.gap = '8px';

    const keyInput = document.createElement('input');
    keyInput.type = 'password';
    keyInput.className = 'dialog-input';
    keyInput.style.cssText = 'flex: 1; font-size: 12px; font-family: monospace;';

    const statusDot = document.createElement('span');
    statusDot.style.cssText = 'font-size: 11px; white-space: nowrap;';

    const saveBtn = document.createElement('button');
    saveBtn.className = 'dialog-btn dialog-btn-primary';
    saveBtn.textContent = 'Save';
    saveBtn.style.fontSize = '11px';

    const updateStatus = () => {
      if (this.hasKey) {
        statusDot.textContent = `Active (${providerLabel(this.selectedProvider)})`;
        statusDot.style.color = 'var(--accent)';
      } else {
        statusDot.textContent = 'Not set';
        statusDot.style.color = 'var(--text-secondary)';
      }
    };

    keyRow.appendChild(keyInput);
    keyRow.appendChild(saveBtn);
    keyRow.appendChild(statusDot);
    keySection.appendChild(keyRow);
    container.appendChild(keySection);

    // -- Model section --
    const modelSection = document.createElement('div');
    modelSection.className = 'settings-section';
    const modelTitle = document.createElement('div');
    modelTitle.className = 'settings-section-title';
    modelSection.appendChild(modelTitle);

    const modelRow = document.createElement('div');
    modelRow.className = 'shortcut-row';
    modelRow.style.gap = '8px';

    const modelInput = document.createElement('input');
    modelInput.type = 'text';
    modelInput.className = 'dialog-input';
    modelInput.style.cssText = 'flex: 1; font-size: 12px;';
    modelRow.appendChild(modelInput);
    modelSection.appendChild(modelRow);
    container.appendChild(modelSection);

    // -- API Base URL section (openai-compatible only) --
    const baseUrlSection = document.createElement('div');
    baseUrlSection.className = 'settings-section';
    const baseUrlTitle = document.createElement('div');
    baseUrlTitle.className = 'settings-section-title';
    baseUrlTitle.textContent = 'API Base URL';
    baseUrlSection.appendChild(baseUrlTitle);

    const baseUrlHint = document.createElement('div');
    baseUrlHint.style.cssText =
      'padding: 0 12px; font-size: 10px; color: var(--text-secondary); margin-bottom: 8px;';
    baseUrlSection.appendChild(baseUrlHint);

    const baseUrlRow = document.createElement('div');
    baseUrlRow.className = 'shortcut-row';
    baseUrlRow.style.gap = '8px';

    const baseUrlInput = document.createElement('input');
    baseUrlInput.type = 'text';
    baseUrlInput.className = 'dialog-input';
    baseUrlInput.style.cssText = 'flex: 1; font-size: 12px; font-family: monospace;';
    baseUrlRow.appendChild(baseUrlInput);
    baseUrlSection.appendChild(baseUrlRow);
    container.appendChild(baseUrlSection);

    const syncUiFromProvider = (provider: BranchAiProvider) => {
      const meta = this.getProviderMeta(provider);
      keyTitle.textContent = meta.keyTitle;
      keyHint.textContent = meta.keyHint;
      modelTitle.textContent = meta.modelLabel;
      modelInput.placeholder = meta.modelPlaceholder;
      baseUrlHint.textContent = meta.baseUrlHint;
      baseUrlInput.placeholder = meta.baseUrlPlaceholder;
      baseUrlSection.style.display = meta.showBaseUrl ? '' : 'none';
    };

    const loadProviderStateIntoFields = (provider: BranchAiProvider) => {
      const savedKey = this.getSavedApiKey(provider) ?? '';
      const savedModel = this.getSavedModel(provider) ?? defaultModelForProvider(provider);
      const savedBaseUrl = this.getSavedApiBaseUrl(provider) ?? '';

      this.selectedModel = savedModel;
      this.apiBaseUrl = savedBaseUrl;
      this.hasKey = Boolean(savedKey);

      keyInput.value = savedKey;
      modelInput.value = savedModel;
      baseUrlInput.value = savedBaseUrl;
      updateStatus();
    };

    const applyProviderToBackend = async (provider: BranchAiProvider): Promise<void> => {
      const key = keyInput.value.trim();
      const model = modelInput.value.trim() || defaultModelForProvider(provider);
      const apiBaseUrl = baseUrlInput.value.trim();

      await llmSetProvider(provider);
      await llmSetApiKey(key || null);
      await llmSetModel(model);
      await llmSetApiBaseUrl(provider === 'openai-compatible' ? (apiBaseUrl || null) : null);
    };

    providerInput.onchange = async () => {
      const provider = normalizeProvider(providerInput.value);
      providerInput.disabled = true;

      this.selectedProvider = provider;
      this.ctx.setSetting(PROVIDER_SETTINGS_KEY, provider);
      syncUiFromProvider(provider);
      loadProviderStateIntoFields(provider);

      // Ensure a default model exists in settings for this provider.
      if (!this.getSavedModel(provider)) {
        this.setSavedModel(provider, this.selectedModel);
      }

      try {
        await applyProviderToBackend(provider);
        this.hasKey = await llmHasApiKey();
        updateStatus();
      } catch (e) {
        statusDot.textContent = `Error: ${e}`;
        statusDot.style.color = 'var(--error)';
      } finally {
        providerInput.disabled = false;
      }
    };

    saveBtn.onclick = async () => {
      const key = keyInput.value.trim();
      saveBtn.disabled = true;
      saveBtn.textContent = 'Saving...';
      try {
        await llmSetApiKey(key || null);
        this.setSavedApiKey(this.selectedProvider, key || null);
        this.hasKey = Boolean(key);
        updateStatus();
      } catch (e) {
        statusDot.textContent = `Error: ${e}`;
        statusDot.style.color = 'var(--error)';
      } finally {
        saveBtn.disabled = false;
        saveBtn.textContent = 'Save';
      }
    };

    modelInput.onchange = async () => {
      const model = modelInput.value.trim();
      if (!model) return;
      this.selectedModel = model;
      this.setSavedModel(this.selectedProvider, model);
      try {
        await llmSetModel(model);
      } catch (e) {
        console.warn('[BranchNameAI] Failed to set model:', e);
      }
    };

    baseUrlInput.onchange = async () => {
      if (this.selectedProvider !== 'openai-compatible') return;
      const apiBaseUrl = baseUrlInput.value.trim();
      this.apiBaseUrl = apiBaseUrl;
      this.setSavedApiBaseUrl(this.selectedProvider, apiBaseUrl || null);
      try {
        await llmSetApiBaseUrl(apiBaseUrl || null);
      } catch (e) {
        console.warn('[BranchNameAI] Failed to set API base URL:', e);
      }
    };

    syncUiFromProvider(this.selectedProvider);
    loadProviderStateIntoFields(this.selectedProvider);

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
