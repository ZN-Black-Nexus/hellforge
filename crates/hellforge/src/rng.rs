//! A small, fast, seedable random number generator (xorshift64*).
//!
//! `Ctx` has one built in (`ctx.rand_range(0, 10)` ...). It is seeded from the
//! clock in normal play and with a fixed seed (or `--seed N`) in headless
//! runs, so tests and screenshots are repeatable.

#[derive(Clone, Debug)]
pub struct Rng {
    s: u64,
}

impl Rng {
    pub const fn new(seed: u64) -> Rng {
        let s = (seed ^ 0x9e37_79b9_7f4a_7c15).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        Rng { s: if s == 0 { 0x2545_f491_4f6c_dd1d } else { s } }
    }

    /// A random `u32`.
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.s;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.s = x;
        (x.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 32) as u32
    }

    /// A random integer in `lo..hi` (`hi` excluded). Returns `lo` if the range is empty.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        let span = (hi as i64 - lo as i64) as u64;
        (lo as i64 + ((self.next_u32() as u64 * span) >> 32) as i64) as i32
    }

    /// A random `f32` in `0.0..1.0`.
    pub fn float(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// A random `f32` in `lo..hi`.
    pub fn range_f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.float()
    }

    /// True with probability `p` (0.0 = never, 1.0 = always).
    pub fn chance(&mut self, p: f32) -> bool {
        self.float() < p
    }

    /// A random element of `items` (panics if it is empty).
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.range(0, items.len() as i32) as usize]
    }

    /// Shuffle `items` in place.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.range(0, i as i32 + 1) as usize;
            items.swap(i, j);
        }
    }
}
