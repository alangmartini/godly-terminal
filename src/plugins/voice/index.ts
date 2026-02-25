import type { GodlyPlugin, PluginContext } from '../types';
import {
  whisperGetStatus,
  whisperGetConfig,
  whisperSetConfig,
  whisperLoadModel,
  whisperListModels,
  whisperStartRecording,
  whisperStopRecording,
  type WhisperStatus,
} from './whisper-service';
import { WHISPER_MODEL_PRESETS } from './model-presets';

export class VoiceToTextPlugin implements GodlyPlugin {
  id = 'voice-to-text';
  name = 'Voice to Text';
  description = 'Dictate text into the terminal using Whisper speech-to-text';
  version = '1.0.0';

  private ctx!: PluginContext;
  private status: WhisperStatus | null = null;

  async init(ctx: PluginContext): Promise<void> {
    this.ctx = ctx;
    try {
      this.status = await whisperGetStatus();
    } catch {
      // Sidecar not running yet — that's ok
      this.status = null;
    }
  }

  async enable(): Promise<void> {
    try {
      this.status = await whisperGetStatus();
    } catch {
      // Will retry on first use
    }
  }

  async disable(): Promise<void> {
    if (this.status?.state === 'recording') {
      try {
        await whisperStopRecording();
      } catch (e) {
        console.warn('[VoiceToText] Failed to stop recording on disable:', e);
      }
    }
  }

  destroy(): void {
    // Nothing to clean up
  }

  renderSettings(): HTMLElement {
    const container = document.createElement('div');
    container.className = 'voice-plugin-settings';

    // ── Status indicator ──
    const statusRow = this.createRow('Status');
    const statusValue = document.createElement('span');
    statusValue.className = 'shortcut-keys';
    statusValue.textContent = 'Checking...';
    statusRow.appendChild(statusValue);
    container.appendChild(statusRow);

    // ── Section A: Model ──
    const modelSection = document.createElement('div');
    modelSection.className = 'settings-section';
    const modelTitle = document.createElement('div');
    modelTitle.className = 'settings-section-title';
    modelTitle.textContent = 'Model';
    modelSection.appendChild(modelTitle);

    // Model dropdown
    const modelRow = this.createRow('Model');
    const modelSelect = document.createElement('select');
    modelSelect.className = 'dialog-input';
    modelSelect.style.cssText = 'width: auto; font-size: 12px; padding: 4px 8px;';

    for (const preset of WHISPER_MODEL_PRESETS) {
      const option = document.createElement('option');
      option.value = preset.fileName;
      option.textContent = `${preset.name} (${preset.size})`;
      if (preset.recommended) option.selected = true;
      modelSelect.appendChild(option);
    }
    modelRow.appendChild(modelSelect);
    modelSection.appendChild(modelRow);

    // Model description hint
    const descRow = document.createElement('div');
    descRow.style.cssText = 'padding: 0 12px; font-size: 10px; color: var(--text-secondary);';
    const updateDesc = () => {
      const preset = WHISPER_MODEL_PRESETS.find(p => p.fileName === modelSelect.value);
      descRow.textContent = preset ? preset.description : '';
    };
    modelSelect.addEventListener('change', updateDesc);
    updateDesc();
    modelSection.appendChild(descRow);

    // Available models display
    const availableRow = document.createElement('div');
    availableRow.style.cssText = 'padding: 4px 12px; font-size: 10px; color: var(--text-secondary);';
    modelSection.appendChild(availableRow);

    // Load model button
    const loadRow = this.createRow('');
    const loadBtn = document.createElement('button');
    loadBtn.className = 'dialog-btn dialog-btn-primary';
    loadBtn.textContent = 'Load Model';
    loadBtn.onclick = async () => {
      loadBtn.disabled = true;
      loadBtn.textContent = 'Loading...';
      try {
        const gpuCheckbox = container.querySelector('.voice-gpu-checkbox') as HTMLInputElement;
        const gpuDeviceInput = container.querySelector('.voice-gpu-device-input') as HTMLInputElement;
        const langSelect = container.querySelector('.voice-language-select') as HTMLSelectElement;
        await whisperLoadModel(
          modelSelect.value,
          gpuCheckbox?.checked ?? true,
          parseInt(gpuDeviceInput?.value ?? '0') || 0,
          langSelect?.value ?? '',
        );
        this.status = await whisperGetStatus();
        this.updateStatusDisplay(statusValue);
        loadBtn.textContent = 'Loaded!';
        setTimeout(() => { loadBtn.textContent = 'Load Model'; loadBtn.disabled = false; }, 2000);
      } catch (e) {
        loadBtn.textContent = 'Error';
        console.warn('[VoiceToText] Load model failed:', e);
        setTimeout(() => { loadBtn.textContent = 'Load Model'; loadBtn.disabled = false; }, 2000);
      }
    };
    loadRow.appendChild(loadBtn);
    modelSection.appendChild(loadRow);

    container.appendChild(modelSection);

    // ── Section B: GPU Acceleration ──
    const gpuSection = document.createElement('div');
    gpuSection.className = 'settings-section';
    const gpuTitle = document.createElement('div');
    gpuTitle.className = 'settings-section-title';
    gpuTitle.textContent = 'GPU Acceleration';
    gpuSection.appendChild(gpuTitle);

    const gpuRow = this.createRow('Use GPU (CUDA)');
    const gpuCheckbox = document.createElement('input');
    gpuCheckbox.type = 'checkbox';
    gpuCheckbox.className = 'notification-checkbox voice-gpu-checkbox';
    gpuCheckbox.checked = true;
    gpuRow.appendChild(gpuCheckbox);
    gpuSection.appendChild(gpuRow);

    const deviceRow = this.createRow('GPU Device');
    const gpuDeviceInput = document.createElement('input');
    gpuDeviceInput.type = 'number';
    gpuDeviceInput.className = 'dialog-input voice-gpu-device-input';
    gpuDeviceInput.style.cssText = 'width: 60px; font-size: 12px; padding: 4px 8px;';
    gpuDeviceInput.value = '0';
    gpuDeviceInput.min = '0';
    gpuDeviceInput.max = '7';
    deviceRow.appendChild(gpuDeviceInput);
    gpuSection.appendChild(deviceRow);

    container.appendChild(gpuSection);

    // ── Section C: Language ──
    const langSection = document.createElement('div');
    langSection.className = 'settings-section';
    const langTitle = document.createElement('div');
    langTitle.className = 'settings-section-title';
    langTitle.textContent = 'Language';
    langSection.appendChild(langTitle);

    const langRow = this.createRow('Language');
    const langSelect = document.createElement('select');
    langSelect.className = 'dialog-input voice-language-select';
    langSelect.style.cssText = 'width: auto; font-size: 12px; padding: 4px 8px;';

    for (const lang of [
      { value: '', label: 'Auto-detect' },
      { value: 'en', label: 'English' },
      { value: 'es', label: 'Spanish' },
      { value: 'fr', label: 'French' },
      { value: 'de', label: 'German' },
      { value: 'pt', label: 'Portuguese' },
      { value: 'zh', label: 'Chinese' },
      { value: 'ja', label: 'Japanese' },
      { value: 'ko', label: 'Korean' },
    ]) {
      const option = document.createElement('option');
      option.value = lang.value;
      option.textContent = lang.label;
      langSelect.appendChild(option);
    }
    langRow.appendChild(langSelect);
    langSection.appendChild(langRow);

    container.appendChild(langSection);

    // Save config on GPU/language/model changes
    const saveConfig = async () => {
      try {
        await whisperSetConfig({
          modelName: modelSelect.value,
          language: langSelect.value,
          useGpu: gpuCheckbox.checked,
          gpuDevice: parseInt(gpuDeviceInput.value) || 0,
        });
      } catch {
        // Config save failed silently
      }
    };
    gpuCheckbox.addEventListener('change', saveConfig);
    gpuDeviceInput.addEventListener('change', saveConfig);
    langSelect.addEventListener('change', saveConfig);

    // ── Section D: Test Recording ──
    const testSection = document.createElement('div');
    testSection.className = 'settings-section';
    const testTitle = document.createElement('div');
    testTitle.className = 'settings-section-title';
    testTitle.textContent = 'Test';
    testSection.appendChild(testTitle);

    const testRow = document.createElement('div');
    testRow.className = 'shortcut-row';
    testRow.style.flexDirection = 'column';
    testRow.style.alignItems = 'stretch';
    testRow.style.gap = '8px';

    const testResultRow = document.createElement('div');
    testResultRow.style.display = 'flex';
    testResultRow.style.gap = '8px';
    testResultRow.style.alignItems = 'center';

    const testBtn = document.createElement('button');
    testBtn.className = 'dialog-btn dialog-btn-secondary';
    testBtn.textContent = 'Test Recording (3s)';
    testBtn.style.fontSize = '11px';

    const testResult = document.createElement('span');
    testResult.style.cssText = 'font-family: monospace; font-size: 12px;';

    testBtn.onclick = async () => {
      testBtn.disabled = true;
      testBtn.textContent = 'Recording...';
      testResult.textContent = '';
      testResult.style.color = '';
      try {
        await whisperStartRecording();
        await new Promise(resolve => setTimeout(resolve, 3000));
        testBtn.textContent = 'Transcribing...';
        const text = await whisperStopRecording();
        testResult.textContent = text ? `"${text}"` : '(no speech detected)';
        testResult.style.color = 'var(--accent)';
      } catch (e) {
        testResult.textContent = `Error: ${e}`;
        testResult.style.color = 'var(--error)';
      } finally {
        testBtn.disabled = false;
        testBtn.textContent = 'Test Recording (3s)';
      }
    };

    testResultRow.appendChild(testBtn);
    testResultRow.appendChild(testResult);
    testRow.appendChild(testResultRow);
    testSection.appendChild(testRow);
    container.appendChild(testSection);

    // Load current state from sidecar
    this.refreshSettingsState(container, statusValue, modelSelect, gpuCheckbox, gpuDeviceInput, langSelect, availableRow);

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

  private updateStatusDisplay(statusValue: HTMLElement): void {
    if (!this.status) {
      statusValue.textContent = 'Unable to connect';
      return;
    }
    if (!this.status.sidecarRunning) {
      statusValue.textContent = 'Sidecar not running';
      return;
    }
    if (this.status.modelLoaded) {
      statusValue.textContent = `Connected — ${this.status.modelName ?? 'model loaded'}`;
    } else {
      statusValue.textContent = 'Connected — no model loaded';
    }
  }

  private async refreshSettingsState(
    _container: HTMLElement,
    statusValue: HTMLElement,
    modelSelect: HTMLSelectElement,
    gpuCheckbox: HTMLInputElement,
    gpuDeviceInput: HTMLInputElement,
    langSelect: HTMLSelectElement,
    availableRow: HTMLElement,
  ): Promise<void> {
    try {
      const [status, config, models] = await Promise.all([
        whisperGetStatus(),
        whisperGetConfig(),
        whisperListModels(),
      ]);

      this.status = status;
      this.updateStatusDisplay(statusValue);

      // Update config fields
      if (config.modelName) modelSelect.value = config.modelName;
      gpuCheckbox.checked = config.useGpu;
      gpuDeviceInput.value = String(config.gpuDevice);
      langSelect.value = config.language;

      // Show available models
      if (models.length > 0) {
        availableRow.textContent = `Downloaded: ${models.join(', ')}`;
      }
    } catch {
      statusValue.textContent = 'Unable to connect';
    }
  }
}
