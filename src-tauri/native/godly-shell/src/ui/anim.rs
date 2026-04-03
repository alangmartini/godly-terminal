//! Frame-rate independent animation primitives for smooth UI transitions.
//!
//! Each `Anim` holds a `current` value that smoothly converges toward a
//! `target` using exponential decay parameterized by **half-life** (seconds).
//! This means animations look identical at 60 Hz, 144 Hz, or any other
//! refresh rate — no more "twitchy at 144 Hz, sluggish at 30 Hz".
//!
//! The formula: `current += (target - current) * (1 - 0.5^(dt / half_life))`
//! After `half_life` seconds, exactly 50% of the remaining distance is covered.

/// Standard half-life constants (seconds).  Tweak these to change the global
/// feel — everything that uses the same constant stays consistent.
pub mod timing {
    /// Snappy hover transitions (~58 ms half-life, fully settled in ~250 ms).
    pub const HOVER: f32 = 0.058;
    /// Medium transitions — scrollbar proximity, focus dim (~75 ms).
    pub const MEDIUM: f32 = 0.075;
    /// Slower transitions — focus dim overlay (~90 ms).
    pub const SLOW: f32 = 0.090;
    /// Cursor blink fade (~52 ms half-life for crisp on/off).
    pub const BLINK: f32 = 0.052;
}

/// A single animated float that lerps toward a target value.
#[derive(Debug, Clone, Copy)]
pub struct Anim {
    current: f32,
    target: f32,
}

impl Default for Anim {
    fn default() -> Self {
        Self { current: 0.0, target: 0.0 }
    }
}

impl Anim {
    /// Set the target value (0.0 = fully off, 1.0 = fully on).
    pub fn set(&mut self, target: f32) {
        self.target = target;
    }

    /// Current interpolated value.
    pub fn value(&self) -> f32 {
        self.current
    }

    /// Advance the animation by `dt` seconds using exponential decay.
    /// `half_life` is the time in seconds for the value to cover 50% of the
    /// remaining distance.  Returns `true` if still animating.
    pub fn tick(&mut self, half_life: f32, dt: f32) -> bool {
        let diff = self.target - self.current;
        if diff.abs() < 0.005 {
            self.current = self.target;
            return false;
        }
        // Frame-rate independent exponential decay:
        // factor = 1 - 2^(-dt/half_life) = 1 - exp(-dt * ln2 / half_life)
        let factor = 1.0 - (-dt * std::f32::consts::LN_2 / half_life).exp();
        self.current += diff * factor;
        true
    }

    /// Snap to a value immediately (skip animation).
    pub fn snap(&mut self, value: f32) {
        self.current = value;
        self.target = value;
    }

    /// Whether the value is effectively non-zero (visible).
    pub fn is_visible(&self) -> bool {
        self.current > 0.005
    }
}

/// Fixed-size array of animations (avoids heap allocation for small counts).
/// Used for tab hover states, sidebar items, window buttons, etc.
#[derive(Debug)]
pub struct AnimArray<const N: usize> {
    anims: [Anim; N],
}

impl<const N: usize> Default for AnimArray<N> {
    fn default() -> Self {
        Self { anims: [Anim::default(); N] }
    }
}

impl<const N: usize> AnimArray<N> {
    pub fn get(&self, i: usize) -> f32 {
        if i < N { self.anims[i].value() } else { 0.0 }
    }

    pub fn set(&mut self, i: usize, target: f32) {
        if i < N { self.anims[i].set(target); }
    }

    /// Tick all animations. Returns `true` if any are still animating.
    pub fn tick(&mut self, half_life: f32, dt: f32) -> bool {
        let mut animating = false;
        for a in &mut self.anims {
            animating |= a.tick(half_life, dt);
        }
        animating
    }
}

/// Dynamically-sized animation array (for variable tab/item counts).
#[derive(Debug, Default)]
pub struct AnimVec {
    anims: Vec<Anim>,
}

impl AnimVec {
    /// Ensure the vec has at least `n` entries, adding defaults if needed.
    pub fn ensure_len(&mut self, n: usize) {
        if self.anims.len() < n {
            self.anims.resize(n, Anim::default());
        }
    }

    pub fn get(&self, i: usize) -> f32 {
        self.anims.get(i).map_or(0.0, |a| a.value())
    }

    pub fn set(&mut self, i: usize, target: f32) {
        self.ensure_len(i + 1);
        self.anims[i].set(target);
    }

    /// Tick all animations. Returns `true` if any are still animating.
    pub fn tick(&mut self, half_life: f32, dt: f32) -> bool {
        let mut animating = false;
        for a in &mut self.anims {
            animating |= a.tick(half_life, dt);
        }
        animating
    }
}

// ---------------------------------------------------------------------------
// Color interpolation helpers
// ---------------------------------------------------------------------------

/// Linearly interpolate between two RGBA colors.
pub fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

/// Linearly interpolate between two floats.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
