import { invoke } from '@tauri-apps/api/core';

export interface WhisperStatus {
  state: 'idle' | 'recording' | 'transcribing';
  modelLoaded: boolean;
  modelName: string | null;
  gpuAvailable: boolean;
  gpuInUse: boolean;
  sidecarRunning: boolean;
}

export interface WhisperConfig {
  modelName: string;
  language: string;
  useGpu: boolean;
  gpuDevice: number;
  microphoneDeviceId: string | null;
}

export async function whisperGetStatus(): Promise<WhisperStatus> {
  return invoke<WhisperStatus>('whisper_get_status');
}

export async function whisperStartRecording(): Promise<void> {
  return invoke<void>('whisper_start_recording');
}

export async function whisperStopRecording(): Promise<string> {
  return invoke<string>('whisper_stop_recording');
}

export async function whisperLoadModel(
  modelName: string,
  useGpu: boolean,
  gpuDevice: number,
  language: string,
): Promise<void> {
  return invoke<void>('whisper_load_model', { modelName, useGpu, gpuDevice, language });
}

export async function whisperListModels(): Promise<string[]> {
  return invoke<string[]>('whisper_list_models');
}

export async function whisperStartSidecar(): Promise<string> {
  return invoke<string>('whisper_start_sidecar');
}

export async function whisperGetConfig(): Promise<WhisperConfig> {
  return invoke<WhisperConfig>('whisper_get_config');
}

export async function whisperSetConfig(config: WhisperConfig): Promise<void> {
  return invoke<void>('whisper_set_config', { config });
}
