use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Manages audio capture for whisper transcription.
/// Records at the device's native sample rate/channels, then resamples to 16kHz mono on stop.
pub struct AudioRecorder {
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<Vec<f32>>>,
    native_sample_rate: u32,
    native_channels: u16,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
            native_sample_rate: 16_000,
            native_channels: 1,
        }
    }

    /// Start recording audio using the device's native config.
    pub fn start(&mut self) -> Result<(), String> {
        if self.stream.is_some() {
            return Err("Already recording".to_string());
        }

        // Clear any leftover samples
        if let Ok(mut buf) = self.buffer.lock() {
            buf.clear();
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No audio input device found")?;

        // Use the device's default config — most devices don't support 16kHz directly
        let default_config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get default input config: {}", e))?;

        let sample_rate = default_config.sample_rate().0;
        let channels = default_config.channels();

        eprintln!(
            "[whisper] Recording at native {}Hz {}ch (will resample to 16kHz mono)",
            sample_rate, channels
        );

        let config = cpal::StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        self.native_sample_rate = sample_rate;
        self.native_channels = channels;

        let buffer = self.buffer.clone();
        let stream = device
            .build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if let Ok(mut buf) = buffer.lock() {
                        buf.extend_from_slice(data);
                    }
                },
                |err| {
                    eprintln!("[whisper] Audio capture error: {}", err);
                },
                None,
            )
            .map_err(|e| format!("Failed to build audio stream: {}", e))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start audio stream: {}", e))?;

        self.stream = Some(stream);
        Ok(())
    }

    /// Stop recording and return the captured PCM samples (16kHz mono f32).
    pub fn stop(&mut self) -> Result<Vec<f32>, String> {
        let stream = self.stream.take().ok_or("Not recording")?;

        drop(stream);

        let raw_samples: Vec<f32> = self
            .buffer
            .lock()
            .map_err(|e| format!("Buffer lock poisoned: {}", e))?
            .drain(..)
            .collect();

        // Convert to mono if multi-channel
        let mono = if self.native_channels > 1 {
            to_mono(&raw_samples, self.native_channels)
        } else {
            raw_samples
        };

        // Resample to 16kHz if device recorded at a different rate
        let samples = if self.native_sample_rate != 16_000 {
            resample(&mono, self.native_sample_rate, 16_000)
        } else {
            mono
        };

        Ok(samples)
    }

    pub fn is_recording(&self) -> bool {
        self.stream.is_some()
    }
}

/// Mix multi-channel audio down to mono by averaging channels.
fn to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    let ch = channels as usize;
    samples
        .chunks_exact(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

/// Linear interpolation resample from source rate to target rate.
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if samples.is_empty() || from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = i as f64 * ratio;
        let idx = src_idx as usize;
        let frac = src_idx - idx as f64;

        let sample = if idx + 1 < samples.len() {
            samples[idx] as f64 * (1.0 - frac) + samples[idx + 1] as f64 * frac
        } else {
            samples[idx.min(samples.len() - 1)] as f64
        };

        output.push(sample as f32);
    }

    output
}
