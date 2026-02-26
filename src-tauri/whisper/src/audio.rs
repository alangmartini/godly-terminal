use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Manages audio capture for whisper transcription.
pub struct AudioRecorder {
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<Vec<f32>>>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Start recording audio at 16kHz mono (what whisper expects).
    pub fn start(&mut self) -> Result<(), String> {
        if self.stream.is_some() {
            return Err("Already recording".to_string());
        }

        // Clear any leftover samples
        if let Ok(mut buf) = self.buffer.lock() {
            buf.clear();
        }

        let host = cpal::default_host();
        let device = host.default_input_device()
            .ok_or("No audio input device found")?;

        let config = cpal::StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(16_000),
            buffer_size: cpal::BufferSize::Default,
        };

        let buffer = self.buffer.clone();
        let stream = device.build_input_stream(
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
        ).map_err(|e| format!("Failed to build audio stream: {}", e))?;

        stream.play().map_err(|e| format!("Failed to start audio stream: {}", e))?;

        self.stream = Some(stream);
        Ok(())
    }

    /// Stop recording and return the captured PCM samples (16kHz mono f32).
    pub fn stop(&mut self) -> Result<Vec<f32>, String> {
        let stream = self.stream.take()
            .ok_or("Not recording")?;

        drop(stream);

        let samples = self.buffer.lock()
            .map_err(|e| format!("Buffer lock poisoned: {}", e))?
            .drain(..)
            .collect();

        Ok(samples)
    }

    pub fn is_recording(&self) -> bool {
        self.stream.is_some()
    }
}
