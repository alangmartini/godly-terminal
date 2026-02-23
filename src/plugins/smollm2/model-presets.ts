export interface ModelPreset {
  id: string;
  label: string;
  size: string;
  quality: string;
  hfRepo: string;
  hfFilename: string;
  tokenizerRepo: string;
  subdir: string;
}

// Only Llama-architecture models are supported as presets because the engine
// uses candle's quantized_llama::ModelWeights.
export const MODEL_PRESETS: ModelPreset[] = [
  {
    id: 'smollm2-135m-q4',
    label: 'SmolLM2 135M (Q4_K_M)',
    size: '~110 MB',
    quality: 'Fast, basic quality',
    hfRepo: 'bartowski/SmolLM2-135M-Instruct-GGUF',
    hfFilename: 'SmolLM2-135M-Instruct-Q4_K_M.gguf',
    tokenizerRepo: 'HuggingFaceTB/SmolLM2-135M-Instruct',
    subdir: 'smollm2-135m',
  },
  {
    id: 'smollm2-360m-q4',
    label: 'SmolLM2 360M (Q4_K_M)',
    size: '~250 MB',
    quality: 'Better quality, slower',
    hfRepo: 'bartowski/SmolLM2-360M-Instruct-GGUF',
    hfFilename: 'SmolLM2-360M-Instruct-Q4_K_M.gguf',
    tokenizerRepo: 'HuggingFaceTB/SmolLM2-360M-Instruct',
    subdir: 'smollm2-360m',
  },
  {
    id: 'tinyllama-1.1b-q4',
    label: 'TinyLlama 1.1B (Q4_K_M)',
    size: '~670 MB',
    quality: 'Good quality, Llama arch',
    hfRepo: 'TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF',
    hfFilename: 'tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf',
    tokenizerRepo: 'TinyLlama/TinyLlama-1.1B-Chat-v1.0',
    subdir: 'tinyllama-1.1b',
  },
];

export const DEFAULT_PRESET_ID = 'smollm2-135m-q4';
