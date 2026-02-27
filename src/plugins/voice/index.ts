import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { GodlyPlugin, PluginContext } from '../types';
import {
  whisperGetStatus,
  whisperGetConfig,
  whisperSetConfig,
  whisperLoadModel,
  whisperListModels,
  whisperDownloadModel,
  whisperStartSidecar,
  whisperRestartSidecar,
  whisperStartRecording,
  whisperStopRecording,
  type WhisperStatus,
} from './whisper-service';
import { WHISPER_MODEL_PRESETS } from './model-presets';

const DEFAULT_MODEL = WHISPER_MODEL_PRESETS.find(p => p.recommended)?.fileName ?? 'ggml-base.bin';

interface DownloadProgress {
  model: string;
  downloaded: number;
  total: number;
  phase: 'downloading' | 'complete';
}

export class VoiceToTextPlugin implements GodlyPlugin {
  id = 'voice-to-text';
  name = 'Voice to Text';
  description = 'Dictate text into the terminal using Whisper speech-to-text';
  version = '1.0.0';

  private status: WhisperStatus | null = null;
  private progressUnlisten: UnlistenFn | null = null;
  private statusElement: HTMLElement | null = null;

  async init(_ctx: PluginContext): Promise<void> {
    try {
      this.status = await whisperGetStatus();
    } catch {
      this.status = null;
    }
  }

  async enable(): Promise<void> {
    // Fire-and-forget auto-setup so we don't block plugin init
    this.autoSetup();
  }

  async disable(): Promise<void> {
    if (this.status?.state === 'recording') {
      try {
        await whisperStopRecording();
      } catch (e) {
        console.warn('[VoiceToText] Failed to stop recording on disable:', e);
      }
    }
    if (this.progressUnlisten) {
      this.progressUnlisten();
      this.progressUnlisten = null;
    }
  }

  destroy(): void {
    if (this.progressUnlisten) {
      this.progressUnlisten();
      this.progressUnlisten = null;
    }
  }

  /**
   * Auto-setup: download default model if needed → start sidecar → load model.
   * Runs in background on enable(). Updates status element if settings panel is open.
   */
  private async autoSetup(): Promise<void> {
    try {
      // 1. Check if any model is downloaded
      let models: string[] = [];
      try {
        models = await whisperListModels();
      } catch {
        // whisper state not initialized yet, skip
      }

      // 2. Download default model if none exist
      if (models.length === 0) {
        this.setStatusText(`Downloading ${DEFAULT_MODEL}...`);
        try {
          await whisperDownloadModel(DEFAULT_MODEL);
          models = [DEFAULT_MODEL];
        } catch (e) {
          this.setStatusText(`Download failed: ${e}`);
          console.warn('[VoiceToText] Auto-download failed:', e);
          return;
        }
      }

      // 3. Start sidecar if not running
      try {
        this.status = await whisperGetStatus();
      } catch {
        this.status = null;
      }

      if (!this.status?.sidecarRunning) {
        this.setStatusText('Starting sidecar...');
        try {
          await whisperStartSidecar();
        } catch (e) {
          // Sidecar binary may not exist yet — not fatal
          this.setStatusText('Model downloaded — sidecar not available');
          console.warn('[VoiceToText] Sidecar start failed:', e);
          return;
        }
      }

      // 4. Load the first available model
      const modelToLoad = models[0];
      this.setStatusText(`Loading ${modelToLoad}...`);
      try {
        const config = await whisperGetConfig();
        await whisperLoadModel(
          modelToLoad,
          config.useGpu,
          config.gpuDevice,
          config.language,
        );
      } catch (e) {
        this.setStatusText('Model downloaded — load failed');
        console.warn('[VoiceToText] Model load failed:', e);
        return;
      }

      // 5. Refresh status
      try {
        this.status = await whisperGetStatus();
        this.updateStatusEl();
      } catch {
        // ignore
      }
    } catch (e) {
      console.warn('[VoiceToText] Auto-setup failed:', e);
    }
  }

  private setStatusText(text: string): void {
    if (this.statusElement) {
      this.statusElement.textContent = text;
    }
  }

  renderSettings(): HTMLElement {
    const container = document.createElement('div');
    container.className = 'voice-plugin-settings';

    // ── Status indicator ──
    const statusRow = this.createRow('Status');
    const statusValue = document.createElement('span');
    statusValue.className = 'shortcut-keys';
    statusValue.textContent = 'Checking...';
    this.statusElement = statusValue;
    statusRow.appendChild(statusValue);
    container.appendChild(statusRow);

    // ── Restart Sidecar button ──
    const restartRow = this.createRow('');
    const restartBtn = document.createElement('button');
    restartBtn.className = 'dialog-btn dialog-btn-secondary';
    restartBtn.textContent = 'Restart Sidecar';
    restartBtn.style.fontSize = '11px';
    restartBtn.onclick = async () => {
      restartBtn.disabled = true;
      restartBtn.textContent = 'Restarting...';
      try {
        await whisperRestartSidecar();
        this.status = await whisperGetStatus();
        this.updateStatusEl();
        restartBtn.textContent = 'Restarted!';
        setTimeout(() => { restartBtn.textContent = 'Restart Sidecar'; restartBtn.disabled = false; }, 2000);
      } catch (e) {
        restartBtn.textContent = 'Error';
        console.warn('[VoiceToText] Restart sidecar failed:', e);
        setTimeout(() => { restartBtn.textContent = 'Restart Sidecar'; restartBtn.disabled = false; }, 2000);
      }
    };
    restartRow.appendChild(restartBtn);
    container.appendChild(restartRow);

    // ── Download progress bar ──
    const progressRow = document.createElement('div');
    progressRow.style.cssText = 'padding: 0 12px; display: none;';
    const progressBar = document.createElement('div');
    progressBar.style.cssText = 'height: 4px; background: var(--border); border-radius: 2px; overflow: hidden;';
    const progressFill = document.createElement('div');
    progressFill.style.cssText = 'height: 100%; width: 0%; background: var(--accent); transition: width 0.1s;';
    progressBar.appendChild(progressFill);
    const progressLabel = document.createElement('div');
    progressLabel.style.cssText = 'font-size: 10px; color: var(--text-secondary); margin-top: 2px;';
    progressRow.appendChild(progressBar);
    progressRow.appendChild(progressLabel);
    container.appendChild(progressRow);

    // Listen for download progress events
    this.listenForProgress(progressRow, progressFill, progressLabel, statusValue);

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

    // Download + Load buttons
    const btnRow = this.createRow('');
    btnRow.style.gap = '8px';

    const downloadBtn = document.createElement('button');
    downloadBtn.className = 'dialog-btn dialog-btn-secondary';
    downloadBtn.textContent = 'Download';
    downloadBtn.style.fontSize = '11px';
    downloadBtn.onclick = async () => {
      downloadBtn.disabled = true;
      downloadBtn.textContent = 'Downloading...';
      try {
        await whisperDownloadModel(modelSelect.value);
        downloadBtn.textContent = 'Downloaded!';
        // Refresh available models list
        const models = await whisperListModels();
        availableRow.textContent = models.length > 0 ? `Downloaded: ${models.join(', ')}` : '';
        setTimeout(() => { downloadBtn.textContent = 'Download'; downloadBtn.disabled = false; }, 2000);
      } catch (e) {
        downloadBtn.textContent = 'Error';
        console.warn('[VoiceToText] Download failed:', e);
        setTimeout(() => { downloadBtn.textContent = 'Download'; downloadBtn.disabled = false; }, 2000);
      }
    };

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
        this.updateStatusEl();
        loadBtn.textContent = 'Loaded!';
        setTimeout(() => { loadBtn.textContent = 'Load Model'; loadBtn.disabled = false; }, 2000);
      } catch (e) {
        loadBtn.textContent = 'Error';
        console.warn('[VoiceToText] Load model failed:', e);
        setTimeout(() => { loadBtn.textContent = 'Load Model'; loadBtn.disabled = false; }, 2000);
      }
    };

    btnRow.appendChild(downloadBtn);
    btnRow.appendChild(loadBtn);
    modelSection.appendChild(btnRow);

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
          microphoneDeviceId: null,
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

  private createRow(label: string): HTMLElement {
    const row = document.createElement('div');
    row.className = 'shortcut-row';
    const lbl = document.createElement('span');
    lbl.className = 'shortcut-label';
    lbl.textContent = label;
    row.appendChild(lbl);
    return row;
  }

  private updateStatusEl(): void {
    if (!this.statusElement) return;
    if (!this.status) {
      this.statusElement.textContent = 'Unable to connect';
      return;
    }
    if (!this.status.sidecarRunning) {
      this.statusElement.textContent = 'Sidecar not running';
      return;
    }
    if (this.status.modelLoaded) {
      this.statusElement.textContent = `Connected — ${this.status.modelName ?? 'model loaded'}`;
    } else {
      this.statusElement.textContent = 'Connected — no model loaded';
    }
  }

  private async listenForProgress(
    progressRow: HTMLElement,
    progressFill: HTMLElement,
    progressLabel: HTMLElement,
    statusValue: HTMLElement,
  ): Promise<void> {
    try {
      this.progressUnlisten = await listen<DownloadProgress>('whisper-download-progress', (event) => {
        const { model, downloaded, total, phase } = event.payload;
        if (phase === 'complete') {
          progressRow.style.display = 'none';
          statusValue.textContent = `${model} downloaded`;
          return;
        }
        progressRow.style.display = '';
        if (total > 0) {
          const pct = Math.round((downloaded / total) * 100);
          progressFill.style.width = `${pct}%`;
          const dlMB = (downloaded / 1024 / 1024).toFixed(1);
          const totalMB = (total / 1024 / 1024).toFixed(0);
          progressLabel.textContent = `${model}: ${dlMB} / ${totalMB} MB (${pct}%)`;
          statusValue.textContent = `Downloading ${model}... ${pct}%`;
        } else {
          const dlMB = (downloaded / 1024 / 1024).toFixed(1);
          progressLabel.textContent = `${model}: ${dlMB} MB`;
          statusValue.textContent = `Downloading ${model}...`;
        }
      });
    } catch {
      // listen not available (e.g. in tests)
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
      this.updateStatusEl();

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
