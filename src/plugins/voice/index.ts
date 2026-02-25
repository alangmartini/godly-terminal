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

  private status: WhisperStatus | null = null;
  private audioContext: AudioContext | null = null;
  private audioStream: MediaStream | null = null;
  private volumeAnimFrame: number = 0;

  async init(_ctx: PluginContext): Promise<void> {
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
    this.stopVolumeMonitor();
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

    // ── Section: Microphone ──
    const micSection = document.createElement('div');
    micSection.className = 'settings-section';
    const micTitle = document.createElement('div');
    micTitle.className = 'settings-section-title';
    micTitle.textContent = 'Microphone';
    micSection.appendChild(micTitle);

    // Device dropdown
    const micRow = this.createRow('Input Device');
    const micSelect = document.createElement('select');
    micSelect.className = 'dialog-input';
    micSelect.style.cssText = 'width: auto; font-size: 12px; padding: 4px 8px; min-width: 200px;';

    // Default option
    const defaultOpt = document.createElement('option');
    defaultOpt.value = '';
    defaultOpt.textContent = 'Default Microphone';
    micSelect.appendChild(defaultOpt);

    micRow.appendChild(micSelect);
    micSection.appendChild(micRow);

    // Volume meter row
    const volumeRow = this.createRow('Level');
    const volumeContainer = document.createElement('div');
    volumeContainer.style.cssText = 'display: flex; align-items: center; gap: 8px; flex: 1;';

    const volumeBar = document.createElement('div');
    volumeBar.style.cssText = 'flex: 1; height: 8px; background: var(--bg-secondary, #2a2a2a); border-radius: 4px; overflow: hidden;';
    const volumeFill = document.createElement('div');
    volumeFill.style.cssText = 'height: 100%; width: 0%; background: var(--accent, #4a9eff); border-radius: 4px; transition: width 0.05s;';
    volumeBar.appendChild(volumeFill);

    const volumeLabel = document.createElement('span');
    volumeLabel.style.cssText = 'font-size: 10px; color: var(--text-secondary); min-width: 30px;';
    volumeLabel.textContent = '\u2014';

    volumeContainer.appendChild(volumeBar);
    volumeContainer.appendChild(volumeLabel);
    volumeRow.appendChild(volumeContainer);
    micSection.appendChild(volumeRow);

    container.appendChild(micSection);

    // Populate devices and start volume monitoring
    this.setupMicrophoneUI(micSelect, volumeFill, volumeLabel);

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
      // Check if sidecar is running first
      if (!this.status?.sidecarRunning) {
        loadBtn.textContent = 'Start sidecar first!';
        loadBtn.style.color = 'var(--error, #f44)';
        setTimeout(() => { loadBtn.textContent = 'Load Model'; loadBtn.style.color = ''; }, 2000);
        return;
      }
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
          microphoneDeviceId: micSelect.value || null,
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
    // Clear previous content
    statusValue.innerHTML = '';

    if (!this.status || !this.status.sidecarRunning) {
      const wrapper = document.createElement('span');
      wrapper.style.cssText = 'display: flex; align-items: center; gap: 8px;';

      const text = document.createElement('span');
      text.textContent = 'Sidecar not running';
      text.style.color = 'var(--text-secondary)';
      wrapper.appendChild(text);

      const startBtn = document.createElement('button');
      startBtn.className = 'dialog-btn dialog-btn-primary';
      startBtn.style.cssText = 'font-size: 11px; padding: 2px 10px;';
      startBtn.textContent = 'Start';
      startBtn.onclick = async () => {
        startBtn.disabled = true;
        startBtn.textContent = 'Starting...';
        try {
          const { whisperStartSidecar } = await import('./whisper-service');
          const result = await whisperStartSidecar();
          text.textContent = result;
          text.style.color = 'var(--accent)';
          // Refresh status after a brief delay for sidecar to initialize
          setTimeout(async () => {
            try {
              const { whisperGetStatus: getStatus } = await import('./whisper-service');
              this.status = await getStatus();
              this.updateStatusDisplay(statusValue);
            } catch { /* ignore */ }
          }, 2000);
        } catch (e) {
          text.textContent = `Failed: ${e}`;
          text.style.color = 'var(--error, #f44)';
          startBtn.disabled = false;
          startBtn.textContent = 'Retry';
        }
      };
      wrapper.appendChild(startBtn);
      statusValue.appendChild(wrapper);
      return;
    }

    if (this.status.modelLoaded) {
      statusValue.textContent = `Connected \u2014 ${this.status.modelName ?? 'model loaded'}`;
    } else {
      statusValue.textContent = 'Connected \u2014 no model loaded';
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

  private async setupMicrophoneUI(
    micSelect: HTMLSelectElement,
    volumeFill: HTMLElement,
    volumeLabel: HTMLElement,
  ): Promise<void> {
    // Enumerate audio input devices
    try {
      // Request mic permission first (needed to get device labels)
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });

      const devices = await navigator.mediaDevices.enumerateDevices();
      const audioInputs = devices.filter(d => d.kind === 'audioinput');

      for (const device of audioInputs) {
        const option = document.createElement('option');
        option.value = device.deviceId;
        option.textContent = device.label || `Microphone ${micSelect.options.length}`;
        micSelect.appendChild(option);
      }

      // Start volume monitoring with the permission stream we already have
      this.startVolumeMonitor(stream, volumeFill, volumeLabel);

      // When device changes, restart monitoring
      micSelect.addEventListener('change', async () => {
        this.stopVolumeMonitor();
        try {
          const constraints: MediaStreamConstraints = {
            audio: micSelect.value ? { deviceId: { exact: micSelect.value } } : true,
          };
          const newStream = await navigator.mediaDevices.getUserMedia(constraints);
          this.startVolumeMonitor(newStream, volumeFill, volumeLabel);

          // Save to config
          const { whisperSetConfig: setConfig, whisperGetConfig: getConfig } = await import('./whisper-service');
          const config = await getConfig();
          config.microphoneDeviceId = micSelect.value || null;
          await setConfig(config);
        } catch (e) {
          volumeLabel.textContent = 'Error';
          console.warn('[VoiceToText] Failed to switch mic:', e);
        }
      });
    } catch (e) {
      // Mic permission denied or not available
      const option = document.createElement('option');
      option.textContent = 'Microphone access denied';
      option.disabled = true;
      micSelect.appendChild(option);
      volumeLabel.textContent = 'N/A';
      console.warn('[VoiceToText] Mic enumeration failed:', e);
    }
  }

  private startVolumeMonitor(
    stream: MediaStream,
    volumeFill: HTMLElement,
    volumeLabel: HTMLElement,
  ): void {
    this.stopVolumeMonitor();

    this.audioStream = stream;
    this.audioContext = new AudioContext();
    const source = this.audioContext.createMediaStreamSource(stream);
    const analyser = this.audioContext.createAnalyser();
    analyser.fftSize = 256;
    analyser.smoothingTimeConstant = 0.5;
    source.connect(analyser);

    const dataArray = new Uint8Array(analyser.frequencyBinCount);

    const update = () => {
      analyser.getByteFrequencyData(dataArray);
      // Calculate RMS-like level from frequency data
      let sum = 0;
      for (let i = 0; i < dataArray.length; i++) {
        sum += dataArray[i];
      }
      const avg = sum / dataArray.length;
      const level = Math.min(100, Math.round((avg / 128) * 100));

      volumeFill.style.width = `${level}%`;
      // Color: green for low, yellow for medium, red for high
      if (level > 70) {
        volumeFill.style.background = '#f44336';
      } else if (level > 40) {
        volumeFill.style.background = '#ff9800';
      } else {
        volumeFill.style.background = 'var(--accent, #4a9eff)';
      }
      volumeLabel.textContent = `${level}%`;

      this.volumeAnimFrame = requestAnimationFrame(update);
    };

    this.volumeAnimFrame = requestAnimationFrame(update);
  }

  private stopVolumeMonitor(): void {
    if (this.volumeAnimFrame) {
      cancelAnimationFrame(this.volumeAnimFrame);
      this.volumeAnimFrame = 0;
    }
    if (this.audioStream) {
      this.audioStream.getTracks().forEach(t => t.stop());
      this.audioStream = null;
    }
    if (this.audioContext) {
      this.audioContext.close().catch(() => {});
      this.audioContext = null;
    }
  }
}
