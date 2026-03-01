import { store } from '../state/store';
import { terminalService } from '../services/terminal-service';

let voicePollInterval: ReturnType<typeof setInterval> | null = null;

/** Toggle voice recording on/off and handle transcription. */
export async function handleVoiceToggle(): Promise<void> {
  try {
    const { whisperGetStatus, whisperStartRecording, whisperStopRecording, whisperGetAudioLevel } = await import('../plugins/voice/whisper-service');
    const status = await whisperGetStatus();

    if (status.state === 'idle') {
      await whisperStartRecording();
      updateMicButtonState('recording');
      // Start polling audio levels every 60ms
      voicePollInterval = setInterval(async () => {
        try {
          const level = await whisperGetAudioLevel();
          updateMicLevel(level.rms, level.durationMs);
        } catch {
          // ignore polling errors
        }
      }, 60);
    } else if (status.state === 'recording') {
      // Stop polling
      if (voicePollInterval) {
        clearInterval(voicePollInterval);
        voicePollInterval = null;
      }
      updateMicButtonState('transcribing');
      const result = await whisperStopRecording();
      updateMicButtonState('idle');
      if (result.text && store.getState().activeTerminalId) {
        await terminalService.writeToTerminal(store.getState().activeTerminalId!, result.text);
      }
      showTranscriptionToast(result.text, result.durationMs);
    }
  } catch (err) {
    console.error('Voice toggle failed:', err);
    if (voicePollInterval) {
      clearInterval(voicePollInterval);
      voicePollInterval = null;
    }
    updateMicButtonState('idle');
  }
}

function updateMicButtonState(state: 'idle' | 'recording' | 'transcribing'): void {
  const micBtn = document.querySelector('.mic-btn');
  if (!micBtn) return;
  micBtn.className = `mic-btn mic-${state}`;
  micBtn.setAttribute('title',
    state === 'idle' ? 'Voice input (Ctrl+Shift+M)' :
    state === 'recording' ? 'Stop recording (Ctrl+Shift+M)' :
    'Transcribing...'
  );
}

function updateMicLevel(rms: number, durationMs: number): void {
  const levelBar = document.querySelector('.mic-level-bar') as HTMLElement;
  const timer = document.querySelector('.mic-timer') as HTMLElement;
  if (levelBar) {
    // Scale RMS (typically 0-0.3) to height percentage
    const height = Math.min(100, rms * 300);
    levelBar.style.height = `${height}%`;
  }
  if (timer) {
    const secs = Math.floor(durationMs / 1000);
    const mins = Math.floor(secs / 60);
    const remainder = secs % 60;
    timer.textContent = `${mins}:${String(remainder).padStart(2, '0')}`;
  }
}

function showTranscriptionToast(text: string, durationMs: number): void {
  // Remove any existing toast
  document.querySelector('.mic-toast')?.remove();

  const toast = document.createElement('div');
  toast.className = 'mic-toast';
  const speed = durationMs > 0 ? `${durationMs}ms` : '';
  toast.textContent = text ? `"${text.slice(0, 50)}${text.length > 50 ? '...' : ''}" ${speed}` : '(no speech detected)';

  const micBtn = document.querySelector('.mic-btn');
  if (micBtn) {
    micBtn.parentElement?.appendChild(toast);
    setTimeout(() => toast.remove(), 3000);
  }
}
