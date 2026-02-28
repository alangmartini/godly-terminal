use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use godly_protocol::AudioDeviceInfo;

/// Manages audio capture for whisper transcription.
/// Records at the device's native sample rate/channels, then resamples to 16kHz mono on stop.
pub struct AudioRecorder {
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<Vec<f32>>>,
    native_sample_rate: u32,
    native_channels: u16,
    last_recording: Option<Vec<f32>>,
    recording_start: Option<std::time::Instant>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
            native_sample_rate: 16_000,
            native_channels: 1,
            last_recording: None,
            recording_start: None,
        }
    }

    /// Enumerate available audio input devices.
    pub fn list_devices() -> Result<Vec<AudioDeviceInfo>, String> {
        let host = cpal::default_host();
        let default_name = host
            .default_input_device()
            .and_then(|d| d.name().ok());

        let devices = host
            .input_devices()
            .map_err(|e| format!("Failed to enumerate input devices: {}", e))?;

        let mut result = Vec::new();
        for device in devices {
            if let Ok(name) = device.name() {
                let is_default = default_name.as_deref() == Some(&name);
                result.push(AudioDeviceInfo { name, is_default });
            }
        }
        Ok(result)
    }

    /// Start recording audio using the device's native config.
    /// If `device_name` is provided, uses that device; otherwise uses the system default.
    pub fn start(&mut self, device_name: Option<&str>) -> Result<(), String> {
        if self.stream.is_some() {
            return Err("Already recording".to_string());
        }

        // Clear any leftover samples
        if let Ok(mut buf) = self.buffer.lock() {
            buf.clear();
        }

        let host = cpal::default_host();
        let device = if let Some(name) = device_name {
            // Find device by name, fallback to default
            host.input_devices()
                .map_err(|e| format!("Failed to enumerate devices: {}", e))?
                .find(|d| d.name().ok().as_deref() == Some(name))
                .or_else(|| host.default_input_device())
                .ok_or("No audio input device found")?
        } else {
            host.default_input_device()
                .ok_or("No audio input device found")?
        };

        // Use the device's default config -- most devices don't support 16kHz directly
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
        self.recording_start = Some(std::time::Instant::now());
        Ok(())
    }

    /// Stop recording and return the captured PCM samples (16kHz mono f32).
    /// Also stores the samples for later playback.
    pub fn stop(&mut self) -> Result<Vec<f32>, String> {
        self.recording_start = None;
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

        // Store for playback
        self.last_recording = Some(samples.clone());

        Ok(samples)
    }

    pub fn is_recording(&self) -> bool {
        self.stream.is_some()
    }

    pub fn has_last_recording(&self) -> bool {
        self.last_recording.is_some()
    }

    /// Compute RMS and peak from the last ~50ms of the buffer.
    pub fn current_levels(&self) -> (f32, f32) {
        let buf = match self.buffer.lock() {
            Ok(b) => b,
            Err(_) => return (0.0, 0.0),
        };
        if buf.is_empty() {
            return (0.0, 0.0);
        }
        // Look at last ~50ms of samples (native_sample_rate * native_channels * 0.05)
        let window = (self.native_sample_rate as usize * self.native_channels as usize / 20).max(1);
        let start = buf.len().saturating_sub(window);
        let slice = &buf[start..];

        let mut sum_sq = 0.0f64;
        let mut peak = 0.0f32;
        for &s in slice {
            sum_sq += (s as f64) * (s as f64);
            let abs = s.abs();
            if abs > peak {
                peak = abs;
            }
        }
        let rms = (sum_sq / slice.len() as f64).sqrt() as f32;
        (rms, peak)
    }

    pub fn recording_duration_ms(&self) -> u64 {
        self.recording_start
            .map(|start| start.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    /// Play back the last recording through the default output device.
    /// Blocks until playback completes.
    pub fn playback_last_recording(&self) -> Result<(), String> {
        let samples = self
            .last_recording
            .as_ref()
            .ok_or("No recording available")?;

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("No audio output device found")?;

        let default_config = device
            .default_output_config()
            .map_err(|e| format!("Failed to get output config: {}", e))?;

        let output_rate = default_config.sample_rate().0;
        let output_channels = default_config.channels() as usize;

        // Resample from 16kHz to output rate
        let resampled = if output_rate != 16_000 {
            resample(samples, 16_000, output_rate)
        } else {
            samples.clone()
        };

        // Expand mono to output channels
        let mut output_samples = Vec::with_capacity(resampled.len() * output_channels);
        for &s in &resampled {
            for _ in 0..output_channels {
                output_samples.push(s);
            }
        }

        let data = Arc::new(output_samples);
        let position = Arc::new(Mutex::new(0usize));
        let done = Arc::new(Mutex::new(false));

        let data_clone = data.clone();
        let pos_clone = position.clone();
        let done_clone = done.clone();

        let config = cpal::StreamConfig {
            channels: default_config.channels(),
            sample_rate: cpal::SampleRate(output_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = device
            .build_output_stream(
                &config,
                move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut pos = pos_clone.lock().unwrap();
                    for sample in out.iter_mut() {
                        if *pos < data_clone.len() {
                            *sample = data_clone[*pos];
                            *pos += 1;
                        } else {
                            *sample = 0.0;
                            *done_clone.lock().unwrap() = true;
                        }
                    }
                },
                |err| {
                    eprintln!("[whisper] Playback error: {}", err);
                },
                None,
            )
            .map_err(|e| format!("Failed to build output stream: {}", e))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start playback: {}", e))?;

        // Wait for playback to complete
        loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if *done.lock().unwrap() {
                // Small tail to let the buffer drain
                std::thread::sleep(std::time::Duration::from_millis(100));
                break;
            }
        }

        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_mono_single_channel() {
        let input = vec![0.5, -0.3, 0.7];
        let result = to_mono(&input, 1);
        assert_eq!(result, vec![0.5, -0.3, 0.7]);
    }

    #[test]
    fn to_mono_stereo() {
        let input = vec![0.4, 0.6, -0.2, 0.8];
        let result = to_mono(&input, 2);
        assert_eq!(result.len(), 2);
        assert!((result[0] - 0.5).abs() < 1e-6); // (0.4 + 0.6) / 2
        assert!((result[1] - 0.3).abs() < 1e-6); // (-0.2 + 0.8) / 2
    }

    #[test]
    fn to_mono_empty() {
        let result = to_mono(&[], 2);
        assert!(result.is_empty());
    }

    #[test]
    fn resample_same_rate() {
        let input = vec![1.0, 2.0, 3.0];
        let result = resample(&input, 16000, 16000);
        assert_eq!(result, input);
    }

    #[test]
    fn resample_downsample() {
        // 48kHz to 16kHz = 3:1 ratio
        let input: Vec<f32> = (0..48000).map(|i| (i as f32) / 48000.0).collect();
        let result = resample(&input, 48000, 16000);
        // Should produce approximately 16000 samples
        assert!((result.len() as i32 - 16000).abs() < 2);
    }

    #[test]
    fn resample_upsample() {
        // 8kHz to 16kHz = 1:2 ratio
        let input: Vec<f32> = (0..800).map(|i| (i as f32) / 800.0).collect();
        let result = resample(&input, 8000, 16000);
        // Should produce approximately 1600 samples
        assert!((result.len() as i32 - 1600).abs() < 2);
    }

    #[test]
    fn resample_empty() {
        let result = resample(&[], 48000, 16000);
        assert!(result.is_empty());
    }

    #[test]
    fn resample_interpolation_accuracy() {
        // Simple case: 2 samples at 2Hz resampled to 4Hz
        let input = vec![0.0, 1.0];
        let result = resample(&input, 2, 4);
        // Should interpolate: 0.0, 0.5, 1.0, (possibly 1.0)
        assert!(result.len() >= 3);
        assert!((result[0] - 0.0).abs() < 1e-5);
        assert!((result[1] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn audio_recorder_new_defaults() {
        let recorder = AudioRecorder::new();
        assert!(!recorder.is_recording());
    }
}
