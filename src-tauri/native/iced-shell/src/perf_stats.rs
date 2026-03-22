use std::time::Instant;

/// Rolling performance statistics with exponential moving average.
pub struct PerfStats {
    /// Exponential moving average of frame time (seconds).
    ema_frame_time: f64,
    /// Last render() call duration in microseconds.
    last_render_us: f64,
    /// When the last frame started.
    last_frame_time: Option<Instant>,
    /// Smoothing factor (0..1). Higher = more responsive.
    alpha: f64,
}

impl PerfStats {
    pub fn new() -> Self {
        Self {
            ema_frame_time: 1.0 / 60.0,
            last_render_us: 0.0,
            last_frame_time: None,
            alpha: 0.1,
        }
    }

    /// Call on each frame/heartbeat to update FPS tracking.
    pub fn frame_tick(&mut self) {
        let now = Instant::now();
        if let Some(prev) = self.last_frame_time {
            let dt = now.duration_since(prev).as_secs_f64();
            if dt > 0.0 && dt < 1.0 {
                self.ema_frame_time = self.alpha * dt + (1.0 - self.alpha) * self.ema_frame_time;
            }
        }
        self.last_frame_time = Some(now);
    }

    /// Record how long the render() call took.
    pub fn record_render_duration_us(&mut self, us: f64) {
        self.last_render_us = us;
    }

    pub fn fps(&self) -> f32 {
        if self.ema_frame_time > 0.0 {
            (1.0 / self.ema_frame_time) as f32
        } else {
            0.0
        }
    }

    pub fn frame_ms(&self) -> f32 {
        (self.ema_frame_time * 1000.0) as f32
    }

    pub fn render_ms(&self) -> f32 {
        (self.last_render_us / 1000.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_near_60fps() {
        let stats = PerfStats::new();
        let fps = stats.fps();
        assert!(
            (fps - 60.0).abs() < 1.0,
            "Expected fps near 60, got {}",
            fps
        );
    }

    #[test]
    fn record_render_updates() {
        let mut stats = PerfStats::new();
        stats.record_render_duration_us(500.0);
        let ms = stats.render_ms();
        assert!(
            (ms - 0.5).abs() < 0.01,
            "Expected render_ms near 0.5, got {}",
            ms
        );
    }
}
