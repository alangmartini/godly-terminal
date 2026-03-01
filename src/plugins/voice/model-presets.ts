export interface WhisperModelPreset {
  name: string;
  fileName: string;
  size: string;
  description: string;
  recommended: boolean;
}

export const WHISPER_MODEL_PRESETS: WhisperModelPreset[] = [
  {
    name: 'Tiny',
    fileName: 'ggml-tiny.bin',
    size: '75 MB',
    description: 'Fastest, lowest accuracy. Good for quick commands.',
    recommended: false,
  },
  {
    name: 'Base',
    fileName: 'ggml-base.bin',
    size: '142 MB',
    description: 'Fast but lower accuracy. May struggle with technical terms.',
    recommended: false,
  },
  {
    name: 'Small',
    fileName: 'ggml-small.bin',
    size: '466 MB',
    description: 'Higher accuracy, slower. Good for longer dictation.',
    recommended: false,
  },
  {
    name: 'Medium',
    fileName: 'ggml-medium.bin',
    size: '1.5 GB',
    description: 'High accuracy, requires more RAM/VRAM.',
    recommended: false,
  },
  {
    name: 'Large v3 Turbo',
    fileName: 'ggml-large-v3-turbo.bin',
    size: '1.5 GB',
    description: 'Best accuracy with optimized speed. Recommended for reliable transcription.',
    recommended: true,
  },
];
