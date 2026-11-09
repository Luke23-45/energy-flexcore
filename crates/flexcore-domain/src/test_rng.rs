// P1.06 — Deterministic pseudo-random generator for property tests.
//
// Recorded choice: the domain crate takes **no test dependencies** (the
// dependency rule — `tools/check_dependency_rule.py` — allows only
// flexcore-contracts, and that holds for dev-dependencies too). The usual
// property-test crates (proptest, quickcheck) are therefore unavailable, so
// property tests use this small generator instead: **splitmix64** (Steele
// et al., 2014; the standard reference implementation, public domain /
// CC0) with fixed seeds per test, which makes every property run fully
// deterministic and reproducible. Only the raw mixing function is needed —
// no statistical quality beyond "covers the input space across a few
// thousand draws" is required for these tests.
//
// This module is compiled only under `cfg(test)` (see the declaration in
// `lib.rs`); it is test support, not part of the domain API surface.

#![allow(dead_code)] // shared test support; not every helper is used by every test module

/// splitmix64 state and the next-value step.
pub(crate) struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Creates a generator from a fixed seed. Property tests must pass a
    /// literal seed so runs are reproducible.
    pub(crate) const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The raw splitmix64 step (golden-ratio increment + finalizer).
    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniform-ish value in `[0, bound)` (simple modulo; the tiny bias is
    /// irrelevant for test coverage purposes).
    pub(crate) fn below(&mut self, bound: usize) -> usize {
        debug_assert!(bound > 0);
        (self.next_u64() % bound as u64) as usize
    }

    /// Picks one element of a non-empty slice.
    pub(crate) fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    /// A finite f64 uniformly spread over `[low, high]` by scaling a 53-bit
    /// integer; used for ramp rates and similar numeric fields.
    pub(crate) fn f64_in(&mut self, low: f64, high: f64) -> f64 {
        debug_assert!(low <= high && low.is_finite() && high.is_finite());
        let unit = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64; // [0, 1]
        low + unit * (high - low)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_seed_is_reproducible() {
        let mut a = SplitMix64::new(42);
        let mut b = SplitMix64::new(42);
        for _ in 0..64 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn below_stays_in_range() {
        let mut rng = SplitMix64::new(7);
        for _ in 0..1000 {
            assert!(rng.below(5) < 5);
        }
    }

    #[test]
    fn f64_in_stays_in_range_and_finite() {
        let mut rng = SplitMix64::new(11);
        for _ in 0..1000 {
            let v = rng.f64_in(0.5, 2.0);
            assert!(v.is_finite());
            assert!((0.5..=2.0).contains(&v));
        }
    }
}
