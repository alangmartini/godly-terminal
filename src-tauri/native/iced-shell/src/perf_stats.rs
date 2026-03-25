use std::time::Instant;

use godly_terminal_surface::render_stats::RenderStats;

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
    /// Frames where dt > 16.67ms (missed 60fps target).
    dropped_frame_count: u64,
    /// Histogram buckets: <8ms, 8-16ms, 16-33ms, 33-100ms, >100ms.
    frame_time_histogram: [u32; 5],
    /// Per-phase breakdown from the pixel renderer.
    last_render_stats: RenderStats,
    /// Total frames counted.
    total_frames: u64,
}

impl PerfStats {
    pub fn new() -> Self {
        Self {
            ema_frame_time: 1.0 / 60.0,
            last_render_us: 0.0,
            last_frame_time: None,
            alpha: 0.1,
            dropped_frame_count: 0,
            frame_time_histogram: [0; 5],
            last_render_stats: RenderStats::default(),
            total_frames: 0,
        }
    }

    /// Call on each frame/heartbeat to update FPS tracking.
    pub fn frame_tick(&mut self) {
        let now = Instant::now();
        if let Some(prev) = self.last_frame_time {
            let dt = now.duration_since(prev).as_secs_f64();
            if dt > 0.0 && dt < 1.0 {
                self.ema_frame_time = self.alpha * dt + (1.0 - self.alpha) * self.ema_frame_time;

                // Dropped frame detection: dt > 16.67ms
                if dt > 0.01667 {
                    self.dropped_frame_count += 1;
                }

                // Bucket into histogram
                let ms = dt * 1000.0;
                let bucket = if ms < 8.0 {
                    0
                } else if ms < 16.0 {
                    1
                } else if ms < 33.0 {
                    2
                } else if ms < 100.0 {
                    3
                } else {
                    4
                };
                self.frame_time_histogram[bucket] += 1;

                self.total_frames += 1;
            }
        }
        self.last_frame_time = Some(now);
    }

    /// Record full render stats from the pixel renderer.
    pub fn record_render_stats(&mut self, stats: &RenderStats) {
        self.last_render_us = stats.total.as_micros() as f64;
        self.last_render_stats = stats.clone();
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

    pub fn dropped_frames(&self) -> u64 {
        self.dropped_frame_count
    }

    /// Returns (bg_ms, glyph_ms, cursor_ms, selection_ms) from the last render.
    pub fn render_phase_ms(&self) -> (f32, f32, f32, f32) {
        let s = &self.last_render_stats;
        (
            s.bg_fill.as_secs_f32() * 1000.0,
            s.glyph_phase.as_secs_f32() * 1000.0,
            s.cursor_phase.as_secs_f32() * 1000.0,
            s.selection_phase.as_secs_f32() * 1000.0,
        )
    }

    pub fn cells_rendered(&self) -> u32 {
        self.last_render_stats.cells_rendered
    }

    pub fn histogram(&self) -> &[u32; 5] {
        &self.frame_time_histogram
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
    fn record_render_stats_updates_render_ms() {
        let mut stats = PerfStats::new();
        let rs = RenderStats {
            total: std::time::Duration::from_micros(500),
            ..Default::default()
        };
        stats.record_render_stats(&rs);
        let ms = stats.render_ms();
        assert!(
            (ms - 0.5).abs() < 0.01,
            "Expected render_ms near 0.5, got {}",
            ms
        );
    }

    #[test]
    fn dropped_frame_detection_threshold() {
        let mut stats = PerfStats::new();
        // Simulate two frames: first sets last_frame_time, second checks dt
        stats.last_frame_time = Some(Instant::now());
        // Simulate a fast frame (< 16.67ms) — no drop
        std::thread::sleep(std::time::Duration::from_millis(5));
        stats.frame_tick();
        assert_eq!(stats.dropped_frames(), 0, "Fast frame should not be dropped");

        // Simulate a slow frame (> 16.67ms) — dropped
        std::thread::sleep(std::time::Duration::from_millis(20));
        stats.frame_tick();
        assert_eq!(stats.dropped_frames(), 1, "Slow frame should be dropped");
    }

    #[test]
    fn histogram_bucketing_correctness() {
        let mut stats = PerfStats::new();
        // Directly test the bucketing logic by simulating known dt values.
        // We'll manually set last_frame_time and advance by known durations.

        // Bucket 0: < 8ms
        stats.last_frame_time = Some(Instant::now());
        std::thread::sleep(std::time::Duration::from_millis(5));
        stats.frame_tick();

        // Bucket 2: 16-33ms (also a dropped frame)
        std::thread::sleep(std::time::Duration::from_millis(20));
        stats.frame_tick();

        let h = stats.histogram();
        // The 5ms frame should land in bucket 0 (<8ms)
        assert!(h[0] >= 1, "Expected at least 1 frame in <8ms bucket, got {}", h[0]);
        // The 20ms frame should land in bucket 2 (16-33ms)
        assert!(h[2] >= 1, "Expected at least 1 frame in 16-33ms bucket, got {}", h[2]);
        assert_eq!(stats.total_frames, 2, "Should have counted 2 total frames");
    }

    #[test]
    fn render_phase_ms_extracts_durations() {
        let mut stats = PerfStats::new();
        let rs = RenderStats {
            bg_fill: std::time::Duration::from_micros(1000),
            glyph_phase: std::time::Duration::from_micros(2000),
            cursor_phase: std::time::Duration::from_micros(300),
            selection_phase: std::time::Duration::from_micros(150),
            total: std::time::Duration::from_micros(3450),
            cells_rendered: 42,
            rows_rendered: 5,
        };
        stats.record_render_stats(&rs);
        let (bg, glyph, cursor, sel) = stats.render_phase_ms();
        assert!((bg - 1.0).abs() < 0.01);
        assert!((glyph - 2.0).abs() < 0.01);
        assert!((cursor - 0.3).abs() < 0.01);
        assert!((sel - 0.15).abs() < 0.01);
        assert_eq!(stats.cells_rendered(), 42);
    }
}
