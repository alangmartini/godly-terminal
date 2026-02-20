import type { GodlyPlugin, PluginContext } from '../types';
import { listen } from '@tauri-apps/api/event';
import {
  llmGetStatus,
  llmDownloadModel,
  llmLoadModel,
  llmUnloadModel,
  llmGenerate,
  isModelReady,
  isModelDownloaded,
  getStatusLabel,
  type LlmStatus,
} from './llm-service';

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
    // Auto-load model if downloaded and auto-load is enabled
    const autoLoad = this.ctx.getSetting('autoLoad', true);
    if (autoLoad && isModelDownloaded(this.status) && !isModelReady(this.status)) {
      try {
        await llmLoadModel();
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

    // Status indicator
    const statusRow = document.createElement('div');
    statusRow.className = 'shortcut-row';
    const statusLabel = document.createElement('span');
    statusLabel.className = 'shortcut-label';
    statusLabel.textContent = 'Status';
    statusRow.appendChild(statusLabel);
    const statusValue = document.createElement('span');
    statusValue.className = 'shortcut-keys';
    statusValue.textContent = getStatusLabel(this.status);
    statusRow.appendChild(statusValue);
    container.appendChild(statusRow);

    // Download / Load / Unload buttons
    const actionRow = document.createElement('div');
    actionRow.className = 'shortcut-row';
    const actionLabel = document.createElement('span');
    actionLabel.className = 'shortcut-label';
    actionLabel.textContent = 'Model';
    actionRow.appendChild(actionLabel);

    const actionBtnContainer = document.createElement('div');
    actionBtnContainer.style.display = 'flex';
    actionBtnContainer.style.gap = '8px';
    actionBtnContainer.style.alignItems = 'center';

    // Progress bar (hidden by default)
    const progressBar = document.createElement('div');
    progressBar.style.cssText = 'width: 100px; height: 6px; background: var(--bg-tertiary); border-radius: 3px; overflow: hidden; display: none;';
    const progressFill = document.createElement('div');
    progressFill.style.cssText = 'height: 100%; background: var(--accent); width: 0%; transition: width 0.3s;';
    progressBar.appendChild(progressFill);

    const updateButtons = () => {
      actionBtnContainer.innerHTML = '';

      if (this.status.status === 'NotDownloaded') {
        const downloadBtn = document.createElement('button');
        downloadBtn.className = 'dialog-btn dialog-btn-primary';
        downloadBtn.textContent = 'Download (~110MB)';
        downloadBtn.onclick = async () => {
          downloadBtn.disabled = true;
          downloadBtn.textContent = 'Downloading...';
          progressBar.style.display = 'block';

          const unlisten = await listen<number>('llm-download-progress', (event) => {
            const progress = event.payload;
            progressFill.style.width = `${Math.round(progress * 100)}%`;
          });

          try {
            await llmDownloadModel();
            this.status = await llmGetStatus();
            statusValue.textContent = getStatusLabel(this.status);
          } catch (e) {
            statusValue.textContent = `Error: ${e}`;
          } finally {
            unlisten();
            progressBar.style.display = 'none';
            updateButtons();
          }
        };
        actionBtnContainer.appendChild(downloadBtn);
        actionBtnContainer.appendChild(progressBar);
      } else if (this.status.status === 'Downloaded') {
        const loadBtn = document.createElement('button');
        loadBtn.className = 'dialog-btn dialog-btn-primary';
        loadBtn.textContent = 'Load Model';
        loadBtn.onclick = async () => {
          loadBtn.disabled = true;
          loadBtn.textContent = 'Loading...';
          try {
            await llmLoadModel();
            this.status = await llmGetStatus();
            statusValue.textContent = getStatusLabel(this.status);
          } catch (e) {
            statusValue.textContent = `Error: ${e}`;
          }
          updateButtons();
        };
        actionBtnContainer.appendChild(loadBtn);
      } else if (this.status.status === 'Ready') {
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
          updateButtons();
        };
        actionBtnContainer.appendChild(unloadBtn);
      } else if (this.status.status === 'Loading' || this.status.status === 'Downloading') {
        const spinner = document.createElement('span');
        spinner.textContent = this.status.status === 'Loading' ? 'Loading...' : 'Downloading...';
        spinner.style.color = 'var(--text-secondary)';
        actionBtnContainer.appendChild(spinner);
      } else if (this.status.status === 'Error') {
        const retryBtn = document.createElement('button');
        retryBtn.className = 'dialog-btn dialog-btn-primary';
        retryBtn.textContent = 'Retry Download';
        retryBtn.onclick = async () => {
          retryBtn.disabled = true;
          retryBtn.textContent = 'Downloading...';
          progressBar.style.display = 'block';

          const unlisten = await listen<number>('llm-download-progress', (event) => {
            const progress = event.payload;
            progressFill.style.width = `${Math.round(progress * 100)}%`;
          });

          try {
            await llmDownloadModel();
            this.status = await llmGetStatus();
            statusValue.textContent = getStatusLabel(this.status);
          } catch (e) {
            this.status = { status: 'Error', detail: `${e}` };
            statusValue.textContent = `Error: ${e}`;
          } finally {
            unlisten();
            progressBar.style.display = 'none';
            updateButtons();
          }
        };
        actionBtnContainer.appendChild(retryBtn);
        actionBtnContainer.appendChild(progressBar);
      }
    };

    updateButtons();
    actionRow.appendChild(actionBtnContainer);
    container.appendChild(actionRow);

    // Auto-load toggle
    const autoLoadRow = document.createElement('div');
    autoLoadRow.className = 'shortcut-row';
    const autoLoadLabel = document.createElement('span');
    autoLoadLabel.className = 'shortcut-label';
    autoLoadLabel.textContent = 'Auto-load on enable';
    autoLoadRow.appendChild(autoLoadLabel);
    const autoLoadCheckbox = document.createElement('input');
    autoLoadCheckbox.type = 'checkbox';
    autoLoadCheckbox.className = 'notification-checkbox';
    autoLoadCheckbox.checked = this.ctx.getSetting('autoLoad', true);
    autoLoadCheckbox.onchange = () => {
      this.ctx.setSetting('autoLoad', autoLoadCheckbox.checked);
    };
    autoLoadRow.appendChild(autoLoadCheckbox);
    container.appendChild(autoLoadRow);

    // Test generation input
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
        const result = await llmGenerate(
          `<|im_start|>system\nYou are a git branch name generator. Output ONLY a short kebab-case branch name with a conventional prefix (feat/, fix/, etc).<|im_end|>\n<|im_start|>user\n${desc}<|im_end|>\n<|im_start|>assistant\n`,
          30,
          0.3,
        );
        testResult.textContent = result.trim();
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
