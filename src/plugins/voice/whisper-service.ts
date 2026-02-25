/**
 * Whisper voice-to-text service interface.
 *
 * This module provides the frontend API for voice recording and transcription
 * via the Whisper model running in the daemon. The actual implementation
 * will be wired up when the daemon-side voice support is ready.
 */

export interface WhisperStatus {
  state: 'idle' | 'recording' | 'transcribing' | 'error';
  model?: string;
  error?: string;
}

/**
 * Query the current status of the whisper service.
 */
export async function whisperGetStatus(): Promise<WhisperStatus> {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<WhisperStatus>('whisper_get_status');
  } catch {
    return { state: 'idle' };
  }
}

/**
 * Start recording audio from the microphone.
 */
export async function whisperStartRecording(): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('whisper_start_recording');
}

/**
 * Stop recording and return the transcribed text.
 */
export async function whisperStopRecording(): Promise<string> {
  const { invoke } = await import('@tauri-apps/api/core');
  return await invoke<string>('whisper_stop_recording');
}
