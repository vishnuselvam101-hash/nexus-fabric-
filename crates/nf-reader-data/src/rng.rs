//! Small deterministic PRNG (SplitMix64). No external crate, identical on every host.

#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f64()
    }
    /// Uniform integer in [lo, hi].
    pub fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next_u64() % ((hi - lo + 1) as u64)) as i64
    }
    /// A multiple of `step` in [lo, hi].
    pub fn stepped(&mut self, lo: f64, hi: f64, step: f64) -> f64 {
        let n = ((hi - lo) / step).floor() as i64;
        lo + step * self.int(0, n) as f64
    }
    pub fn chance(&mut self, p: f64) -> bool {
        self.f64() < p
    }
    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[(self.next_u64() % xs.len() as u64) as usize]
    }
}

/// Mix several words into one seed.
pub fn mix(words: &[u64]) -> u64 {
    let mut r = Rng::new(0x1234_5678_9ABC_DEF0);
    let mut h = r.next_u64();
    for w in words {
        r = Rng::new(h ^ w.wrapping_mul(0xD6E8_FEB8_6659_FD93));
        h = r.next_u64();
    }
    h
}
