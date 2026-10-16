// CHECKITOUT: feel free to add more math utility functions here
//
// Degree/radian conversion is built into Rust: `45.0_f32.to_radians()`, `x.to_degrees()`.
// glam (`glam::Vec3`, `glam::Mat4`, ...) covers vectors and matrices.

/// A small seeded pseudo-random number generator (SplitMix64).
///
/// The TypeScript starter used `Math.random()`, which gives different light colors on every
/// run. A fixed seed gives the same colors every run, so screenshots and timings are
/// reproducible.
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        // Unsigned overflow panics in Rust debug builds, so arithmetic that is meant to wrap
        // around has to say so with the `wrapping_*` methods.
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        // The top 24 bits fit exactly in an f32 mantissa.
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}
